use super::record::*;
use super::*;
use crate::models::TreemapSize;

fn resident(kind: u32, value: &[u8]) -> Vec<u8> {
    let len = (24 + value.len()).next_multiple_of(8);
    let mut attr = vec![0; len];
    attr[..4].copy_from_slice(&kind.to_le_bytes());
    attr[4..8].copy_from_slice(&(len as u32).to_le_bytes());
    attr[16..20].copy_from_slice(&(value.len() as u32).to_le_bytes());
    attr[20..22].copy_from_slice(&24u16.to_le_bytes());
    attr[24..24 + value.len()].copy_from_slice(value);
    attr
}
fn file_name(parent: u64, name: &str, namespace: u8) -> Vec<u8> {
    let wide: Vec<_> = name.encode_utf16().collect();
    let mut value = vec![0; 66 + wide.len() * 2];
    value[..8].copy_from_slice(&parent.to_le_bytes());
    value[64] = wide.len() as u8;
    value[65] = namespace;
    for (out, word) in value[66..].chunks_exact_mut(2).zip(wide) {
        out.copy_from_slice(&word.to_le_bytes());
    }
    resident(0x30, &value)
}
fn on_disk(attrs: &[Vec<u8>], directory: bool) -> Vec<u8> {
    let mut b = vec![0; 1024];
    b[..4].copy_from_slice(b"FILE");
    b[4..6].copy_from_slice(&48u16.to_le_bytes());
    b[6..8].copy_from_slice(&3u16.to_le_bytes());
    b[16..18].copy_from_slice(&1u16.to_le_bytes());
    b[20..22].copy_from_slice(&56u16.to_le_bytes());
    b[22..24].copy_from_slice(&(if directory { 3u16 } else { 1u16 }).to_le_bytes());
    let mut end = 56;
    for attr in attrs {
        b[end..end + attr.len()].copy_from_slice(attr);
        end += attr.len();
    }
    b[end..end + 4].fill(255);
    end += 8;
    b[24..28].copy_from_slice(&(end as u32).to_le_bytes());
    // Store tails in the update sequence array, then stamp every 512-byte stride.
    for (i, tail) in [510, 1022].into_iter().enumerate() {
        let saved = [b[tail], b[tail + 1]];
        b[50 + i * 2..52 + i * 2].copy_from_slice(&saved);
        b[tail..tail + 2].copy_from_slice(&0x9876u16.to_le_bytes());
    }
    b[48..50].copy_from_slice(&0x9876u16.to_le_bytes());
    b
}
fn r(id: u64, parent: u64, name: &str, dir: bool, bytes: u64) -> Record {
    Record {
        reference: id | (1 << 48),
        base: 0,
        directory: dir,
        names: vec![Name {
            parent: parent | (1 << 48),
            name: name.into(),
            namespace: 1,
        }],
        logical: bytes,
        allocated: bytes.next_multiple_of(4096),
        has_data: !dir,
        attribute_list: false,
        extra_streams: false,
        index_root: false,
    }
}
#[test]
fn resident_unicode_and_torn_transfer() {
    let mut b = on_disk(
        &[
            file_name(5 | (1 << 48), "тест.txt", 1),
            resident(0x80, b"hello"),
        ],
        false,
    );
    let rec = parse_record(&mut b, 30).unwrap().unwrap();
    assert_eq!(rec.names[0].name, "тест.txt");
    assert_eq!(rec.logical, 5);
    assert_eq!(rec.allocated, 0);
    let mut bad = on_disk(&[], false);
    bad[1022] ^= 1;
    assert!(parse_record(&mut bad, 30).is_err());
}
#[test]
fn malformed_records_and_every_truncation_are_rejected_without_panics() {
    let b = on_disk(
        &[file_name(5 | (1 << 48), "x", 1), resident(0x80, b"hello")],
        false,
    );
    for end in 0..b.len() {
        let _ = parse_record(&mut b[..end].to_vec(), 30);
    }
    let mut bad = b.clone();
    bad[60..64].fill(0);
    assert!(parse_record(&mut bad, 30).is_err());
    let mut bad = b.clone();
    bad[56 + 20..56 + 22].copy_from_slice(&u16::MAX.to_le_bytes());
    assert!(parse_record(&mut bad, 30).is_err());
    let mut seed = 42u64;
    for _ in 0..2048 {
        let mut bad = b.clone();
        for _ in 0..8 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let p = (seed as usize) % bad.len();
            bad[p] ^= (seed >> 32) as u8;
        }
        let _ = parse_record(&mut bad, 30);
    }
}
#[test]
fn signed_runs_sparse_extents_and_overflows() {
    let runs = parse_runs(&[0x11, 3, 10, 0x11, 2, 0xfd, 0x01, 5, 0], 0).unwrap();
    assert_eq!(
        runs,
        vec![
            Run {
                vcn: 0,
                clusters: 3,
                lcn: Some(10)
            },
            Run {
                vcn: 3,
                clusters: 2,
                lcn: Some(7)
            },
            Run {
                vcn: 5,
                clusters: 5,
                lcn: None
            }
        ]
    );
    assert!(parse_runs(&[0x11, 1, 0xff, 0], 0).is_err());
    assert!(parse_runs(&[0x99, 1, 2], 0).is_err());
    assert!(parse_runs(&[0x01, 2, 0], u64::MAX).is_err());
    assert!(parse_runs(&[0x01, 2], 0).is_err());
}
#[test]
fn nonresident_sizes_do_not_use_stale_filename_size() {
    let mut data = vec![0; 80];
    data[..4].copy_from_slice(&0x80u32.to_le_bytes());
    data[4..8].copy_from_slice(&80u32.to_le_bytes());
    data[8] = 1;
    data[32..34].copy_from_slice(&72u16.to_le_bytes());
    data[12..14].copy_from_slice(&0x8000u16.to_le_bytes());
    data[40..48].copy_from_slice(&1_000_000u64.to_le_bytes());
    data[48..56].copy_from_slice(&999_999u64.to_le_bytes());
    data[64..72].copy_from_slice(&4096u64.to_le_bytes());
    let mut b = on_disk(&[file_name(5 | (1 << 48), "sparse.bin", 1), data], false);
    let rec = parse_record(&mut b, 30).unwrap().unwrap();
    assert_eq!(rec.logical, 999_999);
    assert_eq!(rec.allocated, 4096);
}
#[test]
fn aggregates_extensions_links_and_cached_folder_navigation() {
    let mut records = HashMap::new();
    let root = r(5, 5, ".", true, 0);
    let folder = r(30, 5, "папка", true, 0);
    let mut file = r(31, 30, "file.txt", false, 10);
    file.names.push(Name {
        parent: 5 | (1 << 48),
        name: "FILE~1.TXT".into(),
        namespace: 2,
    });
    file.names.push(Name {
        parent: 30 | (1 << 48),
        name: "linked.txt".into(),
        namespace: 1,
    });
    file.has_data = false;
    file.logical = 0;
    file.allocated = 0;
    let mut extension = r(40, 30, "", false, 10);
    extension.base = file.reference;
    extension.names.clear();
    for r in [root, folder, file, extension] {
        records.insert(r.reference, r);
    }
    let cancel = AtomicBool::new(false);
    let snapshot = MftSnapshot::build(
        "C:\\".into(),
        records,
        Diagnostics::new(),
        Diagnostics::new(),
        &cancel,
    )
    .unwrap();
    let root = snapshot
        .report(Path::new("C:/"), &AnalyzerOptions::default(), &cancel)
        .unwrap();
    assert!(root.complete);
    assert_eq!(root.total_files, 1);
    assert_eq!(root.total_bytes, 10);
    assert_eq!(root.subdirs[0].size_bytes, 10);
    assert_eq!(root.allocated_bytes, Some(4096));
    assert_eq!(root.warnings["HARD_LINK_COUNTED_ONCE"], 1);
    let child = snapshot
        .report(Path::new("C:\\папка"), &AnalyzerOptions::default(), &cancel)
        .unwrap();
    assert_eq!(child.total_files, 1);
    assert_eq!(child.largest_files[0].path, "C:\\папка\\file.txt");
    assert!(snapshot
        .report(
            Path::new("C:\\папка\\.."),
            &AnalyzerOptions::default(),
            &cancel
        )
        .is_none());
    assert!(snapshot
        .report(Path::new("D:\\папка"), &AnalyzerOptions::default(), &cancel)
        .is_none());
}
#[test]
fn stale_parents_cycles_and_cancellation_never_invent_paths() {
    let root = r(5, 5, ".", true, 0);
    let mut orphan = r(31, 30, "orphan", false, 999);
    orphan.names[0].parent = 30 | (2 << 48);
    let a = r(40, 41, "cycle_a", true, 0);
    let b = r(41, 40, "cycle_b", true, 0);
    let records = [root, orphan, a, b]
        .into_iter()
        .map(|r| (r.reference, r))
        .collect();
    let cancel = AtomicBool::new(true);
    let snapshot = MftSnapshot::build(
        "C:\\".into(),
        records,
        Diagnostics::new(),
        Diagnostics::new(),
        &cancel,
    )
    .unwrap();
    let report = snapshot
        .report(Path::new("C:\\"), &AnalyzerOptions::default(), &cancel)
        .unwrap();
    assert!(!report.complete);
    assert_eq!(report.total_files, 0);
    assert!(report.largest_files.is_empty());
    assert_eq!(report.diagnostics["MFT_UNREACHABLE"], 3);
    assert_eq!(report.diagnostics["CANCELLED"], 1);
}
#[test]
fn raw_access_accepts_drive_roots_only() {
    assert_eq!(drive_letter(Path::new("c:\\")), Some(b'C'));
    for path in [
        "C:",
        "C:\\folder",
        "\\\\.\\C:",
        "\\\\?\\C:\\",
        "\\\\server\\share",
        "C:\\..",
        "é:\\",
    ] {
        assert_eq!(drive_letter(Path::new(path)), None);
    }
}

