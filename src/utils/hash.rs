use crate::errors::{AsterError, MapAsterErr, Result};
use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};
use sha2::{Digest, Sha256};
use std::fmt::Write;

/// Argon2 密码哈希
pub fn hash_password(password: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_aster_err(AsterError::internal_error)
}

/// 验证密码
pub fn verify_password(password: &str, hash: &str) -> Result<bool> {
    let parsed = PasswordHash::new(hash).map_aster_err(AsterError::internal_error)?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

/// 计算数据的 SHA-256 hex 字符串
pub fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    sha256_digest_to_hex(&hasher.finalize())
}

/// 将 SHA-256 digest 编码为小写 hex
pub fn sha256_digest_to_hex(digest: &[u8]) -> String {
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        let _ = write!(&mut hex, "{byte:02x}");
    }
    hex
}
