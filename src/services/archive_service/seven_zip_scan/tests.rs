use std::io::Cursor;

use super::*;

fn scan_limits() -> ZipScanLimits {
    ZipScanLimits {
        max_uncompressed_bytes: 1024 * 1024,
        max_entries: 100,
        max_files: 100,
        max_directories: 100,
        max_depth: 16,
        max_path_bytes: 4096,
        max_compression_ratio: 100,
        max_entry_compression_ratio: 100,
    }
}

fn create_7z_bytes(entries: &[(&str, Option<&[u8]>)], solid: bool) -> Vec<u8> {
    let cursor = Cursor::new(Vec::new());
    let options = if solid {
        zesven::WriteOptions::new().solid()
    } else {
        zesven::WriteOptions::new()
    };
    let mut writer = zesven::Writer::create(cursor)
        .expect("7z writer should start")
        .options(options);

    for (path, content) in entries {
        match content {
            Some(bytes) => writer
                .add_bytes(
                    zesven::ArchivePath::new(path).expect("7z file path should be valid"),
                    bytes,
                )
                .expect("7z file entry should be writable"),
            None => writer
                .add_directory(
                    zesven::ArchivePath::new(path.trim_end_matches('/'))
                        .expect("7z directory path should be valid"),
                    zesven::write::EntryMeta::directory(),
                )
                .expect("7z directory entry should be writable"),
        }
    }

    let (_, cursor) = writer.finish_into_inner().expect("7z writer should finish");
    cursor.into_inner()
}

fn scan_7z(bytes: Vec<u8>, limits: ZipScanLimits) -> Result<ZipScanResult> {
    let source_archive_size =
        crate::utils::numbers::usize_to_u64(bytes.len(), "test 7z archive size")?;
    let archive = open_seven_zip_streaming_archive(Cursor::new(bytes), limits)?;
    scan_seven_zip_archive(
        &archive,
        limits,
        source_archive_size,
        None,
        ZipScanNamePolicy::StrictAsterName,
        |_| Ok(()),
    )
}

fn scan_7z_error(entries: &[(&str, Option<&[u8]>)]) -> String {
    let bytes = create_7z_bytes(entries, false);
    scan_7z(bytes, scan_limits())
        .expect_err("7z scan should reject archive")
        .message()
        .to_string()
}

#[test]
fn scan_seven_zip_root_file_does_not_count_empty_parent_directory() {
    let bytes = create_7z_bytes(&[("note.txt", Some(b"root file"))], false);
    let result = scan_7z(bytes, scan_limits()).expect("7z scan should succeed");

    assert_eq!(result.file_count, 1);
    assert_eq!(result.directory_count, 0);
    assert_eq!(result.entries[0].parent, None);
}

#[test]
fn scan_seven_zip_rejects_too_many_implicit_directories() {
    let bytes = create_7z_bytes(
        &[
            ("one/file.txt", Some(b"one")),
            ("two/file.txt", Some(b"two")),
        ],
        false,
    );
    let mut limits = scan_limits();
    limits.max_directories = 1;

    let error = scan_7z(bytes, limits)
        .expect_err("7z scan should reject directory limit")
        .message()
        .to_string();
    assert!(error.contains("directories"));
}

#[test]
fn scan_seven_zip_rejects_file_directory_conflicts() {
    let error = scan_7z_error(&[("prefix", Some(b"file")), ("prefix/child", Some(b"child"))]);

    assert!(error.contains("inside file entry"));
}

#[test]
fn scan_seven_zip_marks_display_only_names_extract_incompatible() {
    let bytes = create_7z_bytes(&[("folder/name:with-colon.txt", Some(b"display"))], false);
    let source_archive_size =
        crate::utils::numbers::usize_to_u64(bytes.len(), "test 7z archive size")
            .expect("test 7z archive size should fit u64");
    let archive = open_seven_zip_streaming_archive(Cursor::new(bytes), scan_limits())
        .expect("7z archive should open");
    let result = scan_seven_zip_archive(
        &archive,
        scan_limits(),
        source_archive_size,
        None,
        ZipScanNamePolicy::PreviewDisplayName,
        |_| Ok(()),
    )
    .expect("display-only scan should succeed");

    assert!(!result.extract_compatible);
}

#[test]
fn scan_seven_zip_rejects_high_entry_compression_ratio() {
    let payload = vec![b'a'; 4096];
    let bytes = create_7z_bytes(&[("payload.txt", Some(&payload))], false);
    let mut limits = scan_limits();
    limits.max_entry_compression_ratio = 1;

    let error = scan_7z(bytes, limits)
        .expect_err("7z scan should reject high entry ratio")
        .message()
        .to_string();
    assert!(error.contains("compression ratio"));
}

#[test]
fn scan_seven_zip_accepts_solid_archive() {
    let bytes = create_7z_bytes(
        &[
            ("first.txt", Some(b"first")),
            ("second.txt", Some(b"second")),
        ],
        true,
    );
    let archive = open_seven_zip_streaming_archive(Cursor::new(bytes), scan_limits())
        .expect("solid 7z archive should open");

    assert!(archive.is_solid());
    let source_archive_size = 1024;
    let result = scan_seven_zip_archive(
        &archive,
        scan_limits(),
        source_archive_size,
        None,
        ZipScanNamePolicy::StrictAsterName,
        |_| Ok(()),
    )
    .expect("solid 7z scan should succeed");
    assert_eq!(result.file_count, 2);
}
