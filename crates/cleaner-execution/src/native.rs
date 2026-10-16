//! Read-only Windows object and parent identity checks.
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectIdentity {
    pub volume: u64,
    pub file_id: [u8; 16],
    pub index: u64,
    pub size: u64,
    pub modified_ticks: u64,
}
/// Conservative: any matching browser process blocks its cache category. Failure
/// to enumerate is an error, never proof that the application is closed.
#[cfg(windows)]
pub fn process_running(name: &str) -> Result<bool, &'static str> {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, GetLastError, ERROR_NO_MORE_FILES, INVALID_HANDLE_VALUE},
        System::Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
            TH32CS_SNAPPROCESS,
        },
    };
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot == INVALID_HANDLE_VALUE {
        return Err("PROCESS_CHECK_FAILED");
    }
    struct Snapshot(windows_sys::Win32::Foundation::HANDLE);
    impl Drop for Snapshot {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
    let snapshot = Snapshot(snapshot);
    let mut entry: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
    entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
    let mut present = unsafe { Process32FirstW(snapshot.0, &mut entry) };
    while present != 0 {
        let end = entry
            .szExeFile
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(entry.szExeFile.len());
        if String::from_utf16_lossy(&entry.szExeFile[..end]).eq_ignore_ascii_case(name) {
            return Ok(true);
        }
        present = unsafe { Process32NextW(snapshot.0, &mut entry) };
    }
    if unsafe { GetLastError() } != ERROR_NO_MORE_FILES {
        return Err("PROCESS_CHECK_FAILED");
    }
    Ok(false)
}
#[cfg(not(windows))]
pub fn process_running(_: &str) -> Result<bool, &'static str> {
    Err("UNSUPPORTED_PLATFORM")
}
#[derive(Debug, Clone)]
pub struct TrustedCandidate {
    pub(crate) id: String,
    pub(crate) root: PathBuf,
    pub(crate) relative: PathBuf,
    pub(crate) identity: ObjectIdentity,
    pub(crate) parents: Vec<ObjectIdentity>,
    pub(crate) minimum_age_ms: u64,
    pub(crate) blocked_process: Option<&'static str>,
    pub(crate) directory: bool,
    pub(crate) copy_guard: Option<Box<CopyGuard>>,
}
#[derive(Debug, Clone)]
pub(crate) struct CopyGuard {
    pub(crate) keeper: TrustedCandidate,
    pub(crate) hash: [u8; 32],
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
    pub fn file_reference(&self) -> u64 {
        self.identity.index
    }
    pub fn volume_serial(&self) -> u64 {
        self.identity.volume
    }
    pub fn physical_id(&self) -> (u64, [u8; 16]) {
        (self.identity.volume, self.identity.file_id)
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
        minimum_age_ms: 7 * 24 * 60 * 60 * 1000,
        blocked_process: None,
        directory: false,
        copy_guard: None,
    })
}
pub fn revalidate(candidate: &TrustedCandidate) -> Result<(), &'static str> {
    validate_paths(&candidate.root, &candidate.relative)?;
    let c = inspect_kind(
        &candidate.root,
        &candidate.relative,
        false,
        candidate.directory,
    )?;
    if candidate.identity != c.identity || candidate.parents != c.parents {
        return Err("OBJECT_CHANGED");
    }
    Ok(())
}
/// Rust-only manual-folder preparation. Directory deletion still requires an
/// explicit preview and Windows refuses disposition while children remain.
pub fn capture_directory(root: &Path, relative: &Path) -> Result<TrustedCandidate, &'static str> {
    validate_paths(root, relative)?;
    let c = inspect_kind(root, relative, false, true)?;
    Ok(TrustedCandidate {
        id: uuid::Uuid::new_v4().to_string(),
        root: root.to_owned(),
        relative: relative.to_owned(),
        identity: c.identity,
        parents: c.parents,
        minimum_age_ms: 7 * 24 * 60 * 60 * 1000,
        blocked_process: None,
        directory: true,
        copy_guard: None,
    })
}
struct Inspection {
    identity: ObjectIdentity,
    parents: Vec<ObjectIdentity>,
    #[cfg(windows)]
    _handles: Vec<std::fs::File>,
}

