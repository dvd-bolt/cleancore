//! Сканер неактивных кэшей разработки и виртуальных дисков Docker/WSL (OPT-04).
//!
//! Находит заброшенные каталоги сборки (`node_modules`, `target`, `.venv`)
//! в проектах, которые не модифицировались дольше заданного порога (по умолчанию 60 дней),
//! а также определяет размер образов виртуальных дисков WSL2 и Docker Desktop (`ext4.vhdx`).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::types::{DevCacheEntry, DevCacheSummary};

const DEFAULT_INACTIVE_DAYS: u32 = 60;
const MAX_SEARCH_DEPTH: usize = 6;
const MAX_DIR_ENTRIES: usize = 10_000;

/// Проводит поиск неактивных кэшей разработки и виртуальных дисков.
pub fn scan_dev_caches(
    custom_roots: Option<Vec<PathBuf>>,
    inactive_days: Option<u32>,
) -> DevCacheSummary {
    let threshold_days = inactive_days.unwrap_or(DEFAULT_INACTIVE_DAYS);
    let now = SystemTime::now();

    let roots = custom_roots.unwrap_or_else(get_default_search_roots);
    let mut entries = Vec::new();

    for root in roots {
        if root.exists() && root.is_dir() {
            let mut visited = 0;
            scan_dir_for_caches(&root, 0, now, threshold_days, &mut entries, &mut visited);
        }
    }

    // Поиск образов WSL2 и Docker
    detect_wsl_docker_vhdx(now, &mut entries);

    // Сортировка по размеру (крупнейшие — первыми)
    entries.sort_by_key(|b| std::cmp::Reverse(b.size_bytes));

    let mut total_bytes = 0u64;
    let mut node_modules_count = 0;
    let mut node_modules_bytes = 0u64;
    let mut target_count = 0;
    let mut target_bytes = 0u64;
    let mut venv_count = 0;
    let mut venv_bytes = 0u64;
    let mut wsl_docker_bytes = 0u64;

    for entry in &entries {
        total_bytes = total_bytes.saturating_add(entry.size_bytes);
        match entry.kind.as_str() {
            "node_modules" => {
                node_modules_count += 1;
                node_modules_bytes = node_modules_bytes.saturating_add(entry.size_bytes);
            }
            "target" => {
                target_count += 1;
                target_bytes = target_bytes.saturating_add(entry.size_bytes);
            }
            "venv" => {
                venv_count += 1;
                venv_bytes = venv_bytes.saturating_add(entry.size_bytes);
            }
            "wsl_docker" => {
                wsl_docker_bytes = wsl_docker_bytes.saturating_add(entry.size_bytes);
            }
            _ => {}
        }
    }

    DevCacheSummary {
        total_bytes,
        node_modules_count,
        node_modules_bytes,
        target_count,
        target_bytes,
        venv_count,
        venv_bytes,
        wsl_docker_bytes,
        entries,
    }
}

fn get_default_search_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Ok(user_profile) = std::env::var("USERPROFILE") {
        let base = PathBuf::from(user_profile);
        roots.push(base.join("Projects"));
        roots.push(base.join("Desktop"));
        roots.push(base.join("Documents"));
        roots.push(base.join("source"));
        roots.push(base.join("repos"));
        roots.push(base.join("workspace"));
    }
    roots
}

fn scan_dir_for_caches(
    dir: &Path,
    depth: usize,
    now: SystemTime,
    threshold_days: u32,
    entries: &mut Vec<DevCacheEntry>,
    visited: &mut usize,
) {
    if depth > MAX_SEARCH_DEPTH || *visited >= MAX_DIR_ENTRIES {
        return;
    }

    let read = match fs::read_dir(dir) {
        Ok(r) => r,
        Err(_) => return,
    };

    for item in read.flatten() {
        *visited += 1;
        if *visited >= MAX_DIR_ENTRIES {
            break;
        }

        let file_type = match item.file_type() {
            Ok(ft) => ft,
            Err(_) => continue,
        };

        if !file_type.is_dir() || file_type.is_symlink() {
            continue;
        }

        let name = item.file_name().to_string_lossy().to_string();
        let lower = name.to_lowercase();

        // Проверяем, является ли каталог кэшем
        let cache_kind = match lower.as_str() {
            "node_modules" => Some("node_modules"),
            "target" => Some("target"),
            ".venv" | "venv" | "env" => Some("venv"),
            _ => None,
        };

        if let Some(kind) = cache_kind {
            let manifest_mtime = get_project_manifest_mtime(dir, kind);
            let inactive_days = manifest_mtime
                .and_then(|m| now.duration_since(m).ok())
                .map(|d| (d.as_secs() / 86400) as u32)
                .unwrap_or(0);

            // Если проект неактивен дольше порога
            if inactive_days >= threshold_days {
                let cache_path = item.path();
                let size_bytes = calculate_dir_size(&cache_path, 3);
                if size_bytes > 0 {
                    let last_mod_utc = manifest_mtime
                        .map(format_system_time_utc)
                        .unwrap_or_else(|| "Неизвестно".into());

                    entries.push(DevCacheEntry {
                        path: cache_path.to_string_lossy().to_string(),
                        name: format!(
                            "{}/{}",
                            dir.file_name().unwrap_or_default().to_string_lossy(),
                            name
                        ),
                        kind: kind.into(),
                        size_bytes,
                        last_modified_utc: last_mod_utc,
                        inactive_days,
                    });
                }
            }
            // Не углубляемся внутрь node_modules или target
            continue;
        }

        // Пропускаем скрытые системные папки
        if lower.starts_with('.') && lower != ".venv" {
            continue;
        }

        scan_dir_for_caches(
            &item.path(),
            depth + 1,
            now,
            threshold_days,
            entries,
            visited,
        );
    }
}

