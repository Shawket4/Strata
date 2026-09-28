//! `GET /api/v1/health` (and `HEAD`): unauthenticated liveness. Reveals nothing but "the process answers".

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::wire::MsgPack;

/// Liveness state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    /// The process is up and serving requests.
    Ok,
}

/// Liveness response.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct Health {
    /// Always `ok` when the server answers.
    pub status: HealthStatus,
}

/// Liveness probe.
///
/// `HEAD` answers with the same status and headers and no body.
#[utoipa::path(
    get,
    path = "/health",
    tag = "system",
    operation_id = "health",
    security(()),
    responses((status = 200, description = "The server is alive.", body = Health)),
)]
pub async fn health() -> MsgPack<Health> {
    MsgPack(Health {
        status: HealthStatus::Ok,
    })
}
