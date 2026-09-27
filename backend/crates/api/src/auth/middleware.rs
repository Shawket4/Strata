//! The auth middleware and the [`Authenticated`] extractor (PLAN §5.2, §8, D6, D25).
//!
//! [`authenticate`] wraps the whole `/api/v1` scope. For a request with
//! `Authorization: Bearer <token>` it:
//!
//! 1. verifies the access token (signature, key, algorithm, issuer, audience, expiry);
//! 2. checks the [`RevocationSet`](super::revocation::RevocationSet): a revoked session, a
//!    disabled or purged user → `401 unauthorized`, in the very next request;
//! 3. applies account restrictions: an export-only session (account `deletion_pending`) may
//!    only call [`EXPORT_ONLY_ROUTES`], anything else is `403 account_deletion_pending`; an
//!    account on a temporary password may only call [`PASSWORD_CHANGE_ROUTES`], anything
//!    else is `403 password_change_required`;
//! 4. resolves the [`UserScope`] once through the [`ScopeIssuer`] and stores the
//!    [`AuthContext`] in the request extensions.
//!
//! Requests without a bearer token pass through; handlers that need a user take
//! [`Authenticated`], which answers `401` when there is none. The issuer is private to this
//! module, so a handler can only get a scope for the caller the middleware authenticated.

use std::future::{Ready, ready};

use actix_web::body::{EitherBody, MessageBody};
use actix_web::dev::{Payload, ServiceRequest, ServiceResponse};
use actix_web::http::Method;
use actix_web::http::header::AUTHORIZATION;
use actix_web::middleware::Next;
use actix_web::{Error, FromRequest, HttpMessage, HttpRequest, ResponseError, web};
use strata_common::{DeviceId, SessionId, UserId};
use strata_index::{ScopeIssuer, UserScope};

use crate::app::API_PREFIX;
use crate::auth::AuthState;
use crate::auth::error::AccountError;
use crate::auth::revocation::Revoked;
use crate::auth::tokens::{TokenError, TokenRole, TokenStatus};

/// `(method, path below /api/v1)` an export-only session may call (PLAN §5.2, D25).
pub const EXPORT_ONLY_ROUTES: &[(&str, &str)] = &[
    ("GET", "/me"),
    ("GET", "/me/export"),
    ("POST", "/me/confirm-deletion"),
    ("POST", "/auth/logout"),
];

/// `(method, path below /api/v1)` an account on a temporary password may call.
pub const PASSWORD_CHANGE_ROUTES: &[(&str, &str)] =
    &[("GET", "/me"), ("PATCH", "/me"), ("POST", "/auth/logout")];

/// Holds the capability to mint scopes; only this module can use it.
#[derive(Debug, Clone)]
pub struct Authenticator {
    issuer: ScopeIssuer,
}

impl Authenticator {
    /// Wraps the issuer from `AppDb::new`.
    pub fn new(issuer: ScopeIssuer) -> Self {
        Self { issuer }
    }
}

/// The authenticated caller of a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthContext {
    scope: UserScope,
    /// The device session.
    pub session: SessionId,
    /// The device.
    pub device: DeviceId,
    /// Role at token issue (admin routes re-check the database).
    pub role: TokenRole,
    /// Export-only session (account `deletion_pending`).
    pub export_only: bool,
    /// The account is on a temporary password.
    pub must_change_password: bool,
}

impl AuthContext {
    /// The caller's scope: the only way to open a transaction on their data.
    pub fn scope(&self) -> &UserScope {
        &self.scope
    }

    /// The caller.
    pub fn user_id(&self) -> UserId {
        self.scope.user_id()
    }
}

/// Extractor: the authenticated caller, or `401 unauthorized`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Authenticated(pub AuthContext);

impl std::ops::Deref for Authenticated {
    type Target = AuthContext;

    fn deref(&self) -> &AuthContext {
        &self.0
    }
}

impl FromRequest for Authenticated {
    type Error = AccountError;
    type Future = Ready<Result<Self, AccountError>>;

    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        ready(
            req.extensions()
                .get::<AuthContext>()
                .copied()
                .map(Self)
                .ok_or(AccountError::Unauthorized(
                    "a bearer access token is required",
                )),
        )
    }
}

