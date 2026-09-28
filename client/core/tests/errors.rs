//! How failures travel from the wire to Dart: `strata-client` errors are classified into
//! [`NetError`] (what sync retries, what signs the user out), [`NetError`] becomes
//! [`CoreError`], and [`CoreError`] reaches Dart as a [`CoreFailure`] carrying a stable code,
//! its localisation key and only IDs and codes. Endpoints a transport does not implement
//! answer `NotAvailable` with the operation's name.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::too_many_lines)]

use std::sync::Arc;

use futures::future::BoxFuture;
use pretty_assertions::assert_eq;
use strata_client::types::Problem;
use strata_client::{ApiError, StaticToken};
use strata_core::CoreError;
use strata_core::net::{
    AccountApi, AdminUpdate, MeUpdate, NetError, SessionTokens, Tokens, classify,
};
use strata_core::view::model::{CoreFailure, NewUserRequest, Platform};

fn problem(type_: &str, status: u32) -> strata_client::Error {
    ApiError::from_problem(Problem {
        candidates: Vec::new(),
        current_version: None,
        detail: None,
        errors: Vec::new(),
        instance: None,
        status,
        title: "t".to_owned(),
        type_: type_.to_owned(),
    })
    .into()
}

#[tokio::test]
async fn client_errors_are_classified_for_sync_and_sign_in() {
    let cases = [
        (problem("unauthorized", 401), NetError::Unauthorized),
        (problem("account_pending", 403), NetError::AccountPending),
        (problem("account_disabled", 403), NetError::AccountDisabled),
        (
            problem("account_deletion_pending", 403),
            NetError::AccountDeletionPending,
        ),
        (problem("epoch_changed", 410), NetError::EpochChanged),
        (problem("rate_limited", 429), NetError::RateLimited),
        (
            problem("invalid_credentials", 401),
            NetError::InvalidCredentials,
        ),
        (problem("account_rejected", 403), NetError::AccountRejected),
        // Any other 401 problem ends the session too.
        (problem("token_expired", 401), NetError::Unauthorized),
        (
            problem("not_found", 404),
            NetError::Api {
                status: 404,
                problem_type: "not_found".to_owned(),
            },
        ),
        (
            problem("internal", 70_000),
            NetError::Api {
                status: 500,
                problem_type: "internal".to_owned(),
            },
        ),
        (
            strata_client::Error::UnexpectedResponse {
                operation: "get_me",
                status: 502,
                content_type: Some("text/html".to_owned()),
            },
            NetError::Api {
                status: 502,
                problem_type: "internal".to_owned(),
            },
        ),
        (
            strata_client::Error::UnexpectedResponse {
                operation: "get_me",
                status: 200,
                content_type: None,
            },
            NetError::Protocol("unexpected 200 response (None) from get_me".to_owned()),
        ),
        (
            strata_client::Error::StreamClosed {
                operation: "events",
                attempts: 3,
            },
            NetError::Offline("events".to_owned()),
        ),
        (
            strata_client::Error::Decode {
                operation: "get_me",
                message: "1 trailing bytes".to_owned(),
            },
            NetError::Protocol("cannot decode the response of get_me: 1 trailing bytes".to_owned()),
        ),
    ];
    for (error, expected) in cases {
        assert_eq!(classify(&error), expected, "{error:?}");
    }

    // A refused connection is "offline", which sync retries.
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind")
        .local_addr()
        .expect("addr")
        .port();
    let client = strata_client::Client::new(&format!("http://127.0.0.1:{port}")).expect("client");
    let refused = strata_client::operations::health(&client)
        .await
        .expect_err("refused");
    let net = classify(&refused);
    assert!(matches!(&net, NetError::Offline(_)), "{net:?}");
    assert!(net.is_transient());
}

