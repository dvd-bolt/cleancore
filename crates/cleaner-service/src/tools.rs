//! Модуль 6.5: Безопасный переход к штатным инструментам обслуживания Windows.

use crate::types::{LaunchResult, ServiceError, SystemTool, SystemToolId};
use std::path::PathBuf;

/// Определяет абсолютный путь к системному каталогу System32.
///
/// Разрешается динамически через системную переменную окружения `SystemRoot` или `WINDIR`
/// с безопасным резервным значением `C:\Windows\System32`.
pub fn get_system32_directory() -> PathBuf {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStringExt;
        let mut buffer = vec![0u16; 32768];
        // SAFETY: buffer is writable and its capacity is passed in UTF-16 units.
        let length = unsafe {
            windows_sys::Win32::System::SystemInformation::GetSystemDirectoryW(
                buffer.as_mut_ptr(),
                buffer.len() as u32,
            )
        } as usize;
        if length > 0 && length < buffer.len() {
            return PathBuf::from(std::ffi::OsString::from_wide(&buffer[..length]));
        }
        // No environment/PATH fallback can authorize execution.
        PathBuf::new()
    }
    #[cfg(not(windows))]
    {
        PathBuf::from(r"C:\Windows\System32")
    }
}

/// Возвращает метаданные, целевой исполняемый файл и аргументы для указанной утилиты.
pub fn get_tool_metadata(id: SystemToolId) -> (SystemTool, PathBuf, Vec<String>) {
    let sys32 = get_system32_directory();
    match id {
        SystemToolId::Cleanmgr => {
            let exe = sys32.join("cleanmgr.exe");
            let is_available = exe.is_file();
            (
                SystemTool {
                    id,
                    name: "Очистка диска".to_string(),
                    description: "Штатная утилита Windows для удаления системных временных файлов и предыдущих версий ОС".to_string(),
                    executable: exe.to_string_lossy().to_string(),
                    arguments: Vec::new(),
                    is_available,
                },
                exe,
                Vec::new(),
            )
        }
        SystemToolId::Dfrgui => {
            let exe = sys32.join("dfrgui.exe");
            let is_available = exe.is_file();
            (
                SystemTool {
                    id,
                    name: "Оптимизация дисков".to_string(),
                    description: "Дефрагментация жестких дисков (HDD) и TRIM-оптимизация твердотельных накопителей (SSD)".to_string(),
                    executable: exe.to_string_lossy().to_string(),
                    arguments: Vec::new(),
                    is_available,
                },
                exe,
                Vec::new(),
            )
        }
        SystemToolId::Resmon => {
            let exe = sys32.join("resmon.exe");
            let is_available = exe.is_file();
            (
                SystemTool {
                    id,
                    name: "Монитор ресурсов".to_string(),
                    description: "Детальный мониторинг процессов, потоков, дискового ввода-вывода и сетевых сокетов в реальном времени".to_string(),
                    executable: exe.to_string_lossy().to_string(),
                    arguments: Vec::new(),
                    is_available,
                },
                exe,
                Vec::new(),
            )
        }
        SystemToolId::Taskmgr => {
            let exe = sys32.join("Taskmgr.exe");
            let is_available = exe.is_file();
            (
                SystemTool {
                    id,
                    name: "Диспетчер задач".to_string(),
                    description: "Штатный инструмент Windows для управления процессами, службами, автозагрузкой и мониторинга системы".to_string(),
                    executable: exe.to_string_lossy().to_string(),
                    arguments: Vec::new(),
                    is_available,
                },
                exe,
                Vec::new(),
            )
        }
        SystemToolId::Appwiz => {
            let exe = sys32.join("control.exe");
            let is_available = exe.is_file();
            (
                SystemTool {
                    id,
                    name: "Программы и компоненты".to_string(),
                    description: "Классический апплет панели управления Windows для удаления и изменения установленного программного обеспечения".to_string(),
                    executable: exe.to_string_lossy().to_string(),
                    arguments: vec!["appwiz.cpl".to_string()],
                    is_available,
                },
                exe,
                vec!["appwiz.cpl".to_string()],
            )
        }
    }
}