fn bearer(req: &ServiceRequest) -> Option<Result<&str, AccountError>> {
    let value = req.headers().get(AUTHORIZATION)?;
    let parsed = value
        .to_str()
        .ok()
        .and_then(|v| {
            v.split_once(' ')
                .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("bearer"))
        })
        .map(|(_, token)| token.trim())
        .filter(|t| !t.is_empty())
        .ok_or(AccountError::Unauthorized("malformed Authorization header"));
    Some(parsed)
}

fn route_allowed(method: &Method, path: &str, allowed: &[(&str, &str)]) -> bool {
    let relative = path.strip_prefix(API_PREFIX).unwrap_or(path);
    let relative = relative.strip_suffix('/').unwrap_or(relative);
    allowed
        .iter()
        .any(|(m, p)| method.as_str() == *m && relative == *p)
}

/// Resolves the caller of `req` (see the module docs).
pub fn resolve(
    state: &AuthState,
    req: &ServiceRequest,
) -> Result<Option<AuthContext>, AccountError> {
    let Some(token) = bearer(req) else {
        return Ok(None);
    };
    let claims = state.tokens.verify(token?).map_err(|e| match e {
        TokenError::Expired => AccountError::Unauthorized("access token expired"),
        _ => AccountError::Unauthorized("invalid access token"),
    })?;
    let flags = state
        .revocations
        .check(claims.sid, claims.sub)
        .map_err(|r| match r {
            Revoked::Session => AccountError::Unauthorized("session revoked"),
            Revoked::UserDisabled => AccountError::Unauthorized("account disabled"),
            Revoked::UserPurged => AccountError::Unauthorized("account deleted"),
        })?;
    let export_only = claims.st == TokenStatus::DeletionPending || flags.deletion_pending;
    if export_only && !route_allowed(req.method(), req.path(), EXPORT_ONLY_ROUTES) {
        return Err(AccountError::DeletionPending);
    }
    if flags.must_change_password
        && !route_allowed(req.method(), req.path(), PASSWORD_CHANGE_ROUTES)
    {
        return Err(AccountError::PasswordChangeRequired);
    }
    Ok(Some(AuthContext {
        scope: state.authenticator.issuer.issue(claims.sub),
        session: claims.sid,
        device: claims.did,
        role: claims.role,
        export_only,
        must_change_password: flags.must_change_password,
    }))
}

/// Middleware (via `actix_web::middleware::from_fn`) authenticating bearer tokens. Without
/// [`AuthState`] in the app data (e.g. the wire demo) it passes every request through.
pub async fn authenticate(
    req: ServiceRequest,
    next: Next<impl MessageBody + 'static>,
) -> Result<ServiceResponse<EitherBody<impl MessageBody>>, Error> {
    let Some(state) = req.app_data::<web::Data<AuthState>>().cloned() else {
        return next
            .call(req)
            .await
            .map(ServiceResponse::map_into_left_body);
    };
    match resolve(&state, &req) {
        Ok(Some(ctx)) => {
            req.extensions_mut().insert(ctx);
            next.call(req)
                .await
                .map(ServiceResponse::map_into_left_body)
        }
        Ok(None) => next
            .call(req)
            .await
            .map(ServiceResponse::map_into_left_body),
        Err(err) => Ok(req
            .into_response(err.error_response())
            .map_into_right_body()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allow_lists_match_exact_method_and_path() {
        let allowed = |m: Method, p: &str| route_allowed(&m, p, EXPORT_ONLY_ROUTES);
        assert!(allowed(Method::GET, "/api/v1/me"));
        assert!(allowed(Method::GET, "/api/v1/me/export"));
        assert!(allowed(Method::POST, "/api/v1/me/confirm-deletion"));
        assert!(allowed(Method::POST, "/api/v1/auth/logout"));
        assert!(allowed(Method::GET, "/api/v1/me/"));
        assert!(!allowed(Method::PATCH, "/api/v1/me"));
        assert!(!allowed(Method::GET, "/api/v1/devices"));
        assert!(!allowed(Method::GET, "/api/v1/me/export/x"));
        assert!(!allowed(Method::GET, "/api/v1/mex"));
        let pw = |m: Method, p: &str| route_allowed(&m, p, PASSWORD_CHANGE_ROUTES);
        assert!(pw(Method::PATCH, "/api/v1/me"));
        assert!(!pw(Method::GET, "/api/v1/me/export"));
    }
}
