//! Sync (PLAN §12.1 `sync/`, §12.4): the op and record model ([`model`], from the shared
//! `sync-model` crate), the database side ([`apply`], incl. the D19 merge preview with
//! `sync_model::merge`) and the network engine ([`engine`]).

pub mod apply;
pub mod engine;
pub mod model;
