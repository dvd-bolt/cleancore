//! Менеджер фоновых задач анализа диска и дубликатов с поддержкой отмены.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use tracing::info;
use uuid::Uuid;

use crate::analyzer::{analyze_directory_tree, AnalyzerOptions};
use crate::duplicates::find_duplicates;
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
    disk_tasks: Arc<Mutex<HashMap<String, DiskTaskStatus>>>,
    disk_reports: Arc<Mutex<HashMap<String, DiskAnalysisReport>>>,
    dup_reports: Arc<Mutex<HashMap<String, DuplicateScanResult>>>,
    cancellation_tokens: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
    pub history: Arc<ResourceHistoryTracker>,
}

impl Default for DiskManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DiskManager {
    pub fn new() -> Self {
        Self {
            disk_tasks: Arc::new(Mutex::new(HashMap::new())),
            disk_reports: Arc::new(Mutex::new(HashMap::new())),
            dup_reports: Arc::new(Mutex::new(HashMap::new())),
            cancellation_tokens: Arc::new(Mutex::new(HashMap::new())),
            history: Arc::new(ResourceHistoryTracker::new()),
        }
    }

    /// Возвращает список стандартных пользовательских папок Windows для быстрого выбора.
    pub fn get_quick_directories(&self) -> Vec<QuickDirectory> {
        let mut list = Vec::new();

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
        let path = PathBuf::from(path_str);
        if !path.exists() {
            return Err(format!("Каталог не существует: {path_str}"));
        }
        if !path.is_dir() {
            return Err(format!("Указанный путь не является каталогом: {path_str}"));
        }

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
        let options = AnalyzerOptions::default();

        info!("Запуск задачи анализа диска {thread_task_id} для {path_str}");

        thread::spawn(move || {
            let updater_tasks = Arc::clone(&tasks_ref);
            let updater_id = thread_task_id.clone();

            let result = analyze_directory_tree(
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
                            "Анализ успешно завершён".into()
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
        let path = PathBuf::from(path_str);
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

        info!("Запуск задачи поиска дубликатов {thread_task_id} для {path_str}");

        thread::spawn(move || {
            let updater_tasks = Arc::clone(&tasks_ref);
            let updater_id = thread_task_id.clone();

            let result =
                find_duplicates(&path, cancel_token.clone(), move |msg, current, _total| {
                    if let Ok(mut tasks) = updater_tasks.lock() {
                        if let Some(t) = tasks.get_mut(&updater_id) {
                            t.progress_message = msg.to_string();
                            t.scanned_items = current;
                        }
                    }
                });

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
