use serde::de::Deserializer;

/// Lets a PATCH body distinguish "field absent" (leave unchanged) from "field is null"
/// (clear the column). `Option<T>` alone collapses both cases into `None`.
///
/// Used with `#[serde(default, deserialize_with = "double_option")]`.
pub fn double_option<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    serde::Deserialize::deserialize(deserializer).map(Some)
}
