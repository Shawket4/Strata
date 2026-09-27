//! AI endpoints through the generated client (PLAN §7.5 Search/AI, §9.5, §9.7, D24):
//! semantic and hybrid search, the Ask stream frame sequence (contract-validated frames,
//! citations resolved to real blocks, resume), save as note, AI status, the semantic level of
//! the create check, the `ai_unavailable` / `ai_paused` problems, and principle 6 (AI down,
//! everything else works). Every response is validated against the contract.
#![allow(clippy::expect_used, clippy::too_many_lines, clippy::float_cmp)]

mod ai_harness;

use std::num::NonZeroU32;
use std::time::Duration;

use ai_harness::{H, MODEL, Options, User, plain, problem};
use futures_util::StreamExt;
use pretty_assertions::assert_eq;
use strata_ai::BudgetLimits;
use strata_ai::embed::fake::bag_of_words;
use strata_ai::prompts::{self, ids};
use strata_ai::request::input_hash;
use strata_client::streaming::{StreamEvent, StreamOptions};
use strata_client::{operations as ops, streams, types};
use strata_testkit::Fixture;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use ulid::Ulid;

async fn create(u: &User, path: &str, body: &str) -> types::Note {
    ops::create_note(
        &u.client,
        &types::CreateNoteRequest {
            content: body.to_owned(),
            force: Some(true),
            id: None,
            path: path.to_owned(),
        },
    )
    .await
    .expect("create")
}

fn cosine(a: &[f32], b: &[f32]) -> f64 {
    f64::from(a.iter().zip(b).map(|(x, y)| x * y).sum::<f32>())
}

#[tokio::test]
async fn semantic_and_hybrid_search_return_ranked_hits_with_snippets() {
    let h = H::new().await;
    let alice = h.user("alice").await;
    let c = &alice.client;
    let contract = create(
        &alice,
        "notes/Watanya contract.md",
        "The Watanya contract is stored in the safe.\n",
    )
    .await;
    let arabic = create(&alice, "notes/عقد وطنية.md", "عقد وطنية في الخزنة\n").await;
    let banana = create(&alice, "notes/Banana.md", "Banana bread recipe.\n").await;
    h.embed_all().await;

    let q = bag_of_words("contract safe", 384);
    let score = |title: &str, body: &str| cosine(&q, &bag_of_words(&format!("{title}\n{body}"), 384));
    let semantic = ops::search(c, "contract safe", Some(&types::SearchMode::Semantic), Some(2))
        .await
        .expect("search");
    assert_eq!(semantic.hits.len(), 2);
    assert_eq!(
        (
            semantic.hits[0].id,
            semantic.hits[0].path.as_str(),
            semantic.hits[0].title.as_str(),
            semantic.hits[0].kind,
            semantic.hits[0].snippet.as_deref()
        ),
        (
            contract.id,
            "notes/Watanya contract.md",
            "Watanya contract",
            types::NoteKind::Note,
            Some("The Watanya contract is stored in the safe.")
        )
    );
    assert!(
        (semantic.hits[0].score
            - score("Watanya contract", "The Watanya contract is stored in the safe."))
        .abs()
            < 1e-6
    );
    // The rest have cosine 0 here: chunk order breaks the tie.
    assert_eq!(semantic.hits[1].id, arabic.id);

    // Hybrid: the normalised keyword ranking puts the Arabic note first ("وطنيه" ≈ "وطنية").
    let hybrid = ops::search(c, "وطنيه", Some(&types::SearchMode::Hybrid), None)
        .await
        .expect("search");
    let r = |rank: f64| 1.0 / (60.0 + rank);
    assert_eq!(
        hybrid
            .hits
            .iter()
            .map(|h| (h.id, (h.score * 1e9).round()))
            .collect::<Vec<_>>(),
        vec![
            (arabic.id, ((r(1.0) + r(2.0)) * 1e9).round()),
            (contract.id, (r(1.0) * 1e9).round()),
            (banana.id, (r(3.0) * 1e9).round()),
        ]
    );
    assert_eq!(hybrid.hits[0].snippet.as_deref(), Some("عقد وطنية في الخزنة"));
    // Keyword mode is unchanged.
    let keyword = ops::search(c, "وطنيه", None, None).await.expect("search");
    assert_eq!(keyword.hits.len(), 1);
    assert_eq!(keyword.hits[0].id, arabic.id);
    h.finish().await;
}

