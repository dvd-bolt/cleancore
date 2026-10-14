//! Типизированные ошибки и контракты сообщений CleanCore.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Базовая ошибка ядра CleanCore.
#[derive(Debug, Error, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "details")]
pub enum CoreError {
    #[error("Некорректный путь: {0}")]
    InvalidPath(String),

    #[error("Обнаружена попытка path traversal: {0}")]
    PathTraversalDetected(String),

    #[error("Запрещена обработка reparse point/symlink: {0}")]
    ReparsePointForbidden(String),

    #[error("Доступ запрещён (Access Denied): {0}")]
    AccessDenied(String),

    #[error("Ресурс или файл не найден: {0}")]
    ResourceNotFound(String),

    #[error("Файл занят другим процессом: {0}")]
    FileInUse(String),

    #[error("Операция отменена пользователем")]
    OperationCancelled,

    #[error("Ошибка локального хранилища: {0}")]
    StorageError(String),

    #[error("Системная метрика недоступна: {0}")]
    SystemMetricUnavailable(String),

    #[error("Ошибка сериализации или валидации: {0}")]
    SerializationError(String),

    #[error("Внутренняя ошибка ядра: {0}")]
    Internal(String),
}

/// Безопасное представление ошибки для передачи через IPC в frontend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiError {
    /// Стабильный машиночитаемый код ошибки.
    pub code: String,
    /// Безопасное локализованное сообщение для пользователя (без приватных путей).
    pub message: String,
}

impl From<CoreError> for ApiError {
    fn from(err: CoreError) -> Self {
        let code = match &err {
            CoreError::InvalidPath(_) => "INVALID_PATH",
            CoreError::PathTraversalDetected(_) => "PATH_TRAVERSAL",
            CoreError::ReparsePointForbidden(_) => "REPARSE_POINT_FORBIDDEN",
            CoreError::AccessDenied(_) => "ACCESS_DENIED",
            CoreError::ResourceNotFound(_) => "NOT_FOUND",
            CoreError::FileInUse(_) => "FILE_IN_USE",
            CoreError::OperationCancelled => "CANCELLED",
            CoreError::StorageError(_) => "STORAGE_ERROR",
            CoreError::SystemMetricUnavailable(_) => "METRIC_UNAVAILABLE",
            CoreError::SerializationError(_) => "SERIALIZATION_ERROR",
            CoreError::Internal(_) => "INTERNAL_ERROR",
        };

        ApiError {
            code: code.to_string(),
            message: err.to_string(),
        }
    }
}
