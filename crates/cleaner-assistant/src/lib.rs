pub mod context;
pub mod model;
pub mod runtime;
use context::{AssistantContext, Recommendation};
use model::{ModelInfo, SelectedModel};
use serde::Serialize;
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

#[derive(Debug, thiserror::Error, Serialize)]
#[error("{message}")]
pub struct AssistantError {
    pub code: String,
    pub message: String,
}
pub(crate) fn error(code: &str, message: &str) -> AssistantError {
    AssistantError {
        code: code.into(),
        message: message.into(),
    }
}
impl From<std::io::Error> for AssistantError {
    fn from(_: std::io::Error) -> Self {
        error("ASSISTANT_IO", "Файл или канал runtime недоступен")
    }
}
impl From<serde_json::Error> for AssistantError {
    fn from(_: serde_json::Error) -> Self {
        error("ASSISTANT_DATA", "Некорректные данные помощника")
    }
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    Running,
    Completed,
    Cancelled,
    Failed,
}
#[derive(Debug, Clone, Serialize)]
pub struct Task {
    pub id: String,
    pub state: TaskState,
    pub answer: Option<String>,
    pub error: Option<String>,
    pub recommendations: Vec<Recommendation>,
}
#[derive(Debug, Clone, Serialize)]
pub struct Status {
    pub enabled: bool,
    pub runtime_present: bool,
    pub model: Option<ModelInfo>,
    pub task: Option<Task>,
}
#[derive(Debug, Clone, Serialize)]
pub struct PreparedContext {
    pub id: String,
    pub summary: AssistantContext,
    pub recommendations: Vec<Recommendation>,
}
struct Inner {
    enabled: bool,
    model: Option<Arc<SelectedModel>>,
    context: Option<(PreparedContext, Instant)>,
    task: Option<Task>,
    cancel: Arc<AtomicBool>,
}
pub struct Assistant {
    inner: Arc<Mutex<Inner>>,
    runtime: Mutex<PathBuf>,
}
impl Default for Assistant {
    fn default() -> Self {
        #[cfg(debug_assertions)]
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        #[cfg(not(debug_assertions))]
        let root = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(Path::to_path_buf))
            .unwrap_or_default();
        Self::new(runtime::bundled_runtime(&root))
    }
}
impl Assistant {
    pub fn new(runtime: PathBuf) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                enabled: false,
                model: None,
                context: None,
                task: None,
                cancel: Arc::new(AtomicBool::new(false)),
            })),
            runtime: Mutex::new(runtime),
        }
    }
    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Inner>, AssistantError> {
        self.inner
            .lock()
            .map_err(|_| error("ASSISTANT_STATE", "Помощник недоступен"))
    }
    pub fn set_runtime(&self, path: PathBuf) {
        if let Ok(mut root) = self.runtime.lock() {
            *root = path;
        }
    }
    pub fn status(&self) -> Result<Status, AssistantError> {
        let inner = self.lock()?;
        Ok(Status {
            enabled: inner.enabled,
            runtime_present: self
                .runtime
                .lock()
                .map_err(|_| error("ASSISTANT_STATE", "Runtime недоступен"))?
                .join(runtime::RUNTIME_EXE)
                .is_file(),
            model: inner.model.as_ref().map(|m| m.info.clone()),
            task: inner.task.clone(),
        })
    }
    pub fn select_model(&self, path: &Path) -> Result<ModelInfo, AssistantError> {
        let selected = SelectedModel::open(path)?;
        let mut inner = self.lock()?;
        if inner
            .task
            .as_ref()
            .is_some_and(|t| matches!(t.state, TaskState::Running))
        {
            return Err(error("ASSISTANT_BUSY", "Сначала остановите текущий запрос"));
        }
        inner.enabled = false;
        inner.cancel.store(true, Ordering::Release);
        inner.context = None;
        inner.task = None;
        let info = selected.info.clone();
        inner.model = Some(Arc::new(selected));
        Ok(info)
    }
    pub fn enable(
        &self,
        model_id: &str,
        license_source: &str,
        accepted: bool,
    ) -> Result<(), AssistantError> {
        if !accepted || !(4..=512).contains(&license_source.trim().len()) {
            return Err(error(
                "MODEL_LICENSE",
                "Укажите источник условий модели и подтвердите право её использования",
            ));
        }
        let mut inner = self.lock()?;
        if inner
            .task
            .as_ref()
            .is_some_and(|t| matches!(t.state, TaskState::Running))
        {
            return Err(error("ASSISTANT_BUSY", "Запрос ещё выполняется"));
        }
        let selected = inner
            .model
            .as_mut()
            .ok_or_else(|| error("MODEL_MISSING", "Выберите модель"))?;
        if selected.info.id != model_id {
            return Err(error(
                "MODEL_CHANGED",
                "Модель изменилась; проверьте лицензию снова",
            ));
        }
        let selected = Arc::get_mut(selected)
            .ok_or_else(|| error("ASSISTANT_BUSY", "Модель ещё используется"))?;
        selected.info.license_source = Some(license_source.trim().into());
        selected.info.license_accepted = true;
        inner.enabled = true;
        Ok(())
    }
    pub fn disable(&self) -> Result<(), AssistantError> {
        let mut inner = self.lock()?;
        inner.enabled = false;
        inner.cancel.store(true, Ordering::Release);
        inner.model = None;
        inner.context = None;
        Ok(())
    }
    pub fn cancel(&self) -> Result<(), AssistantError> {
        self.lock()?.cancel.store(true, Ordering::Release);
        Ok(())
    }
    pub fn prepare(&self, summary: AssistantContext) -> Result<PreparedContext, AssistantError> {
        if summary.journal.len() > 5 {
            return Err(error("CONTEXT_LIMIT", "Выберите не более пяти записей"));
        }
        let prepared = PreparedContext {
            id: uuid::Uuid::new_v4().to_string(),
            recommendations: summary.recommendations(),
            summary,
        };
        if serde_json::to_vec(&prepared)?.len() > 8192 {
            return Err(error("CONTEXT_LIMIT", "Сводка слишком велика"));
        }
        let mut inner = self.lock()?;
        if inner
            .task
            .as_ref()
            .is_some_and(|t| matches!(t.state, TaskState::Running))
        {
            return Err(error(
                "ASSISTANT_BUSY",
                "Остановите запрос перед заменой сводки",
            ));
        }
        inner.task = None;
        inner.context = Some((prepared.clone(), Instant::now()));
        Ok(prepared)
    }
    pub fn start(&self, context_id: &str, question: &str) -> Result<String, AssistantError> {
        if question.trim().is_empty() || question.len() > 1024 {
            return Err(error(
                "QUESTION_LIMIT",
                "Вопрос должен содержать от 1 до 1024 байт текста",
            ));
        }
        let mut inner = self.lock()?;
        if !inner.enabled {
            return Err(error("ASSISTANT_DISABLED", "Помощник отключён"));
        }
        if inner
            .task
            .as_ref()
            .is_some_and(|t| matches!(t.state, TaskState::Running))
        {
            return Err(error("ASSISTANT_BUSY", "Уже выполняется один запрос"));
        }
        let model = inner
            .model
            .clone()
            .ok_or_else(|| error("MODEL_MISSING", "Выберите модель"))?;
        if !model.info.license_accepted {
            return Err(error(
                "MODEL_LICENSE",
                "Подтвердите условия использования модели",
            ));
        }
        let (context, created) = inner
            .context
            .as_ref()
            .ok_or_else(|| error("CONTEXT_MISSING", "Сначала подготовьте сводку"))?;
        if context.id != context_id || created.elapsed() > Duration::from_secs(120) {
            return Err(error(
                "CONTEXT_EXPIRED",
                "Сводка изменилась или устарела; подготовьте новую",
            ));
        }
        let prompt = context.summary.prompt(question);
        let recommendations = context.recommendations.clone();
        let id = uuid::Uuid::new_v4().to_string();
        let cancel = Arc::new(AtomicBool::new(false));
        inner.cancel = cancel.clone();
        inner.task = Some(Task {
            id: id.clone(),
            state: TaskState::Running,
            answer: None,
            error: None,
            recommendations,
        });
        let runtime = self
            .runtime
            .lock()
            .map_err(|_| error("ASSISTANT_STATE", "Runtime недоступен"))?
            .clone();
        let worker = self.inner.clone();
        let worker_id = id.clone();
        let spawned = std::thread::Builder::new()
            .name("cleancore-assistant".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    runtime::generate(&runtime, &model, &prompt, &cancel)
                }))
                .unwrap_or_else(|_| {
                    Err(error("RUNTIME_FAILED", "Рабочий процесс помощника прерван"))
                });
                if let Ok(mut inner) = worker.lock() {
                    if let Some(task) = inner.task.as_mut().filter(|t| t.id == worker_id) {
                        match result {
                            Ok(answer) if !cancel.load(Ordering::Acquire) => {
                                task.state = TaskState::Completed;
                                task.answer = Some(answer);
                            }
                            Ok(_) => task.state = TaskState::Cancelled,
                            Err(e) => {
                                task.state = if e.code == "CANCELLED" {
                                    TaskState::Cancelled
                                } else {
                                    TaskState::Failed
                                };
                                task.error = Some(e.message);
                            }
                        }
                    }
                }
            });
        if spawned.is_err() {
            if let Some(task) = inner.task.as_mut() {
                task.state = TaskState::Failed;
                task.error = Some("Не удалось создать рабочий поток".into());
            }
            return Err(error("RUNTIME_WORKER", "Не удалось создать рабочий поток"));
        }
        Ok(id)
    }
}
impl Drop for Assistant {
    fn drop(&mut self) {
        if let Ok(inner) = self.inner.lock() {
            inner.cancel.store(true, Ordering::Release);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn no_model_is_an_honest_disabled_state() {
        let helper = Assistant::new(PathBuf::from("missing-runtime"));
        let status = helper.status().unwrap();
        assert!(!status.enabled);
        assert!(!status.runtime_present);
        assert!(status.model.is_none());
        let context = helper.prepare(AssistantContext::default()).unwrap();
        assert_eq!(
            helper.start(&context.id, "Объясни").unwrap_err().code,
            "ASSISTANT_DISABLED"
        );
        assert!(helper.enable("missing", "test", true).is_err());
        helper.disable().unwrap();
    }
    #[test]
    fn limits_selected_context_and_question() {
        let helper = Assistant::default();
        let summary = context::JournalSummary {
            status: "Unknown".into(),
            item_count: 1,
            unknown_count: 1,
            failed_count: 0,
        };
        assert!(helper
            .prepare(AssistantContext {
                journal: vec![summary; 6],
                ..Default::default()
            })
            .is_err());
        assert_eq!(
            helper.start("unknown", &"x".repeat(1025)).unwrap_err().code,
            "QUESTION_LIMIT"
        );
    }
    #[test]
    fn license_snapshot_binding_and_missing_runtime_never_invent_an_answer() {
        let dir = std::env::temp_dir().join(format!("assistant_fixture_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut bytes = b"GGUF".to_vec();
        bytes.extend(3u32.to_le_bytes());
        bytes.extend(1u64.to_le_bytes());
        bytes.extend(1u64.to_le_bytes());
        let key = b"general.architecture";
        bytes.extend((key.len() as u64).to_le_bytes());
        bytes.extend(key);
        bytes.extend(8u32.to_le_bytes());
        bytes.extend(5u64.to_le_bytes());
        bytes.extend(b"llama");
        bytes.resize(1024, 0);
        let path = dir.join("fixture.gguf");
        std::fs::write(&path, bytes).unwrap();
        let helper = Assistant::new(dir.join("missing-runtime"));
        let model = helper.select_model(&path).unwrap();
        assert_eq!(
            helper
                .enable(&model.id, "MIT at local model card", false)
                .unwrap_err()
                .code,
            "MODEL_LICENSE"
        );
        assert_eq!(
            helper
                .enable("wrong-model", "MIT at local model card", true)
                .unwrap_err()
                .code,
            "MODEL_CHANGED"
        );
        helper
            .enable(&model.id, "MIT at local model card", true)
            .unwrap();
        let old = helper.prepare(AssistantContext::default()).unwrap();
        let current = helper.prepare(AssistantContext::default()).unwrap();
        assert_eq!(
            helper.start(&old.id, "Вопрос").unwrap_err().code,
            "CONTEXT_EXPIRED"
        );
        helper.start(&current.id, "Вопрос").unwrap();
        let until = Instant::now() + Duration::from_secs(5);
        loop {
            let task = helper.status().unwrap().task.unwrap();
            if !matches!(task.state, TaskState::Running) {
                assert!(matches!(task.state, TaskState::Failed));
                assert!(task.answer.is_none());
                break;
            }
            assert!(Instant::now() < until);
            std::thread::sleep(Duration::from_millis(5));
        }
        helper.disable().unwrap();
        drop(helper);
        std::fs::remove_file(&path).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }
}
