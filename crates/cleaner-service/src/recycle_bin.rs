//! Безопасное взаимодействие с системной Корзиной Windows (OPT-04).
//!
//! Использует Win32 Shell API (`SHQueryRecycleBinW` и `SHEmptyRecycleBinW`).
//! Не требует повышенных привилегий и не использует произвольные команды powershell/cmd.

use crate::types::RecycleBinInfo;

/// Запрашивает текущий объём и количество файлов в Корзине Windows.
#[cfg(windows)]
pub fn query_recycle_bin() -> Result<RecycleBinInfo, String> {
    use std::mem::size_of;
    use windows_sys::Win32::UI::Shell::{SHQueryRecycleBinW, SHQUERYRBINFO};

    let mut info = SHQUERYRBINFO {
        cbSize: size_of::<SHQUERYRBINFO>() as u32,
        i64Size: 0,
        i64NumItems: 0,
    };

    // Передача null в pszRootPath опрашивает корзины всех подключенных томов
    let hr = unsafe { SHQueryRecycleBinW(std::ptr::null(), &mut info) };
    if hr < 0 {
        return Err(format!("Не удалось опросить Корзину Windows ({hr:#x})"));
    }

    Ok(RecycleBinInfo {
        total_bytes: info.i64Size.max(0) as u64,
        item_count: info.i64NumItems.max(0) as u64,
    })
}

/// Выполняет безопасную очистку Корзины Windows без всплывающих диалогов.
/// Возвращает снимок объёма и количества файлов, которые были очищены.
#[cfg(windows)]
pub fn empty_recycle_bin() -> Result<RecycleBinInfo, String> {
    use windows_sys::Win32::UI::Shell::{
        SHEmptyRecycleBinW, SHERB_NOCONFIRMATION, SHERB_NOPROGRESSUI, SHERB_NOSOUND,
    };

    let before = query_recycle_bin()?;

    let flags = SHERB_NOCONFIRMATION | SHERB_NOPROGRESSUI | SHERB_NOSOUND;
    let hr = unsafe { SHEmptyRecycleBinW(std::ptr::null_mut(), std::ptr::null(), flags) };
    if hr < 0 {
        return Err(format!("Не удалось очистить Корзину Windows ({hr:#x})"));
    }

    Ok(before)
}

#[cfg(not(windows))]
pub fn query_recycle_bin() -> Result<RecycleBinInfo, String> {
    Ok(RecycleBinInfo {
        total_bytes: 0,
        item_count: 0,
    })
}

#[cfg(not(windows))]
pub fn empty_recycle_bin() -> Result<RecycleBinInfo, String> {
    Ok(RecycleBinInfo {
        total_bytes: 0,
        item_count: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_recycle_bin_runs_without_panics() {
        let res = query_recycle_bin();
        assert!(res.is_ok(), "query_recycle_bin should succeed on Windows");
        let info = res.unwrap();
        assert!(info.item_count <= 1_000_000_000);
    }
}
