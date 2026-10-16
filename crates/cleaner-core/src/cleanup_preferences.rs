use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CleanupPreferences {
    #[serde(default)]
    pub excluded_paths: Vec<String>,
    #[serde(default)]
    pub protected_duplicate_roots: Vec<String>,
}
fn normalized(path: &str) -> String {
    let path = path.replace('/', "\\");
    path.strip_prefix(r"\\?\")
        .unwrap_or(&path)
        .trim_end_matches('\\')
        .to_lowercase()
}
pub fn contains_path(root: &str, path: &str) -> bool {
    let root = normalized(root);
    let path = normalized(path);
    path == root
        || path
            .strip_prefix(&root)
            .is_some_and(|tail| tail.starts_with('\\'))
}
impl CleanupPreferences {
    pub fn validate(&self) -> Result<(), &'static str> {
        for paths in [&self.excluded_paths, &self.protected_duplicate_roots] {
            if paths.len() > 100 {
                return Err("Не более 100 исключений каждого вида");
            }
            for path in paths {
                let normalized = path.replace('/', "\\");
                let bytes = normalized.as_bytes();
                if !(3..=4096).contains(&normalized.len())
                    || !bytes[0].is_ascii_alphabetic()
                    || bytes[1] != b':'
                    || bytes[2] != b'\\'
                    || normalized[2..].contains([':', '\0', '*', '?', '\n', '\r'])
                    || normalized.split('\\').any(|p| p == "." || p == "..")
                {
                    return Err(
                        "Исключение должно быть полным локальным путём без масок и traversal",
                    );
                }
            }
        }
        Ok(())
    }
    pub fn excludes(&self, path: &str) -> bool {
        self.excluded_paths
            .iter()
            .any(|root| contains_path(root, path))
    }
    pub fn protects_duplicate(&self, path: &str) -> bool {
        self.protected_duplicate_roots
            .iter()
            .any(|root| contains_path(root, path))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exclusion_boundaries_do_not_match_sibling_names() {
        assert!(contains_path(r"C:\Keep", r"c:\keep\file.txt"));
        assert!(!contains_path(r"C:\Keep", r"C:\Keeper\file.txt"));
        for path in [r"C:\Keep", r"D:\", r"C:/Users/owned"] {
            assert!(CleanupPreferences {
                excluded_paths: vec![path.into()],
                ..Default::default()
            }
            .validate()
            .is_ok());
        }
        for path in [
            r"relative",
            r"C:\Keep\..\Other",
            r"\\server\share",
            r"C:\x:stream",
            r"C:\*.tmp",
            "",
        ] {
            assert!(CleanupPreferences {
                excluded_paths: vec![path.into()],
                ..Default::default()
            }
            .validate()
            .is_err());
        }
    }
}
