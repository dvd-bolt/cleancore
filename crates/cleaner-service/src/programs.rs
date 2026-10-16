//! Модуль 6.1: Инвентаризация установленных программ и безопасный запуск деинсталляторов.

use crate::types::{InstalledProgram, LaunchResult, ServiceError};
use std::collections::HashSet;
use std::path::Path;

#[cfg(windows)]
use winreg::enums::{
    HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY,
};
#[cfg(windows)]
use winreg::RegKey;

/// Раскрывает системные переменные окружения вида `%VAR%` в строке.
pub fn expand_environment_variables(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '%' {
            let mut var_name = String::new();
            let mut closed = false;
            while let Some(&next_c) = chars.peek() {
                chars.next();
                if next_c == '%' {
                    closed = true;
                    break;
                }
                var_name.push(next_c);
            }
            if closed && !var_name.is_empty() {
                if let Ok(val) = std::env::var(&var_name) {
                    result.push_str(&val);
                } else {
                    result.push('%');
                    result.push_str(&var_name);
                    result.push('%');
                }
            } else {
                result.push('%');
                result.push_str(&var_name);
            }
        } else {
            result.push(c);
        }
    }
    result
}

/// Токенизирует аргументы командной строки с учётом кавычек.
pub fn tokenize_arguments(args_str: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;

    for c in args_str.chars() {
        match c {
            '"' => {
                in_quotes = !in_quotes;
            }
            ' ' | '\t' if !in_quotes => {
                if !current.is_empty() {
                    args.push(current);
                    current = String::new();
                }
            }
            _ => {
                current.push(c);
            }
        }
    }
    if !current.is_empty() {
        args.push(current);
    }
    args
}

/// Проверяет, является ли имя исполняемого файла запрещённым интерпретатором оболочки.
pub fn is_forbidden_interpreter(path: &str) -> bool {
    let filename = Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path)
        .to_lowercase();

    matches!(
        filename.as_str(),
        "cmd.exe"
            | "cmd"
            | "powershell.exe"
            | "powershell"
            | "pwsh.exe"
            | "pwsh"
            | "wscript.exe"
            | "cscript.exe"
            | "bash.exe"
            | "sh.exe"
    )
}

