//! Domain errors of the account and auth endpoints, mapped onto the problem catalogue.

use std::borrow::Cow;

use actix_web::http::StatusCode;
use actix_web::http::header::RETRY_AFTER;
use actix_web::{HttpResponse, ResponseError};
use strata_common::{DomainError, ProblemType};

use crate::auth::password::HashError;
use crate::auth::tokens::TokenError;
use crate::auth::username::NameError;
use crate::wire::{Problem, ProblemFieldError, status_code};

/// Everything an account or auth operation can fail with.
#[derive(Debug, thiserror::Error)]
pub enum AccountError {
    /// Unknown username or wrong password (never says which).
    #[error("invalid credentials")]
    InvalidCredentials,
    /// Missing, invalid, expired or revoked token.
    #[error("unauthorized: {0}")]
    Unauthorized(&'static str),
    /// The caller may not do this (e.g. a member on an admin route).
    #[error("forbidden")]
    Forbidden,
    /// Awaiting approval.
    #[error("account pending")]
    AccountPending,
    /// Sign-up rejected.
    #[error("account rejected")]
    AccountRejected,
    /// Disabled by an admin.
    #[error("account disabled")]
    AccountDisabled,
    /// Export-only session (D25).
    #[error("account deletion pending")]
    DeletionPending,
    /// Temporary password must be changed first.
    #[error("password change required")]
    PasswordChangeRequired,
    /// Nothing with this ID in the caller's scope.
    #[error("not found")]
    NotFound,
    /// The username's skeleton is taken.
    #[error("username taken")]
    UsernameTaken,
    /// The account's state does not allow the action.
    #[error("account state conflict: {0}")]
    StateConflict(&'static str),
    /// A body field failed validation.
    #[error("invalid field {pointer}: {message}")]
    InvalidField {
        /// JSON Pointer of the field.
        pointer: &'static str,
        /// Stable code.
        code: &'static str,
        /// Content-free message.
        message: String,
    },
    /// Too many attempts.
    #[error("rate limited")]
    RateLimited {
        /// Seconds until a retry can succeed.
        retry_after_secs: u64,
        /// Content-free reason.
        reason: &'static str,
    },
    /// Database failure.
    #[error("database: {0}")]
    Db(#[from] strata_index::IndexError),
    /// Password hashing failure.
    #[error("{0}")]
    Hash(#[from] HashError),
    /// Token signing failure (verification failures are `Unauthorized`).
    #[error("{0}")]
    Token(#[from] TokenError),
    /// Filesystem failure (vault provisioning, export, purge).
    #[error("filesystem: {0}")]
    Io(#[from] std::io::Error),
    /// Any other internal failure.
    #[error("{0}")]
    Internal(String),
}

impl From<sqlx::Error> for AccountError {
    fn from(e: sqlx::Error) -> Self {
        Self::Db(e.into())
    }
}

impl AccountError {
    /// `422 invalid_body` for a name that failed validation.
    pub fn name(pointer: &'static str, err: NameError) -> Self {
        let code = match err {
            NameError::DisplayName => "invalid_display_name",
            _ => "invalid_username",
        };
        Self::InvalidField {
            pointer,
            code,
            message: err.to_string(),
        }
    }
}

impl DomainError for AccountError {
    fn problem_type(&self) -> ProblemType {
        match self {
            Self::InvalidCredentials => ProblemType::InvalidCredentials,
            Self::Unauthorized(_) => ProblemType::Unauthorized,
            Self::Forbidden => ProblemType::Forbidden,
            Self::AccountPending => ProblemType::AccountPending,
            Self::AccountRejected => ProblemType::AccountRejected,
            Self::AccountDisabled => ProblemType::AccountDisabled,
            Self::DeletionPending => ProblemType::AccountDeletionPending,
            Self::PasswordChangeRequired => ProblemType::PasswordChangeRequired,
            Self::NotFound => ProblemType::NotFound,
            Self::UsernameTaken => ProblemType::UsernameTaken,
            Self::StateConflict(_) => ProblemType::AccountStateConflict,
            Self::InvalidField { .. } => ProblemType::InvalidBody,
            Self::RateLimited { .. } => ProblemType::RateLimited,
            Self::Db(_) | Self::Hash(_) | Self::Token(_) | Self::Io(_) | Self::Internal(_) => {
                ProblemType::Internal
            }
        }
    }

    fn public_detail(&self) -> Option<Cow<'static, str>> {
        match self {
            Self::Unauthorized(why) | Self::StateConflict(why) => Some(Cow::Borrowed(why)),
            Self::RateLimited { reason, .. } => Some(Cow::Borrowed(reason)),
            Self::InvalidField { message, .. } => Some(Cow::Owned(message.clone())),
            Self::DeletionPending => Some(Cow::Borrowed(
                "this session can only export the vault, confirm the deletion or sign out",
            )),
            Self::PasswordChangeRequired => Some(Cow::Borrowed(
                "set a new password with PATCH /me before using the API",
            )),
            _ => None,
        }
    }
}

impl From<AccountError> for Problem {
    fn from(err: AccountError) -> Self {
        let problem = Problem::from_domain(&err);
        match err {
            AccountError::InvalidField {
                pointer,
                code,
                message,
            } => problem.with_error(ProblemFieldError {
                code: code.to_owned(),
                pointer: Some(pointer.to_owned()),
                message,
            }),
            _ => problem,
        }
    }
}

impl ResponseError for AccountError {
    fn status_code(&self) -> StatusCode {
        status_code(self.problem_type())
    }

    fn error_response(&self) -> HttpResponse {
        let retry_after = match self {
            Self::RateLimited {
                retry_after_secs, ..
            } => Some(*retry_after_secs),
            _ => None,
        };
        // Rebuild the problem from a borrowed view (the error itself is not Clone).
        let problem = match self {
            Self::InvalidField {
                pointer,
                code,
                message,
            } => Problem::from_domain(self).with_error(ProblemFieldError {
                code: (*code).to_owned(),
                pointer: Some((*pointer).to_owned()),
                message: message.clone(),
            }),
            _ => Problem::from_domain(self),
        };
        let mut response = problem.error_response();
        if let Some(secs) = retry_after
            && let Ok(value) = secs.to_string().parse()
        {
            response.headers_mut().insert(RETRY_AFTER, value);
        }
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_map_to_exact_problems() {
        assert_eq!(
            Problem::from(AccountError::AccountPending),
            Problem::new(ProblemType::AccountPending)
        );
        assert_eq!(
            Problem::from(AccountError::StateConflict("user is not pending")),
            Problem::new(ProblemType::AccountStateConflict).with_detail("user is not pending")
        );
        assert_eq!(
            Problem::from(AccountError::name("/username", NameError::UsernameLength)),
            Problem::new(ProblemType::InvalidBody)
                .with_detail("usernames have 3 to 32 characters")
                .with_error(ProblemFieldError {
                    code: "invalid_username".into(),
                    pointer: Some("/username".into()),
                    message: "usernames have 3 to 32 characters".into(),
                })
        );
        assert_eq!(
            Problem::from(AccountError::Internal("secret cause".into())),
            Problem::new(ProblemType::Internal)
        );
    }

    #[test]
    fn rate_limited_responses_carry_retry_after() {
        let err = AccountError::RateLimited {
            retry_after_secs: 42,
            reason: "too many login attempts",
        };
        let response = err.error_response();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(
            response
                .headers()
                .get(RETRY_AFTER)
                .and_then(|v| v.to_str().ok()),
            Some("42")
        );
    }
}
