//! 管理员 API 路由：`common`。

use serde::{Deserialize, de::Error as DeError};

pub(crate) fn deserialize_non_null_policy_group_id<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<i64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    match Option::<i64>::deserialize(deserializer)? {
        Some(policy_group_id) => Ok(Some(policy_group_id)),
        None => Err(D::Error::custom("policy_group_id cannot be null")),
    }
}
