//! AI features behind the API (PLAN §7.5 Search/AI, §9.5): the retrieval and Ask engines and
//! the registry of running Ask answers.
//!
//! `POST /ask` starts an answer and returns its ID; the answer is produced by a background
//! task into an [`AskEntry`] whether or not anyone listens, and `GET /ask/{id}` streams its
//! frames over a WebSocket (D24; WebSocket upgrades are `GET`, so the question travels in the
//! `POST` body, not in a URL). Every frame is kept until the entry expires, so a client that
//! reconnects with `resume_from` gets exactly the frames it missed. Entries belong to one
//! user: another user's ID answers `404` (principle 7). Finished entries expire after
//! [`ASK_TTL`]; each user keeps at most [`MAX_ASKS_PER_USER`].

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use chrono::{DateTime, Utc};
use futures_util::Stream;
use strata_ai::AiService;
use strata_common::{Clock, IdGenerator, UserId};
use strata_index::AppDb;
use strata_jobs::ask::{AskEngine, AskEvent, Citation};
use strata_jobs::retrieval::Retriever;
use tokio::sync::watch;
use ulid::Ulid;

use crate::routes::ai::AskFrame;
use crate::wire::Problem;
use crate::wire::ws::Frame;

/// How long a finished answer stays available (for resuming and "save as note").
pub const ASK_TTL: chrono::Duration = chrono::Duration::minutes(30);

/// Answers kept per user (oldest finished ones are dropped first).
pub const MAX_ASKS_PER_USER: usize = 16;

/// How an answer ended.
#[derive(Debug, Clone, PartialEq)]
pub enum AskEnd {
    /// Complete: the final answer text and its citations.
    Done {
        /// Final answer.
        answer: String,
        /// Citations in order.
        citations: Vec<Citation>,
    },
    /// Failed with this problem.
    Failed(Problem),
}

#[derive(Debug, Default)]
struct AskState {
    frames: Vec<AskFrame>,
    end: Option<AskEnd>,
    finished_at: Option<DateTime<Utc>>,
    citations: Vec<Citation>,
}

/// One answer being produced or kept.
#[derive(Debug)]
pub struct AskEntry {
    user: UserId,
    question: String,
    state: Mutex<AskState>,
    changed: watch::Sender<u64>,
}

impl AskEntry {
    fn lock(&self) -> std::sync::MutexGuard<'_, AskState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The question.
    pub fn question(&self) -> &str {
        &self.question
    }

    /// Records one event of the run.
    pub fn push(&self, event: AskEvent, now: DateTime<Utc>) {
        let mut s = self.lock();
        if s.end.is_some() {
            return;
        }
        match event {
            AskEvent::Tokens(text) => s.frames.push(AskFrame::Tokens { text }),
            AskEvent::Citation(c) => {
                s.frames.push(AskFrame::from_citation(&c));
                s.citations.push(c);
            }
            AskEvent::Done { answer } => {
                s.frames.push(AskFrame::Done {
                    answer: answer.clone(),
                });
                let citations = s.citations.clone();
                s.end = Some(AskEnd::Done { answer, citations });
                s.finished_at = Some(now);
            }
            AskEvent::Failed(e) => {
                s.end = Some(AskEnd::Failed(crate::routes::ai::ask_problem(&e)));
                s.finished_at = Some(now);
            }
        }
        let n = s.frames.len() as u64;
        drop(s);
        self.changed.send_replace(n);
    }

    /// Waits for the end of the answer.
    pub async fn finished(&self) -> AskEnd {
        let mut rx = self.changed.subscribe();
        loop {
            if let Some(end) = self.lock().end.clone() {
                return end;
            }
            if rx.changed().await.is_err() {
                return AskEnd::Failed(Problem::new(crate::wire::ProblemType::Internal));
            }
        }
    }

