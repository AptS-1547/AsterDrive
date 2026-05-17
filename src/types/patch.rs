use serde::{Deserialize, Deserializer};

/// PATCH 请求里的可空字段三态：
/// - `Absent`：字段未传，保持不变
/// - `Null`：字段显式传 `null`，清空该字段
/// - `Value`：字段传具体值，更新为该值
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NullablePatch<T> {
    #[default]
    Absent,
    Null,
    Value(T),
}

impl<T> NullablePatch<T> {
    pub fn is_present(&self) -> bool {
        !matches!(self, Self::Absent)
    }
}

pub fn deserialize_nullable_patch_option<'de, D, T>(
    deserializer: D,
) -> Result<Option<NullablePatch<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(|value| Some(NullablePatch::from(value)))
}

impl<T> From<Option<T>> for NullablePatch<T> {
    fn from(value: Option<T>) -> Self {
        match value {
            Some(value) => Self::Value(value),
            None => Self::Null,
        }
    }
}

impl<'de, T> Deserialize<'de> for NullablePatch<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(match Option::<T>::deserialize(deserializer)? {
            Some(value) => Self::Value(value),
            None => Self::Null,
        })
    }
}
