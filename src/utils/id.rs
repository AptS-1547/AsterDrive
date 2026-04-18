//! 工具子模块：`id`。

use uuid::Uuid;

/// 生成 UUID v4 字符串（用于 share token 等）
pub fn new_uuid() -> String {
    Uuid::new_v4().to_string()
}

/// 生成短 token（32 字符 hex）
pub fn new_short_token() -> String {
    let id = Uuid::new_v4();
    id.simple().to_string()
}

/// 生成分享链接 token（8 位 base62: a-zA-Z0-9）
pub fn new_share_token() -> String {
    use rand::RngExt;
    const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut rng = rand::rng();
    (0..8)
        .map(|_| CHARSET[rng.random_range(0..CHARSET.len())] as char)
        .collect()
}