#[test]
fn reserved_and_index_only_records_are_not_missing_user_data() {
    let root = r(5, 5, ".", true, 0);
    let mut reserved = r(12, 5, "reserved", false, 0);
    reserved.names.clear();
    reserved.has_data = false;
    let mut index = r(30, 5, "$Index", false, 0);
    index.has_data = false;
    index.index_root = true;
    let mut streams = r(31, 5, "$Streams", false, 0);
    streams.has_data = false;
    streams.extra_streams = true;
    let records = [root, reserved, index, streams]
        .into_iter()
        .map(|r| (r.reference, r))
        .collect();
    let cancel = AtomicBool::new(false);
    let snapshot = MftSnapshot::build(
        "C:\\".into(),
        records,
        Diagnostics::new(),
        Diagnostics::new(),
        &cancel,
    )
    .unwrap();
    let report = snapshot
        .report(Path::new("C:\\"), &AnalyzerOptions::default(), &cancel)
        .unwrap();
    assert!(report.complete);
    assert_eq!(report.warnings["SYSTEM_RECORD_WITHOUT_NAME"], 1);
    assert_eq!(report.total_files, 2);
    assert_eq!(report.total_bytes, 0);
}

#[test]
fn treemap_preserves_bytes_with_a_bounded_tree_and_validates_ids() {
    let mut records = HashMap::new();
    for r in [
        r(5, 5, ".", true, 0),
        r(30, 5, "folder", true, 0),
        r(31, 5, "empty.txt", false, 0),
    ] {
        records.insert(r.reference, r);
    }
    for id in 100..13100 {
        let file = r(id, 30, &format!("file{id}.bin"), false, id);
        records.insert(file.reference, file);
    }
    let snapshot = MftSnapshot::build(
        "C:\\".into(),
        records,
        Diagnostics::new(),
        Diagnostics::new(),
        &AtomicBool::new(false),
    )
    .unwrap();
    let tree = snapshot
        .treemap(Path::new("C:\\"), TreemapSize::Logical)
        .unwrap();
    fn check(node: &crate::models::TreemapNode) -> usize {
        if !node.children.is_empty() {
            assert_eq!(
                node.size_bytes,
                node.children.iter().map(|n| n.size_bytes).sum::<u64>()
            );
        }
        1 + node.children.iter().map(check).sum::<usize>()
    }
    assert!(check(&tree.root) <= 12000);
    assert_eq!(tree.empty_file_count, 1);
    assert_eq!(tree.empty_files.len(), 1);
    let expected = (100..13100u64).sum::<u64>();
    assert_eq!(tree.root.size_bytes, expected);
    let folder = snapshot
        .treemap(Path::new("C:\\folder"), TreemapSize::Allocated)
        .unwrap();
    assert_eq!(folder.parent_path, Some("C:\\".into()));
    assert_eq!(
        folder.root.size_bytes,
        (100..13100u64)
            .map(|n| n.next_multiple_of(4096))
            .sum::<u64>()
    );
    assert!(snapshot
        .treemap(Path::new("C:\\folder\\.."), TreemapSize::Logical)
        .is_none());
    assert!(snapshot.entry_path("cmd.exe").is_none());
    assert!(snapshot.entry_path("99999999999").is_none());
    assert_eq!(
        snapshot.entry_path(&(100 | (1u64 << 48)).to_string()),
        Some("C:\\folder\\file100.bin".into())
    );
}

