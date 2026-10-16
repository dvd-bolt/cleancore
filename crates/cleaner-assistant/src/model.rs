use crate::{error, AssistantError};
use cleaner_execution::native::{open_bound_reader, BoundReader};
use serde::Serialize;
use std::{
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

pub const MAX_MODEL_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_METADATA_BYTES: u64 = 16 * 1024 * 1024;
#[derive(Debug, Clone, Serialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub size_bytes: String,
    pub architecture: String,
    pub declared_license: Option<String>,
    pub license_source: Option<String>,
    pub license_accepted: bool,
}
pub struct SelectedModel {
    pub info: ModelInfo,
    pub(crate) path: PathBuf,
    _lease: BoundReader,
}
impl SelectedModel {
    pub fn open(path: &Path) -> Result<Self, AssistantError> {
        if !path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("gguf"))
        {
            return Err(error("MODEL_FORMAT", "Выберите локальный файл .gguf"));
        }
        let parent = path
            .parent()
            .ok_or_else(|| error("MODEL_PATH", "Недопустимый путь модели"))?;
        let name = path
            .file_name()
            .ok_or_else(|| error("MODEL_PATH", "Недопустимый путь модели"))?;
        let lease = open_bound_reader(parent, Path::new(name)).map_err(|_| {
            error(
                "MODEL_ACCESS",
                "Модель недоступна, занята или расположена по ссылке/сетевому пути",
            )
        })?;
        let mut file = lease.file().try_clone()?;
        let size = file.metadata()?.len();
        if !(1024..=MAX_MODEL_BYTES).contains(&size) {
            return Err(error("MODEL_SIZE", "Поддерживаются модели от 1 КБ до 2 ГБ"));
        }
        let (architecture, declared_license) = read_metadata(&mut file, size)?;
        Ok(Self {
            info: ModelInfo {
                id: uuid::Uuid::new_v4().to_string(),
                name: name.to_string_lossy().into_owned(),
                size_bytes: size.to_string(),
                architecture,
                declared_license,
                license_source: None,
                license_accepted: false,
            },
            path: path.to_owned(),
            _lease: lease,
        })
    }
}
fn u32_value(input: &mut impl Read) -> Result<u32, AssistantError> {
    let mut b = [0; 4];
    input.read_exact(&mut b)?;
    Ok(u32::from_le_bytes(b))
}
fn u64_value(input: &mut impl Read) -> Result<u64, AssistantError> {
    let mut b = [0; 8];
    input.read_exact(&mut b)?;
    Ok(u64::from_le_bytes(b))
}
fn string(input: &mut (impl Read + Seek), keep: bool) -> Result<String, AssistantError> {
    let len = u64_value(input)?;
    if len > 65536 {
        return Err(error("MODEL_METADATA", "Слишком длинное поле GGUF"));
    }
    if !keep {
        input.seek(SeekFrom::Current(len as i64))?;
        return Ok(String::new());
    }
    let mut bytes = vec![0; len as usize];
    input.read_exact(&mut bytes)?;
    String::from_utf8(bytes)
        .map_err(|_| error("MODEL_METADATA", "Некорректный UTF-8 в описании модели"))
}
fn skip(input: &mut (impl Read + Seek), kind: u32, depth: u32) -> Result<(), AssistantError> {
    if depth > 1 {
        return Err(error(
            "MODEL_METADATA",
            "Вложенные массивы GGUF не поддерживаются",
        ));
    }
    let bytes = match kind {
        0 | 1 | 7 => 1,
        2 | 3 => 2,
        4..=6 => 4,
        10..=12 => 8,
        8 => {
            string(input, false)?;
            return Ok(());
        }
        9 => {
            let element = u32_value(input)?;
            let count = u64_value(input)?;
            if count > 262144 || element == 9 {
                return Err(error("MODEL_METADATA", "Слишком большой массив GGUF"));
            }
            for _ in 0..count {
                skip(input, element, depth + 1)?;
                if input.stream_position()? > MAX_METADATA_BYTES {
                    return Err(error("MODEL_METADATA", "Метаданные GGUF превышают 16 МБ"));
                }
            }
            return Ok(());
        }
        _ => return Err(error("MODEL_METADATA", "Неизвестный тип поля GGUF")),
    };
    input.seek(SeekFrom::Current(bytes))?;
    Ok(())
}
pub fn read_metadata(
    input: &mut (impl Read + Seek),
    file_size: u64,
) -> Result<(String, Option<String>), AssistantError> {
    let mut magic = [0; 4];
    input.read_exact(&mut magic)?;
    if &magic != b"GGUF" || !matches!(u32_value(input)?, 2 | 3) {
        return Err(error("MODEL_FORMAT", "Нужна модель GGUF версии 2 или 3"));
    }
    let tensors = u64_value(input)?;
    let fields = u64_value(input)?;
    if tensors == 0 || tensors > 100000 || fields == 0 || fields > 4096 {
        return Err(error(
            "MODEL_METADATA",
            "Недопустимое число тензоров/полей GGUF",
        ));
    }
    let mut architecture = None;
    let mut license = None;
    for _ in 0..fields {
        let key = string(input, true)?;
        let kind = u32_value(input)?;
        if matches!(key.as_str(), "general.architecture" | "general.license") && kind == 8 {
            let value = string(input, true)?;
            if key == "general.architecture" {
                architecture = Some(value);
            } else {
                license = Some(
                    value
                        .chars()
                        .filter(|c| !c.is_control())
                        .take(256)
                        .collect(),
                );
            }
        } else {
            skip(input, kind, 0)?;
        }
        if input.stream_position()? > MAX_METADATA_BYTES.min(file_size) {
            return Err(error(
                "MODEL_METADATA",
                "Метаданные GGUF повреждены или слишком велики",
            ));
        }
    }
    let arch =
        architecture.ok_or_else(|| error("MODEL_ARCHITECTURE", "Архитектура модели не указана"))?;
    if ![
        "llama", "qwen2", "qwen3", "gemma", "gemma2", "gemma3", "phi3", "smollm", "gpt2",
    ]
    .contains(&arch.as_str())
    {
        return Err(error(
            "MODEL_ARCHITECTURE",
            "Архитектура не входит в проверяемый список совместимости",
        ));
    }
    Ok((arch, license))
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    pub fn fixture() -> Vec<u8> {
        let mut b = b"GGUF".to_vec();
        b.extend(3u32.to_le_bytes());
        b.extend(1u64.to_le_bytes());
        b.extend(1u64.to_le_bytes());
        let key = b"general.architecture";
        b.extend((key.len() as u64).to_le_bytes());
        b.extend(key);
        b.extend(8u32.to_le_bytes());
        b.extend(5u64.to_le_bytes());
        b.extend(b"llama");
        b
    }
    #[test]
    fn validates_metadata_and_rejects_truncation_and_huge_counts() {
        let b = fixture();
        assert_eq!(
            read_metadata(&mut Cursor::new(&b), b.len() as u64)
                .unwrap()
                .0,
            "llama"
        );
        assert!(read_metadata(&mut Cursor::new(&b[..12]), 12).is_err());
        let mut invalid = b.clone();
        invalid[16..24].copy_from_slice(&u64::MAX.to_le_bytes());
        assert!(read_metadata(&mut Cursor::new(invalid), 1024).is_err());
        let mut invalid = b.clone();
        invalid[..4].copy_from_slice(b"EXEC");
        assert!(read_metadata(&mut Cursor::new(invalid), 1024).is_err());
    }
}
