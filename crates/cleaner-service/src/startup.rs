//! Модуль 6.2: Обзор запускаемых компонентов и элементов автозагрузки Windows.

use crate::programs::expand_environment_variables;
use crate::types::{ServiceError, StartupItem, StartupSource};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[cfg(windows)]
use winreg::enums::{
    HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY,
};
#[cfg(windows)]
use winreg::RegKey;

/// Парсит первый байт структуры Task Manager StartupApproved.
///
/// В протоколе Windows StartupApproved:
/// - `0x02` — элемент включён (enabled).
/// - `0x03` — элемент отключён (disabled).
/// - Нечётные значения обычно означают отключённое состояние.
/// - Отсутствие записи означает стандартное состояние «включён» (true).
pub fn parse_startup_approved_byte(data: &[u8]) -> bool {
    if data.is_empty() {
        return true;
    }
    match data[0] {
        0x02 => true,
        0x03 => false,
        b => (b & 1) == 0,
    }
}

/// Извлекает путь к целевому исполняемому файлу из строки команды автозапуска.
pub fn extract_target_executable_path(command: &str) -> String {
    let expanded = expand_environment_variables(command.trim());
    let trimmed = expanded.trim();

    if let Some(stripped) = trimmed.strip_prefix('"') {
        if let Some(close_idx) = stripped.find('"') {
            return stripped[..close_idx].trim().to_string();
        }
    }

    let lower = trimmed.to_lowercase();
    for ext in &[".exe", ".bat", ".cmd", ".lnk", ".com"] {
        let mut search_from = 0;
        while let Some(pos) = lower[search_from..].find(ext) {
            let actual_pos = search_from + pos + ext.len();
            if actual_pos == trimmed.len() || trimmed.as_bytes()[actual_pos].is_ascii_whitespace() {
                return trimmed[..actual_pos].trim().to_string();
            }
            search_from = actual_pos;
        }
    }

    if let Some(space_pos) = trimmed.find(' ') {
        trimmed[..space_pos].trim().to_string()
    } else {
        trimmed.to_string()
    }
}

/// Получает полный список элементов автозагрузки из реестра и папок Startup.
pub fn list_startup_items() -> Result<Vec<StartupItem>, ServiceError> {
    #[cfg(windows)]
    {
        list_startup_items_windows()
    }
    #[cfg(not(windows))]
    {
        Ok(Vec::new())
    }
}

