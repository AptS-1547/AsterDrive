use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(16))")]
#[serde(rename_all = "snake_case")]
pub enum MediaMetadataKind {
    #[sea_orm(string_value = "image")]
    Image,
    #[sea_orm(string_value = "audio")]
    Audio,
    #[sea_orm(string_value = "video")]
    Video,
}

impl MediaMetadataKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Audio => "audio",
            Self::Video => "video",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(16))")]
#[serde(rename_all = "snake_case")]
pub enum MediaMetadataStatus {
    #[sea_orm(string_value = "ready")]
    Ready,
    #[sea_orm(string_value = "failed")]
    Failed,
    #[sea_orm(string_value = "unsupported")]
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct ImageMediaMetadata {
    pub width: u32,
    pub height: u32,
    pub format: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct AudioMediaMetadata {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub duration_ms: Option<u64>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u8>,
    pub bit_depth: Option<u8>,
    pub overall_bitrate: Option<u32>,
    pub audio_bitrate: Option<u32>,
    pub track_number: Option<u32>,
    pub track_total: Option<u32>,
    pub disc_number: Option<u32>,
    pub disc_total: Option<u32>,
    pub genre: Option<String>,
    pub date: Option<String>,
    pub has_embedded_picture: bool,
    pub embedded_picture_mime_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct VideoMediaMetadata {
    pub duration_ms: Option<u64>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub codec: Option<String>,
    pub container: Option<String>,
    pub frame_rate: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MediaMetadataPayload {
    Image(ImageMediaMetadata),
    Audio(AudioMediaMetadata),
    Video(VideoMediaMetadata),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, DeriveValueType)]
pub struct StoredMediaMetadataPayload(pub String);

impl AsRef<str> for StoredMediaMetadataPayload {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<String> for StoredMediaMetadataPayload {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<StoredMediaMetadataPayload> for String {
    fn from(value: StoredMediaMetadataPayload) -> Self {
        value.0
    }
}
