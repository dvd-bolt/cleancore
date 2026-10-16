use crate::{error, model::SelectedModel, AssistantError};
use cleaner_execution::native::open_bound_reader;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

pub const RUNTIME_FOLDER: &str = "llama-b11429";
pub const RUNTIME_EXE: &str = "llama-completion.exe";
pub const MAX_OUTPUT_BYTES: usize = 32768;
const TIMEOUT: Duration = Duration::from_secs(90);

pub fn bundled_runtime(development_root: &Path) -> PathBuf {
    development_root.join("runtime").join(RUNTIME_FOLDER)
}
pub fn verify_runtime(
    root: &Path,
) -> Result<Vec<cleaner_execution::native::BoundReader>, AssistantError> {
    let expected: BTreeMap<String, String> =
        serde_json::from_str(include_str!("../runtime-manifest.json"))?;
    let mut leases = Vec::new();
    for (name, hash) in expected {
        let mut file = open_bound_reader(root, Path::new(&name)).map_err(|_| {
            error(
                "RUNTIME_MISSING",
                "Локальный runtime отсутствует, занят или изменён. Восстановите поставку llama.cpp",
            )
        })?;
        let mut digest = Sha256::new();
        let mut buffer = [0; 65536];
        loop {
            let read = file.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            digest.update(&buffer[..read]);
        }
        if format!("{:x}", digest.finalize()) != hash {
            return Err(error(
                "RUNTIME_INTEGRITY",
                "Контрольная сумма runtime не совпала; запуск запрещён",
            ));
        }
        leases.push(file);
    }
    Ok(leases)
}
fn build_command(root: &Path, model: &Path) -> Command {
    let mut command = Command::new(root.join(RUNTIME_EXE));
    command.current_dir(root).env_clear();
    // All runtime parameters are fixed. No model URL, server, RPC, shell or arbitrary flags.
    command
        .args([
            "--offline",
            "--conversation",
            "--single-turn",
            "--no-display-prompt",
            "--simple-io",
            "--no-warmup",
            "--no-escape",
            "--log-verbosity",
            "0",
            "--ctx-size",
            "2048",
            "--predict",
            "256",
            "--threads",
            "2",
            "--threads-batch",
            "2",
            "--batch-size",
            "128",
            "--ubatch-size",
            "64",
            "--gpu-layers",
            "0",
            "--device",
            "none",
            "--model",
        ])
        .arg(model)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000 | 0x00004000);
    }
    command
}
struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn read_capped(mut input: impl Read, overflow: Arc<AtomicBool>) -> Vec<u8> {
    let mut bytes = Vec::new();
    if input
        .by_ref()
        .take((MAX_OUTPUT_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .is_err()
        || bytes.len() > MAX_OUTPUT_BYTES
    {
        overflow.store(true, Ordering::Release);
    }
    bytes.truncate(MAX_OUTPUT_BYTES);
    bytes
}
pub(crate) fn generate(
    root: &Path,
    model: &SelectedModel,
    prompt: &str,
    cancel: &AtomicBool,
) -> Result<String, AssistantError> {
    if cancel.load(Ordering::Acquire) {
        return Err(error("CANCELLED", "Запрос отменён"));
    }
    let _runtime_leases = verify_runtime(root)?;
    let mut child = ChildGuard(build_command(root, &model.path).spawn().map_err(|_| {
        error(
            "RUNTIME_START",
            "Не удалось запустить runtime. Проверьте комплектность и Visual C++ Runtime",
        )
    })?);
    #[cfg(windows)]
    let _job = Job::attach(&child.0)?;
    let overflow = Arc::new(AtomicBool::new(false));
    let output = child
        .0
        .stdout
        .take()
        .ok_or_else(|| error("RUNTIME_PIPE", "Канал ответа недоступен"))?;
    let errors = child
        .0
        .stderr
        .take()
        .ok_or_else(|| error("RUNTIME_PIPE", "Канал диагностики недоступен"))?;
    let output_flag = overflow.clone();
    let error_flag = overflow.clone();
    let output_thread = std::thread::Builder::new()
        .name("assistant-output".into())
        .spawn(move || read_capped(output, output_flag))?;
    let error_thread = std::thread::Builder::new()
        .name("assistant-errors".into())
        .spawn(move || read_capped(errors, error_flag))?;
    // The pinned Windows simple-io reader consumes UTF-8 stdin. Keep private
    // summaries/questions out of process command lines and files. A separate writer
    // prevents a model loading slowly from blocking cancellation on a full pipe.
    let mut input = child
        .0
        .stdin
        .take()
        .ok_or_else(|| error("RUNTIME_PIPE", "Канал вопроса недоступен"))?;
    let payload = format!("{}\n", prompt);
    let input_thread = std::thread::Builder::new()
        .name("assistant-input".into())
        .spawn(move || input.write_all(payload.as_bytes()))?;
    let started = Instant::now();
    let result = loop {
        if cancel.load(Ordering::Acquire) {
            break Err(error("CANCELLED", "Запрос отменён"));
        }
        if overflow.load(Ordering::Acquire) {
            break Err(error("OUTPUT_LIMIT", "Ответ превысил допустимый объём"));
        }
        if started.elapsed() > TIMEOUT {
            break Err(error("TIMEOUT", "Истекло время запроса (90 секунд)"));
        }
        match child.0.try_wait() {
            Ok(Some(status)) => {
                break if status.success() {
                    Ok(())
                } else {
                    Err(error("INFERENCE_FAILED","Модель несовместима с runtime, не помещается в лимит памяти или не загрузилась"))
                };
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(40)),
            Err(_) => {
                break Err(error(
                    "RUNTIME_WAIT",
                    "Не удалось получить состояние runtime",
                ))
            }
        }
    };
    let _ = child.0.kill();
    let _ = child.0.wait();
    let _ = input_thread.join();
    let output = output_thread
        .join()
        .map_err(|_| error("RUNTIME_PIPE", "Чтение ответа прервано"))?;
    let _ = error_thread.join();
    result?;
    if overflow.load(Ordering::Acquire) {
        return Err(error("OUTPUT_LIMIT", "Ответ превысил допустимый объём"));
    }
    let text = String::from_utf8_lossy(&output)
        .chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        .collect::<String>();
    let text = text.trim().to_owned();
    if text.is_empty() {
        return Err(error("EMPTY_RESPONSE", "Модель не вернула текст"));
    }
    Ok(text)
}
#[cfg(windows)]
struct Job(windows_sys::Win32::Foundation::HANDLE);
#[cfg(windows)]
impl Job {
    fn attach(child: &Child) -> Result<Self, AssistantError> {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::System::JobObjects::*;
        // SAFETY: process handle is live; structs are initialized and sized for the selected info class.
        unsafe {
            let job = Self(CreateJobObjectW(std::ptr::null(), std::ptr::null()));
            if job.0.is_null() {
                return Err(error(
                    "RESOURCE_LIMIT",
                    "Не удалось создать ограничение процесса",
                ));
            }
            let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
                | JOB_OBJECT_LIMIT_PROCESS_MEMORY
                | JOB_OBJECT_LIMIT_ACTIVE_PROCESS;
            limits.BasicLimitInformation.ActiveProcessLimit = 1;
            limits.ProcessMemoryLimit = 4 * 1024 * 1024 * 1024;
            if SetInformationJobObject(
                job.0,
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                std::mem::size_of_val(&limits) as u32,
            ) == 0
                || AssignProcessToJobObject(job.0, child.as_raw_handle()) == 0
            {
                return Err(error(
                    "RESOURCE_LIMIT",
                    "Система не позволила применить лимиты; запрос остановлен",
                ));
            }
            Ok(job)
        }
    }
}
#[cfg(windows)]
impl Drop for Job {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.0);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parameters_do_not_create_a_shell_or_network_endpoint() {
        let command = build_command(Path::new("C:/runtime"), Path::new("C:/model.gguf"));
        assert!(command
            .get_program()
            .to_string_lossy()
            .ends_with(RUNTIME_EXE));
        let args: Vec<_> = command
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert!(args.contains(&"--offline".to_string()));
        assert_eq!(args.last().unwrap(), "C:/model.gguf");
        assert!(!args
            .iter()
            .any(|a| a == "--prompt" || a == "--file" || a == "--log-disable"));
        assert!(args.contains(&"--single-turn".to_string()));
        assert!(!args
            .iter()
            .any(|a| a == "--server" || a == "--rpc" || a == "--model-url" || a == "--hf-repo"));
        assert!(command.get_envs().next().is_none());
    }
    #[test]
    fn output_is_bounded() {
        let flag = Arc::new(AtomicBool::new(false));
        let data = vec![b'x'; MAX_OUTPUT_BYTES + 5];
        assert_eq!(read_capped(&data[..], flag.clone()).len(), MAX_OUTPUT_BYTES);
        assert!(flag.load(Ordering::Acquire));
    }
    #[cfg(windows)]
    #[test]
    #[ignore = "requires pinned runtime provisioned by scripts/prepare_assistant_runtime.py"]
    fn real_runtime_rejects_truncated_weights_without_leaking_a_process() {
        let root = bundled_runtime(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .parent()
                .unwrap(),
        );
        assert!(!verify_runtime(&root).unwrap().is_empty());
        let dir = std::env::temp_dir().join(format!(
            "assistant_runtime_fixture_{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let mut bytes = b"GGUF".to_vec();
        bytes.extend(3u32.to_le_bytes());
        bytes.extend(1u64.to_le_bytes());
        bytes.extend(1u64.to_le_bytes());
        let key = b"general.architecture";
        bytes.extend((key.len() as u64).to_le_bytes());
        bytes.extend(key);
        bytes.extend(8u32.to_le_bytes());
        bytes.extend(5u64.to_le_bytes());
        bytes.extend(b"llama");
        bytes.resize(1024, 0);
        let path = dir.join("invalid-weights.gguf");
        std::fs::write(&path, bytes).unwrap();
        let model = SelectedModel::open(&path).unwrap();
        let result = generate(
            &root,
            &model,
            "Only explain the fixture",
            &AtomicBool::new(false),
        );
        assert_eq!(result.unwrap_err().code, "INFERENCE_FAILED");
        drop(model);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
