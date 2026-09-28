//! Ask (PLAN §9.5, §11 screen 10) against a scripted server: answers that fail to start or
//! to stream, streams that end early or break, Stop while tokens arrive, repeated citations,
//! the conversation's sources and citation markers, and the AI status panel (budget, pause,
//! embedding progress, AI turned off). The account endpoints are the fake account API's.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::too_many_lines)]

mod common;

use std::collections::VecDeque;
use std::fmt;
use std::sync::{Arc, Mutex, OnceLock};

use chrono::{DateTime, Utc};
use common::{Harness, SERVER};
use futures::future::BoxFuture;
use pretty_assertions::assert_eq;
use strata_core::CoreError;
use strata_core::net::{
    AccountApi, AdminUserInfo, AiStatusInfo, AskEvent, AskStream, DeviceInfo, MeInfo, NetError,
    SessionTokens, Tokens,
};
use strata_core::session::{Core, Session};
use strata_core::sync::engine::Trigger;
use strata_core::testing::FakeAccountApi;
use strata_core::view::build;
use strata_core::view::model::{
    AiStatusView, AskSource, AskSpan, AskView, Availability, Platform, SignInRequest,
};

enum Step {
    Event(Result<AskEvent, NetError>),
    /// The user taps Stop, then this event arrives.
    StopThen(AskEvent),
}

struct Script {
    steps: VecDeque<Step>,
    session: Arc<OnceLock<Arc<Session>>>,
}

impl AskStream for Script {
    fn next(&mut self) -> BoxFuture<'_, Option<Result<AskEvent, NetError>>> {
        let next = match self.steps.pop_front() {
            None => None,
            Some(Step::Event(e)) => Some(e),
            Some(Step::StopThen(e)) => {
                self.session.get().expect("session").stop_ask();
                Some(Ok(e))
            }
        };
        Box::pin(async move { next })
    }
}

/// The fake account API plus scripted Ask and AI status answers.
struct AskApi {
    inner: Arc<FakeAccountApi>,
    asks: Mutex<VecDeque<Result<String, NetError>>>,
    streams: Mutex<VecDeque<Result<Vec<Step>, NetError>>>,
    status: Mutex<Option<AiStatusInfo>>,
    session: Arc<OnceLock<Arc<Session>>>,
}

impl fmt::Debug for AskApi {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AskApi").finish_non_exhaustive()
    }
}

impl AccountApi for AskApi {
    fn signup(
        &self,
        server_url: String,
        username: String,
        password: String,
        display_name: String,
    ) -> BoxFuture<'_, Result<String, NetError>> {
        self.inner
            .signup(server_url, username, password, display_name)
    }
    fn login(
        &self,
        server_url: String,
        username: String,
        password: String,
        device_name: String,
        platform: Platform,
    ) -> BoxFuture<'_, Result<SessionTokens, NetError>> {
        self.inner
            .login(server_url, username, password, device_name, platform)
    }
    fn refresh(
        &self,
        server_url: String,
        refresh_token: String,
    ) -> BoxFuture<'_, Result<SessionTokens, NetError>> {
        self.inner.refresh(server_url, refresh_token)
    }
    fn logout(&self, server_url: String, tokens: Tokens) -> BoxFuture<'_, Result<(), NetError>> {
        self.inner.logout(server_url, tokens)
    }
    fn me(&self, server_url: String, tokens: Tokens) -> BoxFuture<'_, Result<MeInfo, NetError>> {
        self.inner.me(server_url, tokens)
    }
    fn set_device_reminders(
        &self,
        server_url: String,
        tokens: Tokens,
        device_id: String,
        enabled: bool,
    ) -> BoxFuture<'_, Result<(), NetError>> {
        self.inner
            .set_device_reminders(server_url, tokens, device_id, enabled)
    }
    fn admin_users(
        &self,
        server_url: String,
        tokens: Tokens,
    ) -> BoxFuture<'_, Result<Vec<AdminUserInfo>, NetError>> {
        self.inner.admin_users(server_url, tokens)
    }
    fn devices(
        &self,
        _server_url: String,
        _tokens: Tokens,
    ) -> BoxFuture<'_, Result<Vec<DeviceInfo>, NetError>> {
        Box::pin(async { Ok(Vec::new()) })
    }
    fn ai_status(
        &self,
        _server_url: String,
        _tokens: Tokens,
    ) -> BoxFuture<'_, Result<AiStatusInfo, NetError>> {
        let s = self
            .status
            .lock()
            .unwrap()
            .clone()
            .ok_or(NetError::NotAvailable {
                endpoint: "ai_status".into(),
            });
        Box::pin(async move { s })
    }
    fn ask(
        &self,
        _server_url: String,
        _tokens: Tokens,
        _question: String,
        _scope: Option<String>,
    ) -> BoxFuture<'_, Result<String, NetError>> {
        let r = self.asks.lock().unwrap().pop_front().expect("scripted ask");
        Box::pin(async move { r })
    }
    fn ask_stream(
        &self,
        _server_url: String,
        _tokens: Tokens,
        _ask_id: String,
    ) -> Result<Box<dyn AskStream>, NetError> {
        let steps = self
            .streams
            .lock()
            .unwrap()
            .pop_front()
            .expect("scripted stream")?;
        Ok(Box::new(Script {
            steps: steps.into(),
            session: self.session.clone(),
        }))
    }
}

