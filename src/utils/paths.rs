pub const DEFAULT_TEMP_DIR: &str = "data/.tmp";
pub const DEFAULT_UPLOAD_TEMP_DIR: &str = "data/.uploads";

fn join_path(root: &str, leaf: &str) -> String {
    let root_had_leading_slash = root.starts_with('/');
    let root = root.trim_end_matches('/');
    let leaf = leaf.trim_matches('/');

    if root.is_empty() {
        return if leaf.is_empty() {
            if root_had_leading_slash {
                "/".to_string()
            } else {
                String::new()
            }
        } else if root_had_leading_slash {
            format!("/{leaf}")
        } else {
            leaf.to_string()
        };
    }

    if leaf.is_empty() {
        return root.to_string();
    }

    format!("{root}/{leaf}")
}

pub fn temp_file_path(temp_dir: &str, name: &str) -> String {
    join_path(temp_dir, name)
}

pub fn upload_temp_dir(upload_temp_root: &str, upload_id: &str) -> String {
    join_path(upload_temp_root, upload_id)
}

pub fn upload_chunk_path(upload_temp_root: &str, upload_id: &str, chunk_number: i32) -> String {
    join_path(
        &upload_temp_dir(upload_temp_root, upload_id),
        &format!("chunk_{chunk_number}"),
    )
}

pub fn upload_assembled_path(upload_temp_root: &str, upload_id: &str) -> String {
    join_path(&upload_temp_dir(upload_temp_root, upload_id), "_assembled")
}

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_TEMP_DIR, DEFAULT_UPLOAD_TEMP_DIR, temp_file_path, upload_assembled_path,
        upload_chunk_path, upload_temp_dir,
    };

    fn assert_no_double_slash(path: &str) {
        assert!(
            !path.contains("//"),
            "path should not contain double slashes: {path}"
        );
    }

    #[test]
    fn temp_file_path_joins_normal_inputs() {
        let path = temp_file_path(DEFAULT_TEMP_DIR, "abc123");
        assert_eq!(path, "data/.tmp/abc123");
        assert_no_double_slash(&path);
    }

    #[test]
    fn temp_file_path_trims_user_supplied_slashes() {
        let path = temp_file_path("data/.tmp///", "/nested/file.tmp/");
        assert_eq!(path, "data/.tmp/nested/file.tmp");
        assert_no_double_slash(&path);
    }

    #[test]
    fn temp_file_path_preserves_absolute_root_without_double_slash() {
        let path = temp_file_path("/tmp///", "///upload.bin");
        assert_eq!(path, "/tmp/upload.bin");
        assert_no_double_slash(&path);
    }

    #[test]
    fn upload_temp_dir_trims_edge_case_input() {
        let path = upload_temp_dir(DEFAULT_UPLOAD_TEMP_DIR, "/session-123/");
        assert_eq!(path, "data/.uploads/session-123");
        assert_no_double_slash(&path);
    }

    #[test]
    fn upload_temp_dir_handles_root_only_prefix() {
        let path = upload_temp_dir("/", "/session-123/");
        assert_eq!(path, "/session-123");
        assert_no_double_slash(&path);
    }

    #[test]
    fn upload_chunk_path_never_emits_double_slashes_for_user_like_input() {
        let path = upload_chunk_path("data/.uploads///", "///session-123///", 7);
        assert_eq!(path, "data/.uploads/session-123/chunk_7");
        assert_no_double_slash(&path);
    }

    #[test]
    fn upload_assembled_path_never_emits_double_slashes_for_user_like_input() {
        let path = upload_assembled_path("/var/tmp/uploads///", "///session-123///");
        assert_eq!(path, "/var/tmp/uploads/session-123/_assembled");
        assert_no_double_slash(&path);
    }

    #[test]
    fn empty_root_and_leaf_do_not_generate_slashes() {
        let path = temp_file_path("", "");
        assert_eq!(path, "");
        assert_no_double_slash(&path);
    }

    #[test]
    fn empty_leaf_returns_normalized_root() {
        let path = upload_temp_dir("data/.uploads///", "");
        assert_eq!(path, "data/.uploads");
        assert_no_double_slash(&path);
    }
}
