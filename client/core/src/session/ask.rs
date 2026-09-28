//! Ask (PLAN §9.5, §11 screen 10): one conversation per session, answers streamed from the
//! server's `/ask` WebSocket (D24) into the `watch_ask` view. Online only.
//!
//! A conversation can be about one note (owner decision 2026-09-28): [`Session::open_note_thread`]
//! scopes it to the note, the view shows the note's saved thread (synced from the server)
//! followed by the exchange still streaming, and questions go to `POST /notes/{id}/thread`,
//! which appends each finished exchange to the thread.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, MutexGuard, PoisonError};

use chrono::{DateTime, Utc};

use super::Session;
use crate::error::{CoreError, CoreResult};
use crate::net::{AskEvent, NetError};
use crate::view::model::AskView;
use crate::view::model::Connectivity;
use crate::view::{Topics, ViewSink, WatchId};

/// A cited source of an answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AskCitation {
    /// 1-based index.
    pub index: u32,
    /// Note.
    pub note_id: String,
    /// Path.
    pub path: String,
    /// Title.
    pub title: String,
    /// Block ID.
    pub block_id: Option<String>,
    /// Link target (`Note#^id`).
    pub target: String,
}

/// One message of the conversation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AskEntry {
    /// ID (the server's Ask ID for answers).
    pub id: String,
    /// `user` | `assistant`.
    pub role: String,
    /// Text so far.
    pub text: String,
    /// When.
    pub created: DateTime<Utc>,
    /// Citations so far.
    pub citations: Vec<AskCitation>,
    /// Still streaming.
    pub streaming: bool,
    /// The scope label.
    pub scope_label: String,
    /// Why it stopped early.
    pub error_key: Option<String>,
    /// Saved as this note.
    pub saved_note_id: Option<String>,
}

/// The session's conversation.
#[derive(Debug, Default)]
pub struct AskState {
    /// Messages, oldest first.
    pub messages: Vec<AskEntry>,
    /// The note the conversation is about (its thread), if any.
    pub note: Option<String>,
    stop: Option<Arc<AtomicBool>>,
    next_local: u32,
}

impl AskState {
    /// Whether an answer is streaming.
    pub fn streaming(&self) -> bool {
        self.messages.iter().any(|m| m.streaming)
    }

    /// Asks the running stream (if any) to stop.
    pub fn stop(&mut self) {
        if let Some(flag) = self.stop.take() {
            flag.store(true, Ordering::SeqCst);
        }
    }
}

impl Session {
    pub(super) fn ask_state(&self) -> MutexGuard<'_, AskState> {
        self.ask.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn ask_update(&self, f: impl FnOnce(&mut AskState)) {
        f(&mut self.ask_state());
        if let Err(e) = self.write(|_, _| Ok(((), Topics::ASK))) {
            tracing::warn!("ask view refresh failed: {e}");
        }
    }

    /// The note the conversation is about, if any.
    pub fn ask_state_note(&self) -> Option<String> {
        self.ask_state().note.clone()
    }

    /// A copy of the conversation (view builder input).
    pub fn ask_entries(&self) -> Vec<AskEntry> {
        self.ask_state().messages.clone()
    }

    /// The Ask screen's stream: the conversation as it streams. The builder reads the
    /// conversation through its own lock, never the session lock it runs under.
    pub fn watch_ask(&self, sink: impl ViewSink<AskView> + 'static) -> CoreResult<WatchId> {
        let ask = Arc::clone(&self.ask);
        self.watch(
            Topics::ASK
                | Topics::SYNC
                | Topics::REMOTE
                | Topics::ENTITIES
                | Topics::ACCOUNT
                | Topics::NOTES,
            move |c, ctx| {
                let (entries, note) = {
                    let a = ask.lock().unwrap_or_else(PoisonError::into_inner);
                    (a.messages.clone(), a.note.clone())
                };
                crate::view::build::ask(c, ctx, &entries, note.as_deref())
            },
            sink,
        )
    }

