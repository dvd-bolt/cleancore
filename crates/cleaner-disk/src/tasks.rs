//! Менеджер фоновых задач анализа диска и дубликатов с поддержкой отмены.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use tracing::info;
use uuid::Uuid;

use crate::analyzer::{analyze_indexed, AnalyzerOptions};
use crate::duplicates::find_duplicates_cached;
use crate::history::ResourceHistoryTracker;
use crate::models::{DiskAnalysisReport, DuplicateScanResult, QuickDirectory, ResourceHistory};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DiskTaskState {
    Running,
    Completed,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DiskTaskStatus {
    pub task_id: String,
    pub kind: String,
    pub state: DiskTaskState,
    pub progress_message: String,
    pub scanned_items: u64,
    pub scanned_bytes: u64,
    pub error_message: Option<String>,
}

pub struct DiskManager {
    start_gate: Mutex<()>,
    disk_tasks: Arc<Mutex<HashMap<String, DiskTaskStatus>>>,
    disk_reports: Arc<Mutex<HashMap<String, DiskAnalysisReport>>>,
    dup_reports: Arc<Mutex<HashMap<String, DuplicateScanResult>>>,
    cancellation_tokens: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
    mft_snapshot: Arc<Mutex<Option<Arc<crate::ntfs::MftSnapshot>>>>,
    pub history: Arc<ResourceHistoryTracker>,
    hash_cache: Option<Arc<dyn cleaner_core::duplicate_cache::DuplicateHashCache>>,
}

impl Default for DiskManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DiskManager {
    pub fn new() -> Self {
        Self {
            start_gate: Mutex::new(()),
            disk_tasks: Arc::new(Mutex::new(HashMap::new())),
            disk_reports: Arc::new(Mutex::new(HashMap::new())),
            dup_reports: Arc::new(Mutex::new(HashMap::new())),
            cancellation_tokens: Arc::new(Mutex::new(HashMap::new())),
            mft_snapshot: Arc::new(Mutex::new(None)),
            history: Arc::new(ResourceHistoryTracker::new()),
            hash_cache: None,
        }
    }

    pub fn with_hash_cache(
        cache: Arc<dyn cleaner_core::duplicate_cache::DuplicateHashCache>,
    ) -> Self {
        let mut manager = Self::new();
        manager.hash_cache = Some(cache);
        manager
    }
    /// Возвращает список стандартных пользовательских папок Windows для быстрого выбора.
    pub fn get_quick_directories(&self) -> Vec<QuickDirectory> {
        let mut list = Vec::new();
        #[cfg(windows)]
        {
            use windows_sys::Win32::Storage::FileSystem::{GetDriveTypeW, GetLogicalDrives};
            let mask = unsafe { GetLogicalDrives() };
            for index in 0..26 {
                if mask & (1 << index) == 0 {
                    continue;
                }
                let path = format!("{}:\\", (b'A' + index) as char);
                let wide: Vec<u16> = path.encode_utf16().chain(Some(0)).collect();
                let kind = unsafe { GetDriveTypeW(wide.as_ptr()) };
                if kind == 2 || kind == 3 {
                    list.push(QuickDirectory {
                        id: format!("drive-{index}"),
                        name: format!("Диск {path}"),
                        path,
                    });
                }
            }
        }

        if let Ok(user_profile) = std::env::var("USERPROFILE") {
            let user_path = PathBuf::from(&user_profile);

            list.push(QuickDirectory {
                id: "profile".into(),
                name: "Профиль пользователя".into(),
                path: user_path.to_string_lossy().to_string(),
            });

            let known_dirs = [
                ("downloads", "Загрузки", "Downloads"),
                ("documents", "Документы", "Documents"),
                ("desktop", "Рабочий стол", "Desktop"),
                ("pictures", "Изображения", "Pictures"),
                ("videos", "Видео", "Videos"),
                ("music", "Музыка", "Music"),
            ];

            for (id, name, sub) in known_dirs {
                let p = user_path.join(sub);
                if p.exists() {
                    list.push(QuickDirectory {
                        id: id.into(),
                        name: name.into(),
                        path: p.to_string_lossy().to_string(),
                    });
                }
            }
        }

        list
    }

    /// Запускает фоновый анализ структуры и крупнейших файлов каталога.
    pub fn start_disk_analysis(&self, path_str: &str) -> Result<String, String> {
        let _start = self
            .start_gate
            .lock()
            .map_err(|_| "Менеджер задач недоступен")?;
        if self
            .disk_tasks
            .lock()
            .map_err(|_| "Менеджер задач недоступен")?
            .values()
            .any(|task| task.state == DiskTaskState::Running)
        {
            return Err("Дождитесь завершения или отмените текущий анализ диска".into());
        }
        let path = PathBuf::from(path_str);
        if !path.exists() {
            return Err(format!("Каталог не существует: {path_str}"));
        }
        if !path.is_dir() {
            return Err(format!("Указанный путь не является каталогом: {path_str}"));
        }
        // Preserve the previous report on invalid input. Raw MFT access only accepts X:\ roots.
        // Folder objects are checked by the walker; cached navigation reads no filesystem data.
        self.disk_tasks.lock().unwrap().clear();
        self.disk_reports.lock().unwrap().clear();
        self.dup_reports.lock().unwrap().clear();
        self.cancellation_tokens.lock().unwrap().clear();

        let task_id = Uuid::new_v4().to_string();
        let cancel_token = Arc::new(AtomicBool::new(false));

        let initial_status = DiskTaskStatus {
            task_id: task_id.clone(),
            kind: "disk_analysis".into(),
            state: DiskTaskState::Running,
            progress_message: "Инициализация обхода...".into(),
            scanned_items: 0,
            scanned_bytes: 0,
            error_message: None,
        };

        {
            let mut tasks = self.disk_tasks.lock().unwrap();
            tasks.insert(task_id.clone(), initial_status);

            let mut tokens = self.cancellation_tokens.lock().unwrap();
            tokens.insert(task_id.clone(), Arc::clone(&cancel_token));
        }

        let thread_task_id = task_id.clone();
        let tasks_ref = Arc::clone(&self.disk_tasks);
        let reports_ref = Arc::clone(&self.disk_reports);
        let snapshot_ref = Arc::clone(&self.mft_snapshot);
        let options = AnalyzerOptions::default();

        info!("Запуск задачи анализа диска {thread_task_id} для {path_str}");

        thread::spawn(move || {
            let updater_tasks = Arc::clone(&tasks_ref);
            let updater_id = thread_task_id.clone();

            let cached = snapshot_ref.lock().ok().and_then(|s| s.clone());
            let cached_report = if crate::ntfs::drive_letter(&path).is_none() {
                cached
                    .as_ref()
                    .and_then(|s| s.report(&path, &options, &cancel_token))
            } else {
                None
            };
            let result = if let Some(report) = cached_report {
                Ok((report, cached))
            } else {
                analyze_indexed(
                    &path,
                    &options,
                    cancel_token.clone(),
                    move |files, bytes| {
                        if let Ok(mut tasks) = updater_tasks.lock() {
                            if let Some(t) = tasks.get_mut(&updater_id) {
                                t.scanned_items = files;
                                t.scanned_bytes = bytes;
                                t.progress_message = format!("Обработано: {files} файлов");
                            }
                        }
                    },
                )
            };

            let was_cancelled = cancel_token.load(Ordering::Relaxed);
            let mut tasks = tasks_ref.lock().unwrap();

            match result {
                Ok((report, snapshot)) => {
                    *snapshot_ref.lock().unwrap() = snapshot;
                    let final_state = if was_cancelled {
                        DiskTaskState::Cancelled
                    } else {
                        DiskTaskState::Completed
                    };

                    let partial = !report.complete;
                    let total_files = report.total_files;
                    let total_bytes = report.total_bytes;

                    {
                        let mut reports = reports_ref.lock().unwrap();
                        reports.insert(thread_task_id.clone(), report);
                    }

                    if let Some(t) = tasks.get_mut(&thread_task_id) {
                        t.state = final_state;
                        t.scanned_items = total_files;
                        t.scanned_bytes = total_bytes;
                        t.progress_message = if was_cancelled {
                            "Анализ остановлен пользователем".into()
                        } else {
                            if partial {
                                "Анализ завершён частично — часть объектов пропущена".into()
                            } else {
                                "Анализ успешно завершён".into()
                            }
                        };
                    }
                }
                Err(err) => {
                    if let Some(t) = tasks.get_mut(&thread_task_id) {
                        t.state = DiskTaskState::Failed;
                        t.error_message = Some(err);
                    }
                }
            }
        });

        Ok(task_id)
    }

    /// Запускает фоновый трёхфазный поиск дубликатов.
    pub fn start_duplicate_scan(&self, path_str: &str) -> Result<String, String> {
        let _start = self
            .start_gate
            .lock()
            .map_err(|_| "Менеджер задач недоступен")?;
        if self
            .disk_tasks
            .lock()
            .map_err(|_| "Менеджер задач недоступен")?
            .values()
            .any(|task| task.state == DiskTaskState::Running)
        {
            return Err("Дождитесь завершения или отмените текущий анализ диска".into());
        }
        // Keep only the current task/report. Repeated scans cannot retain unbounded trees.
        self.disk_tasks.lock().unwrap().clear();
        self.disk_reports.lock().unwrap().clear();
        self.dup_reports.lock().unwrap().clear();
        self.cancellation_tokens.lock().unwrap().clear();
        let path = PathBuf::from(path_str);
        cleaner_execution::native::lock_directory(&path, std::path::Path::new(""))
            .map_err(str::to_string)?;
        if !path.exists() || !path.is_dir() {
            return Err(format!("Некорректный каталог: {path_str}"));
        }

        let task_id = Uuid::new_v4().to_string();
        let cancel_token = Arc::new(AtomicBool::new(false));

        let initial_status = DiskTaskStatus {
            task_id: task_id.clone(),
            kind: "duplicate_scan".into(),
            state: DiskTaskState::Running,
            progress_message: "Подготовка к поиску дубликатов...".into(),
            scanned_items: 0,
            scanned_bytes: 0,
            error_message: None,
        };

        {
            let mut tasks = self.disk_tasks.lock().unwrap();
            tasks.insert(task_id.clone(), initial_status);

            let mut tokens = self.cancellation_tokens.lock().unwrap();
            tokens.insert(task_id.clone(), Arc::clone(&cancel_token));
        }

        let thread_task_id = task_id.clone();
        let tasks_ref = Arc::clone(&self.disk_tasks);
        let reports_ref = Arc::clone(&self.dup_reports);

        let hash_cache = self.hash_cache.clone();
        info!("Запуск задачи поиска дубликатов {thread_task_id} для {path_str}");

        thread::spawn(move || {
            let updater_tasks = Arc::clone(&tasks_ref);
            let updater_id = thread_task_id.clone();

            let result = find_duplicates_cached(
                &path,
                cancel_token.clone(),
                move |msg, current, _total| {
                    if let Ok(mut tasks) = updater_tasks.lock() {
                        if let Some(t) = tasks.get_mut(&updater_id) {
                            t.progress_message = msg.to_string();
                            t.scanned_items = current;
                        }
                    }
                },
                hash_cache.as_deref(),
            );

            let was_cancelled = cancel_token.load(Ordering::Relaxed);
            let mut tasks = tasks_ref.lock().unwrap();

            match result {
                Ok(report) => {
                    let final_state = if was_cancelled {
                        DiskTaskState::Cancelled
                    } else {
                        DiskTaskState::Completed
                    };

                    let total_wasted = report.total_wasted_bytes;

                    {
                        let mut reports = reports_ref.lock().unwrap();
                        reports.insert(thread_task_id.clone(), report);
                    }

                    if let Some(t) = tasks.get_mut(&thread_task_id) {
                        t.state = final_state;
                        t.scanned_bytes = total_wasted;
                        t.progress_message = if was_cancelled {
                            "Поиск дубликатов остановлен пользователем".into()
                        } else {
                            "Поиск дубликатов успешно завершён".into()
                        };
                    }
                }
                Err(err) => {
                    if let Some(t) = tasks.get_mut(&thread_task_id) {
                        t.state = DiskTaskState::Failed;
                        t.error_message = Some(err);
                    }
                }
            }
        });

        Ok(task_id)
    }

    pub fn cancel_all(&self) {
        if let Ok(tokens) = self.cancellation_tokens.lock() {
            for token in tokens.values() {
                token.store(true, Ordering::Release);
            }
        }
    }

    /// Отменяет выполняющуюся задачу.
    pub fn cancel_task(&self, task_id: &str) -> bool {
        let tokens = self.cancellation_tokens.lock().unwrap();
        if let Some(token) = tokens.get(task_id) {
            token.store(true, Ordering::Relaxed);
            true
        } else {
            false
        }
    }

    /// Возвращает статус задачи.
    pub fn get_task_status(&self, task_id: &str) -> Option<DiskTaskStatus> {
        let tasks = self.disk_tasks.lock().unwrap();
        tasks.get(task_id).cloned()
    }

    /// Возвращает готовый отчёт анализа структуры диска.
    pub fn get_disk_report(&self, report_id: &str) -> Option<DiskAnalysisReport> {
        let reports = self.disk_reports.lock().unwrap();
        reports.get(report_id).cloned()
    }

    pub fn get_indexed_directory(&self, report_id: &str, path: &str) -> Option<DiskAnalysisReport> {
        let _start = self.start_gate.lock().ok()?;
        if !self.disk_reports.lock().ok()?.contains_key(report_id) {
            return None;
        }
        let snapshot = self.mft_snapshot.lock().ok()?.clone()?;
        snapshot.report(
            std::path::Path::new(path),
            &AnalyzerOptions::default(),
            &AtomicBool::new(false),
        )
    }

    pub fn treemap(
        &self,
        report_id: &str,
        path: &str,
        mode: crate::models::TreemapSize,
    ) -> Option<crate::models::TreemapData> {
        let _start = self.start_gate.lock().ok()?;
        if !self.disk_reports.lock().ok()?.contains_key(report_id) {
            return None;
        }
        let snapshot = self.mft_snapshot.lock().ok()?.clone()?;
        snapshot.treemap(std::path::Path::new(path), mode)
    }
    pub fn indexed_entry_path(&self, report_id: &str, entry_id: &str) -> Option<String> {
        let _start = self.start_gate.lock().ok()?;
        if !self.disk_reports.lock().ok()?.contains_key(report_id) {
            return None;
        }
        self.mft_snapshot
            .lock()
            .ok()?
            .as_ref()?
            .entry_path(entry_id)
    }
    pub fn indexed_entry_identity(&self, report_id: &str, entry_id: &str) -> Option<(String, u64)> {
        let _gate = self.start_gate.lock().ok()?;
        if !self.disk_reports.lock().ok()?.contains_key(report_id) {
            return None;
        }
        self.mft_snapshot
            .lock()
            .ok()?
            .as_ref()?
            .entry_identity(entry_id)
    }

    /// Выборка самых крупных файлов тома из активного MFT-индекса (OPT-02).
    pub fn get_space_hogs(
        &self,
        min_size_bytes: u64,
        limit: usize,
        category_filter: Option<cleaner_core::models::LargeFileCategory>,
    ) -> Option<Vec<cleaner_core::models::LargeFileEntry>> {
        let snapshot = self.mft_snapshot.lock().ok()?.clone()?;
        Some(snapshot.space_hogs(min_size_bytes, limit, category_filter))
    }

    /// Discard a snapshot after confirmed mutations. Old callbacks cannot
    /// invalidate a newly started report with a different ID.
    pub fn invalidate_report(&self, report_id: &str) -> bool {
        let Ok(_gate) = self.start_gate.lock() else {
            return false;
        };
        let Ok(mut reports) = self.disk_reports.lock() else {
            return false;
        };
        let disk = reports.remove(report_id).is_some();
        if disk {
            if let Ok(mut snapshot) = self.mft_snapshot.lock() {
                *snapshot = None;
            }
        }
        let duplicate = self
            .dup_reports
            .lock()
            .is_ok_and(|mut r| r.remove(report_id).is_some());
        disk || duplicate
    }

    /// Возвращает готовый отчёт поиска дубликатов.
    pub fn get_duplicate_report(&self, report_id: &str) -> Option<DuplicateScanResult> {
        let reports = self.dup_reports.lock().unwrap();
        reports.get(report_id).cloned()
    }

    /// Возвращает 10-минутную историю ресурсов.
    pub fn get_resource_history(&self) -> ResourceHistory {
        self.history.get_history()
    }
}
