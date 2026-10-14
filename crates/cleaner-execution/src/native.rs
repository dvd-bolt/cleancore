//! Read-only Windows object and parent identity checks.
use std::path::{Component, Path, PathBuf};
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectIdentity {
    pub volume: u32,
    pub index: u64,
    pub size: u64,
    pub modified_ticks: u64,
}
#[derive(Debug, Clone)]
pub struct TrustedCandidate {
    pub(crate) id: String,
    pub(crate) root: PathBuf,
    pub(crate) relative: PathBuf,
    pub(crate) identity: ObjectIdentity,
    pub(crate) parents: Vec<ObjectIdentity>,
}
impl TrustedCandidate {
    pub fn modified_time(&self) -> Option<std::time::SystemTime> {
        let ticks = self
            .identity
            .modified_ticks
            .checked_sub(116_444_736_000_000_000)?;
        std::time::UNIX_EPOCH.checked_add(std::time::Duration::new(
            ticks / 10_000_000,
            ((ticks % 10_000_000) * 100) as u32,
        ))
    }
    pub fn object_id(&self) -> &str {
        &self.id
    }
    pub fn size_bytes(&self) -> u64 {
        self.identity.size
    }
    pub fn display_path(&self) -> String {
        self.root
            .join(&self.relative)
            .to_string_lossy()
            .into_owned()
    }
    pub fn modified_unix_ms(&self) -> u64 {
        self.identity
            .modified_ticks
            .saturating_sub(116_444_736_000_000_000)
            / 10_000
    }
}
fn validate_paths(root: &Path, relative: &Path) -> Result<(), &'static str> {
    if !root.is_absolute()
        || root
            .components()
            .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
        || relative.as_os_str().is_empty()
        || relative
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
        || relative.to_string_lossy().contains([':', '\0'])
    {
        return Err("INVALID_PATH");
    }
    #[cfg(windows)]
    if !matches!(root.components().next(), Some(Component::Prefix(p)) if matches!(p.kind(), std::path::Prefix::Disk(_) | std::path::Prefix::VerbatimDisk(_)))
    {
        return Err("UNSUPPORTED_ROOT");
    }
    Ok(())
}
/// Rust-only: stage 3 must establish a permitted category root before calling.
/// No deserialization or IPC entry point exists for this type.
pub fn capture_candidate(root: &Path, relative: &Path) -> Result<TrustedCandidate, &'static str> {
    validate_paths(root, relative)?;
    let c = inspect(root, relative)?;
    Ok(TrustedCandidate {
        id: uuid::Uuid::new_v4().to_string(),
        root: root.to_owned(),
        relative: relative.to_owned(),
        identity: c.identity,
        parents: c.parents,
    })
}
pub fn revalidate(candidate: &TrustedCandidate) -> Result<(), &'static str> {
    validate_paths(&candidate.root, &candidate.relative)?;
    let c = inspect(&candidate.root, &candidate.relative)?;
    if candidate.identity != c.identity || candidate.parents != c.parents {
        return Err("OBJECT_CHANGED");
    }
    Ok(())
}
struct Inspection {
    identity: ObjectIdentity,
    parents: Vec<ObjectIdentity>,
    #[cfg(windows)]
    _handles: Vec<std::fs::File>,
}

/// Keeps every directory ancestor bound while read_dir traverses the directory.
pub struct DirectoryLease {
    #[cfg(windows)]
    _handles: Vec<std::fs::File>,
}
pub fn lock_directory(root: &Path, relative: &Path) -> Result<DirectoryLease, &'static str> {
    validate_paths(
        root,
        if relative.as_os_str().is_empty() {
            Path::new("placeholder")
        } else {
            relative
        },
    )?;
    #[cfg(windows)]
    {
        let mut paths: Vec<PathBuf> = root.ancestors().map(Path::to_owned).collect();
        paths.reverse();
        let mut path = root.to_owned();
        for part in relative.components() {
            path.push(part.as_os_str());
            paths.push(path.clone());
        }
        let mut handles = Vec::new();
        for path in paths {
            handles.push(open_checked(&path, true)?.0);
        }
        Ok(DirectoryLease { _handles: handles })
    }
    #[cfg(not(windows))]
    {
        Err("UNSUPPORTED_PLATFORM")
    }
}
#[cfg(not(windows))]
fn inspect(_: &Path, _: &Path) -> Result<Inspection, &'static str> {
    Err("UNSUPPORTED_PLATFORM")
}
#[cfg(windows)]
fn inspect(root: &Path, relative: &Path) -> Result<Inspection, &'static str> {
    let mut paths: Vec<PathBuf> = root.ancestors().map(Path::to_owned).collect();
    paths.reverse();
    let mut path = root.to_owned();
    let parts: Vec<_> = relative.components().collect();
    for part in &parts[..parts.len() - 1] {
        path.push(part.as_os_str());
        paths.push(path.clone());
    }
    let mut handles = Vec::new();
    let mut parents = Vec::new();
    for path in paths {
        let (file, identity) = open_checked(&path, true)?;
        handles.push(file);
        parents.push(identity);
    }
    let (file, identity) = open_checked(&root.join(relative), false)?;
    handles.push(file);
    Ok(Inspection {
        identity,
        parents,
        _handles: handles,
    })
}
#[cfg(windows)]
fn open_checked(
    path: &Path,
    directory: bool,
) -> Result<(std::fs::File, ObjectIdentity), &'static str> {
    use std::fs::OpenOptions;
    use std::os::windows::{fs::OpenOptionsExt, io::AsRawHandle};
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_DIRECTORY,
        FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
        FILE_SHARE_READ,
    };
    let file = OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
        .map_err(|e| match e.raw_os_error() {
            Some(5) => "ACCESS_DENIED",
            Some(32 | 33) => "FILE_IN_USE",
            Some(2 | 3) => "NOT_FOUND",
            _ => "INSPECTION_FAILED",
        })?;
    let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
    // SAFETY: the File owns the live handle and info is writable/correctly sized.
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } == 0 {
        return Err("IDENTITY_UNAVAILABLE");
    }
    if info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err("REPARSE_POINT");
    }
    if (info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY != 0) != directory {
        return Err("WRONG_OBJECT_TYPE");
    }
    if !directory && info.nNumberOfLinks != 1 {
        return Err("HARD_LINK");
    }
    Ok((
        file,
        ObjectIdentity {
            volume: info.dwVolumeSerialNumber,
            index: (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow),
            size: (u64::from(info.nFileSizeHigh) << 32) | u64::from(info.nFileSizeLow),
            modified_ticks: if directory {
                0
            } else {
                (u64::from(info.ftLastWriteTime.dwHighDateTime) << 32)
                    | u64::from(info.ftLastWriteTime.dwLowDateTime)
            },
        },
    ))
}

/// Disabled until revalidation + recycle have one proven object binding.
/// Closing these read handles and then deleting by pathname would be racy.
pub struct DisabledRecycle;
impl crate::ObjectAction for DisabledRecycle {
    fn available(&self) -> bool {
        false
    }
    fn apply(&self, c: &TrustedCandidate) -> crate::ActionResult {
        crate::ActionResult::Skipped(
            match revalidate(c) {
                Ok(()) => "RECYCLE_NOT_VALIDATED",
                Err(code) => code,
            }
            .into(),
        )
    }
}