#[test]
fn only_transport_failures_rate_limits_and_server_errors_are_transient() {
    let transient = [
        NetError::Offline("dns".to_owned()),
        NetError::RateLimited,
        NetError::Api {
            status: 503,
            problem_type: "unavailable".to_owned(),
        },
    ];
    let permanent = [
        NetError::Unauthorized,
        NetError::InvalidCredentials,
        NetError::AccountPending,
        NetError::AccountRejected,
        NetError::AccountDisabled,
        NetError::AccountDeletionPending,
        NetError::EpochChanged,
        NetError::NotAvailable {
            endpoint: "ask".to_owned(),
        },
        NetError::Api {
            status: 422,
            problem_type: "invalid_body".to_owned(),
        },
        NetError::Protocol("x".to_owned()),
    ];
    assert!(transient.iter().all(NetError::is_transient));
    assert!(!permanent.iter().any(NetError::is_transient));
}

fn failure(
    code: &str,
    field: Option<&str>,
    reason: Option<&str>,
    count: Option<u32>,
    status: Option<u16>,
) -> CoreFailure {
    CoreFailure {
        code: code.to_owned(),
        message_key: format!("error.{code}"),
        field: field.map(str::to_owned),
        reason: reason.map(str::to_owned),
        count,
        status,
    }
}

#[test]
fn net_errors_reach_dart_as_localisable_failures() {
    let cases = [
        (
            NetError::Offline("tcp".to_owned()),
            failure("offline", None, None, None, None),
        ),
        (
            NetError::Unauthorized,
            failure("session_expired", None, None, None, None),
        ),
        (
            NetError::InvalidCredentials,
            failure("invalid_credentials", None, None, None, None),
        ),
        (
            NetError::AccountPending,
            failure("account_pending", None, None, None, None),
        ),
        (
            NetError::AccountRejected,
            failure("account_rejected", None, None, None, None),
        ),
        (
            NetError::AccountDisabled,
            failure("account_disabled", None, None, None, None),
        ),
        (
            NetError::AccountDeletionPending,
            failure("account_deletion_pending", None, None, None, None),
        ),
        (
            NetError::RateLimited,
            failure("rate_limited", None, None, None, None),
        ),
        (
            NetError::NotAvailable {
                endpoint: "ask".to_owned(),
            },
            failure("not_available", Some("ask"), None, None, None),
        ),
        (
            NetError::EpochChanged,
            failure("server", None, Some("epoch_changed"), None, Some(410)),
        ),
        (
            NetError::Api {
                status: 409,
                problem_type: "version_conflict".to_owned(),
            },
            failure("server", None, Some("version_conflict"), None, Some(409)),
        ),
        (
            NetError::Protocol("bad frame".to_owned()),
            failure("internal", None, None, None, None),
        ),
    ];
    for (net, expected) in cases {
        let core = CoreError::from(net.clone());
        assert_eq!(CoreFailure::from(core), expected, "{net:?}");
    }
}