#[test]
fn treemap_bounds_a_large_extension_legend_without_losing_bytes_or_duplicate_groups() {
    let mut records = HashMap::new();
    let root = r(5, 5, ".", true, 0);
    records.insert(root.reference, root);
    for id in 100..5100 {
        let file = r(id, 5, &format!("file.ext{id}"), false, 1);
        records.insert(file.reference, file);
    }
    let snapshot = MftSnapshot::build(
        "C:\\".into(),
        records,
        Diagnostics::new(),
        Diagnostics::new(),
        &AtomicBool::new(false),
    )
    .unwrap();
    let tree = snapshot
        .treemap(Path::new("C:\\"), TreemapSize::Logical)
        .unwrap();
    assert!(tree.extensions.len() <= 128);
    assert_eq!(
        tree.extensions
            .iter()
            .filter(|e| e.extension == "*")
            .count(),
        1
    );
    assert_eq!(tree.extensions.iter().map(|e| e.bytes).sum::<u64>(), 5000);
    assert_eq!(tree.extensions.iter().map(|e| e.files).sum::<u64>(), 5000);
}

#[test]
fn indexed_deletion_identity_requires_a_captured_volume_serial() {
    let mut records = HashMap::new();
    for record in [r(5, 5, ".", true, 0), r(100, 5, "owned.bin", false, 10)] {
        records.insert(record.reference, record);
    }
    let mut snapshot = MftSnapshot::build(
        "C:\\".into(),
        records,
        Diagnostics::new(),
        Diagnostics::new(),
        &AtomicBool::new(false),
    )
    .unwrap();
    let reference = (100 | (1u64 << 48)).to_string();
    assert!(snapshot.entry_identity(&reference).is_none());
    snapshot.volume_serial = Some(0x12345678abcdef01);
    assert_eq!(
        snapshot.entry_identity(&reference),
        Some(("C:\\owned.bin".into(), 0x12345678abcdef01))
    );
}