/// Проверяет дату последней модификации манифеста проекта (package.json, Cargo.toml, pyproject.toml).
fn get_project_manifest_mtime(project_dir: &Path, cache_kind: &str) -> Option<SystemTime> {
    let manifest_name = match cache_kind {
        "node_modules" => "package.json",
        "target" => "Cargo.toml",
        "venv" => "pyproject.toml",
        _ => return None,
    };

    let manifest_path = project_dir.join(manifest_name);
    if manifest_path.exists() {
        fs::metadata(&manifest_path)
            .ok()
            .and_then(|m| m.modified().ok())
    } else if cache_kind == "venv" {
        // Проверяем requirements.txt или pyvenv.cfg
        let req = project_dir.join("requirements.txt");
        if req.exists() {
            fs::metadata(&req).ok().and_then(|m| m.modified().ok())
        } else {
            let cfg = project_dir.join(".venv").join("pyvenv.cfg");
            fs::metadata(&cfg).ok().and_then(|m| m.modified().ok())
        }
    } else {
        fs::metadata(project_dir)
            .ok()
            .and_then(|m| m.modified().ok())
    }
}

/// Вычисляет приблизительный размер каталога с ограничением по глубине.
fn calculate_dir_size(dir: &Path, max_depth: usize) -> u64 {
    if max_depth == 0 {
        return 0;
    }
    let mut total = 0u64;
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Ok(ft) = entry.file_type() {
                if ft.is_file() {
                    if let Ok(meta) = entry.metadata() {
                        total = total.saturating_add(meta.len());
                    }
                } else if ft.is_dir() && !ft.is_symlink() {
                    total = total.saturating_add(calculate_dir_size(&entry.path(), max_depth - 1));
                }
            }
        }
    }
    total
}

/// Проверяет стандартные пути виртуальных дисков Docker Desktop и WSL2.
fn detect_wsl_docker_vhdx(now: SystemTime, entries: &mut Vec<DevCacheEntry>) {
    let local_app_data = match std::env::var("LOCALAPPDATA") {
        Ok(v) => PathBuf::from(v),
        Err(_) => return,
    };

    // 1. Docker Desktop data
    let docker_vhdx = local_app_data.join(r"Docker\wsl\data\ext4.vhdx");
    check_and_add_vhdx(&docker_vhdx, "Docker WSL Data (ext4.vhdx)", now, entries);

    // 2. Docker Desktop distro
    let docker_distro = local_app_data.join(r"Docker\wsl\distro\ext4.vhdx");
    check_and_add_vhdx(
        &docker_distro,
        "Docker WSL Distro (ext4.vhdx)",
        now,
        entries,
    );

    // 3. WSL2 дистрибутивы в Packages
    let packages_dir = local_app_data.join("Packages");
    if packages_dir.exists() && packages_dir.is_dir() {
        if let Ok(pkgs) = fs::read_dir(&packages_dir) {
            for pkg in pkgs.flatten() {
                let name = pkg.file_name().to_string_lossy().to_string();
                if name.contains("Ubuntu")
                    || name.contains("Debian")
                    || name.contains("openSUSE")
                    || name.contains("Canonical")
                {
                    let vhdx = pkg.path().join(r"LocalState\ext4.vhdx");
                    check_and_add_vhdx(&vhdx, &format!("WSL Distro ({name})"), now, entries);
                }
            }
        }
    }
}

fn check_and_add_vhdx(path: &Path, label: &str, now: SystemTime, entries: &mut Vec<DevCacheEntry>) {
    if path.exists() && path.is_file() {
        if let Ok(meta) = fs::metadata(path) {
            let mtime = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
            let inactive_days = now
                .duration_since(mtime)
                .map(|d| (d.as_secs() / 86400) as u32)
                .unwrap_or(0);

            let last_mod_utc = format_system_time_utc(mtime);

            entries.push(DevCacheEntry {
                path: path.to_string_lossy().to_string(),
                name: label.into(),
                kind: "wsl_docker".into(),
                size_bytes: meta.len(),
                last_modified_utc: last_mod_utc,
                inactive_days,
            });
        }
    }
}

fn format_system_time_utc(time: SystemTime) -> String {
    let duration = time
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let total_secs = duration.as_secs();

    let sec = total_secs % 60;
    let min = (total_secs / 60) % 60;
    let hour = (total_secs / 3600) % 24;

    let days = (total_secs / 86400) as i64;
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i32 + era as i32 * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };

    format!("{y:04}-{m:02}-{d:02}T{hour:02}:{min:02}:{sec:02}Z")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_dev_caches_empty_root() {
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let temp = std::env::temp_dir().join(format!("test_dev_caches_{nanos}"));
        fs::create_dir_all(&temp).unwrap();

        let summary = scan_dev_caches(Some(vec![temp.clone()]), Some(30));
        assert_eq!(summary.node_modules_count, 0);
        assert_eq!(summary.target_count, 0);

        let _ = fs::remove_dir_all(&temp);
    }
}