    /// Asks a question (online only) and streams the answer into the conversation. Returns
    /// the answer's ID once the answer ended (stopped, done or failed).
    #[allow(clippy::too_many_lines)] // one arm per stream event
    pub async fn ask(
        &self,
        question: &str,
        scope: Option<String>,
        scope_label: &str,
    ) -> CoreResult<String> {
        let question = question.trim();
        if question.is_empty() {
            return Err(CoreError::invalid("question", "empty"));
        }
        if self.ctx().connectivity == Connectivity::Offline {
            return Err(CoreError::Offline);
        }
        if self.ask_state().streaming() {
            return Err(CoreError::invalid("ask", "busy"));
        }
        let now = self.env.clock.now();
        let stop = Arc::new(AtomicBool::new(false));
        let local_id = {
            let mut a = self.ask_state();
            a.next_local += 1;
            format!("local-{}", a.next_local)
        };
        self.ask_update(|a| {
            a.stop = Some(stop.clone());
            a.messages.push(AskEntry {
                id: format!("{local_id}-q"),
                role: "user".to_owned(),
                text: question.to_owned(),
                created: now,
                citations: Vec::new(),
                streaming: false,
                scope_label: scope_label.to_owned(),
                error_key: None,
                saved_note_id: None,
            });
            a.messages.push(AskEntry {
                id: local_id.clone(),
                role: "assistant".to_owned(),
                text: String::new(),
                created: now,
                citations: Vec::new(),
                streaming: true,
                scope_label: scope_label.to_owned(),
                error_key: None,
                saved_note_id: None,
            });
        });
        let set = |id: &str, f: &dyn Fn(&mut AskEntry)| {
            self.ask_update(|a| {
                if let Some(m) = a.messages.iter_mut().find(|m| m.id == id) {
                    f(m);
                }
            });
        };
        let fail = |id: &str, e: &NetError| {
            let key = CoreError::from(e.clone()).message_key();
            set(id, &|m: &mut AskEntry| {
                m.streaming = false;
                m.error_key = Some(key.clone());
            });
        };
        let url = self.server_url();
        let api = self.env.account_api.clone();
        let note = self.ask_state().note.clone();
        let started = match note {
            Some(note) => {
                api.ask_about_note(url.clone(), self.tokens(), note, question.to_owned())
                    .await
            }
            None => {
                api.ask(url.clone(), self.tokens(), question.to_owned(), scope)
                    .await
            }
        };
        let ask_id = match started {
            Ok(id) => id,
            Err(e) => {
                fail(&local_id, &e);
                return Err(e.into());
            }
        };
        set(&local_id, &|m: &mut AskEntry| m.id.clone_from(&ask_id));
        let mut stream = match api.ask_stream(url, self.tokens(), ask_id.clone()) {
            Ok(s) => s,
            Err(e) => {
                fail(&ask_id, &e);
                return Err(e.into());
            }
        };
        loop {
            if stop.load(Ordering::SeqCst) {
                set(&ask_id, &|m: &mut AskEntry| {
                    m.streaming = false;
                    m.error_key = Some("stopped".to_owned());
                });
                break;
            }
            match stream.next().await {
                None => {
                    set(&ask_id, &|m: &mut AskEntry| m.streaming = false);
                    break;
                }
                Some(Err(e)) => {
                    fail(&ask_id, &e);
                    break;
                }
                Some(Ok(AskEvent::Tokens(t))) => {
                    set(&ask_id, &|m: &mut AskEntry| m.text.push_str(&t));
                }
                Some(Ok(AskEvent::Citation {
                    index,
                    note_id,
                    path,
                    title,
                    block_id,
                    target,
                })) => {
                    let c = AskCitation {
                        index,
                        note_id,
                        path,
                        title,
                        block_id,
                        target,
                    };
                    set(&ask_id, &|m: &mut AskEntry| {
                        if !m.citations.iter().any(|x| x.index == c.index) {
                            m.citations.push(c.clone());
                        }
                    });
                }
                Some(Ok(AskEvent::Done(answer))) => {
                    set(&ask_id, &|m: &mut AskEntry| {
                        m.text.clone_from(&answer);
                        m.streaming = false;
                    });
                }
            }
        }
        self.ask_state().stop = None;
        Ok(ask_id)
    }

    /// Stops the streaming answer.
    pub fn stop_ask(&self) {
        self.ask_state().stop();
    }

    /// Starts a new conversation (stops a streaming answer), about the whole vault again.
    pub fn new_conversation(&self) {
        self.ask_update(|a| {
            a.stop();
            a.messages.clear();
            a.note = None;
        });
    }

    /// Makes the conversation about note `note_id`: its saved thread is shown and questions
    /// go to that note (stops a streaming answer; the note must exist on this device).
    pub fn open_note_thread(&self, note_id: &str) -> CoreResult<()> {
        let known = self.read(|c, _| Ok(crate::store::notes::current(c, note_id)?.is_some()))?;
        if !known {
            return Err(CoreError::not_found("note"));
        }
        self.ask_update(|a| {
            a.stop();
            a.messages.clear();
            a.note = Some(note_id.to_owned());
        });
        Ok(())
    }

    /// Saves an answer as a note (the server writes it with its citations as links, §9.5).
    pub async fn save_answer_as_note(&self, message_id: &str) -> CoreResult<String> {
        let done = self
            .ask_state()
            .messages
            .iter()
            .any(|m| m.id == message_id && m.role == "assistant" && !m.streaming);
        if !done {
            return Err(CoreError::not_found("answer"));
        }
        let note_id = self
            .env
            .account_api
            .save_ask(
                self.server_url(),
                self.tokens(),
                message_id.to_owned(),
                None,
                self.env.clock.now(),
            )
            .await?;
        self.ask_update(|a| {
            if let Some(m) = a.messages.iter_mut().find(|m| m.id == message_id) {
                m.saved_note_id = Some(note_id.clone());
            }
        });
        Ok(note_id)
    }
}
