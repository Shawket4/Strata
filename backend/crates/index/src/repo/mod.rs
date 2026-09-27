//! Repositories for user-owned tables. Every function takes `&mut ScopedTx`; inserts write
//! `user_id = strata_current_user()` so a row always belongs to the scoped user, and row-level
//! security filters every read and write.

pub mod dedupe;
pub mod devices;
pub mod entities;
pub mod graph;
pub mod jobs;
pub mod notes;
pub mod settings;
pub mod suggestions;
pub mod sync;
pub mod tasks;
pub mod vault;
