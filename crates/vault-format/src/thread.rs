//! A note's AI follow-up thread: `.meta/threads/<note-id>.json` (owner decision 2026-09-28).
//!
//! The questions asked about one note and the AI's answers, oldest first. The thread lives
//! beside the note, not in its sidecar: it grows with every exchange, while the sidecar is
//! rewritten by every linking run. The AI never edits the note itself; answers cite it and the
//! notes it links to with wikilinks, like Ask.
//!
//! JSON on disk, pretty-printed with two-space indentation and a trailing newline. Unknown
//! fields are kept (in `extra`) so older code never drops data written by newer code.

use std::collections::BTreeMap;

use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use ulid::Ulid;

use crate::sidecar::to_pretty_json;

/// Who wrote a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThreadRole {
    /// The user's question.
    User,
    /// The AI's answer.
    Assistant,
}

/// A source an answer cites.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThreadCitation {
    /// The cited note.
    pub note_id: Ulid,
    /// The wikilink target (`Link#^block` or `Link`).
    pub target: String,
    /// The cited block, when a block and not the whole note is cited.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block_id: Option<String>,
}

/// One message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThreadMessage {
    /// Message ID.
    pub id: Ulid,
    /// Who wrote it.
    pub role: ThreadRole,
    /// The text (answers keep their `[[…]]` citations).
    pub text: String,
    /// Sources an answer cites, in order of first appearance.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub citations: Vec<ThreadCitation>,
    /// `provider/model` that wrote an answer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// When it was written.
    pub created: DateTime<FixedOffset>,
}

/// `.meta/threads/<note-id>.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteThread {
    /// The note the thread is about.
    pub note_id: Ulid,
    /// Messages, oldest first.
    #[serde(default)]
    pub messages: Vec<ThreadMessage>,
    /// Fields this version does not know.
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

impl NoteThread {
    /// An empty thread about `note_id`.
    pub fn new(note_id: Ulid) -> Self {
        Self {
            note_id,
            messages: Vec::new(),
            extra: BTreeMap::new(),
        }
    }

    /// The vault path of the thread about `note_id`.
    pub fn path_for(note_id: Ulid) -> String {
        format!(".meta/threads/{note_id}.json")
    }

    /// Parses the JSON file.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Serialises to the on-disk form.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        to_pretty_json(self)
    }

    /// Appends a question and its answer. A message whose ID is already in the thread is not
    /// added again (a retried write).
    pub fn push_exchange(&mut self, question: ThreadMessage, answer: ThreadMessage) {
        for m in [question, answer] {
            if !self.messages.iter().any(|x| x.id == m.id) {
                self.messages.push(m);
            }
        }
    }

    /// The last `n` messages (the context given to the AI with a new question).
    pub fn recent(&self, n: usize) -> &[ThreadMessage] {
        &self.messages[self.messages.len().saturating_sub(n)..]
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    const NOTE: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0V1H";

    fn ulid(s: &str) -> Ulid {
        Ulid::from_string(s).expect("ulid")
    }

    fn at(s: &str) -> DateTime<FixedOffset> {
        DateTime::parse_from_rfc3339(s).expect("time")
    }

    fn exchange() -> (ThreadMessage, ThreadMessage) {
        (
            ThreadMessage {
                id: ulid("01K5THREAD0000000000000001"),
                role: ThreadRole::User,
                text: "What did Ahmed promise?".into(),
                citations: Vec::new(),
                model: None,
                created: at("2026-09-28T19:00:00+03:00"),
            },
            ThreadMessage {
                id: ulid("01K5THREAD0000000000000002"),
                role: ThreadRole::Assistant,
                text: "Delivery by Friday [[Pricing#^a1]].".into(),
                citations: vec![ThreadCitation {
                    note_id: ulid(NOTE),
                    target: "Pricing#^a1".into(),
                    block_id: Some("a1".into()),
                }],
                model: Some("claude_cli/sonnet".into()),
                created: at("2026-09-28T16:00:05Z"),
            },
        )
    }

    #[test]
    fn a_thread_round_trips_in_its_on_disk_form() {
        let mut t = NoteThread::new(ulid(NOTE));
        let (q, a) = exchange();
        t.push_exchange(q.clone(), a.clone());
        // A retried write adds nothing.
        t.push_exchange(q, a);
        let json = t.to_json().expect("json");
        assert_eq!(
            json,
            r#"{
  "note_id": "01J8ZK3M4X7Q9W2E5R6T8Y0V1H",
  "messages": [
    {
      "id": "01K5THREAD0000000000000001",
      "role": "user",
      "text": "What did Ahmed promise?",
      "created": "2026-09-28T19:00:00+03:00"
    },
    {
      "id": "01K5THREAD0000000000000002",
      "role": "assistant",
      "text": "Delivery by Friday [[Pricing#^a1]].",
      "citations": [
        {
          "note_id": "01J8ZK3M4X7Q9W2E5R6T8Y0V1H",
          "target": "Pricing#^a1",
          "block_id": "a1"
        }
      ],
      "model": "claude_cli/sonnet",
      "created": "2026-09-28T16:00:05Z"
    }
  ]
}
"#
        );
        assert_eq!(NoteThread::from_json(&json).expect("parse"), t);
        assert_eq!(
            NoteThread::path_for(ulid(NOTE)),
            ".meta/threads/01J8ZK3M4X7Q9W2E5R6T8Y0V1H.json"
        );
        assert_eq!(t.recent(1).len(), 1);
        assert_eq!(t.recent(9).len(), 2);
    }

    #[test]
    fn unknown_fields_are_kept() {
        let json = "{\n  \"note_id\": \"01J8ZK3M4X7Q9W2E5R6T8Y0V1H\",\n  \"messages\": [],\n  \"pinned\": true\n}\n";
        let t = NoteThread::from_json(json).expect("parse");
        assert_eq!(t.extra.get("pinned"), Some(&serde_json::Value::Bool(true)));
        assert_eq!(t.to_json().expect("json"), json);
    }
}
