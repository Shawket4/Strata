//! Endpoint modules, one per area. Each exposes `configure`, called from
//! [`crate::app::routes`], and its handlers are listed in [`crate::openapi::ApiDoc`].

pub mod admin;
pub mod auth;
pub mod devices;
pub mod me;
