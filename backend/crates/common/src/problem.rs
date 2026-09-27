//! The one catalogue of problem types (RFC 7807, PLAN §7.5 "Errors") and the bridge from domain
//! errors to it.
//!
//! This crate stays wire-agnostic: it names every problem a Strata API can answer with (slug,
//! title, HTTP status) and lets domain error types declare which one they map to
//! ([`DomainError`]). The wire representation — the `Problem` body encoded as
//! `application/problem+msgpack`, its typed extension members and the `OpenAPI` schema — lives in
//! `strata-api` (`strata_api::wire::Problem`), which builds problems from this catalogue only.
//! There is no second list anywhere.

use std::borrow::Cow;
use std::fmt;

macro_rules! problem_types {
    ($($(#[$m:meta])* $variant:ident = ($slug:literal, $status:literal, $title:literal);)*) => {
        /// A known problem type. The wire field `type` carries [`ProblemType::slug`]; clients
        /// treat unknown slugs by `status`.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        #[non_exhaustive]
        pub enum ProblemType {
            $($(#[$m])* $variant,)*
        }

        impl ProblemType {
            /// Every known type, in declaration order.
            pub const ALL: &'static [Self] = &[$(Self::$variant),*];

            /// The value of the wire field `type`, e.g. `not_found`.
            pub const fn slug(self) -> &'static str {
                match self { $(Self::$variant => $slug),* }
            }

            /// The HTTP status this type is always sent with.
            pub const fn status(self) -> u16 {
                match self { $(Self::$variant => $status),* }
            }

            /// Short, fixed, human-readable summary (RFC 7807 `title`).
            pub const fn title(self) -> &'static str {
                match self { $(Self::$variant => $title),* }
            }
        }
    };
}

problem_types! {
    /// The body failed decoding or validation; see the `errors` extension.
    InvalidBody = ("invalid_body", 422, "Request body is invalid");
    /// A query or header parameter is invalid.
    InvalidParameter = ("invalid_parameter", 422, "Request parameter is invalid");
    /// A note or file name is not Obsidian-safe (PLAN §6.2).
    InvalidName = ("invalid_name", 422, "Invalid name");
    /// The request body is not `MessagePack` (or is content-encoded).
    UnsupportedMediaType = ("unsupported_media_type", 415, "Unsupported media type");
    /// The body exceeds the route's limit.
    PayloadTooLarge = ("payload_too_large", 413, "Request body too large");
    /// The `Accept` header excludes `MessagePack`.
    NotAcceptable = ("not_acceptable", 406, "Response media type not acceptable");
    /// Nonexistent, or outside the caller's scope (principle 7: never 403 for foreign IDs).
    NotFound = ("not_found", 404, "Not found");
    /// No route matches the path.
    RouteNotFound = ("route_not_found", 404, "No such route");
    /// The route exists but not for this method.
    MethodNotAllowed = ("method_not_allowed", 405, "Method not allowed");
    /// The request is malformed at the HTTP level.
    BadRequest = ("bad_request", 400, "Bad request");
    /// Missing, invalid, expired or revoked access or refresh token.
    Unauthorized = ("unauthorized", 401, "Authentication required");
    /// Login failed: unknown username or wrong password (never says which).
    InvalidCredentials = ("invalid_credentials", 401, "Invalid username or password");
    /// Authenticated but not allowed (e.g. a member on an admin route).
    Forbidden = ("forbidden", 403, "Forbidden");
    /// Signed up, waiting for admin approval (D22).
    AccountPending = ("account_pending", 403, "Account awaiting approval");
    /// The sign-up was rejected by an admin (D22).
    AccountRejected = ("account_rejected", 403, "Account rejected");
    /// The account was disabled by an admin.
    AccountDisabled = ("account_disabled", 403, "Account disabled");
    /// The account is scheduled for deletion; the session is export-only (D25).
    AccountDeletionPending = ("account_deletion_pending", 403, "Account scheduled for deletion");
    /// The password was reset by an admin; only `GET /me`, `PATCH /me` (password change) and
    /// logout work until the user sets a new one.
    PasswordChangeRequired = ("password_change_required", 403, "Password change required");
    /// `If-Match` / base version is stale (PLAN §7.2); `current_version` carries the server's.
    VersionConflict = ("version_conflict", 409, "Version conflict");
    /// A create looks like a duplicate; `candidates` lists the matches (PLAN §9.7).
    DuplicateCandidates = ("duplicate_candidates", 409, "Possible duplicate");
    /// The username (after normalisation and the confusable check) is taken.
    UsernameTaken = ("username_taken", 409, "Username not available");
    /// The account is not in a state that allows this action (e.g. approving an active user).
    AccountStateConflict = ("account_state_conflict", 409, "Account state does not allow this");
    /// The sync epoch changed; the client must re-bootstrap (PLAN §7.5 sync).
    EpochChanged = ("epoch_changed", 410, "Sync epoch changed");
    /// Too many requests (login, signup, capture, ask); see `Retry-After`.
    RateLimited = ("rate_limited", 429, "Too many requests");
    /// Unexpected server error; details are logged, never returned.
    Internal = ("internal", 500, "Internal server error");
}

impl ProblemType {
    /// Looks a slug up.
    pub fn from_slug(slug: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|t| t.slug() == slug)
    }

    /// True for `5xx` types: the cause is logged and never described to the client.
    pub const fn is_server_error(self) -> bool {
        self.status() >= 500
    }
}

impl fmt::Display for ProblemType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.slug())
    }
}

