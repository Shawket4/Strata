//! Ask (PLAN §9.5, §11 screen 10): one conversation per session, answers streamed from the
//! server's `/ask` WebSocket (D24) into the `watch_ask` view. Online only.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use chrono::{DateTime, Utc};

use super::Session;
use crate::error::{CoreError, CoreResult};
use crate::net::{AskEvent, NetError};
use crate::view::Topics;
use crate::view::model::Connectivity;

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
    fn ask_update(&self, f: impl FnOnce(&mut AskState)) {
        {
            let mut g = self.lock();
            f(&mut g.ask);
        }
        if let Err(e) = self.write(|_, _| Ok(((), Topics::ASK))) {
            tracing::warn!("ask view refresh failed: {e}");
        }
    }

    /// A copy of the conversation (view builder input).
    pub fn ask_entries(&self) -> Vec<AskEntry> {
        self.lock().ask.messages.clone()
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
        if self.lock().ask.streaming() {
            return Err(CoreError::invalid("ask", "busy"));
        }
        let now = self.env.clock.now();
        let stop = Arc::new(AtomicBool::new(false));
        let local_id = {
            let mut g = self.lock();
            g.ask.next_local += 1;
            format!("local-{}", g.ask.next_local)
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
        let url = self.server_url()?;
        let api = self.env.account_api.clone();
        let ask_id = match api
            .ask(url.clone(), self.tokens(), question.to_owned(), scope)
            .await
        {
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
        self.lock().ask.stop = None;
        Ok(ask_id)
    }

    /// Stops the streaming answer.
    pub fn stop_ask(&self) {
        self.lock().ask.stop();
    }

    /// Starts a new conversation (stops a streaming answer).
    pub fn new_conversation(&self) {
        self.ask_update(|a| {
            a.stop();
            a.messages.clear();
        });
    }

    /// Saves an answer as a note (the server writes it with its citations as links, §9.5).
    pub async fn save_answer_as_note(&self, message_id: &str) -> CoreResult<String> {
        let done = self
            .lock()
            .ask
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
                self.server_url()?,
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