/// `ask-000001` → `ask-00000{1+k}` for the fresh IDs of a repeated attempt (IDs < 32).
fn shift_ids(text: &str, k: u32) -> String {
    const ALPHABET: &[u8] = b"0123456789abcdefghjkmnpqrstvwxyz";
    let mut out = text.to_owned();
    for n in (1..=31u32).rev() {
        let digit = |v: u32| ALPHABET[usize::try_from(v).expect("digit")] as char;
        let from = format!("ask-00000{}", digit(n));
        if n + k < 32 && out.contains(&from) {
            out = out.replace(&from, &format!("ask-00000{}", digit(n + k)));
        }
    }
    out
}

async fn raw_frames(h: &H, token: &str, id: Ulid, resume: Option<u64>) -> Vec<Vec<u8>> {
    let url = match resume {
        Some(r) => format!("ws://{}/api/v1/ask/{id}?resume_from={r}", h.addr()),
        None => format!("ws://{}/api/v1/ask/{id}", h.addr()),
    };
    let mut request = url.into_client_request().expect("request");
    request.headers_mut().insert(
        "Authorization",
        format!("Bearer {token}").parse().expect("header"),
    );
    let (mut socket, _) = tokio_tungstenite::connect_async(request)
        .await
        .expect("socket");
    let mut out = Vec::new();
    while let Some(msg) = tokio::time::timeout(Duration::from_secs(10), socket.next())
        .await
        .expect("frame in time")
    {
        match msg.expect("message") {
            Message::Binary(b) => out.push(b.to_vec()),
            Message::Close(_) => break,
            _ => {}
        }
    }
    out
}

const QUESTION: &str = "What did Acme ask for?";