#[cfg(windows)]
fn list_startup_items_windows() -> Result<Vec<StartupItem>, ServiceError> {
    let mut items = Vec::new();
    let mut seen_ids = HashSet::new();

    // 1. Сканирование реестра Run и RunOnce (HKCU и HKLM, 64/32 бит)
    let registry_targets = [
        (
            HKEY_CURRENT_USER,
            "HKCU",
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run",
            StartupSource::RegistryRun,
            KEY_READ | KEY_WOW64_64KEY,
            "Run",
        ),
        (
            HKEY_CURRENT_USER,
            "HKCU",
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\RunOnce",
            StartupSource::RegistryRunOnce,
            KEY_READ | KEY_WOW64_64KEY,
            "RunOnce",
        ),
        (
            HKEY_CURRENT_USER,
            "HKCU",
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run",
            StartupSource::RegistryRun,
            KEY_READ | KEY_WOW64_32KEY,
            "Run32",
        ),
        (
            HKEY_LOCAL_MACHINE,
            "HKLM",
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run",
            StartupSource::RegistryRun,
            KEY_READ | KEY_WOW64_64KEY,
            "Run",
        ),
        (
            HKEY_LOCAL_MACHINE,
            "HKLM",
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\RunOnce",
            StartupSource::RegistryRunOnce,
            KEY_READ | KEY_WOW64_64KEY,
            "RunOnce",
        ),
        (
            HKEY_LOCAL_MACHINE,
            "HKLM",
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run",
            StartupSource::RegistryRun,
            KEY_READ | KEY_WOW64_32KEY,
            "Run32",
        ),
    ];

    for (root_hkey, hive_name, path, source, flags, approved_sub) in registry_targets {
        let root = RegKey::predef(root_hkey);
        let run_key = match root.open_subkey_with_flags(path, flags) {
            Ok(k) => k,
            Err(_) => continue,
        };

        // Читаем ключ StartupApproved для определения активности
        let approved_path = format!(
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\{}",
            approved_sub
        );
        let approved_key = root.open_subkey_with_flags(&approved_path, flags).ok();

        for (name, val) in run_key.enum_values().flatten() {
            let cmd_string = match val.to_string() {
                s if !s.trim().is_empty() => s.trim().to_string(),
                _ => continue,
            };

            let id = format!("{}_{}_{}", hive_name, approved_sub, name);
            if seen_ids.contains(&id) {
                continue;
            }
            seen_ids.insert(id.clone());

            let enabled = approved_key
                .as_ref()
                .and_then(|ak| ak.get_raw_value(&name).ok())
                .map(|raw| parse_startup_approved_byte(&raw.bytes))
                .unwrap_or(true);

            let target_path = extract_target_executable_path(&cmd_string);
            let target_exists = Path::new(&target_path).exists();

            let location = format!(r"{}\{}", hive_name, path);

            items.push(StartupItem {
                id,
                name,
                command: cmd_string,
                source,
                location,
                enabled,
                target_exists,
            });
        }
    }

    // 2. Сканирование папок автозагрузки (User и Common)
    let folder_targets = [
        (
            std::env::var("APPDATA")
                .map(|appdata| {
                    PathBuf::from(appdata).join(r"Microsoft\Windows\Start Menu\Programs\Startup")
                })
                .ok(),
            StartupSource::StartupFolderUser,
            "StartupFolderUser",
        ),
        (
            std::env::var("ProgramData")
                .map(|pdata| {
                    PathBuf::from(pdata).join(r"Microsoft\Windows\Start Menu\Programs\Startup")
                })
                .ok(),
            StartupSource::StartupFolderCommon,
            "StartupFolderCommon",
        ),
    ];

    let hkcu_approved = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\StartupFolder",
            KEY_READ,
        )
        .ok();

    for (maybe_folder, source, id_prefix) in folder_targets {
        let folder = match maybe_folder {
            Some(f) if f.is_dir() => f,
            _ => continue,
        };

        let read_dir = match std::fs::read_dir(&folder) {
            Ok(rd) => rd,
            Err(_) => continue,
        };

        for entry_res in read_dir {
            let entry = match entry_res {
                Ok(e) => e,
                Err(_) => continue,
            };

            let file_name = entry.file_name().to_string_lossy().to_string();
            if file_name.eq_ignore_ascii_case("desktop.ini") {
                continue;
            }

            let path = entry.path();
            if path.is_dir() {
                continue;
            }

            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(&file_name)
                .to_string();

            let id = format!("{}_{}", id_prefix, file_name);
            if seen_ids.contains(&id) {
                continue;
            }
            seen_ids.insert(id.clone());

            let enabled = hkcu_approved
                .as_ref()
                .and_then(|ak| ak.get_raw_value(&file_name).ok())
                .map(|raw| parse_startup_approved_byte(&raw.bytes))
                .unwrap_or(true);

            let cmd_string = path.to_string_lossy().to_string();
            let target_exists = path.exists();
            let location = folder.to_string_lossy().to_string();

            items.push(StartupItem {
                id,
                name,
                command: cmd_string,
                source,
                location,
                enabled,
                target_exists,
            });
        }
    }

    items.sort_by_key(|a| a.name.to_lowercase());
    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_startup_approved_byte_states() {
        // 0x02 = включен
        assert!(parse_startup_approved_byte(&[0x02]));
        assert!(parse_startup_approved_byte(&[
            0x02, 0x00, 0x00, 0x00, 0x12, 0x34
        ]));

        // 0x03 = отключен
        assert!(!parse_startup_approved_byte(&[0x03]));
        assert!(!parse_startup_approved_byte(&[
            0x03, 0x00, 0x00, 0x00, 0xAA, 0xBB
        ]));

        // Нечетный байт = отключен
        assert!(!parse_startup_approved_byte(&[0x01]));
        assert!(!parse_startup_approved_byte(&[0x07]));

        // Четный байт = включен
        assert!(parse_startup_approved_byte(&[0x00]));
        assert!(parse_startup_approved_byte(&[0x06]));

        // Пустой срез = по умолчанию включен
        assert!(parse_startup_approved_byte(&[]));
    }

    #[test]
    fn test_extract_target_executable_path() {
        // Путь в кавычках с аргументами
        let cmd1 = r#""C:\Program Files\Vendor\app.exe" --minimized --autostart"#;
        assert_eq!(
            extract_target_executable_path(cmd1),
            r#"C:\Program Files\Vendor\app.exe"#
        );

        // Путь без кавычек с аргументами
        let cmd2 = r#"C:\Program Files\Vendor\tool.exe -silent"#;
        assert_eq!(
            extract_target_executable_path(cmd2),
            r#"C:\Program Files\Vendor\tool.exe"#
        );

        // Путь с расширением bat
        let cmd3 = r#"C:\Scripts\startup.bat /run"#;
        assert_eq!(
            extract_target_executable_path(cmd3),
            r#"C:\Scripts\startup.bat"#
        );

        // Путь с переменной окружения
        let cmd4 = r#""%SystemRoot%\System32\cmd.exe""#;
        let res4 = extract_target_executable_path(cmd4);
        assert!(!res4.contains("%SystemRoot%"));
        assert!(res4.to_lowercase().ends_with("cmd.exe"));
    }
}
