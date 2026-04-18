//! 工具子模块：`hash`。

use crate::errors::{AsterError, MapAsterErr, Result};
use argon2::{
    Algorithm, Argon2, Params, Version,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};
use sha2::{Digest, Sha256};
use std::fmt::Write;
use std::sync::atomic::{AtomicBool, Ordering};

static FAST_PASSWORD_HASH_FOR_TESTS: AtomicBool = AtomicBool::new(false);

const FAST_TEST_ARGON2_MEMORY_KIB: u32 = 1024;
const FAST_TEST_ARGON2_TIME_COST: u32 = 1;
const FAST_TEST_ARGON2_LANES: u32 = 1;

#[doc(hidden)]
pub fn enable_fast_password_hash_for_test() {
    FAST_PASSWORD_HASH_FOR_TESTS.store(true, Ordering::Relaxed);
}

fn password_hasher() -> Result<Argon2<'static>> {
    if !FAST_PASSWORD_HASH_FOR_TESTS.load(Ordering::Relaxed) {
        return Ok(Argon2::default());
    }

    let params = Params::new(
        FAST_TEST_ARGON2_MEMORY_KIB,
        FAST_TEST_ARGON2_TIME_COST,
        FAST_TEST_ARGON2_LANES,
        None,
    )
    .map_aster_err(AsterError::internal_error)?;

    Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params))
}

/// Argon2 密码哈希
pub fn hash_password(password: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    password_hasher()?
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_aster_err(AsterError::internal_error)
}

/// 验证密码
pub fn verify_password(password: &str, hash: &str) -> Result<bool> {
    let parsed = PasswordHash::new(hash).map_aster_err(AsterError::internal_error)?;
    Ok(password_hasher()?
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

/// 计算数据的 SHA-256 hex 字符串
pub fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    bytes_to_hex(&hasher.finalize())
}

/// 将任意字节切片编码为小写 hex
pub fn bytes_to_hex(bytes: &[u8]) -> String {
    let mut hex = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(&mut hex, "{byte:02x}");
    }
    hex
}

/// 将 SHA-256 digest 编码为小写 hex
pub fn sha256_digest_to_hex(digest: &[u8]) -> String {
    bytes_to_hex(digest)
}