#[tokio::test]
async fn ask_streams_tokens_then_citations_resolved_to_blocks_and_saves_as_a_note() {
    let h = H::new().await;
    let alice = h.user("alice").await;
    let c = &alice.client;
    let call = create(
        &alice,
        "notes/Call.md",
        "Acme prefers weekly invoicing.\n\nThey asked for a discount.\n",
    )
    .await;
    let pricing = create(&alice, "notes/Pricing.md", "Discounts are capped at 5%. ^cap\n").await;
    h.embed_all().await;

    // Without a fixture the provider fails before anything streams: 503 at POST time.
    let err = ops::ask(
        c,
        &types::AskRequest {
            question: QUESTION.into(),
            scope: None,
        },
    )
    .await
    .expect_err("no fixture");
    assert_eq!(
        problem(&err),
        plain(
            "ai_unavailable",
            "AI unavailable",
            503,
            Some("AI is disabled or not available for this account")
        )
    );
    let recorded = h.llm.calls().last().cloned().expect("call");
    let first: serde_json::Value = serde_json::from_str(&recorded.user).expect("json");
    let new_ids = first["sources"]
        .as_array()
        .expect("sources")
        .iter()
        .filter(|s| s["ref"].as_str().is_some_and(|r| r.contains("#^ask-")))
        .count();
    let user = shift_ids(&recorded.user, u32::try_from(new_ids).expect("count"));
    let input: serde_json::Value = serde_json::from_str(&user).expect("json");
    let call_ref = input["sources"]
        .as_array()
        .expect("sources")
        .iter()
        .find(|s| s["note_title"] == "Call")
        .and_then(|s| s["ref"].as_str())
        .expect("call ref")
        .to_owned();
    let block = call_ref.trim_start_matches("Call#^").to_owned();
    let system = prompts::latest(ids::ASK).expect("ask").text;
    let tokens = [
        "Weekly invoicing ".to_owned(),
        format!("[[{call_ref}]]"),
        " and a discount, capped at 5% [[Pricing#^cap]].".to_owned(),
    ];
    h.llm.push(
        &recorded.prompt,
        &input_hash(system, &user),
        Fixture::stream(&tokens.iter().map(String::as_str).collect::<Vec<_>>()),
    );
    let started = ops::ask(
        c,
        &types::AskRequest {
            question: QUESTION.into(),
            scope: None,
        },
    )
    .await
    .expect("ask");

    // Raw frames: data × 4, then `end`; every frame conforms to the contract.
    let frames = raw_frames(&h, &alice.token, started.id, None).await;
    assert_eq!(frames.len(), 5);
    for f in &frames {
        h.conformance
            .contract
            .validate_frame("ask_stream", f)
            .expect("frame conforms");
    }
    let decoded: Vec<strata_api::wire::ws::Frame<strata_api::routes::ai::AskFrame>> = frames
        .iter()
        .map(|b| {
            strata_api::wire::ws::Frame::decode(b, &strata_api::wire::DecodeLimits::default())
                .expect("frame")
        })
        .collect();
    let answer = format!(
        "Weekly invoicing [[{call_ref}]] and a discount, capped at 5% [[Pricing#^cap]]."
    );
    use strata_api::routes::ai::AskFrame as F;
    use strata_api::wire::ws::Frame;
    assert_eq!(
        decoded,
        vec![
            Frame::Data {
                seq: 1,
                payload: F::Tokens {
                    text: tokens.concat()
                }
            },
            Frame::Data {
                seq: 2,
                payload: F::Citation {
                    index: 1,
                    reference: call_ref.clone(),
                    note_id: call.id,
                    path: "notes/Call.md".into(),
                    title: "Call".into(),
                    block_id: Some(block.clone()),
                    target: call_ref.clone(),
                }
            },
            Frame::Data {
                seq: 3,
                payload: F::Citation {
                    index: 2,
                    reference: "Pricing#^cap".into(),
                    note_id: pricing.id,
                    path: "notes/Pricing.md".into(),
                    title: "Pricing".into(),
                    block_id: Some("cap".into()),
                    target: "Pricing#^cap".into(),
                }
            },
            Frame::Data {
                seq: 4,
                payload: F::Done {
                    answer: answer.clone()
                }
            },
            Frame::End { seq: 4 },
        ]
    );
    // The cited block now carries the ID, in one `ai: ask` commit.
    assert_eq!(h.log(alice.id)[0], "ai: ask notes/Call.md");
    assert!(
        h.read(alice.id, "notes/Call.md")
            .contains(&format!("Acme prefers weekly invoicing. ^{block}\n"))
    );

    // The generated subscription resumes after seq 2.
    let mut sub = streams::ask_stream(
        c,
        started.id,
        StreamOptions {
            resume_from: Some(2),
            reconnect: false,
            ..StreamOptions::default()
        },
    )
    .expect("subscription");
    let mut rest = Vec::new();
    while let Some(ev) = sub.next().await {
        match ev.expect("event") {
            StreamEvent::Data { seq, payload } => rest.push((seq, payload)),
            other => panic!("unexpected {other:?}"),
        }
    }
    assert_eq!(
        rest,
        vec![
            (
                3,
                types::AskFrame::Citation {
                    block_id: Some("cap".into()),
                    index: NonZeroU32::new(2).expect("nonzero"),
                    note_id: pricing.id,
                    path: "notes/Pricing.md".into(),
                    ref_: "Pricing#^cap".into(),
                    target: "Pricing#^cap".into(),
                    title: "Pricing".into(),
                }
            ),
            (4, types::AskFrame::Done { answer: answer.clone() }),
        ]
    );

    // Save as note: `notes/<question>.md` with the answer and its citations as links.
    let saved = ops::save_ask(
        c,
        started.id,
        &types::SaveAskRequest {
            force: None,
            title: None,
        },
    )
    .await
    .expect("save");
    assert_eq!(saved.path, "notes/What did Acme ask for.md");
    assert!(
        saved.content.ends_with(&format!(
            "---\n> {QUESTION}\n\n{answer}\n\n## Sources\n\n- [[{call_ref}]]\n- [[Pricing#^cap]]\n"
        )),
        "{}",
        saved.content
    );
    assert_eq!(h.log(alice.id)[0], "user: create notes/What did Acme ask for.md");

    // Another user never sees the answer.
    let bob = h.user("bob").await;
    let err = ops::save_ask(
        &bob.client,
        started.id,
        &types::SaveAskRequest {
            force: None,
            title: Some("Stolen".into()),
        },
    )
    .await
    .expect_err("foreign");
    assert_eq!(problem(&err), plain("not_found", "Not found", 404, None));
    let url = format!("ws://{}/api/v1/ask/{}", h.addr(), started.id);
    let mut request = url.into_client_request().expect("request");
    request.headers_mut().insert(
        "Authorization",
        format!("Bearer {}", bob.token).parse().expect("header"),
    );
    match tokio_tungstenite::connect_async(request).await {
        Err(tokio_tungstenite::tungstenite::Error::Http(resp)) => {
            assert_eq!(resp.status().as_u16(), 404);
        }
        other => panic!("expected 404, got {other:?}"),
    }
    h.finish().await;
}

