use serde::de::Deserializer;

/// Lets a PATCH body distinguish "field absent" (leave unchanged) from "field is null"
/// (clear the field). `Option<T>` alone collapses both cases into `None`.
pub fn double_option<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    serde::Deserialize::deserialize(deserializer).map(Some)
}
