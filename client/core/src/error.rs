//! Typed, localisation-ready errors (PLAN §12.1: "errors reach Dart as typed enums from the
//! core, already localised-ready (message keys)").

/// Everything a core call can fail with. Each variant has a stable [`CoreError::message_key`]
/// the UI maps to a localised string (`strata_l10n`); variants carry only IDs and codes, never
/// user content.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CoreError {
    /// `init_core` has not run.
    #[error("the core is not initialised")]
    NotInitialised,
    /// The call needs a signed-in account.
    #[error("no account is signed in")]
    NotSignedIn,
    /// The call needs the server and the device is offline.
    #[error("offline")]
    Offline,
    /// The server has no endpoint for this yet (or the feature is not built yet).
    #[error("{feature} is not available yet")]
    NotAvailable {
        /// Stable feature name, e.g. `ask`.
        feature: String,
    },
    /// An argument is invalid.
    #[error("invalid {field}: {reason}")]
    InvalidInput {
        /// Which argument.
        field: String,
        /// Stable reason code, e.g. `forbidden_char`.
        reason: String,
    },
    /// The referenced item does not exist locally.
    #[error("{what} not found")]
    NotFound {
        /// What was looked up, e.g. `note`.
        what: String,
    },
    /// Wrong username or password.
    #[error("invalid credentials")]
    InvalidCredentials,
    /// The account awaits admin approval (D22).
    #[error("account pending approval")]
    AccountPending,
    /// The account's sign-up was rejected.
    #[error("account rejected")]
    AccountRejected,
    /// The account was disabled by an admin.
    #[error("account disabled")]
    AccountDisabled,
    /// The account is scheduled for deletion (export-only, D25).
    #[error("account deletion pending")]
    AccountDeletionPending,
    /// The session ended and could not be refreshed; sign in again.
    #[error("session expired")]
    SessionExpired,
    /// Too many requests.
    #[error("rate limited")]
    RateLimited,
    /// Sign-out was asked while ops have not synced; confirm with `force`.
    #[error("{count} changes have not synced")]
    PendingChanges {
        /// Number of unsynced outbox ops.
        count: u32,
    },
    /// A task edit that the task line does not allow (e.g. completing a done task).
    #[error("task change not possible: {reason}")]
    TaskChange {
        /// Stable reason code from `vault-format` (`not_open`, `not_recurring`, …).
        reason: String,
    },
    /// The server answered with a problem this client does not map.
    #[error("server error {status} {problem_type}")]
    Server {
        /// HTTP status.
        status: u16,
        /// Problem type slug.
        problem_type: String,
    },
    /// The local database failed.
    #[error("storage error: {0}")]
    Storage(String),
    /// An invariant broke (a bug).
    #[error("internal error: {0}")]
    Internal(String),
}

impl CoreError {
    /// The stable key the UI localises (`error.<snake_case>`).
    pub fn message_key(&self) -> String {
        let key = match self {
            Self::NotInitialised => "not_initialised",
            Self::NotSignedIn => "not_signed_in",
            Self::Offline => "offline",
            Self::NotAvailable { .. } => "not_available",
            Self::InvalidInput { .. } => "invalid_input",
            Self::NotFound { .. } => "not_found",
            Self::InvalidCredentials => "invalid_credentials",
            Self::AccountPending => "account_pending",
            Self::AccountRejected => "account_rejected",
            Self::AccountDisabled => "account_disabled",
            Self::AccountDeletionPending => "account_deletion_pending",
            Self::SessionExpired => "session_expired",
            Self::RateLimited => "rate_limited",
            Self::PendingChanges { .. } => "pending_changes",
            Self::TaskChange { .. } => "task_change",
            Self::Server { .. } => "server",
            Self::Storage(_) => "storage",
            Self::Internal(_) => "internal",
        };
        format!("error.{key}")
    }

    /// Shorthand for [`CoreError::InvalidInput`].
    pub(crate) fn invalid(field: &str, reason: &str) -> Self {
        Self::InvalidInput {
            field: field.to_owned(),
            reason: reason.to_owned(),
        }
    }

    /// Shorthand for [`CoreError::NotFound`].
    pub(crate) fn not_found(what: &str) -> Self {
        Self::NotFound {
            what: what.to_owned(),
        }
    }
}

impl From<rusqlite::Error> for CoreError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Storage(e.to_string())
    }
}

impl From<rmp_serde::encode::Error> for CoreError {
    fn from(e: rmp_serde::encode::Error) -> Self {
        Self::Internal(format!("encode: {e}"))
    }
}

impl From<rmp_serde::decode::Error> for CoreError {
    fn from(e: rmp_serde::decode::Error) -> Self {
        Self::Storage(format!("decode: {e}"))
    }
}

impl From<std::io::Error> for CoreError {
    fn from(e: std::io::Error) -> Self {
        Self::Storage(format!("io: {e}"))
    }
}

/// Result alias of the core.
pub type CoreResult<T> = Result<T, CoreError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_keys_are_stable() {
        let cases = [
            (CoreError::Offline, "error.offline"),
            (CoreError::PendingChanges { count: 3 }, "error.pending_changes"),
            (
                CoreError::NotAvailable {
                    feature: "ask".into(),
                },
                "error.not_available",
            ),
            (CoreError::AccountDeletionPending, "error.account_deletion_pending"),
            (CoreError::invalid("path", "empty"), "error.invalid_input"),
            (CoreError::not_found("note"), "error.not_found"),
        ];
        for (e, key) in cases {
            assert_eq!(e.message_key(), key);
        }
        assert_eq!(
            CoreError::invalid("path", "empty").to_string(),
            "invalid path: empty"
        );
    }
}
