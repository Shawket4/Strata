//! Endpoint modules, one per area. Each exposes `configure`, called from
//! [`crate::app::routes`], and its handlers are listed in [`crate::openapi::ApiDoc`].

pub mod admin;
pub mod auth;
pub mod devices;
pub mod events;
pub mod me;
pub mod sync;
// The vault store (mounted and documented by `crate::vault`).
pub mod documents;
pub mod entities;
pub mod inbox;
pub mod notes;
pub mod relations;
pub mod search;
pub mod tasks;
pub mod vault_ops;