/// Безопасно парсит строку деинсталляции на исполняемый файл и список аргументов.
///
/// Обрабатывает:
/// - MsiExec.exe вызовы с параметрами (/X, /I, GUID)
/// - Заключённые в кавычки пути с пробелами
/// - Не заключённые в кавычки пути с пробелами
/// - Экспансию переменных окружения (%SystemRoot%, %ProgramFiles%)
/// - Запрет cmd.exe / powershell.exe
pub fn parse_uninstaller_command(cmd_line: &str) -> Result<(String, Vec<String>), ServiceError> {
    let trimmed = cmd_line.trim();
    if trimmed.is_empty() {
        return Err(ServiceError::InvalidCommand(
            "Строка деинсталляции пуста".to_string(),
        ));
    }

    let expanded = expand_environment_variables(trimmed);
    let expanded_trim = expanded.trim();

    // 1. Проверка на MsiExec вызовы
    let lower = expanded_trim.to_ascii_lowercase();
    if lower.starts_with("msiexec.exe") || lower.starts_with("msiexec ") || lower == "msiexec" {
        let msiexec_path = crate::tools::get_system32_directory()
            .join("msiexec.exe")
            .to_string_lossy()
            .into_owned();

        let rest = if lower.starts_with("msiexec.exe") {
            expanded_trim["msiexec.exe".len()..].trim()
        } else {
            expanded_trim["msiexec".len()..].trim()
        };
        let args = tokenize_arguments(rest);
        return Ok((msiexec_path, args));
    }

    // 2. Выделение бинарника и аргументов
    let (raw_exe, raw_args) = if let Some(stripped) = expanded_trim.strip_prefix('"') {
        // Путь в кавычках: ищем закрывающую кавычку
        if let Some(close_idx) = stripped.find('"') {
            let exe = &stripped[..close_idx];
            let rest = &stripped[close_idx + 1..];
            (exe.trim().to_string(), rest.trim())
        } else {
            (stripped.to_string(), "")
        }
    } else {
        // Путь без кавычек: ищем расширение исполняемого файла (.exe, .bat, .cmd)
        let lower = expanded_trim.to_ascii_lowercase();
        let mut split_pos = None;
        for ext in &[".exe", ".bat", ".cmd", ".com"] {
            let mut search_from = 0;
            while let Some(pos) = lower[search_from..].find(ext) {
                let actual_pos = search_from + pos + ext.len();
                if actual_pos == expanded_trim.len()
                    || expanded_trim.as_bytes()[actual_pos].is_ascii_whitespace()
                {
                    split_pos = Some(actual_pos);
                    break;
                }
                search_from = actual_pos;
            }
            if split_pos.is_some() {
                break;
            }
        }

        if let Some(pos) = split_pos {
            let exe = &expanded_trim[..pos];
            let rest = &expanded_trim[pos..];
            (exe.trim().to_string(), rest.trim())
        } else if let Some(space_pos) = expanded_trim.find(' ') {
            let exe = &expanded_trim[..space_pos];
            let rest = &expanded_trim[space_pos..];
            (exe.trim().to_string(), rest.trim())
        } else {
            (expanded_trim.to_string(), "")
        }
    };

    let exe_clean = raw_exe.trim_matches('"').to_string();

    // 3. Запрет командных оболочек
    if is_forbidden_interpreter(&exe_clean) {
        return Err(ServiceError::InvalidCommand(format!(
            "Запрещён запуск командных оболочек или скриптовых хостов: {}",
            exe_clean
        )));
    }

    if !Path::new(&exe_clean).is_absolute()
        || !exe_clean.to_ascii_lowercase().ends_with(".exe")
        || exe_clean.chars().any(|c| c == '\0')
        || Path::new(&exe_clean)
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err(ServiceError::InvalidCommand(
            "Требуется абсолютный путь к EXE; скрипты и поиск по PATH запрещены".into(),
        ));
    }
    let args = tokenize_arguments(raw_args);
    Ok((exe_clean, args))
}

/// Получает список всех установленных программ из 4 веток реестра Windows.
pub fn list_installed_programs(
    include_system_components: bool,
) -> Result<Vec<InstalledProgram>, ServiceError> {
    #[cfg(windows)]
    {
        list_installed_programs_windows(include_system_components)
    }
    #[cfg(not(windows))]
    {
        let _ = include_system_components;
        Ok(Vec::new())
    }
}