async fn world() -> (Harness, Arc<AskApi>, Arc<Session>) {
    let mut h = Harness::new();
    let api = Arc::new(AskApi {
        inner: h.accounts.clone(),
        asks: Mutex::default(),
        streams: Mutex::default(),
        status: Mutex::default(),
        session: Arc::default(),
    });
    let mut env = common::env(
        h.dir.path(),
        &h.server,
        &h.accounts,
        &h.clock,
        &h.ids,
        h.platform,
    );
    env.account_api = api.clone();
    h.core = Core::open(env).expect("core");
    h.core
        .sign_in(SignInRequest {
            server_url: SERVER.to_owned(),
            username: "shawket".to_owned(),
            password: "pw-a".to_owned(),
            device_name: "Shawket's laptop".to_owned(),
        })
        .await
        .expect("sign in");
    let s = h.core.session().expect("session");
    api.session.set(s.clone()).expect("once");
    s.sync(Trigger::Start).await.expect("bootstrap");
    (h, api, s)
}

fn view(s: &Session) -> AskView {
    let entries = s.ask_entries();
    s.read(|c, ctx| build::ask(c, ctx, &entries))
        .expect("ask view")
}

/// `(id, role, text, streaming, error_key)` of every message.
fn messages(s: &Session) -> Vec<(String, String, String, bool, Option<String>)> {
    view(s)
        .messages
        .into_iter()
        .map(|m| (m.id, m.role, m.text, m.streaming, m.error_key))
        .collect()
}

fn msg(
    id: &str,
    role: &str,
    text: &str,
    error: Option<&str>,
) -> (String, String, String, bool, Option<String>) {
    (
        id.to_owned(),
        role.to_owned(),
        text.to_owned(),
        false,
        error.map(str::to_owned),
    )
}

fn tokens(t: &str) -> Step {
    Step::Event(Ok(AskEvent::Tokens(t.to_owned())))
}

fn citation(index: u32, note: &str, title: &str, block: Option<&str>) -> AskEvent {
    AskEvent::Citation {
        index,
        note_id: note.to_owned(),
        path: format!("notes/{title}.md"),
        title: title.to_owned(),
        block_id: block.map(str::to_owned),
        target: block.map_or_else(|| title.to_owned(), |b| format!("{title}#^{b}")),
    }
}

