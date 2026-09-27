//! Vault operations. Writes are `Core` methods run on the user's actor (see
//! [`crate::store`]); [`service`] exposes them on [`crate::VaultService`]. Reads are in
//! [`read`].

pub mod ai;
pub mod ai_apply;
pub mod ai_decide;
pub mod entities;
pub mod files;
pub mod notes;
pub mod read;
pub mod relations;
pub mod service;
pub mod suggestions;
pub mod tasks;
