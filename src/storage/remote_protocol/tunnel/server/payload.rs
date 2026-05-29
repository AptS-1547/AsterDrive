use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(utoipa::ToSchema))]
pub struct RemoteTunnelRequest {
    pub request_id: String,
    pub method: String,
    pub path_and_query: String,
    pub headers: Vec<(String, String)>,
    #[serde(with = "base64_body")]
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = String))]
    pub body: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(utoipa::ToSchema))]
pub struct RemoteTunnelResponse {
    pub request_id: String,
    pub status: u16,
    pub headers: Vec<(String, String)>,
    #[serde(with = "base64_body")]
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = String))]
    pub body: Vec<u8>,
}

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(utoipa::ToSchema))]
pub struct RemoteTunnelPollRequest {
    pub access_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(utoipa::ToSchema))]
pub struct RemoteTunnelPollResponse {
    pub request: Option<RemoteTunnelRequest>,
}

mod base64_body {
    use super::*;
    use serde::{Deserializer, Serializer, de::Error as _};

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum EncodedBody {
        Base64(String),
        LegacyArray(Vec<u8>),
    }

    pub fn serialize<S>(body: &[u8], serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&BASE64_STANDARD.encode(body))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> std::result::Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        match EncodedBody::deserialize(deserializer)? {
            EncodedBody::Base64(value) => BASE64_STANDARD
                .decode(value)
                .map_err(|error| D::Error::custom(format!("invalid base64 body: {error}"))),
            EncodedBody::LegacyArray(body) => Ok(body),
        }
    }
}
