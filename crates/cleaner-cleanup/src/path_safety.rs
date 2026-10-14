//! Проверка безопасности путей, защита от traversal и исключение ссылок Windows.

use cleaner_core::errors::CoreError;
use cleaner_core::safety::PathGuard;
use std::path::{Path, PathBuf};

#[cfg(windows)]
use std::os::windows::fs::MetadataExt;

/// Битовая маска Windows для FILE_ATTRIBUTE_REPARSE_POINT (0x00000400).
/// Reparse points включают NTFS junctions, symlinks, app execution aliases и cloud placeholders.
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;

/// Проверяет, является ли объект файловой системы reparse point, junction или symlink.
/// Любые ссылки строго пропускаются сканером для исключения обхода чужих директорий.
pub fn is_reparse_point_or_symlink(path: &Path) -> bool {
    let symlink_metadata = match std::fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(_) => return true, // При ошибке чтения метаданных считаем объект небезопасным и пропускаем
    };

    if symlink_metadata.file_type().is_symlink() {
        return true;
    }

    #[cfg(windows)]
    {
        let attrs = symlink_metadata.file_attributes();
        if (attrs & FILE_ATTRIBUTE_REPARSE_POINT) != 0 {
            return true;
        }
    }

    false
}

/// Находит и верифицирует безопасный корневой каталог временных файлов текущего пользователя.
///
/// Безопасность:
/// - Проверяет наличие переменной окружения `LOCALAPPDATA` или `TEMP`.
/// - Убеждается, что путь существует, является директорией и не указывает на критические области Windows.
/// - Не доверяет перенаправленным корням, если они указывают на корень диска или системные каталоги.
pub fn resolve_user_temp_root() -> Result<PathBuf, CoreError> {
    let candidate = if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        PathBuf::from(local_app_data).join("Temp")
    } else if let Ok(temp) = std::env::var("TEMP") {
        PathBuf::from(temp)
    } else {
        return Err(CoreError::InvalidPath(
            "Не удалось определить временный каталог текущего пользователя (%LOCALAPPDATA%\\Temp)"
                .into(),
        ));
    };

    if !candidate.exists() {
        return Err(CoreError::InvalidPath(format!(
            "Временный каталог не существует: {}",
            candidate.display()
        )));
    }

    if !candidate.is_dir() {
        return Err(CoreError::InvalidPath(format!(
            "Временный путь не является каталогом: {}",
            candidate.display()
        )));
    }

    // Защита от опасных корней (Windows, System32, Program Files, корень диска)
    validate_temp_root_boundary(&candidate)?;

    // Канонизируем путь для однозначного сопоставления
    let canonical = std::fs::canonicalize(&candidate).unwrap_or(candidate);
    Ok(canonical)
}

/// Проверяет, что каталог не попадает в список критически важных системных областей ОС.
pub fn validate_temp_root_boundary(root: &Path) -> Result<(), CoreError> {
    let path_str = root.to_string_lossy().to_uppercase();

    // Запрет корней дисков (C:\, D:\)
    if path_str.len() <= 3 && path_str.contains(':') {
        return Err(CoreError::PathTraversalDetected(
            "Временный каталог не может указывать на корень диска".into(),
        ));
    }

    // Запрет системных каталогов
    let forbidden_prefixes = [
        "\\WINDOWS",
        "\\SYSTEM32",
        "\\PROGRAM FILES",
        "\\PROGRAM FILES (X86)",
        "\\PROGRAMDATA",
    ];

    for forbidden in &forbidden_prefixes {
        if path_str.contains(forbidden) {
            return Err(CoreError::PathTraversalDetected(format!(
                "Временный каталог находится в запрещённой системной области: {}",
                root.display()
            )));
        }
    }

    // Каталог должен содержать 'TEMP'
    if !path_str.ends_with("TEMP") && !path_str.ends_with("TEMP\\") {
        return Err(CoreError::InvalidPath(format!(
            "Каталог не является временной папкой пользователя (ожидалось окончание на Temp): {}",
            root.display()
        )));
    }

    Ok(())
}

/// Проверяет, что файл является обычным файлом, не ссылкой и находится строго внутри корня.
pub fn is_safe_target_file(root: &Path, file_path: &Path) -> bool {
    // 1. Проверка на выход за пределы корня через PathGuard
    if PathGuard::ensure_within_root(root, file_path).is_err() {
        return false;
    }

    // 2. Проверка на junction / reparse point / symlink
    if is_reparse_point_or_symlink(file_path) {
        return false;
    }

    // 3. Проверка метаданных: должен быть обычный файл
    match std::fs::metadata(file_path) {
        Ok(meta) => meta.is_file(),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_boundary_validation_rejects_system_dirs() {
        let win_dir = PathBuf::from("C:\\Windows\\Temp");
        assert!(validate_temp_root_boundary(&win_dir).is_err());

        let root_dir = PathBuf::from("C:\\");
        assert!(validate_temp_root_boundary(&root_dir).is_err());

        let prog_files = PathBuf::from("C:\\Program Files\\Temp");
        assert!(validate_temp_root_boundary(&prog_files).is_err());
    }

    #[test]
    fn test_boundary_validation_accepts_valid_user_temp() {
        let valid = PathBuf::from("C:\\Users\\User\\AppData\\Local\\Temp");
        assert!(validate_temp_root_boundary(&valid).is_ok());
    }

    #[test]
    fn test_is_safe_target_file_within_root() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("cleancore_test_file.tmp");
        std::fs::write(&test_file, b"test content").unwrap();

        assert!(is_safe_target_file(&temp_dir, &test_file));

        let _ = std::fs::remove_file(test_file);
    }
}