#[test]
fn every_core_error_has_a_stable_code_and_carries_only_ids_and_codes() {
    let cases = [
        (
            CoreError::NotInitialised,
            failure("not_initialised", None, None, None, None),
            "the core is not initialised",
        ),
        (
            CoreError::NotSignedIn,
            failure("not_signed_in", None, None, None, None),
            "no account is signed in",
        ),
        (
            CoreError::InvalidInput {
                field: "path".to_owned(),
                reason: "forbidden_char".to_owned(),
            },
            failure(
                "invalid_input",
                Some("path"),
                Some("forbidden_char"),
                None,
                None,
            ),
            "invalid path: forbidden_char",
        ),
        (
            CoreError::NotFound {
                what: "note".to_owned(),
            },
            failure("not_found", Some("note"), None, None, None),
            "note not found",
        ),
        (
            CoreError::PendingChanges { count: 4 },
            failure("pending_changes", None, None, Some(4), None),
            "4 changes have not synced",
        ),
        (
            CoreError::TaskChange {
                reason: "not_open".to_owned(),
            },
            failure("task_change", None, Some("not_open"), None, None),
            "task change not possible: not_open",
        ),
        (
            CoreError::StaleEdit,
            failure("stale_edit", None, None, None, None),
            "the note changed since it was opened",
        ),
        (
            CoreError::MisconfiguredBuild {
                reason: "missing".to_owned(),
            },
            failure(
                "misconfigured_build",
                Some("server_url"),
                Some("missing"),
                None,
                None,
            ),
            "misconfigured build: server address missing",
        ),
        (
            CoreError::Storage("disk full".to_owned()),
            failure("storage", None, None, None, None),
            "storage error: disk full",
        ),
    ];
    for (error, expected, text) in cases {
        assert_eq!(error.to_string(), text);
        assert_eq!(CoreFailure::from(error), expected);
    }

    // Library failures are storage errors (or internal ones for encoding).
    let io = CoreError::from(std::io::Error::new(
        std::io::ErrorKind::PermissionDenied,
        "denied",
    ));
    assert_eq!(io, CoreError::Storage("io: denied".to_owned()));
    let sqlite = CoreError::from(rusqlite::Error::QueryReturnedNoRows);
    assert_eq!(
        sqlite,
        CoreError::Storage("Query returned no rows".to_owned())
    );
    let decode = CoreError::from(rmp_serde::from_slice::<u8>(&[0xc1]).expect_err("reserved"));
    assert!(
        matches!(&decode, CoreError::Storage(m) if m.starts_with("decode: ")),
        "{decode:?}"
    );
    let encode = CoreError::from(rmp_serde::encode::Error::Syntax("bad".to_owned()));
    assert_eq!(encode, CoreError::Internal("encode: bad".to_owned()));
}

/// A transport that implements only sign-in (as a test double would).
#[derive(Debug)]
struct SignInOnly;

impl AccountApi for SignInOnly {
    fn signup(
        &self,
        _: String,
        username: String,
        _: String,
        _: String,
    ) -> BoxFuture<'_, Result<String, NetError>> {
        Box::pin(async move { Ok(username) })
    }

    fn login(
        &self,
        _: String,
        _: String,
        _: String,
        _: String,
        _: Platform,
    ) -> BoxFuture<'_, Result<SessionTokens, NetError>> {
        Box::pin(async { Err(NetError::InvalidCredentials) })
    }

    fn refresh(&self, _: String, _: String) -> BoxFuture<'_, Result<SessionTokens, NetError>> {
        Box::pin(async { Err(NetError::Unauthorized) })
    }

    fn logout(&self, _: String, _: Tokens) -> BoxFuture<'_, Result<(), NetError>> {
        Box::pin(async { Ok(()) })
    }

    fn me(
        &self,
        _: String,
        _: Tokens,
    ) -> BoxFuture<'_, Result<strata_core::net::MeInfo, NetError>> {
        Box::pin(async { Err(NetError::Unauthorized) })
    }

    fn set_device_reminders(
        &self,
        _: String,
        _: Tokens,
        _: String,
        _: bool,
    ) -> BoxFuture<'_, Result<(), NetError>> {
        Box::pin(async { Ok(()) })
    }

    fn admin_users(
        &self,
        _: String,
        _: Tokens,
    ) -> BoxFuture<'_, Result<Vec<strata_core::net::AdminUserInfo>, NetError>> {
        Box::pin(async { Ok(Vec::new()) })
    }
}

fn na(endpoint: &str) -> NetError {
    NetError::NotAvailable {
        endpoint: endpoint.to_owned(),
    }
}

