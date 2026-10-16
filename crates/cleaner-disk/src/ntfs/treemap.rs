//! Bounded presentation of the immutable index. Aggregates preserve every byte.
use super::*;
use crate::models::{TreemapData, TreemapExtension, TreemapNode, TreemapSize};

impl MftSnapshot {
    fn selected_node(&self, path: &Path) -> Option<usize> {
        let normalized = path.to_str()?.replace('/', "\\");
        let normalized = normalized.trim_end_matches('\\');
        if normalized.eq_ignore_ascii_case(self.drive.trim_end_matches('\\')) {
            return Some(self.root);
        }
        if !normalized.get(..3)?.eq_ignore_ascii_case(&self.drive) {
            return None;
        }
        let mut selected = self.root;
        for part in normalized.get(3..)?.split('\\') {
            selected = *self.nodes[selected]
                .children
                .iter()
                .find(|&&id| self.nodes[id].directory && self.nodes[id].name == part)?;
        }
        Some(selected)
    }
    pub fn entry_path(&self, reference: &str) -> Option<String> {
        let id = *self.index.get(&reference.parse::<u64>().ok()?)?;
        if !self.nodes[id].reachable {
            return None;
        }
        self.path(id, &self.index)
    }
    pub fn entry_identity(&self, reference: &str) -> Option<(String, u64)> {
        Some((self.entry_path(reference)?, self.volume_serial?))
    }
    pub fn treemap(&self, path: &Path, mode: TreemapSize) -> Option<TreemapData> {
        let selected = self.selected_node(path)?;
        let root = self.tile(selected, 12_000, 0, mode)?;
        let mut extensions: HashMap<String, (u64, u64)> = HashMap::new();
        let mut pending = vec![selected];
        let mut empty_files = Vec::new();
        let mut empty_file_count = 0;
        while let Some(id) = pending.pop() {
            let n = &self.nodes[id];
            if n.directory {
                pending.extend(n.children.iter().copied());
            } else {
                let size = mode.bytes(n.logical, n.allocated);
                let mut ext = extension(&n.name);
                if extensions.len() >= 4096 && !extensions.contains_key(&ext) {
                    ext = "*".into();
                }
                let stat = extensions.entry(ext).or_default();
                stat.0 = stat.0.saturating_add(size);
                stat.1 += 1;
                if n.logical == 0 && n.parent == self.nodes[selected].reference {
                    empty_file_count += 1;
                    if empty_files.len() < 100 {
                        empty_files.push(self.tile(id, 1, 0, mode)?);
                    }
                }
            }
        }
        let mut extensions: Vec<_> = extensions
            .into_iter()
            .map(|(extension, (bytes, files))| TreemapExtension {
                extension,
                bytes,
                files,
            })
            .collect();
        extensions
            .sort_unstable_by(|a, b| b.bytes.cmp(&a.bytes).then(a.extension.cmp(&b.extension)));
        let mut others = TreemapExtension {
            extension: "*".into(),
            bytes: 0,
            files: 0,
        };
        if let Some(position) = extensions.iter().position(|e| e.extension == "*") {
            others = extensions.remove(position);
        }
        if extensions.len() > 127 {
            for e in extensions.drain(127..) {
                others.bytes = others.bytes.saturating_add(e.bytes);
                others.files = others.files.saturating_add(e.files);
            }
        }
        if others.files > 0 {
            extensions.push(others);
        }
        let parent_path = if selected == self.root {
            None
        } else {
            self.index
                .get(&self.nodes[selected].parent)
                .and_then(|&id| self.path(id, &self.index))
        };
        Some(TreemapData {
            root,
            extensions,
            empty_files,
            empty_file_count,
            parent_path,
            indexed: true,
        })
    }
    fn tile(
        &self,
        id: usize,
        budget: usize,
        depth: usize,
        mode: TreemapSize,
    ) -> Option<TreemapNode> {
        let n = &self.nodes[id];
        let mut tile = TreemapNode {
            id: n.reference.to_string(),
            name: if id == self.root {
                self.drive.clone()
            } else {
                n.name.clone()
            },
            path: self.path(id, &self.index)?,
            size_bytes: mode.bytes(n.logical, n.allocated),
            logical_bytes: n.logical,
            allocated_bytes: Some(n.allocated),
            is_dir: n.directory,
            is_aggregate: false,
            file_count: n.files,
            extension: extension(&n.name),
            children: Vec::new(),
        };
        if !n.directory || n.children.is_empty() {
            return Some(tile);
        }
        if budget <= 1 || depth >= 24 {
            return Some(tile);
        }
        let size = |id: usize| mode.bytes(self.nodes[id].logical, self.nodes[id].allocated);
        let mut children: Vec<_> = n
            .children
            .iter()
            .copied()
            .filter(|&id| size(id) > 0)
            .collect();
        children.sort_unstable_by(|&a, &b| {
            size(b)
                .cmp(&size(a))
                .then(self.nodes[a].reference.cmp(&self.nodes[b].reference))
        });
        let slots = (budget - 1).min(children.len());
        let visible = if children.len() > slots {
            slots.saturating_sub(1)
        } else {
            slots
        };
        let mut remaining = budget - 1;
        let mut remaining_bytes = tile.size_bytes;
        for (position, &child) in children.iter().take(visible).enumerate() {
            let reserve = visible - position - 1 + usize::from(visible < children.len());
            let share = if remaining_bytes > 0 {
                (remaining.saturating_sub(reserve) as f64 * size(child) as f64
                    / remaining_bytes as f64) as usize
            } else {
                1
            };
            let child_budget = share.max(1).min(remaining.saturating_sub(reserve).max(1));
            if let Some(node) = self.tile(child, child_budget, depth + 1, mode) {
                tile.children.push(node);
            }
            remaining = remaining.saturating_sub(child_budget);
            remaining_bytes = remaining_bytes.saturating_sub(size(child));
        }
        if visible < children.len() {
            let mut group = TreemapNode {
                id: format!("group:{}", n.reference),
                name: format!("Мелкие объекты · {}", children.len() - visible),
                path: tile.path.clone(),
                size_bytes: 0,
                logical_bytes: 0,
                allocated_bytes: Some(0),
                is_dir: true,
                is_aggregate: true,
                file_count: 0,
                extension: String::new(),
                children: vec![],
            };
            for &child in &children[visible..] {
                let c = &self.nodes[child];
                group.size_bytes = group.size_bytes.saturating_add(size(child));
                group.logical_bytes = group.logical_bytes.saturating_add(c.logical);
                group.allocated_bytes = Some(
                    group
                        .allocated_bytes
                        .unwrap_or(0)
                        .saturating_add(c.allocated),
                );
                group.file_count += c.files;
            }
            tile.children.push(group);
        }
        Some(tile)
    }
}
fn extension(name: &str) -> String {
    Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase()
}
