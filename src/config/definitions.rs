//! 运行时配置定义 — 所有 system_config 键的单一数据源
//!
//! 启动时 `ensure_defaults()` 遍历此数组，
//! 对每项执行 INSERT ... ON CONFLICT DO NOTHING。

/// 单条配置定义
pub struct ConfigDef {
    /// 配置键（数据库 unique key）
    pub key: &'static str,
    /// 值类型：前端渲染用
    pub value_type: &'static str,
    /// 默认值生成函数
    pub default_fn: fn() -> String,
    /// 修改后是否需要重启
    pub requires_restart: bool,
    /// 是否敏感值
    pub is_sensitive: bool,
    /// 分类（前端分组用）
    pub category: &'static str,
    /// 描述
    pub description: &'static str,
}

/// 所有运行时配置项
pub static ALL_CONFIGS: &[ConfigDef] = &[
    // ── WebDAV ──────────────────────────────────────────────
    ConfigDef {
        key: "webdav_enabled",
        value_type: "boolean",
        default_fn: || "true".to_string(),
        requires_restart: false,
        is_sensitive: false,
        category: "webdav",
        description: "Enable or disable WebDAV access",
    },
    // ── Storage ─────────────────────────────────────────────
    ConfigDef {
        key: "max_versions_per_file",
        value_type: "number",
        default_fn: || "10".to_string(),
        requires_restart: false,
        is_sensitive: false,
        category: "storage",
        description: "Maximum number of historical versions kept per file",
    },
    ConfigDef {
        key: "trash_retention_days",
        value_type: "number",
        default_fn: || "7".to_string(),
        requires_restart: false,
        is_sensitive: false,
        category: "storage",
        description: "Days before soft-deleted items are permanently purged",
    },
    ConfigDef {
        key: "default_storage_quota",
        value_type: "number",
        default_fn: || "0".to_string(),
        requires_restart: false,
        is_sensitive: false,
        category: "storage",
        description: "Default storage quota for new users in bytes (0 = unlimited)",
    },
    // ── Audit ─────────────────────────────────────────────
    ConfigDef {
        key: "audit_log_enabled",
        value_type: "boolean",
        default_fn: || "true".to_string(),
        requires_restart: false,
        is_sensitive: false,
        category: "audit",
        description: "Enable or disable audit logging",
    },
    ConfigDef {
        key: "audit_log_retention_days",
        value_type: "number",
        default_fn: || "90".to_string(),
        requires_restart: false,
        is_sensitive: false,
        category: "audit",
        description: "Days before audit log entries are permanently deleted",
    },
];