#[cfg(windows)]
fn list_installed_programs_windows(
    include_system_components: bool,
) -> Result<Vec<InstalledProgram>, ServiceError> {
    let mut programs = Vec::new();
    let mut seen_ids = HashSet::new();

    // 4 ветки реестра: HKLM 64/32, HKCU 64/32
    let targets = [
        (
            HKEY_LOCAL_MACHINE,
            "HKLM",
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
            KEY_READ | KEY_WOW64_64KEY,
            false,
        ),
        (
            HKEY_LOCAL_MACHINE,
            "HKLM",
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
            KEY_READ | KEY_WOW64_32KEY,
            true,
        ),
        (
            HKEY_CURRENT_USER,
            "HKCU",
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
            KEY_READ | KEY_WOW64_64KEY,
            false,
        ),
        (
            HKEY_CURRENT_USER,
            "HKCU",
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
            KEY_READ | KEY_WOW64_32KEY,
            true,
        ),
    ];

    for (root_hkey, hive_name, path, flags, is_wow64) in targets {
        let root = RegKey::predef(root_hkey);
        let uninstall_key = match root.open_subkey_with_flags(path, flags) {
            Ok(k) => k,
            Err(e) => {
                tracing::debug!("Ветка реестра {}/{} недоступна: {}", hive_name, path, e);
                continue;
            }
        };

        for subkey_name_res in uninstall_key.enum_keys() {
            let subkey_name = match subkey_name_res {
                Ok(name) => name,
                Err(_) => continue,
            };

            let program_key = match uninstall_key.open_subkey_with_flags(&subkey_name, flags) {
                Ok(k) => k,
                Err(_) => continue,
            };

            // DisplayName — обязательное поле
            let display_name: String = match program_key.get_value("DisplayName") {
                Ok(name) => name,
                Err(_) => continue,
            };

            let trimmed_name = display_name.trim();
            if trimmed_name.is_empty() {
                continue;
            }

            // Проверка системного компонента
            let system_component: u32 = program_key.get_value("SystemComponent").unwrap_or(0);
            let parent_key_name: Option<String> = program_key.get_value("ParentKeyName").ok();
            let is_system = system_component == 1
                || parent_key_name
                    .as_ref()
                    .is_some_and(|p| !p.trim().is_empty());

            if !include_system_components && is_system {
                continue;
            }

            let publisher: Option<String> = program_key
                .get_value("Publisher")
                .ok()
                .map(|s: String| s.trim().to_string())
                .filter(|s| !s.is_empty());

            let display_version: Option<String> = program_key
                .get_value("DisplayVersion")
                .ok()
                .map(|s: String| s.trim().to_string())
                .filter(|s| !s.is_empty());

            let install_date: Option<String> = program_key
                .get_value("InstallDate")
                .ok()
                .map(|s: String| s.trim().to_string())
                .filter(|s| !s.is_empty());

            // EstimatedSize в KiB -> конвертируем в байты
            let estimated_size_bytes: Option<u64> = program_key
                .get_value::<u32, _>("EstimatedSize")
                .ok()
                .map(|kib| (kib as u64) * 1024)
                .or_else(|| {
                    program_key
                        .get_value::<u64, _>("EstimatedSize")
                        .ok()
                        .map(|kib| kib * 1024)
                });

            let uninstall_string: Option<String> = program_key
                .get_value("UninstallString")
                .ok()
                .map(|s: String| s.trim().to_string())
                .filter(|s| !s.is_empty());

            let quiet_uninstall_string: Option<String> = program_key
                .get_value("QuietUninstallString")
                .ok()
                .map(|s: String| s.trim().to_string())
                .filter(|s| !s.is_empty());

            let install_location: Option<String> = program_key
                .get_value("InstallLocation")
                .ok()
                .map(|s: String| s.trim().to_string())
                .filter(|s| !s.is_empty());

            let id = format!(
                "{}_{}_{}",
                hive_name,
                if is_wow64 { "32" } else { "64" },
                subkey_name
            );
            // HKCU Uninstall is shared between registry views on Windows.
            let dedup_key = if hive_name == "HKCU" {
                format!("HKCU_{subkey_name}")
            } else {
                id.clone()
            };
            if !seen_ids.insert(dedup_key) {
                continue;
            }

            programs.push(InstalledProgram {
                id,
                display_name: trimmed_name.to_string(),
                publisher,
                display_version,
                install_date,
                estimated_size_bytes,
                uninstall_string,
                quiet_uninstall_string,
                install_location,
                registry_hive: hive_name.to_string(),
                is_wow64,
                is_system_component: is_system,
            });
        }
    }

    programs.sort_by_key(|a| a.display_name.to_lowercase());
    Ok(programs)
}

