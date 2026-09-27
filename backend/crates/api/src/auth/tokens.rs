//! Access and refresh tokens (PLAN §8, D6 = b).
//!
//! - **Access tokens** are JWS compact tokens signed with Ed25519 (`alg: EdDSA`, RFC 8037) by
//!   `jsonwebtoken` (RustCrypto backend). They live 15 minutes by default and carry the user,
//!   device, session, role, account status, `iat`/`exp` and a claims-format version. The header
//!   names the signing key (`kid` = first 16 hex digits of SHA-256 of the public key).
//!   Expiry is checked against the injected [`Clock`], never the system time, so the fake
//!   clock drives it in tests.
//! - **Refresh tokens** are 256-bit random strings (base64url, no padding). Only their
//!   SHA-256 is stored (`refresh_tokens.token_hash`); each is single-use (rotation) and a
//!   replay revokes the whole device session (`AccountsDb::use_refresh_token`).
//! - **The key file** is an Ed25519 private key in PKCS#8 PEM (RFC 8410, what
//!   `openssl genpkey -algorithm ed25519` writes), created by `stratad keygen`
//!   ([`generate_key_pem`]).

use std::fmt;
use std::sync::Arc;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::{DateTime, Duration, Utc};
use ed25519_dalek::SigningKey;
use ed25519_dalek::pkcs8::{DecodePrivateKey, EncodePrivateKey};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use strata_common::{Clock, DeviceId, SessionId, UserId};
use strata_index::types::{UserRole, UserStatus};

/// Version of the claims layout; tokens with another version are rejected.
pub const CLAIMS_VERSION: u32 = 1;

/// Tolerated clock skew for `iat` in the future (seconds).
const IAT_SKEW_SECS: i64 = 60;

/// Errors loading a key or verifying a token. Messages never contain token or key material.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TokenError {
    /// The key file is not an Ed25519 PKCS#8 PEM private key.
    #[error("invalid signing key: {0}")]
    InvalidKey(String),
    /// Randomness was unavailable.
    #[error("the operating system RNG failed: {0}")]
    Rng(String),
    /// Malformed token, bad signature, wrong key, algorithm, issuer or audience.
    #[error("invalid access token: {0}")]
    Invalid(&'static str),
    /// The token has expired.
    #[error("access token expired")]
    Expired,
    /// Signing failed.
    #[error("cannot sign token: {0}")]
    Sign(String),
}

/// Account status as carried in access tokens: only these two can hold a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenStatus {
    /// Full access.
    Active,
    /// Export-only session of an account scheduled for deletion (D25).
    DeletionPending,
}

impl TokenStatus {
    /// The token status for a user status, if that status may hold a session.
    pub fn from_user_status(status: UserStatus) -> Option<Self> {
        match status {
            UserStatus::Active => Some(Self::Active),
            UserStatus::DeletionPending => Some(Self::DeletionPending),
            _ => None,
        }
    }
}

/// Role as carried in access tokens (informational: admin routes re-check the database).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenRole {
    /// Manages accounts.
    Admin,
    /// Regular user.
    Member,
}

impl From<UserRole> for TokenRole {
    fn from(role: UserRole) -> Self {
        match role {
            UserRole::Admin => Self::Admin,
            UserRole::Member => Self::Member,
        }
    }
}

/// The claims of an access token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessClaims {
    /// Issuer (`auth.issuer`).
    pub iss: String,
    /// Audience (`auth.audience`).
    pub aud: String,
    /// The user (ULID).
    pub sub: UserId,
    /// The device session.
    pub sid: SessionId,
    /// The device.
    pub did: DeviceId,
    /// Role at issue time.
    pub role: TokenRole,
    /// Account status at issue time.
    pub st: TokenStatus,
    /// Issued at (Unix seconds).
    pub iat: i64,
    /// Expires at (Unix seconds).
    pub exp: i64,
    /// Claims layout version ([`CLAIMS_VERSION`]).
    pub ver: u32,
}

/// What an access token is issued for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenSubject {
    /// The user.
    pub user: UserId,
    /// The session.
    pub session: SessionId,
    /// The device.
    pub device: DeviceId,
    /// The user's role.
    pub role: TokenRole,
    /// The account status.
    pub status: TokenStatus,
}

/// An issued access token.
#[derive(Clone, PartialEq, Eq)]
pub struct IssuedToken {
    /// The compact JWS.
    pub token: String,
    /// Expiry.
    pub expires_at: DateTime<Utc>,
}

impl fmt::Debug for IssuedToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IssuedToken")
            .field("token", &"<redacted>")
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

/// The Ed25519 key pair used for access tokens.
#[derive(Clone)]
pub struct SigningKeys {
    encoding: EncodingKey,
    decoding: DecodingKey,
    kid: String,
}

