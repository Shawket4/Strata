//! The flutter_rust_bridge facade (PLAN §12.1 `api/`): the **only** surface Dart can call.
//!
//! Every function forwards to the headless [`crate::session::Core`]; streams are view-model
//! watchers whose sink is the Dart stream. Nothing here decides anything: the facade exists so
//! the rest of the core stays testable without Flutter.
//!
//! | Module | Surface |
//! |---|---|
//! | [`app`] | init, session state, sign-up/in/out, account switching, lifecycle, sync now |
//! | [`views`] | one `watch_*` stream per screen, plus one-shot reads (search, global map, admin) |
//! | [`intents`] | every user mutation (queued through the outbox) |
//! | [`reminders`] | the notification-ops stream and the adapter's results and actions |

pub mod app;
pub mod intents;
pub mod reminders;
pub mod views;

mod runtime;
mod sink;