#[tokio::test]
async fn ask_answers_unavailable_or_paused_problems() {
    let h = H::with(Options {
        limits: BudgetLimits {
            per_user_daily_tokens: 1,
            ..BudgetLimits::default()
        },
        ..Options::default()
    })
    .await;
    let noai = h.user("noai").await;
    let req = types::AskRequest {
        question: "Anything?".into(),
        scope: None,
    };
    assert_eq!(
        problem(&ops::ask(&noai.client, &req).await.expect_err("disabled")),
        plain(
            "ai_unavailable",
            "AI unavailable",
            503,
            Some("AI is disabled or not available for this account")
        )
    );
    let alice = h.user("alice").await;
    let empty = ops::ask(
        &alice.client,
        &types::AskRequest {
            question: " ".into(),
            scope: None,
        },
    )
    .await
    .expect_err("empty");
    assert_eq!(problem(&empty).type_, "invalid_body");
    // One answer uses the day's budget (1 token); the next ask is paused until tomorrow.
    ops::ask(&alice.client, &req).await.expect_err("no fixture yet");
    let recorded = h.llm.calls().last().cloned().expect("call");
    h.llm.push(
        &recorded.prompt,
        &input_hash(prompts::latest(ids::ASK).expect("ask").text, &recorded.user),
        Fixture::stream(&["Nothing."]),
    );
    let started = ops::ask(&alice.client, &req).await.expect("ask");
    let frames = raw_frames(&h, &alice.token, started.id, None).await;
    assert_eq!(frames.len(), 3, "tokens, done, end");
    assert_eq!(
        problem(&ops::ask(&alice.client, &req).await.expect_err("paused")),
        plain(
            "ai_paused",
            "AI paused",
            503,
            Some("user_budget until 2026-09-28T00:00:00+00:00")
        )
    );
    let status = ops::ai_status(&alice.client).await.expect("status");
    assert_eq!(
        status,
        types::AiStatusDto {
            day: "2026-09-27".parse().expect("date"),
            embeddings: Some(types::AiEmbeddingsDto {
                dims: 384,
                embedded: 0,
                loaded: true,
                model_id: MODEL.into(),
                total: 0,
            }),
            enabled: true,
            global_usage: types::AiUsageDto {
                calls: 1,
                cost_micros: 0,
                input_tokens: 100,
                output_tokens: 50,
            },
            limits: types::AiLimitsDto {
                global_daily_cost_micros: 0,
                global_daily_tokens: 0,
                per_user_daily_cost_micros: 0,
                per_user_daily_tokens: 1,
            },
            paused: Some(types::AiPauseDto {
                reason: "user_budget".into(),
                until: Some("2026-09-28T00:00:00Z".parse().expect("time")),
            }),
            provider: Some(types::AiProviderDto {
                last_error: None,
                model: "fake-model".into(),
                name: "claude_cli".into(),
                state: "ready".into(),
            }),
            queue_depth: 0,
            usage: types::AiUsageDto {
                calls: 1,
                cost_micros: 0,
                input_tokens: 100,
                output_tokens: 50,
            },
        }
    );
    h.finish().await;
}

