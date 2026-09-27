//! Bearer tokens.

use std::fmt;

use futures_util::future::BoxFuture;

use crate::Error;

/// Supplies access tokens for authenticated operations.
///
/// The client core (L15) implements this over its token store (D14): `access_token` returns
/// the current token; `refresh` is called once when the server answers `401` and returns the
/// new token (or `None` when the session cannot be refreshed, which surfaces the `401`).
pub trait TokenProvider: Send + Sync + fmt::Debug {
    /// The current access token, if signed in.
    fn access_token(&self) -> BoxFuture<'_, Result<Option<String>, Error>>;
    /// Obtains a new access token after a `401`.
    fn refresh(&self) -> BoxFuture<'_, Result<Option<String>, Error>>;
}

/// A fixed token that cannot be refreshed.
#[derive(Clone)]
pub struct StaticToken(pub String);

impl fmt::Debug for StaticToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("StaticToken(<redacted>)")
    }
}

impl TokenProvider for StaticToken {
    fn access_token(&self) -> BoxFuture<'_, Result<Option<String>, Error>> {
        Box::pin(async move { Ok(Some(self.0.clone())) })
    }

    fn refresh(&self) -> BoxFuture<'_, Result<Option<String>, Error>> {
        Box::pin(async { Ok(None) })
    }
}