/// A domain error that knows which problem type it is answered with.
///
/// Crates define their own `thiserror` enums and implement this trait; the API layer turns any
/// `DomainError` into a wire problem (`strata_api::wire::Problem::from_domain`), logging the
/// error itself for server errors. [`DomainError::public_detail`] is the only text that
/// reaches the client, so it must never contain user content (PLAN §15).
pub trait DomainError: std::error::Error {
    /// The problem type this error is answered with.
    fn problem_type(&self) -> ProblemType;

    /// Occurrence-specific, content-free explanation for the client (`detail`), if any.
    fn public_detail(&self) -> Option<Cow<'static, str>> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn catalogue_is_exact() {
        let table: Vec<(&str, u16)> = ProblemType::ALL
            .iter()
            .map(|t| (t.slug(), t.status()))
            .collect();
        assert_eq!(
            table,
            vec![
                ("invalid_body", 422),
                ("invalid_parameter", 422),
                ("invalid_name", 422),
                ("unsupported_media_type", 415),
                ("payload_too_large", 413),
                ("not_acceptable", 406),
                ("not_found", 404),
                ("route_not_found", 404),
                ("method_not_allowed", 405),
                ("bad_request", 400),
                ("unauthorized", 401),
                ("invalid_credentials", 401),
                ("forbidden", 403),
                ("account_pending", 403),
                ("account_rejected", 403),
                ("account_disabled", 403),
                ("account_deletion_pending", 403),
                ("password_change_required", 403),
                ("version_conflict", 409),
                ("duplicate_candidates", 409),
                ("username_taken", 409),
                ("account_state_conflict", 409),
                ("epoch_changed", 410),
                ("rate_limited", 429),
                ("internal", 500),
            ]
        );
    }

    #[test]
    fn slugs_round_trip_and_are_unique() {
        let mut seen = std::collections::BTreeSet::new();
        for kind in ProblemType::ALL {
            assert_eq!(ProblemType::from_slug(kind.slug()), Some(*kind));
            assert!(seen.insert(kind.slug()), "duplicate slug {}", kind.slug());
            assert_eq!(kind.to_string(), kind.slug());
            assert!(!kind.title().is_empty());
        }
        assert_eq!(ProblemType::from_slug("nope"), None);
        assert!(ProblemType::Internal.is_server_error());
        assert!(!ProblemType::NotFound.is_server_error());
    }

    #[derive(Debug, thiserror::Error)]
    #[error("stale version")]
    struct Stale;

    impl DomainError for Stale {
        fn problem_type(&self) -> ProblemType {
            ProblemType::VersionConflict
        }
    }

    #[test]
    fn domain_errors_name_their_problem_type() {
        let err: &dyn DomainError = &Stale;
        assert_eq!(err.problem_type(), ProblemType::VersionConflict);
        assert_eq!(err.public_detail(), None);
    }
}
