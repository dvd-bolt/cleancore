//! The category root comes from Windows Known Folders, never TEMP/LOCALAPPDATA.
use cleaner_core::errors::CoreError;
use std::path::{Component, Path, PathBuf};

pub fn is_reparse_point_or_symlink(path: &Path) -> bool {
    match std::fs::symlink_metadata(path) {
        Ok(meta) => {
            if meta.file_type().is_symlink() {
                return true;
            }
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                meta.file_attributes() & 0x400 != 0
            }
            #[cfg(not(windows))]
            {
                false
            }
        }
        Err(_) => true,
    }
}

pub fn resolve_user_temp_root() -> Result<PathBuf, CoreError> {
    let candidate = local_app_data()?.join("Temp");
    validate_temp_root_boundary(&candidate)?;
    let _lease =
        cleaner_execution::native::lock_directory(&candidate, Path::new("")).map_err(|code| {
            CoreError::InvalidPath(format!(
                "Временный каталог недоступен или перенаправлен: {code}"
            ))
        })?;
    // Do not silently fall back to an unverified path.
    candidate
        .canonicalize()
        .map_err(|_| CoreError::InvalidPath("Не удалось проверить временный каталог".into()))
}

#[cfg(windows)]
fn local_app_data() -> Result<PathBuf, CoreError> {
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::{
        System::Com::CoTaskMemFree,
        UI::Shell::{FOLDERID_LocalAppData, SHGetKnownFolderPath},
    };
    let mut output = std::ptr::null_mut();
    // SAFETY: valid known-folder GUID and writable output; Windows allocates a NUL-terminated UTF-16 string.
    let result = unsafe {
        SHGetKnownFolderPath(&FOLDERID_LocalAppData, 0, std::ptr::null_mut(), &mut output)
    };
    if result < 0 || output.is_null() {
        return Err(CoreError::InvalidPath(
            "Windows не предоставила LocalAppData текущего пользователя".into(),
        ));
    }
    let mut length = 0;
    // Windows paths are bounded to 32767 UTF-16 code units.
    while length < 32768 && unsafe { *output.add(length) } != 0 {
        length += 1;
    }
    let path = if length < 32768 {
        // SAFETY: length was bounded by the NUL terminator in the returned Windows string.
        Some(PathBuf::from(std::ffi::OsString::from_wide(unsafe {
            std::slice::from_raw_parts(output, length)
        })))
    } else {
        None
    };
    // SAFETY: the pointer was allocated by SHGetKnownFolderPath and is released exactly once.
    unsafe { CoTaskMemFree(output.cast()) };
    path.ok_or_else(|| CoreError::InvalidPath("Некорректный Known Folder path".into()))
}
#[cfg(not(windows))]
fn local_app_data() -> Result<PathBuf, CoreError> {
    Err(CoreError::InvalidPath(
        "Категория поддерживается только в Windows".into(),
    ))
}

/// Lexical rejection complements (but never replaces) native ancestor checks.
pub fn validate_temp_root_boundary(root: &Path) -> Result<(), CoreError> {
    if !root.is_absolute()
        || root
            .components()
            .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
    {
        return Err(CoreError::InvalidPath(
            "Корень должен быть абсолютным без traversal".into(),
        ));
    }
    #[cfg(windows)]
    if !matches!(root.components().next(),Some(Component::Prefix(p)) if matches!(p.kind(),std::path::Prefix::Disk(_)|std::path::Prefix::VerbatimDisk(_)))
    {
        return Err(CoreError::InvalidPath(
            "Сетевые и device-корни не поддерживаются".into(),
        ));
    }
    let normal: Vec<_> = root
        .components()
        .filter_map(|c| {
            if let Component::Normal(n) = c {
                Some(n.to_string_lossy().to_ascii_lowercase())
            } else {
                None
            }
        })
        .collect();
    if normal.last().map(String::as_str) != Some("temp")
        || normal.iter().any(|part| {
            part.contains([':', '\0'])
                || [
                    "windows",
                    "system32",
                    "program files",
                    "program files (x86)",
                    "programdata",
                ]
                .contains(&part.as_str())
        })
    {
        return Err(CoreError::InvalidPath(
            "Недопустимый корень категории временных файлов".into(),
        ));
    }
    Ok(())
}

pub fn is_safe_target_file(root: &Path, path: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(root) else {
        return false;
    };
    cleaner_execution::native::capture_candidate(root, relative).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn boundary_accepts_only_normal_local_temp_paths() {
        #[cfg(windows)]
        {
            assert!(
                validate_temp_root_boundary(Path::new(r"C:\Users\User\AppData\Local\Temp")).is_ok()
            );
            for p in [
                r"C:\Windows\Temp",
                r"C:\",
                r"C:\Program Files\Temp",
                r"C:\Users\User\NotTemp",
                r"C:\Users\User\..\Temp",
                r"\\server\share\Temp",
                r"\\?\GLOBALROOT\Temp",
            ] {
                assert!(validate_temp_root_boundary(Path::new(p)).is_err(), "{p}");
            }
        }
    }
}
