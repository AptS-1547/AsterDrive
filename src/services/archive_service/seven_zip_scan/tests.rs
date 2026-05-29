use std::io::Cursor;

use super::*;
use crate::services::archive_service::test_utils::crc32;

const ENCRYPTED_7Z_ENTRY_FIXTURE: &[u8] =
    include_bytes!("../../../../tests/fixtures/archives/encrypted-entry.7z");

fn scan_limits() -> ArchiveScanLimits {
    ArchiveScanLimits {
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

fn create_7z_bytes_with_anti_item(path: &str) -> Vec<u8> {
    let cursor = Cursor::new(Vec::new());
    let mut writer = zesven::Writer::create(cursor).expect("7z writer should start");
    writer
        .add_anti_item(zesven::ArchivePath::new(path).expect("7z anti path should be valid"))
        .expect("7z anti item should be writable");

    let (_, cursor) = writer.finish_into_inner().expect("7z writer should finish");
    cursor.into_inner()
}

fn create_7z_bytes_with_attrs(path: &str, content: &[u8], attrs: u32) -> Vec<u8> {
    let mut bytes = create_7z_bytes(&[(path, Some(content))], false);
    patch_7z_single_entry_attrs(&mut bytes, attrs);
    bytes
}

fn seven_zip_unix_attrs(mode: u32) -> u32 {
    0x8000_0000 | ((mode & 0x7fff) << 16)
}

fn patch_7z_single_entry_attrs(bytes: &mut Vec<u8>, attrs: u32) {
    let next_header_offset = read_u64_le(bytes, 12);
    let next_header_size = read_u64_le(bytes, 20);
    let next_header_start: usize = (32 + next_header_offset)
        .try_into()
        .expect("test 7z next header offset should fit usize");
    let next_header_size: usize = next_header_size
        .try_into()
        .expect("test 7z next header size should fit usize");
    let next_header_end = next_header_start + next_header_size;
    let mut header = bytes[next_header_start..next_header_end].to_vec();
    let insert_at = find_single_entry_files_info_end(&header);
    let attrs_property = single_entry_attrs_property(attrs);

    header.splice(insert_at..insert_at, attrs_property);
    bytes.splice(next_header_start..next_header_end, header.iter().copied());

    let next_header_size_u64 =
        crate::utils::numbers::usize_to_u64(header.len(), "test 7z next header size")
            .expect("test 7z next header size should fit u64");
    bytes[20..28].copy_from_slice(&next_header_size_u64.to_le_bytes());
    refresh_7z_header_checksums(bytes);
}

fn find_single_entry_files_info_end(header: &[u8]) -> usize {
    let mut cursor = HeaderCursor::new(header);
    assert_eq!(
        cursor.read_u8(),
        0x01,
        "7z next header should start with Header"
    );

    loop {
        let property = cursor.read_u8();
        match property {
            0x00 => panic!("FilesInfo should exist in test 7z header"),
            0x04 => skip_main_streams_info(&mut cursor),
            0x05 => return find_files_info_end(&mut cursor),
            property => panic!("unexpected 7z top-level header property: {property:#x}"),
        }
    }
}

fn skip_main_streams_info(cursor: &mut HeaderCursor<'_>) {
    loop {
        let property = cursor.read_u8();
        match property {
            0x00 => return,
            0x06 => skip_pack_info(cursor),
            0x07 => skip_unpack_info(cursor),
            0x08 => skip_substreams_info(cursor),
            property => panic!("unexpected 7z streams property: {property:#x}"),
        }
    }
}

fn skip_pack_info(cursor: &mut HeaderCursor<'_>) {
    let _pack_pos = cursor.read_var_u64();
    let pack_streams = cursor.read_var_usize();
    loop {
        match cursor.read_u8() {
            0x00 => return,
            0x09 => {
                for _ in 0..pack_streams {
                    let _size = cursor.read_var_u64();
                }
            }
            0x0a => skip_defined_crc(cursor, pack_streams),
            property => panic!("unexpected 7z PackInfo property: {property:#x}"),
        }
    }
}

fn skip_unpack_info(cursor: &mut HeaderCursor<'_>) {
    assert_eq!(
        cursor.read_u8(),
        0x0b,
        "7z UnpackInfo should start with Folder"
    );
    let folders = cursor.read_var_usize();
    assert_eq!(
        cursor.read_u8(),
        0,
        "external 7z folders are not expected in test archive"
    );
    let mut total_out_streams = 0_usize;
    for _ in 0..folders {
        total_out_streams += skip_folder(cursor);
    }

    loop {
        match cursor.read_u8() {
            0x00 => return,
            0x0c => {
                for _ in 0..total_out_streams {
                    let _size = cursor.read_var_u64();
                }
            }
            0x0a => skip_defined_crc(cursor, folders),
            property => panic!("unexpected 7z UnpackInfo property: {property:#x}"),
        }
    }
}

fn skip_folder(cursor: &mut HeaderCursor<'_>) -> usize {
    let coders = cursor.read_var_usize();
    let mut total_in_streams = 0_usize;
    let mut total_out_streams = 0_usize;
    for _ in 0..coders {
        let flags = cursor.read_u8();
        let method_id_size = usize::from(flags & 0x0f);
        let is_complex = flags & 0x10 != 0;
        let has_properties = flags & 0x20 != 0;
        cursor.skip(method_id_size);

        let (in_streams, out_streams) = if is_complex {
            (cursor.read_var_usize(), cursor.read_var_usize())
        } else {
            (1, 1)
        };
        total_in_streams += in_streams;
        total_out_streams += out_streams;

        if has_properties {
            let properties_size = cursor.read_var_usize();
            cursor.skip(properties_size);
        }
    }
    let bind_pairs = total_out_streams.saturating_sub(1);
    for _ in 0..bind_pairs {
        let _in_index = cursor.read_var_u64();
        let _out_index = cursor.read_var_u64();
    }
    let packed_indices = total_in_streams.saturating_sub(bind_pairs);
    if packed_indices > 1 {
        for _ in 0..packed_indices {
            let _index = cursor.read_var_u64();
        }
    }
    total_out_streams
}

fn skip_substreams_info(cursor: &mut HeaderCursor<'_>) {
    match cursor.read_u8() {
        0x00 => {}
        0x0d | 0x09 | 0x0a => {
            panic!("test helper only supports simple single-stream 7z substreams")
        }
        property => panic!("unexpected 7z SubStreamsInfo property: {property:#x}"),
    }
}

fn find_files_info_end(cursor: &mut HeaderCursor<'_>) -> usize {
    let files = cursor.read_var_usize();
    assert_eq!(files, 1, "test helper only supports one 7z file entry");
    loop {
        let property_start = cursor.position;
        let property = cursor.read_u8();
        match property {
            0x00 => return property_start,
            0x11 => {
                let size = cursor.read_var_usize();
                cursor.skip(size);
            }
            0x0e..=0x10 => {
                let size = cursor.read_var_usize();
                cursor.skip(size);
            }
            0x12..=0x16 => {
                let size = cursor.read_var_usize();
                cursor.skip(size);
            }
            property => panic!("unexpected 7z FilesInfo property: {property:#x}"),
        }
    }
}

fn skip_defined_crc(cursor: &mut HeaderCursor<'_>, count: usize) {
    let all_defined = cursor.read_u8();
    if all_defined == 0 {
        cursor.skip(count.div_ceil(8));
    }
    cursor.skip(4 * count);
}

fn single_entry_attrs_property(attrs: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.push(0x15);
    write_7z_var_u64(&mut bytes, 6);
    bytes.push(1);
    bytes.push(0);
    bytes.extend_from_slice(&attrs.to_le_bytes());
    bytes
}

fn write_7z_var_u64(bytes: &mut Vec<u8>, value: u64) {
    if value < 0x80 {
        bytes.push(u64_to_7z_single_byte(value));
    } else {
        panic!("test helper only writes single-byte 7z integers");
    }
}

fn u64_to_7z_single_byte(value: u64) -> u8 {
    u8::try_from(value).expect("test 7z integer should fit one byte")
}

struct HeaderCursor<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> HeaderCursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    fn read_u8(&mut self) -> u8 {
        let value = self.bytes[self.position];
        self.position += 1;
        value
    }

    fn read_var_usize(&mut self) -> usize {
        self.read_var_u64()
            .try_into()
            .expect("test 7z integer should fit usize")
    }

    fn read_var_u64(&mut self) -> u64 {
        let first = u64::from(self.read_u8());
        let mut mask = 0x80_u64;
        let mut value = 0_u64;
        for i in 0..8 {
            if first & mask == 0 {
                return value | ((first & (mask - 1)) << (8 * i));
            }
            value |= u64::from(self.read_u8()) << (8 * i);
            mask >>= 1;
        }
        value
    }

    fn skip(&mut self, count: usize) {
        self.position += count;
        assert!(
            self.position <= self.bytes.len(),
            "test 7z header cursor should stay in bounds"
        );
    }
}

fn create_7z_bytes_with_patched_name(original_name: &str, patched_name: &str) -> Vec<u8> {
    assert_eq!(
        original_name.encode_utf16().count(),
        patched_name.encode_utf16().count(),
        "patched 7z name must keep the UTF-16 length unchanged"
    );
    let mut bytes = create_7z_bytes(&[(original_name, Some(b"payload".as_slice()))], false);
    let original = utf16le_null_terminated(original_name);
    let patched = utf16le_null_terminated(patched_name);
    let offset = bytes
        .windows(original.len())
        .position(|window| window == original.as_slice())
        .expect("7z encoded file name should be present");
    bytes[offset..offset + patched.len()].copy_from_slice(&patched);
    refresh_7z_header_checksums(&mut bytes);
    bytes
}

fn refresh_7z_header_checksums(bytes: &mut [u8]) {
    let next_header_offset = read_u64_le(bytes, 12);
    let next_header_size = read_u64_le(bytes, 20);
    let next_header_start: usize = (32 + next_header_offset)
        .try_into()
        .expect("test 7z next header offset should fit usize");
    let next_header_size: usize = next_header_size
        .try_into()
        .expect("test 7z next header size should fit usize");
    let next_header_crc = crc32(&bytes[next_header_start..next_header_start + next_header_size]);
    bytes[28..32].copy_from_slice(&next_header_crc.to_le_bytes());

    let start_header_crc = crc32(&bytes[12..32]);
    bytes[8..12].copy_from_slice(&start_header_crc.to_le_bytes());
}

fn read_u64_le(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(
        bytes[offset..offset + 8]
            .try_into()
            .expect("test 7z header should contain u64"),
    )
}

fn utf16le_null_terminated(value: &str) -> Vec<u8> {
    let mut bytes = Vec::new();
    for code_unit in value.encode_utf16() {
        bytes.extend_from_slice(&code_unit.to_le_bytes());
    }
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    bytes
}

fn scan_7z(bytes: Vec<u8>, limits: ArchiveScanLimits) -> Result<ArchiveScanResult> {
    let source_archive_size =
        crate::utils::numbers::usize_to_u64(bytes.len(), "test 7z archive size")?;
    let archive = open_seven_zip_streaming_archive(Cursor::new(bytes), limits)?;
    scan_seven_zip_archive(
        &archive,
        limits,
        source_archive_size,
        None,
        ArchiveScanNamePolicy::StrictAsterName,
        |_| Ok(()),
    )
}

fn scan_7z_raw(bytes: Vec<u8>, limits: ArchiveScanLimits) -> Result<ArchiveRawScanResult> {
    let source_archive_size =
        crate::utils::numbers::usize_to_u64(bytes.len(), "test 7z archive size")?;
    let archive = open_seven_zip_streaming_archive(Cursor::new(bytes), limits)?;
    scan_seven_zip_archive_raw(&archive, limits, source_archive_size, None)
}

fn scan_7z_with_source_size(
    bytes: Vec<u8>,
    limits: ArchiveScanLimits,
    source_archive_size: u64,
) -> Result<ArchiveScanResult> {
    scan_7z_with_open_limits(bytes, limits, limits, source_archive_size)
}

fn scan_7z_with_open_limits(
    bytes: Vec<u8>,
    open_limits: ArchiveScanLimits,
    scan_limits: ArchiveScanLimits,
    source_archive_size: u64,
) -> Result<ArchiveScanResult> {
    let archive = open_seven_zip_streaming_archive(Cursor::new(bytes), open_limits)?;
    scan_seven_zip_archive(
        &archive,
        scan_limits,
        source_archive_size,
        None,
        ArchiveScanNamePolicy::StrictAsterName,
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
fn scan_seven_zip_rejects_too_many_entries() {
    let bytes = create_7z_bytes(
        &[
            ("first.txt", Some(b"first")),
            ("second.txt", Some(b"second")),
        ],
        false,
    );
    let mut limits = scan_limits();
    limits.max_entries = 1;

    let error = scan_7z_with_open_limits(bytes, scan_limits(), limits, 1024)
        .expect_err("7z scan should reject entry limit")
        .message()
        .to_string();
    assert!(
        error.contains("entries") || error.contains("entry count"),
        "unexpected entry limit error: {error}"
    );
}

#[test]
fn scan_seven_zip_rejects_too_many_files() {
    let bytes = create_7z_bytes(
        &[
            ("first.txt", Some(b"first")),
            ("second.txt", Some(b"second")),
        ],
        false,
    );
    let mut limits = scan_limits();
    limits.max_files = 1;

    let error = scan_7z(bytes, limits)
        .expect_err("7z scan should reject file limit")
        .message()
        .to_string();
    assert!(error.contains("files"));
}

#[test]
fn scan_seven_zip_rejects_uncompressed_size_limit() {
    let payload = vec![b'a'; 2048];
    let bytes = create_7z_bytes(&[("payload.txt", Some(&payload))], false);
    let mut limits = scan_limits();
    limits.max_uncompressed_bytes = 1024;

    let error = scan_7z(bytes, limits)
        .expect_err("7z scan should reject uncompressed size limit")
        .message()
        .to_string();
    assert!(error.contains("uncompressed size"));
}

#[test]
fn scan_seven_zip_rejects_file_directory_conflicts() {
    let error = scan_7z_error(&[("prefix", Some(b"file")), ("prefix/child", Some(b"child"))]);

    assert!(error.contains("inside file entry"));
}

#[test]
fn scan_seven_zip_rejects_skipped_entries_from_path_traversal() {
    let bytes = create_7z_bytes_with_patched_name("safe-path.txt", "../escape.txt");
    let error = scan_7z(bytes, scan_limits())
        .expect_err("7z scan should reject skipped unsafe entries")
        .message()
        .to_string();

    assert!(
        error.contains("unsafe path") || error.contains("path traversal"),
        "unexpected unsafe path error: {error}"
    );
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
        ArchiveScanNamePolicy::PreviewDisplayName,
        |_| Ok(()),
    )
    .expect("display-only scan should succeed");

    assert!(!result.extract_compatible);
}

#[test]
fn scan_seven_zip_rejects_anti_items() {
    let bytes = create_7z_bytes_with_anti_item("deleted.txt");
    let error = scan_7z(bytes, scan_limits())
        .expect_err("7z scan should reject anti item")
        .message()
        .to_string();

    assert!(error.contains("anti-item"));
}

#[test]
fn scan_seven_zip_raw_rejects_anti_items() {
    let bytes = create_7z_bytes_with_anti_item("deleted.txt");
    let error = scan_7z_raw(bytes, scan_limits())
        .expect_err("7z raw scan should reject anti item")
        .message()
        .to_string();

    assert!(error.contains("anti-item"));
}

#[test]
fn scan_seven_zip_rejects_symlink_entries() {
    let bytes = create_7z_bytes_with_attrs("link.txt", b"target.txt", 0o120777_u32 << 16);
    let error = scan_7z(bytes, scan_limits())
        .expect_err("7z scan should reject symlink entries")
        .message()
        .to_string();

    assert!(
        error.contains("symbolic link"),
        "unexpected 7z symlink error: {error}"
    );
}

#[test]
fn scan_seven_zip_rejects_special_file_entries() {
    let bytes = create_7z_bytes_with_attrs("device", b"", seven_zip_unix_attrs(0o060666));
    let error = scan_7z(bytes, scan_limits())
        .expect_err("7z scan should reject special file entries")
        .message()
        .to_string();

    assert!(
        error.contains("special file"),
        "unexpected 7z special file error: {error}"
    );
}

#[test]
fn scan_seven_zip_rejects_encrypted_entries() {
    let error = scan_7z(ENCRYPTED_7Z_ENTRY_FIXTURE.to_vec(), scan_limits())
        .expect_err("7z scan should reject encrypted entries")
        .message()
        .to_string();

    assert!(error.contains("encrypted"));
}

#[test]
fn seven_zip_open_error_maps_password_failures_to_validation_error() {
    let error = map_seven_zip_open_error(zesven::Error::PasswordRequired);

    assert_eq!(error.message(), "encrypted 7z archives are not supported");
}

#[test]
fn seven_zip_streaming_config_maps_resource_limits() {
    let mut limits = scan_limits();
    limits.max_entries = 17;
    limits.max_entry_compression_ratio = 23;
    limits.max_compression_ratio = 31;

    let config = seven_zip_streaming_config(limits).expect("7z streaming config should be created");

    assert_eq!(config.max_entries, 17);
    assert_eq!(config.max_compression_ratio, 23);
    assert_eq!(config.decoder_pool_capacity, None);
}

#[test]
fn seven_zip_streaming_config_rejects_out_of_range_ratio() {
    let mut limits = scan_limits();
    limits.max_entry_compression_ratio = u64::from(u32::MAX) + 1;
    limits.max_compression_ratio = u64::from(u32::MAX) + 1;

    let error = seven_zip_streaming_config(limits)
        .expect_err("7z streaming config should reject ratios beyond parser range");

    assert!(error.message().contains("parser range"));
}

#[test]
fn scan_seven_zip_rejects_total_compression_ratio() {
    let payload = vec![b'a'; 4096];
    let bytes = create_7z_bytes(&[("payload.txt", Some(&payload))], false);
    let open_limits = scan_limits();
    let mut limits = scan_limits();
    limits.max_entry_compression_ratio = u64::MAX;
    limits.max_compression_ratio = 1;

    let error = scan_7z_with_open_limits(bytes, open_limits, limits, 1024)
        .expect_err("7z scan should reject total compression ratio")
        .message()
        .to_string();
    assert!(error.contains("total compression ratio"));
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
fn scan_seven_zip_rejects_entry_compression_ratio_against_source_size() {
    let payload = vec![b'a'; 4096];
    let bytes = create_7z_bytes(&[("payload.txt", Some(&payload))], false);
    let mut limits = scan_limits();
    limits.max_entry_compression_ratio = 1;
    limits.max_compression_ratio = u64::MAX;

    let error = scan_7z_with_source_size(bytes, limits, 1024)
        .expect_err("7z scan should reject entry compression ratio")
        .message()
        .to_string();
    assert!(error.contains("compression ratio"));
}

#[test]
fn build_seven_zip_scan_result_uses_single_source_size_for_total_ratio() {
    let mut limits = scan_limits();
    limits.max_entry_compression_ratio = u64::MAX;
    limits.max_compression_ratio = 1;
    let raw_entries = vec![
        ArchiveRawScanEntry {
            index: 0,
            raw_name: b"first.txt".to_vec(),
            display_name: "first.txt".to_string(),
            raw_name_utf8: true,
            kind: ArchiveScanEntryKind::File,
            size: 60,
            compressed_size: 100,
            modified_at: None,
        },
        ArchiveRawScanEntry {
            index: 1,
            raw_name: b"second.txt".to_vec(),
            display_name: "second.txt".to_string(),
            raw_name_utf8: true,
            kind: ArchiveScanEntryKind::File,
            size: 60,
            compressed_size: 100,
            modified_at: None,
        },
    ];

    let error = build_seven_zip_scan_result_from_raw_entries(
        &raw_entries,
        100,
        limits,
        None,
        ArchiveScanNamePolicy::StrictAsterName,
        |_| Ok(()),
    )
    .expect_err("7z raw replay should reject against the source archive size once");

    assert!(error.message().contains("total compression ratio"));
}

#[test]
fn scan_seven_zip_raw_returns_manifest_and_rejects_replay_path_traversal() {
    let bytes = create_7z_bytes(&[("safe/file.txt", Some(b"payload".as_slice()))], false);
    let raw = scan_7z_raw(bytes, scan_limits()).expect("7z raw scan should succeed");

    assert_eq!(raw.entry_count, 1);
    assert_eq!(raw.file_count, 1);
    assert_eq!(raw.total_uncompressed_bytes, 7);
    assert_eq!(raw.entries[0].display_name, "safe/file.txt");
    assert_eq!(raw.entries[0].raw_name, b"safe/file.txt");

    let mut tampered_entries = raw.entries;
    tampered_entries[0].raw_name = b"../escape.txt".to_vec();
    tampered_entries[0].display_name = "../escape.txt".to_string();
    let error = build_seven_zip_scan_result_from_raw_entries(
        &tampered_entries,
        raw.total_compressed_base,
        scan_limits(),
        None,
        ArchiveScanNamePolicy::StrictAsterName,
        |_| Ok(()),
    )
    .expect_err("7z raw replay should reject unsafe paths");

    assert!(error.message().contains("unsafe path"));
}

#[test]
fn build_seven_zip_scan_result_rejects_raw_replay_entry_limit() {
    let raw_entries = vec![
        ArchiveRawScanEntry {
            index: 0,
            raw_name: b"first.txt".to_vec(),
            display_name: "first.txt".to_string(),
            raw_name_utf8: true,
            kind: ArchiveScanEntryKind::File,
            size: 1,
            compressed_size: 1,
            modified_at: None,
        },
        ArchiveRawScanEntry {
            index: 1,
            raw_name: b"second.txt".to_vec(),
            display_name: "second.txt".to_string(),
            raw_name_utf8: true,
            kind: ArchiveScanEntryKind::File,
            size: 1,
            compressed_size: 1,
            modified_at: None,
        },
    ];
    let mut limits = scan_limits();
    limits.max_entries = 1;

    let error = build_seven_zip_scan_result_from_raw_entries(
        &raw_entries,
        2,
        limits,
        None,
        ArchiveScanNamePolicy::StrictAsterName,
        |_| Ok(()),
    )
    .expect_err("7z raw replay should reject entry limit");

    assert!(error.message().contains("entries"));
}

#[test]
fn build_seven_zip_scan_result_rejects_raw_replay_extracted_size_overflow() {
    let raw_entries = vec![ArchiveRawScanEntry {
        index: 0,
        raw_name: b"payload.txt".to_vec(),
        display_name: "payload.txt".to_string(),
        raw_name_utf8: true,
        kind: ArchiveScanEntryKind::File,
        size: 11,
        compressed_size: 11,
        modified_at: None,
    }];
    let mut limits = scan_limits();
    limits.max_uncompressed_bytes = 10;

    let error = build_seven_zip_scan_result_from_raw_entries(
        &raw_entries,
        11,
        limits,
        None,
        ArchiveScanNamePolicy::StrictAsterName,
        |_| Ok(()),
    )
    .expect_err("7z raw replay should reject extracted size above preflight limit");

    assert!(error.message().contains("uncompressed size"));
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
        ArchiveScanNamePolicy::StrictAsterName,
        |_| Ok(()),
    )
    .expect("solid 7z scan should succeed");
    assert_eq!(result.file_count, 2);
}