    /// The frames after `resume_from`, then `end` or `error` (D24 envelope).
    pub fn frames(
        self: Arc<Self>,
        resume_from: Option<u64>,
    ) -> impl Stream<Item = Frame<AskFrame>> + Unpin + 'static {
        let rx = self.changed.subscribe();
        let next = usize::try_from(resume_from.unwrap_or(0)).unwrap_or(usize::MAX);
        Box::pin(futures_util::stream::unfold(
            (self, rx, next, false),
            |(entry, mut rx, next, done)| async move {
                if done {
                    return None;
                }
                loop {
                    let step = {
                        let s = entry.lock();
                        if next < s.frames.len() {
                            Some((
                                Frame::Data {
                                    seq: next as u64 + 1,
                                    payload: s.frames[next].clone(),
                                },
                                false,
                            ))
                        } else {
                            let last = s.frames.len() as u64;
                            match &s.end {
                                Some(AskEnd::Done { .. }) => Some((Frame::End { seq: last }, true)),
                                Some(AskEnd::Failed(p)) => Some((
                                    Frame::Error {
                                        seq: last,
                                        problem: p.clone(),
                                    },
                                    true,
                                )),
                                None => None,
                            }
                        }
                    };
                    match step {
                        Some((frame, end)) => {
                            let n = if end { next } else { next + 1 };
                            return Some((frame, (entry, rx, n, end)));
                        }
                        None => {
                            if rx.changed().await.is_err() {
                                return None;
                            }
                        }
                    }
                }
            },
        ))
    }
}

/// Running and recent answers, per user.
#[derive(Debug, Default)]
pub struct AskRegistry {
    entries: Mutex<HashMap<Ulid, Arc<AskEntry>>>,
}

impl AskRegistry {
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<Ulid, Arc<AskEntry>>> {
        self.entries.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Registers a new answer of `user` under `id` (expiring old ones).
    pub fn insert(
        &self,
        id: Ulid,
        user: UserId,
        question: &str,
        now: DateTime<Utc>,
    ) -> Arc<AskEntry> {
        let entry = Arc::new(AskEntry {
            user,
            question: question.to_owned(),
            state: Mutex::new(AskState::default()),
            changed: watch::channel(0).0,
        });
        let mut all = self.lock();
        all.retain(|_, e| e.lock().finished_at.is_none_or(|at| now - at < ASK_TTL));
        let mut mine: Vec<(Ulid, Option<DateTime<Utc>>)> = all
            .iter()
            .filter(|(_, e)| e.user == user)
            .map(|(k, e)| (*k, e.lock().finished_at))
            .collect();
        if mine.len() >= MAX_ASKS_PER_USER {
            // Drop the oldest finished answers first (IDs are time-ordered).
            mine.sort_by_key(|(k, at)| (at.is_none(), *k));
            for (k, _) in mine.iter().take(mine.len() + 1 - MAX_ASKS_PER_USER) {
                all.remove(k);
            }
        }
        all.insert(id, entry.clone());
        entry
    }

    /// The answer `id` if it belongs to `user`.
    pub fn get(&self, id: Ulid, user: UserId) -> Option<Arc<AskEntry>> {
        self.lock().get(&id).filter(|e| e.user == user).cloned()
    }
}

/// The AI features the API serves; registered as app data by the composition root. Without
/// it, semantic/hybrid search and Ask answer `503 ai_unavailable` (principle 6).
#[derive(Debug)]
pub struct AiApi {
    /// Routing, budgets, status.
    pub ai: Arc<AiService>,
    /// The `strata_app` database.
    pub db: AppDb,
    /// Semantic/hybrid retrieval (`None` without embeddings).
    pub retriever: Option<Retriever>,
    /// Ask.
    pub ask: AskEngine,
    /// Clock.
    pub clock: Arc<dyn Clock>,
    /// Answer IDs.
    pub ids: Arc<dyn IdGenerator>,
    /// Running and recent answers.
    pub asks: AskRegistry,
}

impl AiApi {
    /// The API's AI features.
    pub fn new(
        ai: Arc<AiService>,
        db: AppDb,
        retriever: Option<Retriever>,
        ask: AskEngine,
        clock: Arc<dyn Clock>,
        ids: Arc<dyn IdGenerator>,
    ) -> Self {
        Self {
            ai,
            db,
            retriever,
            ask,
            clock,
            ids,
            asks: AskRegistry::default(),
        }
    }
}
