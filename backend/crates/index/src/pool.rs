//! Connection pools for the three roles.

use std::str::FromStr;

use sqlx::PgPool;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};

use crate::bootstrap::SEARCH_PATH;
use crate::error::Result;

/// Parses `url` and pins `search_path` to [`SEARCH_PATH`] (so pools work even where the
/// per-database role setting from bootstrap is missing).
pub fn connect_options(url: &str) -> Result<PgConnectOptions> {
    Ok(PgConnectOptions::from_str(url)?.options([("search_path", SEARCH_PATH)]))
}

/// Opens a pool of at most `max_connections` connections.
pub async fn connect(options: PgConnectOptions, max_connections: u32) -> Result<PgPool> {
    Ok(PgPoolOptions::new()
        .max_connections(max_connections)
        .connect_with(options)
        .await?)
}
