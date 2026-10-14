//! События для оповещения пользовательского интерфейса через IPC.

use crate::models::{MetricSnapshot, TaskStatus};
use serde::{Deserialize, Serialize};

/// Уровень важности пользовательского уведомления.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NotificationLevel {
    Info,
    Success,
    Warning,
    Error,
}

/// Событие уведомления для UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotificationEvent {
    pub level: NotificationLevel,
    pub title: String,
    pub message: String,
    pub timestamp_utc: String,
}

/// Типизированные системные события CleanCore.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event", content = "payload")]
pub enum AppEvent {
    /// Обновление системных метрик.
    MetricsUpdated(MetricSnapshot),
    /// Прогресс выполнения фоновой задачи.
    TaskProgress(TaskStatus),
    /// Системное уведомление для пользователя.
    Notification(NotificationEvent),
}
