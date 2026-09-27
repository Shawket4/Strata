//! Raw bytes on the wire (MessagePack `bin`).

use std::borrow::Cow;
use std::fmt;

use serde::{Deserialize, Serialize};
use utoipa::openapi::schema::{KnownFormat, ObjectBuilder, SchemaFormat, Type};
use utoipa::openapi::{RefOr, Schema};
use utoipa::{PartialSchema, ToSchema};

/// Raw bytes, encoded as MessagePack `bin` (never as an array of integers).
///
/// Documented as the named component `Binary` (`type: string, format: binary`); the client
/// generator maps it to `serde_bytes::ByteBuf`. `Debug` prints only the length so payloads never
/// reach logs.
#[derive(Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Binary(#[serde(with = "serde_bytes")] pub Vec<u8>);

impl fmt::Debug for Binary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Binary({} bytes)", self.0.len())
    }
}

impl From<Vec<u8>> for Binary {
    fn from(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }
}

impl PartialSchema for Binary {
    fn schema() -> RefOr<Schema> {
        ObjectBuilder::new()
            .schema_type(Type::String)
            .format(Some(SchemaFormat::KnownFormat(KnownFormat::Binary)))
            .description(Some("Raw bytes, carried as MessagePack `bin`."))
            .build()
            .into()
    }
}

impl ToSchema for Binary {
    fn name() -> Cow<'static, str> {
        Cow::Borrowed("Binary")
    }
}
