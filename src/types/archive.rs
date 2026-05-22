use serde::{Deserialize, Serialize};
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::ToSchema;

/// ZIP entry filename decoding strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum ArchiveFilenameEncoding {
    #[default]
    Auto,
    Utf8,
    Gb18030,
    Cp437,
}

impl ArchiveFilenameEncoding {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Utf8 => "utf8",
            Self::Gb18030 => "gb18030",
            Self::Cp437 => "cp437",
        }
    }
}
