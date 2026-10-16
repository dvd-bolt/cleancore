use cleaner_core::errors::ApiError;

/// Select a folder without starting analysis or changing its contents.
#[tauri::command]
pub async fn choose_analysis_folder(
    window: tauri::WebviewWindow,
) -> Result<Option<String>, ApiError> {
    #[cfg(windows)]
    {
        let owner = window.hwnd().map_err(|e| dialog_error(e.to_string()))?.0 as isize;
        tauri::async_runtime::spawn_blocking(move || choose_folder(owner))
            .await
            .map_err(|e| dialog_error(e.to_string()))?
    }
    #[cfg(not(windows))]
    {
        let _ = window;
        Err(dialog_error("Выбор папки доступен в Windows".into()))
    }
}

fn dialog_error(message: String) -> ApiError {
    ApiError {
        code: "FOLDER_DIALOG".into(),
        message,
    }
}

#[cfg(windows)]
fn choose_folder(owner: isize) -> Result<Option<String>, ApiError> {
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::System::Com::{
        CoInitializeEx, CoTaskMemFree, CoUninitialize, COINIT_APARTMENTTHREADED,
    };
    use windows_sys::Win32::UI::Shell::{
        SHBrowseForFolderW, SHGetPathFromIDListEx, BIF_NEWDIALOGSTYLE, BIF_NONEWFOLDERBUTTON,
        BIF_RETURNONLYFSDIRS, BROWSEINFOW,
    };

    // SAFETY: COM is initialized and balanced on this dedicated blocking thread.
    let hr = unsafe { CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as u32) };
    if hr < 0 {
        return Err(dialog_error(format!(
            "Не удалось открыть выбор папки: {hr:#x}"
        )));
    }
    struct ComGuard;
    impl Drop for ComGuard {
        fn drop(&mut self) {
            unsafe { CoUninitialize() };
        }
    }
    let _com = ComGuard;
    let title: Vec<u16> = "Выберите папку для анализа\0".encode_utf16().collect();
    let mut display_name = vec![0u16; 260];
    // SAFETY: zero initializes optional pointers; all supplied buffers outlive the dialog.
    let mut dialog: BROWSEINFOW = unsafe { std::mem::zeroed() };
    dialog.hwndOwner = owner as _;
    dialog.pszDisplayName = display_name.as_mut_ptr();
    dialog.lpszTitle = title.as_ptr();
    dialog.ulFlags = BIF_RETURNONLYFSDIRS | BIF_NEWDIALOGSTYLE | BIF_NONEWFOLDERBUTTON;
    let item = unsafe { SHBrowseForFolderW(&dialog) };
    if item.is_null() {
        return Ok(None);
    }
    let mut path = vec![0u16; 32768];
    // SAFETY: item is the live shell allocation and path has the stated capacity.
    let success = unsafe { SHGetPathFromIDListEx(item, path.as_mut_ptr(), path.len() as u32, 0) };
    unsafe { CoTaskMemFree(item.cast()) };
    if success == 0 {
        return Err(dialog_error("Windows не вернула путь к папке".into()));
    }
    let end = path.iter().position(|&c| c == 0).unwrap_or(path.len());
    let path = std::ffi::OsString::from_wide(&path[..end])
        .into_string()
        .map_err(|_| dialog_error("Путь содержит неподдерживаемые символы".into()))?;
    Ok(Some(path))
}
