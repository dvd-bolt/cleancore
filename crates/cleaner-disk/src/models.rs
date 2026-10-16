//! Модели данных для анализа дискового пространства, поиска дубликатов и истории ресурсов.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TreemapSize {
    Logical,
    Allocated,
}
impl TreemapSize {
    pub fn bytes(self, logical: u64, allocated: u64) -> u64 {
        match self {
            Self::Logical => logical,
            Self::Allocated => allocated,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreemapNode {
    pub id: String,
    pub name: String,
    pub path: String,
    pub size_bytes: u64,
    pub logical_bytes: u64,
    pub allocated_bytes: Option<u64>,
    pub is_dir: bool,
    pub is_aggregate: bool,
    pub file_count: u64,
    pub extension: String,
    pub children: Vec<TreemapNode>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreemapExtension {
    pub extension: String,
    pub bytes: u64,
    pub files: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreemapData {
    pub root: TreemapNode,
    pub extensions: Vec<TreemapExtension>,
    pub empty_files: Vec<TreemapNode>,
    pub empty_file_count: u64,
    pub parent_path: Option<String>,
    pub indexed: bool,
}

/// Категории файлов для построения карты распределения дискового пространства.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FileCategory {
    Videos,
    Images,
    Documents,
    Archives,
    Audio,
    Code,
    Other,
}

impl FileCategory {
    pub fn from_extension(ext: &str) -> Self {
        let ext = ext.to_lowercase();
        match ext.as_str() {
            "mp4" | "mkv" | "avi" | "mov" | "wmv" | "flv" | "webm" | "m4v" | "3gp" => {
                FileCategory::Videos
            }
            "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" | "svg" | "ico" | "tiff" | "psd"
            | "raw" | "heic" => FileCategory::Images,
            "pdf" | "docx" | "doc" | "xlsx" | "xls" | "pptx" | "ppt" | "txt" | "rtf" | "odt"
            | "csv" | "epub" | "md" => FileCategory::Documents,
            "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz" | "iso" | "cab" | "tgz" => {
                FileCategory::Archives
            }
            "mp3" | "wav" | "flac" | "aac" | "ogg" | "wma" | "m4a" | "opus" | "mid" => {
                FileCategory::Audio
            }
            "rs" | "ts" | "tsx" | "js" | "jsx" | "py" | "c" | "cpp" | "h" | "cs" | "java"
            | "go" | "html" | "css" | "json" | "xml" | "sql" | "toml" | "yaml" | "yml" => {
                FileCategory::Code
            }
            _ => FileCategory::Other,
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            FileCategory::Videos => "Видео",
            FileCategory::Images => "Изображения",
            FileCategory::Documents => "Документы",
            FileCategory::Archives => "Архивы и образы",
            FileCategory::Audio => "Аудио",
            FileCategory::Code => "Код и разметка",
            FileCategory::Other => "Прочее",
        }
    }
}

/// Элемент файловой структуры (файл или каталог).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiskEntry {
    pub path: String,
    pub name: String,
    pub size_bytes: u64,
    pub is_dir: bool,
    pub file_count: u64,
    pub percentage: f32,
    pub category: FileCategory,
}

/// Сводка объёма по отдельной категории файлов.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CategorySummary {
    pub category: FileCategory,
    pub display_name: String,
    pub total_bytes: u64,
    pub file_count: u64,
    pub percentage: f32,
}

/// Итоговый отчёт анализа распределения дискового пространства выбранного каталога.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiskAnalysisReport {
    pub root_path: String,
    pub total_bytes: u64,
    pub total_files: u64,
    pub total_dirs: u64,
    pub categories: Vec<CategorySummary>,
    pub largest_files: Vec<DiskEntry>,
    pub largest_dirs: Vec<DiskEntry>,
    pub subdirs: Vec<DiskEntry>,
    pub complete: bool,
    pub diagnostics: std::collections::BTreeMap<String, u64>,
    #[serde(default)]
    pub engine: String,
    #[serde(default)]
    pub engine_note: Option<String>,
    #[serde(default)]
    pub elapsed_ms: u64,
    #[serde(default)]
    pub allocated_bytes: Option<u64>,
    #[serde(default)]
    pub volume_used_bytes: Option<u64>,
    #[serde(default)]
    pub warnings: std::collections::BTreeMap<String, u64>,
}

/// Экземпляр файла в группе дубликатов.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DuplicateFileEntry {
    pub path: String,
    pub modified_utc: String,
    pub size_bytes: u64,
}

/// Совпадения размера и полного хеша, включая кеш анализа; не разрешение удаления.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DuplicateGroup {
    pub group_id: String,
    pub file_size_bytes: u64,
    pub full_hash: String,
    pub files: Vec<DuplicateFileEntry>,
    /// Избыточно расходуемое место: size_bytes * (files.len() - 1)
    pub reclaimable_bytes: u64,
}

/// Итоговый отчёт сканирования дубликатов.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DuplicateScanResult {
    pub root_path: String,
    pub groups: Vec<DuplicateGroup>,
    pub total_wasted_bytes: u64,
    pub total_duplicate_files: usize,
    #[serde(default)]
    pub cached_hashes: u64,
}

/// Стандартный каталог Windows для быстрого выбора пользователем.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuickDirectory {
    pub id: String,
    pub name: String,
    pub path: String,
}

/// Исторический срез динамики метрик (до 10 минут = 600 секунд).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResourceHistory {
    pub timestamps_utc: Vec<String>,
    pub cpu_percentages: Vec<Option<f32>>,
    pub memory_percentages: Vec<Option<f32>>,
    pub network_rx_bytes_sec: Vec<Option<u64>>,
    pub network_tx_bytes_sec: Vec<Option<u64>>,
}

pub use cleaner_core::models::{LargeFileCategory, LargeFileEntry, StaleFileEntry};