/// Holds the verified ancestors and file against rename/write while reading.
/// Rust-only read API; it grants no mutation capability.
pub struct BoundReader {
    file: std::fs::File,
    _inspection: Inspection,
}
impl BoundReader {
    pub fn file(&self) -> &std::fs::File {
        &self.file
    }
}
impl std::io::Read for BoundReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        std::io::Read::read(&mut self.file, buf)
    }
}
pub fn open_bound_reader(root: &Path, relative: &Path) -> Result<BoundReader, &'static str> {
    validate_paths(root, relative)?;
    let inspection = inspect(root, relative)?;
    #[cfg(windows)]
    {
        let file = inspection
            ._handles
            .last()
            .ok_or("IDENTITY_UNAVAILABLE")?
            .try_clone()
            .map_err(|_| "INSPECTION_FAILED")?;
        Ok(BoundReader {
            file,
            _inspection: inspection,
        })
    }
    #[cfg(not(windows))]
    {
        let _ = inspection;
        Err("UNSUPPORTED_PLATFORM")
    }
}

/// Keeps every directory ancestor bound while read_dir traverses the directory.
pub struct DirectoryLease {
    #[cfg(windows)]
    _handles: Vec<std::fs::File>,
    stamp: Vec<ObjectIdentity>,
}
impl DirectoryLease {
    /// Directory timestamps are excluded by open_checked. This binds a resumed
    /// enumeration to the same ancestors without keeping them locked while idle.
    pub fn stamp(&self) -> Vec<ObjectIdentity> {
        self.stamp.clone()
    }
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
        let mut stamp = Vec::new();
        for path in paths {
            let (handle, identity) = open_checked(&path, true)?;
            handles.push(handle);
            stamp.push(identity);
        }
        Ok(DirectoryLease {
            _handles: handles,
            stamp,
        })
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
#[cfg(not(windows))]
fn inspect_kind(_: &Path, _: &Path, _: bool, _: bool) -> Result<Inspection, &'static str> {
    Err("UNSUPPORTED_PLATFORM")
}
#[cfg(windows)]
fn inspect(root: &Path, relative: &Path) -> Result<Inspection, &'static str> {
    inspect_access(root, relative, false)
}
#[cfg(windows)]
fn inspect_access(
    root: &Path,
    relative: &Path,
    deleting: bool,
) -> Result<Inspection, &'static str> {
    inspect_kind(root, relative, deleting, false)
}
#[cfg(windows)]
fn inspect_kind(
    root: &Path,
    relative: &Path,
    deleting: bool,
    directory: bool,
) -> Result<Inspection, &'static str> {
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
    let (file, identity) = open_checked_access(&root.join(relative), directory, deleting)?;
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
    open_checked_access(path, directory, false)
}
#[cfg(windows)]
fn open_checked_access(
    path: &Path,
    directory: bool,
    deleting: bool,
) -> Result<(std::fs::File, ObjectIdentity), &'static str> {
    use std::fs::OpenOptions;
    use std::os::windows::{fs::OpenOptionsExt, io::AsRawHandle};
    use windows_sys::Win32::Storage::FileSystem::{
        FileIdInfo, GetFileInformationByHandle, GetFileInformationByHandleEx,
        BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT,
        FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_ID_INFO, FILE_SHARE_READ,
    };
    let file = OpenOptions::new()
        .read(true)
        .access_mode(if deleting { 0x80010000 } else { 0x80000000 }) // GENERIC_READ | DELETE
        .share_mode(if deleting { 0 } else { FILE_SHARE_READ })
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
    let mut file_id: FILE_ID_INFO = unsafe { std::mem::zeroed() };
    // Full volume serial + 128-bit identity: truncating the volume to 32 bits
    // cannot establish correspondence with the raw NTFS index.
    if unsafe {
        GetFileInformationByHandleEx(
            file.as_raw_handle(),
            FileIdInfo,
            (&mut file_id as *mut FILE_ID_INFO).cast(),
            std::mem::size_of_val(&file_id) as u32,
        )
    } == 0
    {
        return Err("IDENTITY_UNAVAILABLE");
    }
    Ok((
        file,
        ObjectIdentity {
            volume: file_id.VolumeSerialNumber,
            file_id: file_id.FileId.Identifier,
            index: (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow),
            size: if directory {
                0
            } else {
                (u64::from(info.nFileSizeHigh) << 32) | u64::from(info.nFileSizeLow)
            },
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

/// Explicit permanent cleanup. No pathname deletion and no Recycle Bin claim.
pub struct BoundDelete;
impl crate::ObjectAction for BoundDelete {
    fn available(&self) -> bool {
        cfg!(windows)
    }
    fn irreversible(&self) -> bool {
        true
    }
    fn apply(&self, candidate: &TrustedCandidate) -> crate::ActionResult {
        self.apply_with_cancel(candidate, &std::sync::atomic::AtomicBool::new(false))
    }
    fn apply_with_cancel(
        &self,
        candidate: &TrustedCandidate,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> crate::ActionResult {
        #[cfg(windows)]
        {
            match delete_bound_cancel(candidate, cancel, || {}) {
                Ok(()) => crate::ActionResult::Deleted,
                Err(code) => crate::ActionResult::Skipped(code.into()),
            }
        }
        #[cfg(not(windows))]
        {
            let _ = (candidate, cancel);
            crate::ActionResult::Skipped("UNSUPPORTED_PLATFORM".into())
        }
    }
}
#[cfg(all(test, windows))]
fn delete_bound(candidate: &TrustedCandidate, before: impl FnOnce()) -> Result<(), &'static str> {
    delete_bound_cancel(
        candidate,
        &std::sync::atomic::AtomicBool::new(false),
        before,
    )
}
#[cfg(windows)]
fn delete_bound_cancel(
    candidate: &TrustedCandidate,
    cancel: &std::sync::atomic::AtomicBool,
    before: impl FnOnce(),
) -> Result<(), &'static str> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        FileDispositionInfo, SetFileInformationByHandle, FILE_DISPOSITION_INFO,
    };
    validate_paths(&candidate.root, &candidate.relative)?;
    if let Some(process) = candidate.blocked_process {
        if process_running(process)? {
            return Err("APPLICATION_RUNNING");
        }
    }
    if candidate.modified_unix_ms() > crate::now_ms()
        || crate::now_ms().saturating_sub(candidate.modified_unix_ms()) < candidate.minimum_age_ms
    {
        return Err("TOO_RECENT");
    }
    // Parents deny write/delete; final file additionally denies all sharing.
    // Identity is checked on the very handle used to set deletion disposition.
    let inspection = inspect_kind(
        &candidate.root,
        &candidate.relative,
        true,
        candidate.directory,
    )?;
    if inspection.identity != candidate.identity || inspection.parents != candidate.parents {
        return Err("OBJECT_CHANGED");
    }
    let file = inspection._handles.last().ok_or("IDENTITY_UNAVAILABLE")?;
    let _keeper = if let Some(guard) = &candidate.copy_guard {
        let keeper = inspect(&guard.keeper.root, &guard.keeper.relative)?;
        if keeper.identity != guard.keeper.identity || keeper.parents != guard.keeper.parents {
            return Err("KEEPER_CHANGED");
        }
        let preserved = keeper._handles.last().ok_or("IDENTITY_UNAVAILABLE")?;
        if hash_bound(preserved, cancel)? != guard.hash || hash_bound(file, cancel)? != guard.hash {
            return Err("DUPLICATE_CHANGED");
        }
        Some(keeper)
    } else {
        None
    };
    before();
    if cancel.load(std::sync::atomic::Ordering::Acquire) {
        return Err("CANCELLED");
    }
    let mut info = FILE_DISPOSITION_INFO { DeleteFile: true };
    // SAFETY: live exclusive file handle with DELETE access; correct information class and struct.
    if unsafe {
        SetFileInformationByHandle(
            file.as_raw_handle(),
            FileDispositionInfo,
            (&mut info as *mut FILE_DISPOSITION_INFO).cast(),
            std::mem::size_of_val(&info) as u32,
        )
    } == 0
    {
        return Err("DELETE_REFUSED");
    }
    // No other final-file handles exist (exclusive open); closing commits disposition.
    drop(inspection);
    Ok(())
}
#[cfg(windows)]
fn hash_bound(
    file: &std::fs::File,
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<[u8; 32], &'static str> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let mut file = file.try_clone().map_err(|_| "HASH_FAILED")?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        if cancel.load(std::sync::atomic::Ordering::Acquire) {
            return Err("CANCELLED");
        }
        let count = file.read(&mut buffer).map_err(|_| "HASH_FAILED")?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(hash.finalize().into())
}
#[cfg(all(test, windows))]
mod deletion_tests {
    use super::*;
    use crate::ObjectAction;
    fn fixture() -> (PathBuf, TrustedCandidate) {
        let root =
            std::env::temp_dir().join(format!("cleancore_delete_fixture_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("child")).unwrap();
        let path = root.join("child/old.tmp");
        std::fs::write(&path, b"fixture only").unwrap();
        let file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
        file.set_times(std::fs::FileTimes::new().set_modified(
            std::time::SystemTime::now() - std::time::Duration::from_secs(8 * 86400),
        ))
        .unwrap();
        drop(file);
        let candidate = capture_candidate(&root, Path::new("child/old.tmp")).unwrap();
        (root, candidate)
    }
    #[test]
    fn deletes_exact_fixture_and_blocks_replacement_during_mutation() {
        let (root, c) = fixture();
        delete_bound(&c, || {
            assert!(std::fs::write(root.join("child/old.tmp"), b"replacement").is_err());
            assert!(std::fs::rename(root.join("child"), root.join("moved")).is_err());
            assert!(std::fs::rename(root.join("child/old.tmp"), root.join("other.tmp")).is_err());
        })
        .unwrap();
        assert!(!root.join("child/old.tmp").exists());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn busy_changed_and_hardlinked_objects_survive() {
        let (root, c) = fixture();
        let path = root.join("child/old.tmp");
        let busy = std::fs::File::open(&path).unwrap();
        assert!(matches!(
            BoundDelete.apply(&c),
            crate::ActionResult::Skipped(_)
        ));
        drop(busy);
        std::fs::write(&path, b"changed").unwrap();
        assert!(matches!(
            BoundDelete.apply(&c),
            crate::ActionResult::Skipped(_)
        ));
        let c = capture_candidate(&root, Path::new("child/old.tmp")).unwrap();
        std::fs::hard_link(&path, root.join("alias.tmp")).unwrap();
        assert!(matches!(
            BoundDelete.apply(&c),
            crate::ActionResult::Skipped(_)
        ));
        assert_eq!(std::fs::read(path).unwrap(), b"changed");
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(all(test, windows))]
mod process_tests {
    #[test]
    fn process_guard_detects_current_executable_and_handles_absent_name() {
        let exe = std::env::current_exe().unwrap();
        assert!(super::process_running(exe.file_name().unwrap().to_str().unwrap()).unwrap());
        assert!(!super::process_running("cleancore_nonexistent_8a03b73c.exe").unwrap());
    }
}

#[cfg(all(test, windows))]
mod duplicate_guard_tests {
    use super::*;
    use crate::ObjectAction;
    use sha2::{Digest, Sha256};
    fn fixture() -> (PathBuf, TrustedCandidate, TrustedCandidate, [u8; 32]) {
        let root =
            std::env::temp_dir().join(format!("cleancore_copy_guard_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        for name in ["original.bin", "copy.bin"] {
            std::fs::write(root.join(name), b"owned identical fixture").unwrap();
        }
        let original = capture_candidate(&root, Path::new("original.bin")).unwrap();
        let mut copy = capture_candidate(&root, Path::new("copy.bin")).unwrap();
        let hash: [u8; 32] = Sha256::digest(b"owned identical fixture").into();
        copy.minimum_age_ms = 0;
        copy.copy_guard = Some(Box::new(CopyGuard {
            keeper: original.clone(),
            hash,
        }));
        (root, original, copy, hash)
    }
    #[test]
    fn preserves_the_keeper_through_the_entire_delete_operation() {
        let (root, _, copy, _) = fixture();
        delete_bound(&copy, || {
            assert!(std::fs::write(root.join("original.bin"), b"changed").is_err());
            assert!(std::fs::remove_file(root.join("original.bin")).is_err());
        })
        .unwrap();
        assert!(root.join("original.bin").exists());
        assert!(!root.join("copy.bin").exists());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn missing_changed_and_nonmatching_keeper_block_deletion() {
        let (root, _, copy, _) = fixture();
        std::fs::write(root.join("original.bin"), b"changed").unwrap();
        assert!(matches!(
            BoundDelete.apply(&copy),
            crate::ActionResult::Skipped(_)
        ));
        assert!(root.join("copy.bin").exists());
        std::fs::remove_file(root.join("original.bin")).unwrap();
        assert!(matches!(
            BoundDelete.apply(&copy),
            crate::ActionResult::Skipped(_)
        ));
        std::fs::remove_dir_all(root).unwrap();
        let (root, _, mut copy, _) = fixture();
        copy.copy_guard.as_mut().unwrap().hash = [0; 32];
        assert!(
            matches!(BoundDelete.apply(&copy), crate::ActionResult::Skipped(code) if code == "DUPLICATE_CHANGED")
        );
        assert!(root.join("copy.bin").exists());
        assert!(
            delete_bound_cancel(&copy, &std::sync::atomic::AtomicBool::new(true), || {}).is_err()
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