/// Запускает официальный деинсталлятор программы по её идентификатору.
pub fn launch_uninstaller(program_id: &str) -> Result<LaunchResult, ServiceError> {
    let programs = list_installed_programs(true)?;
    let program = programs
        .into_iter()
        .find(|p| p.id == program_id)
        .ok_or_else(|| ServiceError::ProgramNotFound(program_id.to_string()))?;

    let cmd_line = program
        .uninstall_string
        .ok_or_else(|| ServiceError::NoUninstallString(program.display_name.clone()))?;

    let (exe_path, args) = parse_uninstaller_command(&cmd_line)?;

    #[cfg(windows)]
    {
        if !matches!(Path::new(&exe_path).components().next(), Some(std::path::Component::Prefix(prefix)) if matches!(prefix.kind(), std::path::Prefix::Disk(_) | std::path::Prefix::VerbatimDisk(_)))
        {
            return Err(ServiceError::InvalidCommand(
                "Запуск с сетевых путей не поддерживается".into(),
            ));
        }
        if !Path::new(&exe_path).is_file() {
            return Err(ServiceError::ExecutableNotFound(exe_path));
        }

        let mut cmd = std::process::Command::new(&exe_path);
        cmd.args(&args);

        match cmd.spawn() {
            Ok(child) => {
                let pid = child.id();
                tracing::info!(
                    "Деинсталлятор программы '{}' запущен (PID {})",
                    program.display_name,
                    pid
                );
                Ok(LaunchResult {
                    success: true,
                    pid: Some(pid),
                    message: format!(
                        "Деинсталлятор программы '{}' успешно запущен (PID {})",
                        program.display_name, pid
                    ),
                })
            }
            Err(e) => Err(ServiceError::ProcessLaunchFailed(format!(
                "Не удалось запустить деинсталлятор '{}': {}",
                exe_path, e
            ))),
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (exe_path, args);
        Err(ServiceError::ProcessLaunchFailed(
            "Запуск деинсталлятора поддерживается только на Windows".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_quoted_command_with_arguments() {
        let cmd = r#""C:\Program Files\VideoLAN\VLC\uninstall.exe" /S /NORESTART"#;
        let (exe, args) = parse_uninstaller_command(cmd).expect("Parse quoted command");
        assert_eq!(exe, r#"C:\Program Files\VideoLAN\VLC\uninstall.exe"#);
        assert_eq!(args, vec!["/S", "/NORESTART"]);
    }

    #[test]
    fn test_parse_quoted_command_without_arguments() {
        let cmd = r#""C:\Program Files (x86)\Notepad++\uninstall.exe""#;
        let (exe, args) = parse_uninstaller_command(cmd).expect("Parse quoted command");
        assert_eq!(exe, r#"C:\Program Files (x86)\Notepad++\uninstall.exe"#);
        assert!(args.is_empty());
    }

    #[test]
    fn test_parse_unquoted_command_with_spaces() {
        let cmd = r#"C:\Program Files\App\unins000.exe -uninstall -silent"#;
        let (exe, args) = parse_uninstaller_command(cmd).expect("Parse unquoted command");
        assert_eq!(exe, r#"C:\Program Files\App\unins000.exe"#);
        assert_eq!(args, vec!["-uninstall", "-silent"]);
    }

    #[test]
    fn test_parse_msiexec_command() {
        let cmd = "MsiExec.exe /X{12345678-ABCD-1234-ABCD-1234567890AB} /qn";
        let (exe, args) = parse_uninstaller_command(cmd).expect("Parse msiexec command");
        assert!(exe.to_lowercase().ends_with("msiexec.exe"));
        assert_eq!(
            args,
            vec!["/X{12345678-ABCD-1234-ABCD-1234567890AB}", "/qn"]
        );
    }

    #[test]
    fn test_parse_msiexec_without_extension() {
        let cmd = "msiexec /I{GUID-HERE}";
        let (exe, args) = parse_uninstaller_command(cmd).expect("Parse msiexec without ext");
        assert!(exe.to_lowercase().ends_with("msiexec.exe"));
        assert_eq!(args, vec!["/I{GUID-HERE}"]);
    }

    #[test]
    fn test_rejects_cmd_and_powershell() {
        let cmd1 = "cmd.exe /c del /f C:\\file.txt";
        let res1 = parse_uninstaller_command(cmd1);
        assert!(matches!(res1, Err(ServiceError::InvalidCommand(_))));

        let cmd2 = "powershell.exe -ExecutionPolicy Bypass -File script.ps1";
        let res2 = parse_uninstaller_command(cmd2);
        assert!(matches!(res2, Err(ServiceError::InvalidCommand(_))));

        let cmd3 = "wscript.exe malware.vbs";
        let res3 = parse_uninstaller_command(cmd3);
        assert!(matches!(res3, Err(ServiceError::InvalidCommand(_))));
    }

    #[test]
    fn test_empty_command_returns_error() {
        let res = parse_uninstaller_command("   ");
        assert!(matches!(res, Err(ServiceError::InvalidCommand(_))));
    }

    #[test]
    fn test_expand_environment_variables() {
        let path = "%SystemRoot%\\System32\\calc.exe";
        let expanded = expand_environment_variables(path);
        assert!(!expanded.contains("%SystemRoot%"));
        assert!(expanded.to_lowercase().contains("system32"));
    }
}