impl fmt::Debug for SigningKeys {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SigningKeys")
            .field("kid", &self.kid)
            .finish_non_exhaustive()
    }
}

impl SigningKeys {
    /// Loads a PKCS#8 PEM Ed25519 private key.
    pub fn from_pem(pem: &str) -> Result<Self, TokenError> {
        let signing = SigningKey::from_pkcs8_pem(pem)
            .map_err(|_| TokenError::InvalidKey("not an Ed25519 PKCS#8 PEM private key".into()))?;
        let der = signing
            .to_pkcs8_der()
            .map_err(|e| TokenError::InvalidKey(e.to_string()))?;
        let public = signing.verifying_key().to_bytes();
        Ok(Self {
            encoding: EncodingKey::from_ed_der(der.as_bytes()),
            decoding: DecodingKey::from_ed_der(&public),
            kid: key_id(&public),
        })
    }

    /// The key ID written to token headers.
    pub fn kid(&self) -> &str {
        &self.kid
    }
}

fn key_id(public: &[u8; 32]) -> String {
    hex_prefix(&Sha256::digest(public), 8)
}

fn hex_prefix(bytes: &[u8], n: usize) -> String {
    use std::fmt::Write as _;
    bytes.iter().take(n).fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// Generates a new Ed25519 private key as PKCS#8 PEM (`stratad keygen`).
pub fn generate_key_pem() -> Result<String, TokenError> {
    let mut seed = [0u8; 32];
    getrandom::fill(&mut seed).map_err(|e| TokenError::Rng(e.to_string()))?;
    let key = SigningKey::from_bytes(&seed);
    let pem = key
        .to_pkcs8_pem(ed25519_dalek::pkcs8::spki::der::pem::LineEnding::LF)
        .map_err(|e| TokenError::InvalidKey(e.to_string()))?;
    Ok(pem.as_str().to_owned())
}

/// Issues and verifies access tokens.
#[derive(Clone)]
pub struct TokenService {
    keys: SigningKeys,
    issuer: String,
    audience: String,
    ttl: Duration,
    clock: Arc<dyn Clock>,
}

impl fmt::Debug for TokenService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TokenService")
            .field("kid", &self.keys.kid)
            .field("issuer", &self.issuer)
            .field("audience", &self.audience)
            .field("ttl", &self.ttl)
            .finish_non_exhaustive()
    }
}

impl TokenService {
    /// A service signing with `keys` for `issuer`/`audience`, tokens living `ttl`.
    pub fn new(
        keys: SigningKeys,
        issuer: impl Into<String>,
        audience: impl Into<String>,
        ttl: Duration,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            keys,
            issuer: issuer.into(),
            audience: audience.into(),
            ttl,
            clock,
        }
    }

    /// Access-token lifetime.
    pub fn ttl(&self) -> Duration {
        self.ttl
    }

    /// Issues an access token for `subject`, valid from now for the configured lifetime.
    pub fn issue(&self, subject: &TokenSubject) -> Result<IssuedToken, TokenError> {
        let now = self.clock.now();
        let expires_at = now + self.ttl;
        let claims = AccessClaims {
            iss: self.issuer.clone(),
            aud: self.audience.clone(),
            sub: subject.user,
            sid: subject.session,
            did: subject.device,
            role: subject.role,
            st: subject.status,
            iat: now.timestamp(),
            exp: expires_at.timestamp(),
            ver: CLAIMS_VERSION,
        };
        let mut header = Header::new(Algorithm::EdDSA);
        header.kid = Some(self.keys.kid.clone());
        let token = jsonwebtoken::encode(&header, &claims, &self.keys.encoding)
            .map_err(|e| TokenError::Sign(e.to_string()))?;
        Ok(IssuedToken {
            token,
            expires_at: DateTime::from_timestamp(claims.exp, 0).unwrap_or(expires_at),
        })
    }

    /// Verifies signature, algorithm, key ID, issuer, audience, claims version and expiry.
    pub fn verify(&self, token: &str) -> Result<AccessClaims, TokenError> {
        let header =
            jsonwebtoken::decode_header(token).map_err(|_| TokenError::Invalid("malformed"))?;
        if header.alg != Algorithm::EdDSA {
            return Err(TokenError::Invalid("algorithm"));
        }
        if header.kid.as_deref() != Some(self.keys.kid.as_str()) {
            return Err(TokenError::Invalid("unknown key"));
        }
        let mut validation = Validation::new(Algorithm::EdDSA);
        // Time is checked below against the injected clock.
        validation.validate_exp = false;
        validation.validate_nbf = false;
        validation.set_issuer(&[&self.issuer]);
        validation.set_audience(&[&self.audience]);
        validation.set_required_spec_claims(&["exp", "sub", "iss", "aud"]);
        let data = jsonwebtoken::decode::<AccessClaims>(token, &self.keys.decoding, &validation)
            .map_err(|e| {
                use jsonwebtoken::errors::ErrorKind;
                TokenError::Invalid(match e.kind() {
                    ErrorKind::InvalidSignature => "signature",
                    ErrorKind::InvalidAudience => "audience",
                    ErrorKind::InvalidIssuer => "issuer",
                    ErrorKind::InvalidAlgorithm => "algorithm",
                    _ => "malformed",
                })
            })?;
        let claims = data.claims;
        if claims.ver != CLAIMS_VERSION {
            return Err(TokenError::Invalid("claims version"));
        }
        let now = self.clock.now().timestamp();
        if claims.iat > now + IAT_SKEW_SECS || claims.exp <= claims.iat {
            return Err(TokenError::Invalid("issued-at"));
        }
        if claims.exp <= now {
            return Err(TokenError::Expired);
        }
        Ok(claims)
    }
}