#[tokio::test]
async fn failures_to_start_or_stream_end_the_answer_with_an_error() {
    let (_h, api, s) = world().await;
    assert_eq!(
        s.ask("  ", None, "All notes").await,
        Err(CoreError::InvalidInput {
            field: "question".into(),
            reason: "empty".into()
        })
    );

    // The server refuses the question.
    api.asks.lock().unwrap().push_back(Err(NetError::Api {
        status: 503,
        problem_type: "ai_unavailable".into(),
    }));
    assert_eq!(
        s.ask("What changed in pricing?", None, "All notes").await,
        Err(CoreError::Server {
            status: 503,
            problem_type: "ai_unavailable".into()
        })
    );
    // The stream cannot be opened.
    api.asks.lock().unwrap().push_back(Ok("ask-2".into()));
    api.streams
        .lock()
        .unwrap()
        .push_back(Err(NetError::Offline("reset".into())));
    assert_eq!(
        s.ask("And churn?", None, "All notes").await,
        Err(CoreError::Offline)
    );
    // The stream breaks after some text: the text stays, with the error.
    api.asks.lock().unwrap().push_back(Ok("ask-3".into()));
    api.streams.lock().unwrap().push_back(Ok(vec![
        tokens("Churn fell "),
        Step::Event(Err(NetError::Offline("reset".into()))),
    ]));
    assert_eq!(
        s.ask("Churn in Q3?", None, "All notes").await,
        Ok("ask-3".to_owned())
    );
    assert_eq!(
        messages(&s),
        [
            msg("local-1-q", "user", "What changed in pricing?", None),
            msg("local-1", "assistant", "", Some("error.server")),
            msg("local-2-q", "user", "And churn?", None),
            msg("ask-2", "assistant", "", Some("error.offline")),
            msg("local-3-q", "user", "Churn in Q3?", None),
            msg("ask-3", "assistant", "Churn fell ", Some("error.offline")),
        ]
    );
    // Only a finished answer can be saved.
    assert_eq!(
        s.save_answer_as_note("local-1-q").await,
        Err(CoreError::NotFound {
            what: "answer".into()
        })
    );

    s.new_conversation();
    assert_eq!(messages(&s), []);
}

#[tokio::test]
async fn stop_ends_the_stream_and_an_early_end_keeps_what_arrived() {
    let (_h, api, s) = world().await;
    api.asks.lock().unwrap().push_back(Ok("ask-1".into()));
    api.streams.lock().unwrap().push_back(Ok(vec![
        tokens("Prices rose "),
        Step::StopThen(AskEvent::Tokens("8%".into())),
        tokens(" in November."),
    ]));
    assert_eq!(
        s.ask("Prices?", None, "All notes").await,
        Ok("ask-1".into())
    );

    // A stream that ends without its final answer.
    api.asks.lock().unwrap().push_back(Ok("ask-2".into()));
    api.streams
        .lock()
        .unwrap()
        .push_back(Ok(vec![tokens("Mona leads it.")]));
    assert_eq!(
        s.ask("Who leads?", None, "All notes").await,
        Ok("ask-2".into())
    );
    assert_eq!(
        messages(&s),
        [
            msg("local-1-q", "user", "Prices?", None),
            msg("ask-1", "assistant", "Prices rose 8%", Some("stopped")),
            msg("local-2-q", "user", "Who leads?", None),
            msg("ask-2", "assistant", "Mona leads it.", None),
        ]
    );
    assert!(!view(&s).streaming);
}