/// Возвращает список всех зарегистрированных штатных системных утилит.
pub fn list_system_tools() -> Vec<SystemTool> {
    let tool_ids = [
        SystemToolId::Cleanmgr,
        SystemToolId::Dfrgui,
        SystemToolId::Resmon,
        SystemToolId::Taskmgr,
        SystemToolId::Appwiz,
    ];

    tool_ids.iter().map(|&id| get_tool_metadata(id).0).collect()
}

/// Запускает штатную утилиту Windows строго по белому списку из доверенного каталога System32.
///
/// Запрещён запуск произвольных файлов, относительных путей или интерпретаторов оболочки.
pub fn launch_system_tool(tool_id: SystemToolId) -> Result<LaunchResult, ServiceError> {
    let (tool, exe_path, args) = get_tool_metadata(tool_id);

    // Строгая проверка каталога размещения (только System32)
    let sys32 = get_system32_directory();
    if !sys32.is_absolute() || !exe_path.starts_with(&sys32) {
        return Err(ServiceError::UnauthorizedTool(format!(
            "Исполняемый файл '{}' находится вне доверенного каталога System32",
            exe_path.display()
        )));
    }

    #[cfg(windows)]
    {
        if !exe_path.is_file() {
            return Err(ServiceError::ExecutableNotFound(format!(
                "Системная утилита '{}' не найдена по пути '{}'",
                tool.name,
                exe_path.display()
            )));
        }

        let mut cmd = std::process::Command::new(&exe_path);
        cmd.args(&args);

        match cmd.spawn() {
            Ok(child) => {
                let pid = child.id();
                tracing::info!(
                    "Системная утилита '{}' успешно запущена (PID {})",
                    tool.name,
                    pid
                );
                Ok(LaunchResult {
                    success: true,
                    pid: Some(pid),
                    message: format!("Утилита '{}' успешно запущена (PID {})", tool.name, pid),
                })
            }
            Err(e) => Err(ServiceError::ProcessLaunchFailed(format!(
                "Не удалось запустить утилиту '{}': {}",
                tool.name, e
            ))),
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (exe_path, args);
        Err(ServiceError::ProcessLaunchFailed(
            "Запуск системных утилит Windows поддерживается только на платформе Windows"
                .to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_system_tools_returns_all_five_tools() {
        let tools = list_system_tools();
        assert_eq!(tools.len(), 5);

        let ids: Vec<SystemToolId> = tools.iter().map(|t| t.id).collect();
        assert!(ids.contains(&SystemToolId::Cleanmgr));
        assert!(ids.contains(&SystemToolId::Dfrgui));
        assert!(ids.contains(&SystemToolId::Resmon));
        assert!(ids.contains(&SystemToolId::Taskmgr));
        assert!(ids.contains(&SystemToolId::Appwiz));
    }

    #[test]
    fn test_all_tools_resolve_into_system32() {
        let sys32 = get_system32_directory();
        for tool_id in [
            SystemToolId::Cleanmgr,
            SystemToolId::Dfrgui,
            SystemToolId::Resmon,
            SystemToolId::Taskmgr,
            SystemToolId::Appwiz,
        ] {
            let (_, exe_path, _) = get_tool_metadata(tool_id);
            assert!(
                exe_path.starts_with(&sys32),
                "Tool {:?} path '{}' must be in System32 '{}'",
                tool_id,
                exe_path.display(),
                sys32.display()
            );
        }
    }

    #[test]
    fn test_appwiz_has_cpl_argument() {
        let (tool, _, args) = get_tool_metadata(SystemToolId::Appwiz);
        assert_eq!(tool.arguments, vec!["appwiz.cpl"]);
        assert_eq!(args, vec!["appwiz.cpl"]);
    }

    #[test]
    fn test_system_tool_id_serialization() {
        let json = serde_json::to_string(&SystemToolId::Cleanmgr).unwrap();
        assert_eq!(json, "\"cleanmgr\"");

        let deserialized: SystemToolId = serde_json::from_str("\"cleanmgr\"").unwrap();
        assert_eq!(deserialized, SystemToolId::Cleanmgr);

        let invalid = serde_json::from_str::<SystemToolId>("\"cmd.exe\"");
        assert!(invalid.is_err());
    }
}