#[test]
fn space_hogs_filters_and_sorts_correctly() {
    let mut records = HashMap::new();
    let root = r(5, 5, ".", true, 0);
    records.insert(root.reference, root);

    // 1. Маленький файл (1 МБ) - должен отсечься по порогу
    let small = r(101, 5, "small.txt", false, 1_000_000);
    // 2. Видео 1 ГБ
    let video = r(102, 5, "movie.mkv", false, 1_000_000_000);
    // 3. ISO образ 4 ГБ
    let iso = r(103, 5, "win11.iso", false, 4_000_000_000);
    // 4. pagefile.sys 8 ГБ (должен получить флаг is_system_locked = true)
    let pagefile = r(104, 5, "pagefile.sys", false, 8_000_000_000);

    for rec in [small, video, iso, pagefile] {
        records.insert(rec.reference, rec);
    }

    let snapshot = MftSnapshot::build(
        "C:\\".into(),
        records,
        Diagnostics::new(),
        Diagnostics::new(),
        &AtomicBool::new(false),
    )
    .unwrap();

    // Порог 500 МБ
    let hogs = snapshot.space_hogs(500_000_000, 10, None);
    assert_eq!(hogs.len(), 3);
    // Первый по размеру - pagefile.sys
    assert_eq!(hogs[0].path, "C:\\pagefile.sys");
    assert!(hogs[0].is_system_locked);
    assert_eq!(hogs[0].size_bytes, 8_000_000_000);

    // Второй - win11.iso
    assert_eq!(hogs[1].path, "C:\\win11.iso");
    assert!(!hogs[1].is_system_locked);
    assert_eq!(hogs[1].category, LargeFileCategory::DiskImage);

    // Третий - movie.mkv
    assert_eq!(hogs[2].path, "C:\\movie.mkv");
    assert!(!hogs[2].is_system_locked);
    assert_eq!(hogs[2].category, LargeFileCategory::Video);

    // Фильтр только по DiskImage
    let disk_images = snapshot.space_hogs(500_000_000, 10, Some(LargeFileCategory::DiskImage));
    assert_eq!(disk_images.len(), 1);
    assert_eq!(disk_images[0].path, "C:\\win11.iso");
}
