//! Управление фоновыми задачами сканирования и порционной выдачей preview.

use cleaner_core::errors::ApiError;
use cleaner_core::models::{CleanupCandidate, CleanupPreviewPage, TaskState, TaskStatus};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use tracing::info;
use uuid::Uuid;

use crate::path_safety::resolve_user_temp_root;
use crate::rules::RuleCatalog;
use crate::scanner::{scan_directory, ScannerOptions};

/// Снимок результатов сканирования, сохранённый в памяти backend.
#[derive(Debug, Clone)]
pub struct StoredPreview {
    pub preview_id: String,
    pub category_id: String,
    pub scanned_time_utc: String,
    pub total_bytes: u64,
    pub candidates: Vec<CleanupCandidate>,
}

/// Менеджер задач анализа и кэширования результатов preview.
pub struct TaskManager {
    catalog: RuleCatalog,
    tasks: Arc<Mutex<HashMap<String, TaskStatus>>>,
    cancellation_tokens: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
    previews: Arc<Mutex<HashMap<String, StoredPreview>>>,
}

impl Default for TaskManager {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskManager {
    pub fn new() -> Self {
        Self {
            catalog: RuleCatalog::new(),
            tasks: Arc::new(Mutex::new(HashMap::new())),
            cancellation_tokens: Arc::new(Mutex::new(HashMap::new())),
            previews: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Инициирует фоновую задачу безопасного сканирования категории.
    pub fn start_scan(&self, category_id: &str) -> Result<String, ApiError> {
        let rule = self
            .catalog
            .get_rule(category_id)
            .ok_or_else(|| ApiError {
                code: "UNKNOWN_CATEGORY".into(),
                message: format!("Неизвестная категория очистки: {category_id}"),
            })?
            .clone();

        let temp_root = resolve_user_temp_root().map_err(|e| ApiError {
            code: "INVALID_TEMP_ROOT".into(),
            message: format!("Ошибка проверки временного каталога: {e}"),
        })?;

        let task_id = Uuid::new_v4().to_string();
        let cancel_token = Arc::new(AtomicBool::new(false));

        // Регистрация начального статуса задачи
        let initial_status = TaskStatus {
            id: task_id.clone(),
            kind: format!("scan:{}", rule.id),
            state: TaskState::Running,
            processed_items: 0,
            total_items: None,
            error_message: None,
        };

        {
            let mut tasks = self.tasks.lock().unwrap();
            tasks.insert(task_id.clone(), initial_status);

            let mut tokens = self.cancellation_tokens.lock().unwrap();
            tokens.insert(task_id.clone(), Arc::clone(&cancel_token));
        }

        // Запуск рабочего потока сканирования
        let thread_task_id = task_id.clone();
        let tasks_ref = Arc::clone(&self.tasks);
        let previews_ref = Arc::clone(&self.previews);
        let options = ScannerOptions::default();

        info!(
            "Запуск задачи сканирования {thread_task_id} для каталога {}",
            temp_root.display()
        );

        thread::spawn(move || {
            let status_updater_tasks = Arc::clone(&tasks_ref);
            let updater_task_id = thread_task_id.clone();

            let result = scan_directory(
                &temp_root,
                &rule,
                &options,
                cancel_token,
                move |scanned, _matched, _bytes| {
                    if let Ok(mut tasks) = status_updater_tasks.lock() {
                        if let Some(st) = tasks.get_mut(&updater_task_id) {
                            st.processed_items = scanned;
                        }
                    }
                },
            );

            // Фиксация завершения задачи
            let final_state = if result.was_cancelled {
                TaskState::Cancelled
            } else {
                TaskState::Completed
            };

            let preview_id = thread_task_id.clone();
            let total_bytes = result.total_matched_bytes;

            // Сохранение preview
            {
                let mut previews = previews_ref.lock().unwrap();
                previews.insert(
                    preview_id.clone(),
                    StoredPreview {
                        preview_id: preview_id.clone(),
                        category_id: rule.id,
                        scanned_time_utc: crate::scanner::system_time_to_iso(
                            std::time::SystemTime::now(),
                        ),
                        total_bytes,
                        candidates: result.candidates,
                    },
                );
            }

            // Обновление финального статуса задачи
            {
                let mut tasks = tasks_ref.lock().unwrap();
                if let Some(st) = tasks.get_mut(&thread_task_id) {
                    st.state = final_state;
                    st.processed_items = result.total_scanned_items;
                    st.total_items = Some(result.total_scanned_items);
                }
            }

            info!(
                "Задача сканирования {thread_task_id} завершена со статусом {:?}",
                final_state
            );
        });

        Ok(task_id)
    }

    /// Запрашивает отмену выполняющейся задачи.
    pub fn cancel_scan(&self, task_id: &str) -> bool {
        let tokens = self.cancellation_tokens.lock().unwrap();
        if let Some(token) = tokens.get(task_id) {
            token.store(true, Ordering::Relaxed);
            true
        } else {
            false
        }
    }

    /// Возвращает текущий статус задачи.
    pub fn get_task_status(&self, task_id: &str) -> Option<TaskStatus> {
        let tasks = self.tasks.lock().unwrap();
        tasks.get(task_id).cloned()
    }

    /// Возвращает страницу кандидатов preview для неблокирующей выдачи в UI.
    pub fn get_preview_page(
        &self,
        preview_id: &str,
        offset: usize,
        limit: usize,
    ) -> Result<CleanupPreviewPage, ApiError> {
        let previews = self.previews.lock().unwrap();
        let stored = previews.get(preview_id).ok_or_else(|| ApiError {
            code: "PREVIEW_NOT_FOUND".into(),
            message: format!("Снимок предварительного просмотра не найден: {preview_id}"),
        })?;

        let total_candidates = stored.candidates.len();
        let total_bytes = stored.total_bytes;

        let items = if offset >= total_candidates {
            Vec::new()
        } else {
            let end = (offset + limit).min(total_candidates);
            stored.candidates[offset..end].to_vec()
        };

        Ok(CleanupPreviewPage {
            preview_id: stored.preview_id.clone(),
            category_id: stored.category_id.clone(),
            offset,
            limit,
            total_candidates,
            total_bytes,
            items,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_manager_flow() {
        let manager = TaskManager::new();
        let task_res = manager.start_scan("user-temp-v1");
        assert!(task_res.is_ok());

        let task_id = task_res.unwrap();
        let status = manager.get_task_status(&task_id);
        assert!(status.is_some());

        // Проверка отмены
        assert!(manager.cancel_scan(&task_id));
    }

    #[test]
    fn test_invalid_category_rejected() {
        let manager = TaskManager::new();
        let res = manager.start_scan("invalid_unknown_category");
        assert!(res.is_err());
    }
}
