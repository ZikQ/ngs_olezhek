use std::{collections::BTreeMap, fmt};

use axum::{
    Json,
    body::Bytes,
    extract::{FromRequest, Request},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Serialize, de::DeserializeOwned};
use serde_path_to_error::Segment;

/// Answers 422 with `{"message": "...", "errors": {"name": ["..."]}}`. The client puts `errors`
/// into `err.fields`, and `Form` highlights the inputs with those names. Nested fields are
/// joined with dots: `address.city`, `tags.0`.
#[derive(Clone, Debug, Default, Serialize)]
pub struct FieldErrors {
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
    errors: BTreeMap<String, Vec<String>>,
}

impl FieldErrors {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn field(mut self, name: impl Into<String>, message: impl Into<String>) -> Self {
        self.add(name, message);
        self
    }

    pub fn add(&mut self, name: impl Into<String>, message: impl Into<String>) -> &mut Self {
        self.errors.entry(name.into()).or_default().push(message.into());
        self
    }

    /// Adds `message` to `name` unless `ok`.
    pub fn check(&mut self, ok: bool, name: impl Into<String>, message: impl Into<String>) -> &mut Self {
        if !ok {
            self.add(name, message);
        }
        self
    }

    /// Text for the form as a whole (shown above the fields, or instead of them).
    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }

    pub fn set_message(&mut self, message: impl Into<String>) -> &mut Self {
        self.message = Some(message.into());
        self
    }

    pub fn get(&self, name: &str) -> &[String] {
        self.errors.get(name).map_or(&[], Vec::as_slice)
    }

    pub fn is_empty(&self) -> bool {
        self.errors.is_empty() && self.message.is_none()
    }

    /// `Err(self)` if anything was added, so a handler can end with `errors.result()?`.
    pub fn result(self) -> Result<(), Self> {
        if self.is_empty() { Ok(()) } else { Err(self) }
    }
}

impl fmt::Display for FieldErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(m) = &self.message {
            write!(f, "{m}")?;
        }
        for (name, list) in &self.errors {
            write!(f, "{}{name}: {}", if f.alternate() { "\n" } else { "; " }, list.join(", "))?;
        }
        Ok(())
    }
}

impl std::error::Error for FieldErrors {}

impl IntoResponse for FieldErrors {
    fn into_response(self) -> Response {
        (StatusCode::UNPROCESSABLE_ENTITY, Json(self)).into_response()
    }
}

/// Checks done after deserialization; report problems with `errors.add("field", "message")`.
pub trait Validate {
    fn validate(&self, errors: &mut FieldErrors);
}

/// A JSON body like `Json<T>`, except that a body that does not fit `T` and whatever
/// `T::validate` reports both answer [`FieldErrors`] (422) naming the fields.
#[derive(Clone, Debug)]
pub struct Valid<T>(pub T);

impl<S: Send + Sync, T: DeserializeOwned + Validate> FromRequest<S> for Valid<T> {
    type Rejection = Response;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let bytes = Bytes::from_request(req, state).await.map_err(IntoResponse::into_response)?;
        let value = decode::<T>(&bytes).map_err(IntoResponse::into_response)?;
        let mut errors = FieldErrors::new();
        value.validate(&mut errors);
        errors.result().map_err(IntoResponse::into_response)?;
        Ok(Valid(value))
    }
}

fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, FieldErrors> {
    let de = &mut serde_json::Deserializer::from_slice(bytes);
    let fail = |e: serde_path_to_error::Error<serde_json::Error>| {
        let mut path: Vec<String> = e
            .path()
            .iter()
            .filter_map(|s| match s {
                Segment::Seq { index } => Some(index.to_string()),
                Segment::Map { key } => Some(key.clone()),
                Segment::Enum { variant } => Some(variant.clone()),
                Segment::Unknown => None,
            })
            .collect();
        let text = e.inner().to_string();
        // serde_json appends the position, which means nothing to the person filling the form.
        let text = text.rsplit_once(" at line ").map_or(text.as_str(), |(t, _)| t).to_owned();
        let missing = text.strip_prefix("missing field `").and_then(|t| t.strip_suffix('`'));
        let message = match missing {
            Some(name) => {
                path.push(name.to_owned());
                "required".to_owned()
            }
            None => text,
        };
        if path.is_empty() {
            FieldErrors::new().message(message)
        } else {
            FieldErrors::new().field(path.join("."), message)
        }
    };
    let value = serde_path_to_error::deserialize(&mut *de).map_err(fail)?;
    de.end().map_err(|e| FieldErrors::new().message(e.to_string()))?;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(serde::Deserialize, Debug)]
    #[allow(dead_code)]
    struct Signup {
        name: String,
        age: u32,
        address: Address,
    }

    #[derive(serde::Deserialize, Debug)]
    #[allow(dead_code)]
    struct Address {
        city: String,
    }

    #[test]
    fn decode_errors_name_the_field() {
        let e = decode::<Signup>(br#"{"name":"a","age":"x","address":{"city":"c"}}"#).unwrap_err();
        assert_eq!(e.get("age").len(), 1, "{e}");
        let e = decode::<Signup>(br#"{"name":"a","age":1,"address":{}}"#).unwrap_err();
        assert_eq!(e.get("address.city"), ["required"]);
        let e = decode::<Signup>(br#"{"name":"a","#).unwrap_err();
        assert!(e.errors.is_empty() && e.message.is_some());
        assert!(decode::<Signup>(br#"{"name":"a","age":1,"address":{"city":"c"}} x"#).is_err());
    }
}
