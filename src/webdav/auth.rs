use base64::Engine;

use crate::db::repository::{user_repo, webdav_account_repo};
use crate::errors::{AsterError, MapAsterErr};
use crate::runtime::AppState;
use crate::services::auth_service;
use crate::utils::hash;

/// WebDAV 认证结果
pub struct WebdavAuthResult {
    pub user_id: i64,
    /// 限制访问范围：None = 全部，Some(folder_id) = 只能访问该文件夹及子目录
    pub root_folder_id: Option<i64>,
}

/// 从 WebDAV 请求头提取并认证用户
///
/// 支持：
/// 1. `Authorization: Basic base64(username:password)` — 查 webdav_accounts 表
/// 2. `Authorization: Bearer <jwt_token>` — JWT 认证（API 客户端用，全部访问权限）
pub async fn authenticate_webdav(
    headers: &http::HeaderMap,
    state: &AppState,
) -> Result<WebdavAuthResult, AsterError> {
    let auth_header = headers
        .get(http::header::AUTHORIZATION)
        .and_then(|v: &http::HeaderValue| v.to_str().ok())
        .ok_or_else(|| AsterError::auth_token_invalid("missing Authorization header"))?;

    if let Some(basic) = auth_header.strip_prefix("Basic ") {
        let (user_id, root_folder_id) = authenticate_basic(basic.trim(), state).await?;
        Ok(WebdavAuthResult {
            user_id,
            root_folder_id,
        })
    } else if let Some(bearer) = auth_header.strip_prefix("Bearer ") {
        let user_id = authenticate_bearer(bearer.trim(), state).await?;
        Ok(WebdavAuthResult {
            user_id,
            root_folder_id: None, // JWT = 全部访问
        })
    } else {
        Err(AsterError::auth_token_invalid(
            "unsupported auth scheme, use Basic or Bearer",
        ))
    }
}

/// Basic Auth: 查 webdav_accounts 表（独立于登录密码）
/// 返回 (user_id, root_folder_id)
async fn authenticate_basic(
    encoded: &str,
    state: &AppState,
) -> Result<(i64, Option<i64>), AsterError> {
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_aster_err_with(|| AsterError::auth_invalid_credentials("invalid base64"))?;

    let credentials = String::from_utf8(decoded)
        .map_aster_err_with(|| AsterError::auth_invalid_credentials("invalid utf8"))?;

    let (username, password) = credentials
        .split_once(':')
        .ok_or_else(|| AsterError::auth_invalid_credentials("invalid basic auth format"))?;

    // 查 WebDAV 专用账号
    let account = webdav_account_repo::find_by_username(&state.db, username)
        .await?
        .ok_or_else(|| AsterError::auth_invalid_credentials("WebDAV account not found"))?;

    if !account.is_active {
        return Err(AsterError::auth_forbidden("WebDAV account is disabled"));
    }

    if !hash::verify_password(password, &account.password_hash)? {
        return Err(AsterError::auth_invalid_credentials("wrong password"));
    }

    // 确认关联用户仍然活跃
    let user = user_repo::find_by_id(&state.db, account.user_id).await?;
    if !user.status.is_active() {
        return Err(AsterError::auth_forbidden("user account is disabled"));
    }

    Ok((account.user_id, account.root_folder_id))
}

/// Bearer JWT: verify_token → Claims.user_id
async fn authenticate_bearer(token: &str, state: &AppState) -> Result<i64, AsterError> {
    let (claims, _) = auth_service::authenticate_access_token(state, token).await?;
    Ok(claims.user_id)
}
