pub mod hash;
pub mod id;
pub mod paths;

use crate::errors::{AsterError, Result};

/// 临时文件目录（上传流式处理用）
pub use paths::DEFAULT_TEMP_DIR as TEMP_DIR;

/// 校验资源归属权，不匹配则返回 403
pub fn verify_owner(entity_user_id: i64, user_id: i64, entity_name: &str) -> Result<()> {
    if entity_user_id != user_id {
        return Err(AsterError::auth_forbidden(format!(
            "not your {entity_name}"
        )));
    }
    Ok(())
}

/// 清理临时文件/目录，失败时记录 warn 日志而不是静默忽略
pub async fn cleanup_temp_file(path: &str) {
    if let Err(e) = tokio::fs::remove_file(path).await
        && e.kind() != std::io::ErrorKind::NotFound
    {
        tracing::warn!("failed to cleanup temp file {path}: {e}");
    }
}

pub async fn cleanup_temp_dir(path: &str) {
    // macOS Spotlight/Finder 可能在删除过程中往目录里塞 .DS_Store 等文件，
    // 导致 remove_dir_all 的最终 rmdir 返回 ENOTEMPTY，重试即可。
    for _ in 0..3 {
        match tokio::fs::remove_dir_all(path).await {
            Ok(()) => return,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return,
            Err(e) if e.kind() == std::io::ErrorKind::DirectoryNotEmpty => {
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
            Err(e) => {
                tracing::warn!("failed to cleanup temp dir {path}: {e}");
                return;
            }
        }
    }
    if let Err(e) = tokio::fs::remove_dir_all(path).await
        && e.kind() != std::io::ErrorKind::NotFound
    {
        tracing::warn!("failed to cleanup temp dir {path}: {e}");
    }
}

/// 文件名最大长度
const MAX_FILENAME_LEN: usize = 255;

/// 文件/文件夹名禁止字符
const FORBIDDEN_CHARS: &[char] = &['/', '\\', '\0', ':', '*', '?', '"', '<', '>', '|'];

/// 校验文件/文件夹名合法性
pub fn validate_name(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(AsterError::validation_error("name cannot be empty"));
    }
    if name.len() > MAX_FILENAME_LEN {
        return Err(AsterError::validation_error(format!(
            "name too long (max {MAX_FILENAME_LEN} chars)"
        )));
    }
    if name == "." || name == ".." {
        return Err(AsterError::validation_error("invalid name"));
    }
    if let Some(c) = name.chars().find(|c| FORBIDDEN_CHARS.contains(c)) {
        return Err(AsterError::validation_error(format!(
            "name contains forbidden character '{c}'"
        )));
    }
    if name.chars().any(|c| c.is_ascii_control()) {
        return Err(AsterError::validation_error(
            "name contains control characters",
        ));
    }
    if name != name.trim() || name.ends_with('.') {
        return Err(AsterError::validation_error(
            "name cannot start/end with spaces or end with a dot",
        ));
    }
    Ok(())
}

/// 根据 SHA256 hex hash 计算内容寻址存储路径：`ab/cd/abcdef...`
pub fn storage_path_from_hash(hash: &str) -> String {
    format!("{}/{}/{}", &hash[..2], &hash[2..4], hash)
}

/// macOS / Office 生成的隐藏文件名，不在目录列表中显示
pub fn is_hidden_name(name: &str) -> bool {
    name.starts_with("._")
        || name.starts_with("~$")
        || name == ".DS_Store"
        || name == ".Spotlight-V100"
        || name == ".Trashes"
}

/// 生成副本名称（macOS/Windows 风格）
///
/// 规则：
/// - `test.txt` → `test (1).txt`
/// - `test (1).txt` → `test (2).txt`
/// - `test (99).txt` → `test (100).txt`
/// - `folder` → `folder (1)` （无扩展名）
/// - `folder (3)` → `folder (4)`
pub fn next_copy_name(name: &str) -> String {
    // 分离扩展名
    let (stem, ext) = match name.rfind('.') {
        Some(dot) if dot > 0 => (&name[..dot], Some(&name[dot..])),
        _ => (name, None),
    };

    // 检查 stem 是否已经有 " (N)" 后缀
    let (base, next_n) = if let Some(paren_start) = stem.rfind(" (") {
        let after_paren = &stem[paren_start + 2..];
        if let Some(num_str) = after_paren.strip_suffix(')') {
            if let Ok(n) = num_str.parse::<u32>() {
                (&stem[..paren_start], n + 1)
            } else {
                (stem, 1)
            }
        } else {
            (stem, 1)
        }
    } else {
        (stem, 1)
    };

    match ext {
        Some(ext) => format!("{base} ({next_n}){ext}"),
        None => format!("{base} ({next_n})"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_hidden_name() {
        assert!(is_hidden_name("._file.txt"));
        assert!(is_hidden_name("~$document.docx"));
        assert!(is_hidden_name(".DS_Store"));
        assert!(is_hidden_name(".Spotlight-V100"));
        assert!(is_hidden_name(".Trashes"));
        assert!(!is_hidden_name("normal.txt"));
    }

    #[test]
    fn test_validate_name() {
        // 有效名称
        assert!(validate_name("hello.txt").is_ok());
        assert!(validate_name(".gitignore").is_ok());
        assert!(validate_name("file (1).txt").is_ok());

        // 空名
        assert!(validate_name("").is_err());

        // 禁止字符
        assert!(validate_name("a/b").is_err());
        assert!(validate_name("a\\b").is_err());
        assert!(validate_name("a:b").is_err());
        assert!(validate_name("a*b").is_err());
        assert!(validate_name("a?b").is_err());
        assert!(validate_name("a\"b").is_err());
        assert!(validate_name("a<b").is_err());
        assert!(validate_name("a>b").is_err());
        assert!(validate_name("a|b").is_err());

        // 特殊名称
        assert!(validate_name(".").is_err());
        assert!(validate_name("..").is_err());

        // 控制字符
        assert!(validate_name("a\x01b").is_err());
        assert!(validate_name("a\nb").is_err());
        assert!(validate_name("a\tb").is_err());

        // 首尾空格 / 末尾点号
        assert!(validate_name(" leading").is_err());
        assert!(validate_name("trailing ").is_err());
        assert!(validate_name("ends.").is_err());

        // 超长
        let long_name = "a".repeat(256);
        assert!(validate_name(&long_name).is_err());
        let ok_name = "a".repeat(255);
        assert!(validate_name(&ok_name).is_ok());
    }

    #[test]
    fn test_next_copy_name() {
        assert_eq!(next_copy_name("test.txt"), "test (1).txt");
        assert_eq!(next_copy_name("test (1).txt"), "test (2).txt");
        assert_eq!(next_copy_name("test (99).txt"), "test (100).txt");
        assert_eq!(next_copy_name("folder"), "folder (1)");
        assert_eq!(next_copy_name("folder (3)"), "folder (4)");
        assert_eq!(next_copy_name("my.file.tar.gz"), "my.file.tar (1).gz");
        assert_eq!(next_copy_name("photo (1).jpg"), "photo (2).jpg");
        assert_eq!(next_copy_name(".hidden"), ".hidden (1)");
    }

    #[test]
    fn test_storage_path_from_hash() {
        let hash = "abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890";
        assert_eq!(storage_path_from_hash(hash), format!("ab/cd/{hash}"));
    }
}