/// A new refresh token (256 random bits, base64url without padding).
pub fn new_refresh_token() -> Result<String, TokenError> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| TokenError::Rng(e.to_string()))?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

/// SHA-256 of a refresh token, as stored in `refresh_tokens.token_hash`.
pub fn hash_refresh_token(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use strata_common::FakeClock;

    fn ids() -> TokenSubject {
        TokenSubject {
            user: "01M3HBS0G00000000000000001".parse().expect("ulid"),
            session: "01M3HBS0G00000000000000002".parse().expect("ulid"),
            device: "01M3HBS0G00000000000000003".parse().expect("ulid"),
            role: TokenRole::Member,
            status: TokenStatus::Active,
        }
    }

    fn service(pem: &str, clock: &FakeClock, audience: &str) -> TokenService {
        TokenService::new(
            SigningKeys::from_pem(pem).expect("valid key"),
            "strata",
            audience,
            Duration::minutes(15),
            Arc::new(clock.clone()),
        )
    }

    #[test]
    fn issued_tokens_verify_and_carry_exact_claims() {
        let pem = generate_key_pem().expect("key");
        let clock = FakeClock::at_default_epoch();
        let svc = service(&pem, &clock, "strata-api");
        let issued = svc.issue(&ids()).expect("issue");
        assert_eq!(issued.expires_at.to_rfc3339(), "2026-09-27T12:15:00+00:00");
        let claims = svc.verify(&issued.token).expect("valid");
        assert_eq!(
            claims,
            AccessClaims {
                iss: "strata".into(),
                aud: "strata-api".into(),
                sub: ids().user,
                sid: ids().session,
                did: ids().device,
                role: TokenRole::Member,
                st: TokenStatus::Active,
                iat: 1_790_510_400,
                exp: 1_790_511_300,
                ver: 1,
            }
        );
        let header = jsonwebtoken::decode_header(&issued.token).expect("header");
        assert_eq!(header.alg, Algorithm::EdDSA);
        assert_eq!(header.kid.as_deref(), Some(svc.keys.kid()));
        assert_eq!(svc.keys.kid().len(), 16);
    }

    #[test]
    fn expiry_follows_the_injected_clock() {
        let pem = generate_key_pem().expect("key");
        let clock = FakeClock::at_default_epoch();
        let svc = service(&pem, &clock, "strata-api");
        let token = svc.issue(&ids()).expect("issue").token;
        clock.advance(Duration::seconds(899));
        assert!(svc.verify(&token).is_ok());
        clock.advance(Duration::seconds(1));
        assert_eq!(svc.verify(&token), Err(TokenError::Expired));
    }

    #[test]
    fn tokens_from_the_future_are_rejected() {
        let pem = generate_key_pem().expect("key");
        let clock = FakeClock::at_default_epoch();
        let svc = service(&pem, &clock, "strata-api");
        let token = svc.issue(&ids()).expect("issue").token;
        clock.advance(Duration::seconds(-61));
        assert_eq!(svc.verify(&token), Err(TokenError::Invalid("issued-at")));
    }

    #[test]
    fn a_different_key_is_rejected() {
        let clock = FakeClock::at_default_epoch();
        let ours = service(&generate_key_pem().expect("key"), &clock, "strata-api");
        let theirs = service(&generate_key_pem().expect("key"), &clock, "strata-api");
        let token = theirs.issue(&ids()).expect("issue").token;
        assert_eq!(ours.verify(&token), Err(TokenError::Invalid("unknown key")));
        // Same kid claimed, different key: the signature check fails.
        let mut forged_header = Header::new(Algorithm::EdDSA);
        forged_header.kid = Some(ours.keys.kid().to_owned());
        let forged = jsonwebtoken::encode(
            &forged_header,
            &theirs.verify(&token).expect("theirs"),
            &theirs.keys.encoding,
        )
        .expect("sign");
        assert_eq!(ours.verify(&forged), Err(TokenError::Invalid("signature")));
    }

    #[test]
    fn tampered_payloads_are_rejected() {
        let clock = FakeClock::at_default_epoch();
        let svc = service(&generate_key_pem().expect("key"), &clock, "strata-api");
        let token = svc.issue(&ids()).expect("issue").token;
        let parts: Vec<&str> = token.split('.').collect();
        let mut claims: serde_json::Value =
            serde_json::from_slice(&URL_SAFE_NO_PAD.decode(parts[1]).expect("b64")).expect("json");
        claims["role"] = serde_json::json!("admin");
        let payload = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).expect("json"));
        let tampered = format!("{}.{payload}.{}", parts[0], parts[2]);
        assert_eq!(svc.verify(&tampered), Err(TokenError::Invalid("signature")));
        assert_eq!(
            svc.verify("not-a-token"),
            Err(TokenError::Invalid("malformed"))
        );
        let truncated = format!("{}.{}", parts[0], parts[1]);
        assert_eq!(svc.verify(&truncated), Err(TokenError::Invalid("malformed")));
    }

    #[test]
    fn wrong_audience_and_issuer_are_rejected() {
        let pem = generate_key_pem().expect("key");
        let clock = FakeClock::at_default_epoch();
        let api = service(&pem, &clock, "strata-api");
        let other = service(&pem, &clock, "other-api");
        let token = other.issue(&ids()).expect("issue").token;
        assert_eq!(api.verify(&token), Err(TokenError::Invalid("audience")));
        let foreign_issuer = TokenService::new(
            SigningKeys::from_pem(&pem).expect("key"),
            "someone-else",
            "strata-api",
            Duration::minutes(15),
            Arc::new(clock.clone()),
        );
        let token = foreign_issuer.issue(&ids()).expect("issue").token;
        assert_eq!(api.verify(&token), Err(TokenError::Invalid("issuer")));
    }

    #[test]
    fn other_algorithms_are_rejected() {
        let clock = FakeClock::at_default_epoch();
        let svc = service(&generate_key_pem().expect("key"), &clock, "strata-api");
        let claims = svc.verify(&svc.issue(&ids()).expect("issue").token).expect("ok");
        let mut header = Header::new(Algorithm::HS256);
        header.kid = Some(svc.keys.kid().to_owned());
        let hs = jsonwebtoken::encode(&header, &claims, &EncodingKey::from_secret(b"guess"))
            .expect("sign");
        assert_eq!(svc.verify(&hs), Err(TokenError::Invalid("algorithm")));
    }

    #[test]
    fn claims_of_another_version_are_rejected() {
        let clock = FakeClock::at_default_epoch();
        let svc = service(&generate_key_pem().expect("key"), &clock, "strata-api");
        let mut claims = svc.verify(&svc.issue(&ids()).expect("issue").token).expect("ok");
        claims.ver = 2;
        let mut header = Header::new(Algorithm::EdDSA);
        header.kid = Some(svc.keys.kid().to_owned());
        let token = jsonwebtoken::encode(&header, &claims, &svc.keys.encoding).expect("sign");
        assert_eq!(svc.verify(&token), Err(TokenError::Invalid("claims version")));
    }

    #[test]
    fn keys_load_from_pkcs8_pem_only() {
        let pem = generate_key_pem().expect("key");
        assert!(pem.starts_with("-----BEGIN PRIVATE KEY-----\n"));
        let a = SigningKeys::from_pem(&pem).expect("valid");
        let b = SigningKeys::from_pem(&pem).expect("valid");
        assert_eq!(a.kid(), b.kid());
        assert_eq!(
            SigningKeys::from_pem("garbage").map(|_| ()),
            Err(TokenError::InvalidKey(
                "not an Ed25519 PKCS#8 PEM private key".into()
            ))
        );
        // RFC 8410 §10.3 example key (the form `openssl genpkey -algorithm ed25519` writes).
        let rfc = "-----BEGIN PRIVATE KEY-----\nMC4CAQAwBQYDK2VwBCIEINTuctv5E1hK1bbY8fdp+K06/nwoy/HU++CXqI9EdVhC\n-----END PRIVATE KEY-----\n";
        assert!(SigningKeys::from_pem(rfc).is_ok());
    }

    #[test]
    fn refresh_tokens_are_random_and_hashed_with_sha256() {
        let a = new_refresh_token().expect("rng");
        let b = new_refresh_token().expect("rng");
        assert_eq!(a.len(), 43);
        assert_ne!(a, b);
        assert_eq!(
            hex_prefix(&hash_refresh_token("abc"), 32),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
