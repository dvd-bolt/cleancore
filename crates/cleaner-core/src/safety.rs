//! Модель рисков и базовые правила безопасности файловых путей.

use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};

use crate::errors::CoreError;

/// Уровень потенциального риска для операций очистки.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RiskLevel {
    /// Низкий риск: временные кэши, безопасные для очистки.
    Low,
    /// Средний риск: файлы логов или дампы, требующие внимания.
    Medium,
    /// Высокий риск: пользовательские данные или системные журналы.
    High,
    /// Критический риск: системные компоненты Windows (удаление запрещено).
    Critical,
}

/// Защитник путей от traversal, выхода за пределы корня и недопустимых компонентов.
pub struct PathGuard;

impl PathGuard {
    /// Проверяет, что относительный путь безопасен: не содержит обратных переходов `..`,
    /// префиксов дисков, корней или null-байтов.
    pub fn validate_relative_path<P: AsRef<Path>>(path: P) -> Result<PathBuf, CoreError> {
        let p = path.as_ref();
        let s = p.to_string_lossy();

        if s.contains('\0') {
            return Err(CoreError::InvalidPath("Путь содержит null-байт".into()));
        }

        let mut clean = PathBuf::new();
        for comp in p.components() {
            match comp {
                Component::Normal(c) => clean.push(c),
                Component::CurDir => {}
                Component::ParentDir => {
                    return Err(CoreError::PathTraversalDetected(format!(
                        "Обнаружен переход на родительский каталог '..' в пути: {}",
                        s
                    )));
                }
                Component::RootDir | Component::Prefix(_) => {
                    return Err(CoreError::InvalidPath(format!(
                        "Относительный путь не должен содержать корень или префикс диска: {}",
                        s
                    )));
                }
            }
        }

        if clean.as_os_str().is_empty() {
            return Err(CoreError::InvalidPath(
                "Пустой путь после нормализации".into(),
            ));
        }

        Ok(clean)
    }

    /// Проверяет, что целевой путь строго находится внутри разрешённого корневого каталога.
    pub fn ensure_within_root<R: AsRef<Path>, T: AsRef<Path>>(
        root: R,
        target: T,
    ) -> Result<PathBuf, CoreError> {
        let root_path = root.as_ref();
        let target_path = target.as_ref();

        // Проверяем компоненты без следования по ссылкам
        let rel = match target_path.strip_prefix(root_path) {
            Ok(r) => r,
            Err(_) => {
                return Err(CoreError::PathTraversalDetected(format!(
                    "Путь {} выходит за пределы разрешённого корня {}",
                    target_path.display(),
                    root_path.display()
                )));
            }
        };

        Self::validate_relative_path(rel)?;
        Ok(target_path.to_path_buf())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_relative_path() {
        let safe = PathGuard::validate_relative_path("temp/cache/file.tmp").unwrap();
        assert_eq!(safe, PathBuf::from("temp/cache/file.tmp"));
    }

    #[test]
    fn test_reject_parent_traversal() {
        let res = PathGuard::validate_relative_path("temp/../system32/cmd.exe");
        assert!(matches!(res, Err(CoreError::PathTraversalDetected(_))));
    }

    #[test]
    fn test_reject_absolute_in_relative() {
        let res = PathGuard::validate_relative_path("C:\\Windows\\System32");
        assert!(matches!(res, Err(CoreError::InvalidPath(_))));
    }

    #[test]
    fn test_ensure_within_root() {
        let root = PathBuf::from("C:\\Users\\User\\AppData\\Local\\Temp");
        let valid_target = PathBuf::from("C:\\Users\\User\\AppData\\Local\\Temp\\app\\data.log");
        assert!(PathGuard::ensure_within_root(&root, &valid_target).is_ok());

        let invalid_target = PathBuf::from("C:\\Users\\User\\AppData\\Local\\Microsoft");
        assert!(matches!(
            PathGuard::ensure_within_root(&root, &invalid_target),
            Err(CoreError::PathTraversalDetected(_))
        ));
    }
}
