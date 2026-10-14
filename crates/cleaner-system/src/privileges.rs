//! Read-only current-process elevation query. Never requests elevation.
#[cfg(windows)]
pub fn is_elevated() -> Option<bool> {
    use windows_sys::Win32::{
        Foundation::CloseHandle,
        Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY},
        System::Threading::{GetCurrentProcess, OpenProcessToken},
    };
    let mut token = std::ptr::null_mut();
    // SAFETY: valid current process pseudo-handle and writable token output.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return None;
    }
    let mut elevation: TOKEN_ELEVATION = unsafe { std::mem::zeroed() };
    let mut size = 0;
    // SAFETY: live token and correctly sized writable TOKEN_ELEVATION buffer.
    let ok = unsafe {
        GetTokenInformation(
            token,
            TokenElevation,
            (&mut elevation as *mut TOKEN_ELEVATION).cast(),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut size,
        )
    } != 0;
    // SAFETY: owned token handle, released exactly once.
    unsafe { CloseHandle(token) };
    if ok {
        Some(elevation.TokenIsElevated != 0)
    } else {
        None
    }
}
#[cfg(not(windows))]
pub fn is_elevated() -> Option<bool> {
    None
}
