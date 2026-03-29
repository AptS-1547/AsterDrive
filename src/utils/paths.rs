pub const DEFAULT_TEMP_DIR: &str = "data/.tmp";
pub const DEFAULT_UPLOAD_TEMP_DIR: &str = "data/.uploads";

pub fn temp_file_path(temp_dir: &str, name: &str) -> String {
    let root = temp_dir.trim_end_matches('/');
    format!("{root}/{name}")
}

pub fn upload_temp_dir(upload_temp_root: &str, upload_id: &str) -> String {
    let root = upload_temp_root.trim_end_matches('/');
    format!("{root}/{upload_id}")
}

pub fn upload_chunk_path(upload_temp_root: &str, upload_id: &str, chunk_number: i32) -> String {
    format!(
        "{}/chunk_{chunk_number}",
        upload_temp_dir(upload_temp_root, upload_id)
    )
}

pub fn upload_assembled_path(upload_temp_root: &str, upload_id: &str) -> String {
    format!(
        "{}/_assembled",
        upload_temp_dir(upload_temp_root, upload_id)
    )
}