#[tokio::test]
async fn without_ai_everything_else_keeps_working() {
    // No AI features registered: semantic search and Ask are 503, the rest works.
    let h = H::with(Options {
        ai: false,
        ..Options::default()
    })
    .await;
    let alice = h.user("alice").await;
    let c = &alice.client;
    let note = create(&alice, "notes/Acme.md", "Weekly invoicing.\n").await;
    assert_eq!(
        problem(
            &ops::ask(
                c,
                &types::AskRequest {
                    question: "Q".into(),
                    scope: None
                }
            )
            .await
            .expect_err("no ai")
        ),
        plain(
            "ai_unavailable",
            "AI unavailable",
            503,
            Some("the AI subsystem is not configured on this server")
        )
    );
    assert_eq!(
        problem(&ops::ai_status(c).await.expect_err("no ai")).type_,
        "ai_unavailable"
    );
    let hits = ops::search(c, "weekly", None, None).await.expect("keyword");
    assert_eq!(hits.hits.iter().map(|h| h.id).collect::<Vec<_>>(), vec![note.id]);
    h.finish().await;

    // AI registered but the embedding model fails: semantic search is 503, keyword search
    // and creates (exact + near duplicate levels only) keep working.
    let h = H::new().await;
    let alice = h.user("alice").await;
    let c = &alice.client;
    create(&alice, "notes/Acme.md", "Weekly invoicing.\n").await;
    h.embed_all().await;
    h.embedder
        .fail_with(Some(strata_ai::embed::EmbedError::Load("model missing".into())));
    assert_eq!(
        problem(
            &ops::search(c, "weekly", Some(&types::SearchMode::Semantic), None)
                .await
                .expect_err("embedder down")
        ),
        plain(
            "ai_unavailable",
            "AI unavailable",
            503,
            Some("the embedding model could not run; use mode=keyword")
        )
    );
    assert_eq!(
        ops::search(c, "weekly", None, None).await.expect("keyword").hits.len(),
        1
    );
    let created = ops::create_note(
        c,
        &types::CreateNoteRequest {
            content: "x\n".into(),
            force: None,
            id: None,
            path: "notes/Acme invoicing.md".into(),
        },
    )
    .await
    .expect("create without the semantic level");
    assert_eq!(created.path, "notes/Acme invoicing.md");
    h.finish().await;
}

#[tokio::test]
async fn creates_report_semantic_duplicates_with_their_level() {
    let h = H::new().await;
    let alice = h.user("alice").await;
    let mut a = vec![0.0; 384];
    a[0] = 1.0;
    let mut b = vec![0.0; 384];
    b[0] = 0.98;
    b[1] = (1.0f32 - 0.98 * 0.98).sqrt();
    h.embedder.set("Car insurance renewal", &a);
    h.embedder.set("Vehicle policy renewal", &b);
    let first = create(&alice, "notes/Car insurance renewal.md", "x\n").await;
    h.embed_all().await;
    let err = ops::create_note(
        &alice.client,
        &types::CreateNoteRequest {
            content: "x\n".into(),
            force: None,
            id: None,
            path: "notes/Vehicle policy renewal.md".into(),
        },
    )
    .await
    .expect_err("duplicate");
    let p = problem(&err);
    assert_eq!((p.type_.as_str(), p.status), ("duplicate_candidates", 409));
    assert_eq!(p.candidates.len(), 1);
    let cand = &p.candidates[0];
    assert_eq!(
        (cand.id, cand.kind.as_str(), cand.title.as_str(), cand.match_level),
        (
            first.id,
            "note",
            "Car insurance renewal",
            types::MatchLevel::Semantic
        )
    );
    assert!((cand.score - 0.98).abs() < 1e-6);
    h.finish().await;
}