#[tokio::test]
async fn endpoints_a_transport_does_not_implement_answer_not_available() {
    let api = SignInOnly;
    let url = || "https://strata.example".to_owned();
    let t = || -> Tokens { Arc::new(StaticToken("t".to_owned())) };
    let id = || "01J8ZK3M4X7Q9W2E5R6T8Y0V1B".to_owned();

    assert_eq!(
        api.update_me(url(), t(), MeUpdate::default())
            .await
            .map(|_| ()),
        Err(na("update_me"))
    );
    assert_eq!(api.devices(url(), t()).await, Err(na("list_devices")));
    assert_eq!(
        api.rename_device(url(), t(), id(), "x".to_owned()).await,
        Err(na("update_device"))
    );
    assert_eq!(
        api.revoke_device(url(), t(), id()).await,
        Err(na("delete_device"))
    );
    assert_eq!(
        api.admin_approve(url(), t(), id()).await,
        Err(na("admin_approve_user"))
    );
    assert_eq!(
        api.admin_reject(url(), t(), id()).await,
        Err(na("admin_reject_user"))
    );
    assert_eq!(
        api.admin_update(url(), t(), id(), AdminUpdate::default())
            .await,
        Err(na("admin_update_user"))
    );
    assert_eq!(
        api.admin_schedule_deletion(url(), t(), id()).await,
        Err(na("admin_delete_user"))
    );
    assert_eq!(
        api.admin_cancel_deletion(url(), t(), id()).await,
        Err(na("admin_cancel_deletion"))
    );
    assert_eq!(
        api.admin_create(
            url(),
            t(),
            NewUserRequest {
                username: "u".to_owned(),
                display_name: "U".to_owned(),
                password: "p".to_owned(),
                role: "member".to_owned(),
            }
        )
        .await,
        Err(na("admin_create_user"))
    );
    assert_eq!(api.export_me(url(), t()).await, Err(na("export_me")));
    assert_eq!(
        api.confirm_deletion(url(), t()).await,
        Err(na("confirm_deletion"))
    );
    assert_eq!(api.export_vault(url(), t()).await, Err(na("export_vault")));
    assert_eq!(
        api.import_vault(url(), t(), Vec::new()).await,
        Err(na("import_vault"))
    );
    assert_eq!(
        api.note_history(url(), t(), id()).await,
        Err(na("get_note_history"))
    );
    assert_eq!(
        api.note_revision(url(), t(), id(), "c".to_owned()).await,
        Err(na("get_note_revision"))
    );
    assert_eq!(
        api.revert_note(url(), t(), id(), "c".to_owned()).await,
        Err(na("revert_note"))
    );
    assert_eq!(
        api.search(url(), t(), "q".to_owned(), "keyword".to_owned(), 20)
            .await,
        Err(na("search"))
    );
    assert_eq!(api.ai_status(url(), t()).await, Err(na("ai_status")));
    assert_eq!(api.integrity(url(), t()).await, Err(na("get_integrity")));
    assert_eq!(
        api.ask(url(), t(), "why?".to_owned(), None).await,
        Err(na("ask"))
    );
    assert_eq!(
        api.ask_stream(url(), t(), id())
            .map(|_| ())
            .expect_err("stream"),
        na("ask_stream")
    );
    assert_eq!(
        api.ai_decisions(url(), t(), 10).await,
        Err(na("list_ai_decisions"))
    );
    assert_eq!(
        api.reject_ai_decision(url(), t(), id()).await,
        Err(na("reject_ai_decision"))
    );
    assert_eq!(
        api.repoint_ai_decision(url(), t(), id(), id(), None).await,
        Err(na("repoint_ai_decision"))
    );
    assert_eq!(
        api.retype_ai_decision(url(), t(), id(), "related".to_owned())
            .await,
        Err(na("retype_ai_decision"))
    );
    assert_eq!(api.similarity_edges(url(), t()).await, Err(na("get_graph")));
    assert_eq!(
        api.put_map(url(), t(), id(), "{}".to_owned()).await,
        Err(na("put_map"))
    );
    assert_eq!(
        api.save_ask(
            url(),
            t(),
            id(),
            None,
            "2026-09-27T10:00:00Z".parse().expect("time")
        )
        .await,
        Err(na("save_ask"))
    );
    // What the UI shows for them.
    assert_eq!(
        CoreFailure::from(CoreError::from(na("put_map"))),
        failure("not_available", Some("put_map"), None, None, None)
    );
}
