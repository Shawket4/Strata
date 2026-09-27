//! Sync (PLAN §12.1 `sync/`, §12.4): the op and record model ([`model`], a seam to the shared
//! `sync-model` crate), the database side ([`apply`]), the network engine ([`engine`]) and the
//! merge-preview seam ([`merge`]).

pub mod apply;
pub mod engine;
pub mod merge;
pub mod model;