#[tokio::test]
async fn citations_become_sources_and_markers() {
    const PRICING: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0V1B";
    const CHURN: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0V1C";
    let (_h, api, s) = world().await;
    api.asks.lock().unwrap().push_back(Ok("ask-1".into()));
    api.streams.lock().unwrap().push_back(Ok(vec![
        tokens("Prices rose [[Pricing#^b1]] while churn fell [[Churn]]; "),
        Step::Event(Ok(citation(2, CHURN, "Churn", None))),
        Step::Event(Ok(citation(1, PRICING, "Pricing", Some("b1")))),
        // A repeated index is kept once.
        Step::Event(Ok(citation(1, PRICING, "Pricing", Some("b1")))),
        Step::Event(Ok(citation(3, PRICING, "Pricing", Some("b7")))),
        tokens("see [[Pricing#^b7]]"),
    ]));
    s.ask("Q4?", Some("notes".into()), "notes/")
        .await
        .expect("ask");

    let answer = view(&s).messages.pop().expect("answer");
    assert_eq!(
        answer.text,
        "Prices rose [[Pricing#^b1]] while churn fell [[Churn]]; see [[Pricing#^b7]]"
    );
    assert_eq!(
        answer.spans,
        [
            AskSpan {
                text: "Prices rose ".into(),
                citation: None
            },
            AskSpan {
                text: String::new(),
                citation: Some(1)
            },
            AskSpan {
                text: " while churn fell ".into(),
                citation: None
            },
            AskSpan {
                text: String::new(),
                citation: Some(2)
            },
            AskSpan {
                text: "; see ".into(),
                citation: None
            },
            AskSpan {
                text: String::new(),
                citation: Some(3)
            },
        ]
    );
    assert_eq!(
        answer.sources,
        [
            AskSource {
                note_id: PRICING.into(),
                title: "Pricing".into(),
                path: "notes/Pricing.md".into(),
                anchors: vec!["b1".into(), "b7".into()],
                indexes: vec![1, 3],
            },
            AskSource {
                note_id: CHURN.into(),
                title: "Churn".into(),
                path: "notes/Churn.md".into(),
                anchors: Vec::new(),
                indexes: vec![2],
            },
        ]
    );
    assert_eq!(
        (answer.source_count, answer.scope_label.as_str()),
        (2, "Scope: notes/")
    );
    assert_eq!(
        answer
            .citations
            .iter()
            .map(|c| (c.note_id.as_deref(), c.target.as_str(), c.anchor.as_deref()))
            .collect::<Vec<_>>(),
        [
            (Some(PRICING), "Pricing#^b1", Some("b1")),
            (Some(CHURN), "Churn", None),
            (Some(PRICING), "Pricing#^b7", Some("b7")),
        ]
    );
}

#[tokio::test]
async fn the_ai_status_panel_reads_the_cached_status() {
    let (h, api, s) = world().await;
    assert_eq!(
        (view(&s).availability, view(&s).ai_status),
        (Availability::Available, None)
    );
    let paused_until = DateTime::parse_from_rfc3339("2026-09-27T13:00:00Z")
        .expect("ts")
        .with_timezone(&Utc);
    *api.status.lock().unwrap() = Some(AiStatusInfo {
        enabled: true,
        provider: Some("claude".into()),
        paused_until: Some(paused_until),
        paused_reason: Some("budget".into()),
        queue_depth: 4,
        tokens_used: 150_000,
        tokens_limit: 120_000,
        embedded: Some((3, 12)),
    });
    s.refresh_settings().await.expect("refresh");
    assert_eq!(
        view(&s).ai_status,
        Some(AiStatusView {
            enabled: true,
            provider: Some("claude".into()),
            paused_label: Some("Paused until 16:00".into()),
            queue_depth: 4,
            budget_used_percent: 100,
            budget_label: "100% used".into(),
            embedding_percent: Some(25),
        })
    );
    // No budget and nothing to embed; AI turned off for the account.
    *api.status.lock().unwrap() = Some(AiStatusInfo {
        enabled: false,
        provider: None,
        paused_until: None,
        paused_reason: None,
        queue_depth: 0,
        tokens_used: 10,
        tokens_limit: 0,
        embedded: Some((0, 0)),
    });
    s.refresh_settings().await.expect("refresh");
    let v = view(&s);
    assert_eq!(v.availability, Availability::NotAllowed);
    assert_eq!(
        v.ai_status
            .map(|a| (a.budget_used_percent, a.embedding_percent)),
        Some((0, Some(100)))
    );
    // Offline: Ask is unavailable and refuses questions.
    h.server.set_offline(true);
    s.sync(Trigger::Manual).await.expect("cycle");
    assert_eq!(view(&s).availability, Availability::Offline);
    assert_eq!(
        s.ask("Q4?", None, "All notes").await,
        Err(CoreError::Offline)
    );
}
