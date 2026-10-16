//! The category root comes from Windows Known Folders, never TEMP/LOCALAPPDATA.
use cleaner_core::errors::CoreError;
use std::path::{Component, Path, PathBuf};

pub use cleaner_execution::native::is_reparse_point_or_symlink;

pub fn resolve_user_temp_root() -> Result<PathBuf, CoreError> {
    let candidate = local_app_data()?.join("Temp");
    ensure_local_fixed(&candidate)?;
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
pub fn known_folder(folder: &windows_sys::core::GUID) -> Result<PathBuf, CoreError> {
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::{System::Com::CoTaskMemFree, UI::Shell::SHGetKnownFolderPath};
    let mut output = std::ptr::null_mut();
    // SAFETY: valid known-folder GUID and writable output; Windows allocates a NUL-terminated UTF-16 string.
    let result = unsafe { SHGetKnownFolderPath(folder, 0, std::ptr::null_mut(), &mut output) };
    if result < 0 || output.is_null() {
        return Err(CoreError::InvalidPath(
            "Windows не предоставила каталог текущей учётной записи".into(),
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
#[cfg(windows)]
pub(crate) fn local_app_data() -> Result<PathBuf, CoreError> {
    known_folder(&windows_sys::Win32::UI::Shell::FOLDERID_LocalAppData)
}
#[cfg(not(windows))]
pub(crate) fn local_app_data() -> Result<PathBuf, CoreError> {
    Err(CoreError::InvalidPath(
        "Категория поддерживается только в Windows".into(),
    ))
}

/// Explicit local cache roots only. No environment-variable or imported-path expansion.
pub fn resolve_category_roots(category: &str) -> Result<Vec<PathBuf>, CoreError> {
    if matches!(category, "user-temp-v1" | "user-temp-logs-v1") {
        return Ok(vec![resolve_user_temp_root()?]);
    }
    #[cfg(windows)]
    match category {
        "windows-temp-v1" => {
            return category_roots_at(
                &known_folder(&windows_sys::Win32::UI::Shell::FOLDERID_Windows)?,
                category,
            )
        }
        "windows-wer-v1" => {
            return category_roots_at(
                &known_folder(&windows_sys::Win32::UI::Shell::FOLDERID_ProgramData)?,
                category,
            )
        }
        _ => {}
    }
    let base = local_app_data()?;
    category_roots_at(&base, category)
}
pub(crate) fn category_roots_at(base: &Path, category: &str) -> Result<Vec<PathBuf>, CoreError> {
    ensure_local_fixed(base)?;
    let direct = match category {
        "directx-cache-v1" => Some("D3DSCache"),
        "crash-dumps-v1" => Some("CrashDumps"),
        "windows-temp-v1" => Some("Temp"),
        _ => None,
    };
    let mut roots = Vec::new();
    if category == "windows-wer-v1" {
        for name in [
            "Microsoft/Windows/WER/ReportArchive",
            "Microsoft/Windows/WER/ReportQueue",
        ] {
            roots.push(base.join(name));
        }
    } else if let Some(name) = direct {
        roots.push(base.join(name));
    } else {
        let (profiles, chromium) = match category {
            "chrome-cache-v1" => (base.join("Google/Chrome/User Data"), true),
            "edge-cache-v1" => (base.join("Microsoft/Edge/User Data"), true),
            "firefox-cache-v1" => (base.join("Mozilla/Firefox/Profiles"), false),
            _ => return Err(CoreError::InvalidPath("Неизвестная категория".into())),
        };
        let _lease =
            cleaner_execution::native::lock_directory(&profiles, Path::new("")).map_err(|_| {
                CoreError::InvalidPath("Профили отсутствуют, недоступны или перенаправлены".into())
            })?;
        let entries = std::fs::read_dir(&profiles)
            .map_err(|_| CoreError::InvalidPath("Профили недоступны".into()))?;
        for (count, entry) in entries.enumerate() {
            if count >= 256 {
                return Err(CoreError::InvalidPath(
                    "Слишком много записей профилей; категория не сканируется частично".into(),
                ));
            }
            let entry =
                entry.map_err(|_| CoreError::InvalidPath("Не удалось прочитать профиль".into()))?;
            if !entry.file_type().is_ok_and(|t| t.is_dir())
                || is_reparse_point_or_symlink(&entry.path())
            {
                continue;
            }
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if chromium {
                if name != "Default"
                    && !name
                        .strip_prefix("Profile ")
                        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
                {
                    continue;
                }
                for cache in ["Cache", "Code Cache", "GPUCache"] {
                    roots.push(entry.path().join(cache));
                }
            } else {
                roots.push(entry.path().join("cache2"));
            }
        }
    }
    // A missing optional cache is normal. An existing redirected or inaccessible
    // cache fails the category rather than silently claiming a complete scan.
    let mut existing = Vec::new();
    for root in roots {
        match std::fs::symlink_metadata(&root) {
            Ok(_) => existing.push(root),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => {
                return Err(CoreError::InvalidPath(
                    "Не удалось определить доступность кеша".into(),
                ))
            }
        }
    }
    let mut roots = existing;
    for root in &roots {
        cleaner_execution::native::lock_directory(root, Path::new("")).map_err(|_| {
            CoreError::InvalidPath("Каталог кеша недоступен или перенаправлен".into())
        })?;
    }
    roots.sort();
    roots.dedup();
    if roots.is_empty() {
        return Err(CoreError::InvalidPath(
            "Поддерживаемые каталоги категории не найдены".into(),
        ));
    }
    Ok(roots)
}

pub fn browser_process(category: &str) -> Option<&'static str> {
    match category {
        "chrome-cache-v1" => Some("chrome.exe"),
        "edge-cache-v1" => Some("msedge.exe"),
        "firefox-cache-v1" => Some("firefox.exe"),
        _ => None,
    }
}
pub fn current_profile_context() -> Result<String, CoreError> {
    Ok(local_app_data()?.to_string_lossy().into_owned())
}

pub fn resolve_project_cache(project: &Path, category: &str) -> Result<PathBuf, CoreError> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::Shell::{
            FOLDERID_Desktop, FOLDERID_Documents, FOLDERID_Downloads,
        };
        for folder in [&FOLDERID_Desktop, &FOLDERID_Documents, &FOLDERID_Downloads] {
            let Ok(root) = known_folder(folder) else {
                continue;
            };
            if project
                .strip_prefix(&root)
                .is_ok_and(|relative| !relative.as_os_str().is_empty())
            {
                return project_cache_at(&root, project, category);
            }
        }
        Err(CoreError::InvalidPath("Проект должен находиться внутри Рабочего стола, Документов или Загрузок текущей учётной записи".into()))
    }
    #[cfg(not(windows))]
    {
        let _ = (project, category);
        Err(CoreError::InvalidPath("Только Windows".into()))
    }
}
pub(crate) fn project_cache_at(
    boundary: &Path,
    project: &Path,
    category: &str,
) -> Result<PathBuf, CoreError> {
    ensure_local_fixed(boundary)?;
    use std::io::Read;
    let (marker, cache) = match category {
        "project-rust-target-v1" => ("Cargo.toml", "target"),
        "project-node-modules-v1" => ("package.json", "node_modules"),
        _ => {
            return Err(CoreError::InvalidPath(
                "Неизвестная категория проекта".into(),
            ))
        }
    };
    let relative = project
        .strip_prefix(boundary)
        .map_err(|_| CoreError::InvalidPath("Проект вне разрешённой области".into()))?;
    if relative.as_os_str().is_empty() {
        return Err(CoreError::InvalidPath(
            "Выберите отдельную папку проекта".into(),
        ));
    }
    let _lease = cleaner_execution::native::lock_directory(boundary, relative)
        .map_err(|code| CoreError::InvalidPath(code.into()))?;
    let reader =
        cleaner_execution::native::open_bound_reader(project, Path::new(marker)).map_err(|_| {
            CoreError::InvalidPath("Нет доступного обычного файла манифеста проекта".into())
        })?;
    let mut data = String::new();
    reader
        .take(1024 * 1024 + 1)
        .read_to_string(&mut data)
        .map_err(|_| CoreError::InvalidPath("Не удалось прочитать манифест проекта".into()))?;
    if data.len() > 1024 * 1024 {
        return Err(CoreError::InvalidPath("Манифест слишком большой".into()));
    }
    let valid = if category == "project-rust-target-v1" {
        data.parse::<toml::Table>().is_ok_and(|table| {
            table.get("package").is_some_and(toml::Value::is_table)
                || table.get("workspace").is_some_and(toml::Value::is_table)
        })
    } else {
        serde_json::from_str::<serde_json::Value>(&data).is_ok_and(|v| v.is_object())
    };
    if !valid {
        return Err(CoreError::InvalidPath(
            "Манифест не соответствует типу проекта".into(),
        ));
    }
    let root = project.join(cache);
    cleaner_execution::native::lock_directory(&root, Path::new("")).map_err(|_| {
        CoreError::InvalidPath("Кеш отсутствует, недоступен или перенаправлен".into())
    })?;
    Ok(root)
}

/// Reject network letters, UNC/device paths and removable media before any walk.
pub fn ensure_local_fixed(path: &Path) -> Result<(), CoreError> {
    if !path.is_absolute() {
        return Err(CoreError::InvalidPath(
            "Нужен абсолютный локальный путь".into(),
        ));
    }
    #[cfg(windows)]
    {
        use std::path::Prefix;
        let letter = match path.components().next() {
            Some(Component::Prefix(prefix)) => match prefix.kind() {
                Prefix::Disk(letter) | Prefix::VerbatimDisk(letter) => letter,
                _ => {
                    return Err(CoreError::InvalidPath(
                        "Разрешены только локальные фиксированные диски".into(),
                    ))
                }
            },
            _ => {
                return Err(CoreError::InvalidPath(
                    "Нужен абсолютный локальный путь".into(),
                ))
            }
        };
        let root: Vec<u16> = format!("{}:\\", letter as char)
            .encode_utf16()
            .chain(Some(0))
            .collect();
        const DRIVE_FIXED: u32 = 3; // Windows GetDriveTypeW documented return value.
        if unsafe { windows_sys::Win32::Storage::FileSystem::GetDriveTypeW(root.as_ptr()) }
            != DRIVE_FIXED
        {
            return Err(CoreError::InvalidPath(
                "Сетевые, съёмные и неопределённые диски не поддерживаются для очистки".into(),
            ));
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        Err(CoreError::InvalidPath("Только Windows".into()))
    }
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

#[cfg(all(test, windows))]
mod category_tests {
    use super::*;
    #[test]
    fn cache_discovery_excludes_profile_secrets_and_unrecognised_profiles() {
        let base = std::env::temp_dir().join(format!("cleancore_cache_{}", uuid::Uuid::new_v4()));
        for relative in [
            "Google/Chrome/User Data/Default/Cache/Cache_Data",
            "Google/Chrome/User Data/Profile 2/Code Cache",
            "Google/Chrome/User Data/Default/Local Storage",
            "Google/Chrome/User Data/Unrecognised/Cache",
            "Mozilla/Firefox/Profiles/owned.default/cache2",
            "D3DSCache",
            "CrashDumps",
            "Temp",
            "Microsoft/Windows/WER/ReportArchive",
            "Microsoft/Windows/WER/ReportQueue",
        ] {
            std::fs::create_dir_all(base.join(relative)).unwrap();
        }
        std::fs::write(
            base.join("Google/Chrome/User Data/Default/Cookies"),
            b"secret fixture",
        )
        .unwrap();
        let roots = category_roots_at(&base, "chrome-cache-v1").unwrap();
        assert_eq!(roots.len(), 2);
        assert!(roots.contains(&base.join("Google/Chrome/User Data/Default/Cache")));
        assert!(roots.contains(&base.join("Google/Chrome/User Data/Profile 2/Code Cache")));
        assert_eq!(
            category_roots_at(&base, "firefox-cache-v1").unwrap(),
            vec![base.join("Mozilla/Firefox/Profiles/owned.default/cache2")]
        );
        assert_eq!(
            category_roots_at(&base, "directx-cache-v1").unwrap(),
            vec![base.join("D3DSCache")]
        );
        assert_eq!(
            category_roots_at(&base, "windows-temp-v1").unwrap(),
            vec![base.join("Temp")]
        );
        assert_eq!(category_roots_at(&base, "windows-wer-v1").unwrap().len(), 2);
        assert!(category_roots_at(&base, "invented").is_err());
        std::fs::remove_dir_all(base).unwrap();
    }
}

#[cfg(all(test, windows))]
mod project_tests {
    use super::*;
    #[test]
    fn project_cache_requires_manifest_boundary_and_fixed_cache_child() {
        let boundary =
            std::env::temp_dir().join(format!("cleancore_project_{}", uuid::Uuid::new_v4()));
        let project = boundary.join("owned");
        std::fs::create_dir_all(project.join("target")).unwrap();
        std::fs::create_dir_all(project.join("node_modules")).unwrap();
        std::fs::write(
            project.join("Cargo.toml"),
            "[package]\nname='owned-fixture'\nversion='0.1.0'\n",
        )
        .unwrap();
        std::fs::write(
            project.join("package.json"),
            r#"{"name":"owned-fixture","private":true}"#,
        )
        .unwrap();
        assert_eq!(
            project_cache_at(&boundary, &project, "project-rust-target-v1").unwrap(),
            project.join("target")
        );
        assert_eq!(
            project_cache_at(&boundary, &project, "project-node-modules-v1").unwrap(),
            project.join("node_modules")
        );
        assert!(
            project_cache_at(&project.join("other"), &project, "project-rust-target-v1").is_err()
        );
        assert!(project_cache_at(&boundary, &boundary, "project-rust-target-v1").is_err());
        assert!(project_cache_at(&boundary, &project, "user-temp-v1").is_err());
        std::fs::write(project.join("Cargo.toml"), "invalid [[[").unwrap();
        assert!(project_cache_at(&boundary, &project, "project-rust-target-v1").is_err());
        assert!(project.join("target").exists());
        std::fs::remove_dir_all(boundary).unwrap();
    }
}

#[cfg(all(test, windows))]
mod drive_tests {
    use super::*;
    #[test]
    fn drive_gate_rejects_unc_and_devices_without_visiting_them() {
        for path in [
            r"\\server\share\folder",
            r"\\?\GLOBALROOT\Device\HarddiskVolume1",
            r"relative",
        ] {
            assert!(ensure_local_fixed(Path::new(path)).is_err());
        }
        assert!(ensure_local_fixed(&std::env::current_exe().unwrap()).is_ok());
    }
}
