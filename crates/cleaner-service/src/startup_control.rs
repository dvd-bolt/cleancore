//! Narrow HKCU StartupApproved control. Run commands/files are never deleted or executed.
use crate::types::ServiceError;
#[cfg(windows)]
use winreg::{enums::*, RegKey, RegValue};
fn error(message: &str) -> ServiceError {
    ServiceError::RegistryError(message.into())
}
pub fn set_enabled(
    id: &str,
    expected_command: &str,
    enabled: bool,
    acknowledged: bool,
) -> Result<(), ServiceError> {
    if !acknowledged {
        return Err(error("Требуется подтверждение изменения автозагрузки"));
    }
    if id.len() > 2048 || expected_command.len() > 32768 {
        return Err(error("Слишком длинная запись"));
    }
    #[cfg(windows)]
    {
        let items = crate::startup::list_startup_items()?;
        let item = items
            .iter()
            .find(|i| i.id == id)
            .ok_or_else(|| error("Запись исчезла; обновите список"))?;
        if item.command != expected_command {
            return Err(error("Команда изменилась; обновите список"));
        }
        let (sub, name) = if id.starts_with("HKCU_Run_") {
            ("Run", item.name.as_str())
        } else if let Some(file) = id.strip_prefix("StartupFolderUser_") {
            ("StartupFolder", file)
        } else {
            return Err(error(
                "Для системных записей и RunOnce используйте Диспетчер задач",
            ));
        };
        if name.is_empty()
            || name
                .chars()
                .any(|c| c == '/' || c as u32 == 92 || c as u32 == 0)
            || name.len() > 512
        {
            return Err(error("Неподдерживаемое имя записи"));
        }
        let root = RegKey::predef(HKEY_CURRENT_USER);
        let path =
            format!(r"SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\{sub}");
        let (key, _) = root
            .create_subkey_with_flags(path, KEY_READ | KEY_WRITE | KEY_WOW64_64KEY)
            .map_err(|e| error(&e.to_string()))?;
        write_approved(&key, name, enabled)
            .map_err(|e| error(&format!("Windows не приняла изменение: {e}")))
    }
    #[cfg(not(windows))]
    {
        let _ = (id, expected_command, enabled);
        Err(error("Доступно только в Windows"))
    }
}
#[cfg(windows)]
fn write_approved(key: &RegKey, name: &str, enabled: bool) -> std::io::Result<()> {
    use std::io::{Error, ErrorKind};
    let previous = match key.get_raw_value(name) {
        Ok(raw) => Some(raw),
        Err(e) if e.kind() == ErrorKind::NotFound => None,
        Err(e) => return Err(e),
    };
    if previous.as_ref().is_some_and(|raw| {
        raw.vtype != REG_BINARY || raw.bytes.len() != 12 || !matches!(raw.bytes[0], 2 | 3 | 6 | 7)
    }) {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "Неизвестный формат StartupApproved; используйте настройки Windows",
        ));
    }
    let mut bytes = previous
        .as_ref()
        .map(|raw| raw.bytes.clone())
        .unwrap_or_else(|| vec![0u8; 12]);
    let base = if bytes[0] == 6 || bytes[0] == 7 { 6 } else { 2 };
    bytes[0] = base + u8::from(!enabled);
    bytes[4..].fill(0);
    if !enabled {
        let ticks = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
            / 100
            + 116_444_736_000_000_000) as u64;
        bytes[4..].copy_from_slice(&ticks.to_le_bytes());
    }
    key.set_raw_value(
        name,
        &RegValue {
            bytes: bytes.clone(),
            vtype: REG_BINARY,
        },
    )?;
    if key.get_raw_value(name)?.bytes != bytes {
        return Err(Error::other(
            "Состояние изменено другой программой; обновите список",
        ));
    }
    Ok(())
}
#[cfg(all(test, windows))]
mod tests {
    use super::*;
    #[test]
    fn toggles_only_own_fixture_value_and_refuses_unknown_format() {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let path = format!(
            "Software\\\\CleanCoreTests\\\\Startup-{}",
            std::process::id()
        );
        let (key, _) = hkcu.create_subkey(&path).unwrap();
        write_approved(&key, "Fixture", false).unwrap();
        let raw = key.get_raw_value("Fixture").unwrap();
        assert_eq!(raw.bytes.len(), 12);
        assert_eq!(raw.bytes[0], 3);
        assert_eq!(raw.vtype, REG_BINARY);
        write_approved(&key, "Fixture", true).unwrap();
        assert_eq!(key.get_raw_value("Fixture").unwrap().bytes[0], 2);
        key.set_value("Unknown", &"leave unchanged").unwrap();
        assert!(write_approved(&key, "Unknown", false).is_err());
        assert_eq!(
            key.get_value::<String, _>("Unknown").unwrap(),
            "leave unchanged"
        );
        drop(key);
        hkcu.delete_subkey_all(path).unwrap();
        assert!(set_enabled("HKLM_Run_X", "test", false, false).is_err());
    }
}
