use serde::Serialize;
use serde_json;

pub fn to_string<T>(value: &T) -> serde_json::Result<String>
where
    T: ?Sized + Serialize,
{
    Ok(serde_json::to_string(value)?
        .replace(',', ", ")
        .replace(':', ": "))
}
