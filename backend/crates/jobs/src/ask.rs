//! Ask (RAG, PLAN §9.5).
//!
//! 1. **Retrieve**: hybrid chunk retrieval ([`Retriever::ask_chunks`]: vector and full-text
//!    rankings fused by RRF) → the top chunks. Without an embedding model (or when it fails)
//!    the keyword ranking alone is used, on chunks computed from the note text, so Ask still
//!    works from full-text search.
//! 2. **Cite**: every source is cited through the chunk's first block. A block that already
//!    has an ID keeps it; otherwise a fresh ID is chosen now and given to the model in the
//!    source's `ref` (`Link#^id`), and only the IDs the answer actually cites are appended
//!    afterwards, all in one `ai: ask <path>` commit.
//! 3. **Answer**: the `ask` prompt with the question (and its `asked_at` time in the user's
//!    time zone) and the sources, streamed from the provider; the caller forwards token
//!    batches as they arrive.
//! 4. **Resolve**: the answer's `[[ref]]` citations, in order of first appearance, become
//!    [`Citation`]s with note IDs, paths and the block IDs that now exist; refs the sources do
//!    not contain are dropped from the final text (brackets removed).
//!
//! Nothing is saved unless the user asks: [`note_content`] renders the answer as a note for
//! `notes/` with its citations as links.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use chrono::{DateTime, TimeZone, Utc};
use chrono_tz::Tz;
use futures::StreamExt;
use serde::Serialize;
use strata_ai::outputs::ask_citations;
use strata_ai::prompts::{self, ids};
use strata_ai::request::StreamEvent;
use strata_ai::{AiCaller, AiError, AiService, PauseReason};
use strata_common::{Clock, IdGenerator, NoteId};
use strata_index::repo::notes::{self, Note};
use strata_index::{AppDb, UserScope};
use strata_vault::VaultService;
use strata_vault::ops::ai::{CiteOutcome, CiteRequest};
use text_normalize::normalize_for_search;
use vault_format::Document;
use vault_format::body::{self, BlockKind};

use crate::chunk::{self, content_range};
use crate::retrieval::{RetrievalError, Retriever};

/// Job label of the citation commit (`ai: ask <path>`).
pub const ASK_JOB: &str = "ask";

/// Ask settings.
#[derive(Debug, Clone, PartialEq)]
pub struct AskConfig {
    /// Chunks given to the model.
    pub top_k: usize,
    /// Output token limit.
    pub max_tokens: u32,
    /// Time zone for users without one.
    pub default_tz: Tz,
    /// Stream items batched into one token frame at most.
    pub batch: usize,
}

impl Default for AskConfig {
    fn default() -> Self {
        Self {
            top_k: 8,
            max_tokens: 1500,
            default_tz: Tz::UTC,
            batch: 64,
        }
    }
}

/// Why an Ask cannot start or failed.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum AskError {
    /// The question is empty.
    #[error("the question is empty")]
    EmptyQuestion,
    /// AI is disabled or not configured for the user, or the provider failed.
    #[error("AI unavailable: {0}")]
    Unavailable(String),
    /// A budget or the provider's usage limit is reached.
    #[error("AI paused ({reason:?}) until {until:?}")]
    Paused {
        /// Why.
        reason: PauseReason,
        /// When it ends, if known.
        until: Option<DateTime<Utc>>,
    },
    /// Anything else (logged; content-free).
    #[error("internal: {0}")]
    Internal(String),
}

impl From<AiError> for AskError {
    fn from(e: AiError) -> Self {
        match e {
            AiError::Paused { reason, until } => Self::Paused { reason, until },
            AiError::Disabled => Self::Unavailable("AI is disabled for this account".into()),
            AiError::ProviderNotConfigured(p) => {
                Self::Unavailable(format!("provider {p} is not configured"))
            }
            other => Self::Unavailable(other.to_string()),
        }
    }
}

/// One retrieved source given to the model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    /// Citation target given to the model: `Link` or `Link#^block`.
    pub reference: String,
    /// Note.
    pub note_id: NoteId,
    /// Path.
    pub path: String,
    /// Title.
    pub title: String,
    /// Wikilink text of the note (shortest unambiguous).
    pub link: String,
    /// Note `created`.
    pub created: DateTime<Utc>,
    /// Chunk text.
    pub text: String,
    /// Note version the block was located in.
    pub version: String,
    /// Body offset of the cited block.
    pub block_start: usize,
    /// The block's span text (to relocate it).
    pub block_text: String,
    /// The block's existing ID.
    pub existing_id: Option<String>,
    /// The ID to append when cited (the block has none).
    pub new_id: Option<String>,
}

impl Source {
    fn block_id(&self) -> Option<&str> {
        self.existing_id.as_deref().or(self.new_id.as_deref())
    }
}

/// A resolved citation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Citation {
    /// 1-based order of first appearance in the answer.
    pub index: u32,
    /// The ref as written in the answer.
    pub reference: String,
    /// Note.
    pub note_id: NoteId,
    /// Path.
    pub path: String,
    /// Title.
    pub title: String,
    /// The block that now carries this ID (`None`: cite the note).
    pub block_id: Option<String>,
    /// The wikilink target to use (`Link#^block` or `Link`).
    pub target: String,
}

/// What an Ask run produces, in order: token batches, then citations, then `Done` (or
/// `Failed` at any point).
#[derive(Debug, Clone, PartialEq)]
pub enum AskEvent {
    /// The next piece of answer text.
    Tokens(String),
    /// One resolved citation.
    Citation(Citation),
    /// The answer is complete.
    Done {
        /// The final answer with citations as resolvable wikilinks.
        answer: String,
    },
    /// The run failed.
    Failed(AskError),
}

#[derive(Debug, Serialize)]
struct QuestionInput<'a> {
    text: &'a str,
    asked_at: String,
}

#[derive(Debug, Serialize)]
struct SourceInput<'a> {
    #[serde(rename = "ref")]
    reference: &'a str,
    note_title: &'a str,
    created: String,
    text: &'a str,
}

#[derive(Debug, Serialize)]
struct Input<'a> {
    question: QuestionInput<'a>,
    sources: Vec<SourceInput<'a>>,
}

/// Ask for one deployment.
#[derive(Debug, Clone)]
pub struct AskEngine {
    db: AppDb,
    vault: VaultService,
    ai: Arc<AiService>,
    retriever: Option<Retriever>,
    ids: Arc<dyn IdGenerator>,
    clock: Arc<dyn Clock>,
    config: AskConfig,
}

/// A started Ask: the sources and the provider stream.
pub struct AskRun {
    engine: AskEngine,
    scope: UserScope,
    sources: Vec<Source>,
    stream: strata_ai::AiTokenStream,
}

impl std::fmt::Debug for AskRun {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AskRun")
            .field("sources", &self.sources.len())
            .finish_non_exhaustive()
    }
}

/// `[[...]]` spans of `text` with their inner text, in order.
fn wikilink_spans(text: &str) -> Vec<(std::ops::Range<usize>, &str)> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(start) = text[from..].find("[[").map(|i| from + i) {
        let Some(end) = text[start + 2..].find("]]").map(|i| start + 2 + i) else {
            break;
        };
        let inner = &text[start + 2..end];
        if !inner.contains('\n') && !inner.is_empty() {
            out.push((start..end + 2, inner));
        }
        from = end + 2;
    }
    out
}

/// The answer with `[[ref]]` replaced by `[[target]]` for resolved refs and by the plain ref
/// text for unknown ones.
pub fn rewrite_answer(answer: &str, targets: &BTreeMap<String, String>) -> String {
    let mut out = String::with_capacity(answer.len());
    let mut last = 0;
    for (span, inner) in wikilink_spans(answer) {
        out.push_str(&answer[last..span.start]);
        match targets.get(inner) {
            Some(t) => {
                out.push_str("[[");
                out.push_str(t);
                out.push_str("]]");
            }
            None => out.push_str(inner),
        }
        last = span.end;
    }
    out.push_str(&answer[last..]);
    out
}

/// A saved answer as a note body: the question, the answer with its citations as links, and
/// a `## Sources` list (PLAN §9.5 "save as note").
pub fn note_content(question: &str, answer: &str, citations: &[Citation]) -> String {
    let mut s = String::new();
    for line in question.trim().lines() {
        s.push_str("> ");
        s.push_str(line);
        s.push('\n');
    }
    s.push('\n');
    s.push_str(answer.trim());
    s.push('\n');
    let mut seen = BTreeSet::new();
    let targets: Vec<&str> = citations
        .iter()
        .map(|c| c.target.as_str())
        .filter(|t| seen.insert(*t))
        .collect();
    if !targets.is_empty() {
        s.push_str("\n## Sources\n\n");
        for t in targets {
            s.push_str("- [[");
            s.push_str(t);
            s.push_str("]]\n");
        }
    }
    s
}

fn file_stem(path: &str) -> &str {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.strip_suffix(".md").unwrap_or(name)
}

impl AskEngine {
    /// Ask over `vault` and the index, answering with `ai`; `retriever` is `None` without
    /// embeddings (keyword retrieval only).
    pub fn new(
        db: AppDb,
        vault: VaultService,
        ai: Arc<AiService>,
        retriever: Option<Retriever>,
        ids: Arc<dyn IdGenerator>,
        clock: Arc<dyn Clock>,
        config: AskConfig,
    ) -> Self {
        Self {
            db,
            vault,
            ai,
            retriever,
            ids,
            clock,
            config,
        }
    }

    async fn user_tz(&self, scope: &UserScope) -> Tz {
        let setting = async {
            let mut tx = self.db.begin(scope).await.ok()?;
            let v = strata_index::repo::settings::get_setting(
                &mut tx,
                strata_vault::store::SETTING_TIMEZONE,
            )
            .await
            .ok()
            .flatten();
            tx.commit().await.ok()?;
            v
        }
        .await;
        setting
            .and_then(|b| rmp_serde::from_slice::<String>(&b).ok())
            .and_then(|n| n.parse::<Tz>().ok())
            .unwrap_or(self.config.default_tz)
    }

    /// Keyword-only chunks: full-text notes in rank order, chunked on the fly, best chunks by
    /// query terms (Ask without embeddings).
    async fn keyword_chunks(
        &self,
        scope: &UserScope,
        question: &str,
        folder: Option<&str>,
    ) -> Result<Vec<(Note, chunk::ChunkDraft, String)>, AskError> {
        let normalized = normalize_for_search(question);
        if normalized.is_empty() {
            return Ok(Vec::new());
        }
        let terms: Vec<String> = normalized
            .split_whitespace()
            .filter(|t| !t.starts_with('-') && *t != "or")
            .map(|t| t.trim_matches('"').to_owned())
            .filter(|t| !t.is_empty())
            .collect();
        let mut tx = self
            .db
            .begin(scope)
            .await
            .map_err(|e| AskError::Internal(e.to_string()))?;
        let hits = notes::search_notes(&mut tx, &normalized, 20)
            .await
            .map_err(|e| AskError::Internal(e.to_string()))?;
        let mut rows = Vec::new();
        for h in hits {
            if let Some(n) = notes::get_note(&mut tx, h.note_id)
                .await
                .map_err(|e| AskError::Internal(e.to_string()))?
            {
                rows.push(n);
            }
        }
        tx.commit()
            .await
            .map_err(|e| AskError::Internal(e.to_string()))?;
        let mut out = Vec::new();
        for n in rows {
            if n.trashed
                || folder.is_some_and(|f| !n.path.starts_with(&format!("{}/", f.trim_end_matches('/'))))
            {
                continue;
            }
            let Ok(view) = self.vault.note(scope, n.id).await else {
                continue;
            };
            let doc = Document::parse(&view.content);
            let mut chunks = chunk::chunk_body(doc.body());
            let hits = |c: &chunk::ChunkDraft| {
                let t = normalize_for_search(&c.text);
                terms.iter().filter(|x| t.contains(x.as_str())).count()
            };
            chunks.sort_by(|a, b| hits(b).cmp(&hits(a)).then(a.ord.cmp(&b.ord)));
            if let Some(best) = chunks.into_iter().next() {
                out.push((n, best, view.version.clone()));
            }
            if out.len() == self.config.top_k {
                break;
            }
        }
        Ok(out)
    }

    async fn link_text(&self, scope: &UserScope, path: &str) -> String {
        let stem = file_stem(path);
        let count: i64 = async {
            let mut tx = self.db.begin(scope).await.ok()?;
            let n: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM notes WHERE NOT trashed \
                 AND lower(regexp_replace(path, '^.*/', '')) = lower($1)",
            )
            .bind(format!("{stem}.md"))
            .fetch_one(tx.conn())
            .await
            .ok()?;
            tx.commit().await.ok()?;
            Some(n)
        }
        .await
        .unwrap_or(1);
        if count > 1 {
            path.strip_suffix(".md").unwrap_or(path).to_owned()
        } else {
            stem.to_owned()
        }
    }

    fn new_block_id(&self, taken: &BTreeSet<String>) -> String {
        loop {
            let u = NoteId::generate(self.ids.as_ref())
                .as_ulid()
                .to_string()
                .to_lowercase();
            let id = format!("ask-{}", &u[u.len() - 6..]);
            if !taken.contains(&id) {
                return id;
            }
        }
    }

    /// Builds the sources for the retrieved chunks: locates each chunk's first block in the
    /// note as it is now and picks its citation ID.
    async fn sources(
        &self,
        scope: &UserScope,
        chunks: Vec<(Note, crate::chunk::ChunkDraft, String)>,
    ) -> Vec<Source> {
        let mut out: Vec<Source> = Vec::new();
        let mut taken_by_note: BTreeMap<NoteId, BTreeSet<String>> = BTreeMap::new();
        let mut views: BTreeMap<NoteId, Option<strata_vault::model::NoteView>> = BTreeMap::new();
        for (note, c, chunk_version) in chunks {
            let view = if let Some(v) = views.get(&note.id) {
                v.clone()
            } else {
                let v = self.vault.note(scope, note.id).await.ok().filter(|v| !v.trashed);
                views.insert(note.id, v.clone());
                v
            };
            let Some(view) = view else { continue };
            let doc = Document::parse(&view.content);
            let body_text = doc.body().to_owned();
            let analysis = body::analyze(&body_text);
            let located = if view.version == chunk_version {
                analysis.blocks.iter().find(|b| {
                    b.kind != BlockKind::Heading && {
                        let r = content_range(&body_text, b);
                        r.start <= c.start && c.start < r.end.max(r.start + 1)
                    }
                })
            } else if c.anchor_len > 0 {
                let anchor = c.text.get(..c.anchor_len).unwrap_or("");
                analysis.blocks.iter().find(|b| {
                    b.kind != BlockKind::Heading
                        && body_text.get(content_range(&body_text, b)) == Some(anchor)
                })
            } else {
                None
            };
            let taken = taken_by_note.entry(note.id).or_insert_with(|| {
                analysis
                    .blocks
                    .iter()
                    .filter_map(|b| b.id.as_ref().map(|i| i.id.clone()))
                    .collect()
            });
            let link = self.link_text(scope, &view.path).await;
            let (existing_id, new_id, block_start, block_text) = match located {
                Some(b) => {
                    let existing = b.id.as_ref().map(|i| i.id.clone());
                    // Two chunks citing one block share its ID.
                    let shared = out
                        .iter()
                        .find(|s| s.note_id == note.id && s.block_start == b.span.start)
                        .and_then(|s| s.new_id.clone());
                    let new = if existing.is_some() {
                        None
                    } else if shared.is_some() {
                        shared
                    } else {
                        let id = self.new_block_id(taken);
                        taken.insert(id.clone());
                        Some(id)
                    };
                    (
                        existing,
                        new,
                        b.span.start,
                        body_text[b.span.clone()].to_owned(),
                    )
                }
                None => (None, None, 0, String::new()),
            };
            let mut s = Source {
                reference: String::new(),
                note_id: note.id,
                path: view.path.clone(),
                title: view.title.clone(),
                link: link.clone(),
                created: view.created,
                text: if c.text.is_empty() {
                    view.title.clone()
                } else {
                    c.text.clone()
                },
                version: view.version.clone(),
                block_start,
                block_text,
                existing_id,
                new_id,
            };
            s.reference = match s.block_id() {
                Some(id) => format!("{link}#^{id}"),
                None => link,
            };
            if out.iter().any(|o| o.reference == s.reference) {
                continue;
            }
            out.push(s);
        }
        out
    }

    /// Retrieves, builds the prompt and starts the provider stream. Errors here happen before
    /// anything is streamed (the API answers them as problems).
    pub async fn start(
        &self,
        scope: UserScope,
        username: String,
        question: &str,
        folder: Option<&str>,
    ) -> Result<AskRun, AskError> {
        let question = question.trim();
        if question.is_empty() {
            return Err(AskError::EmptyQuestion);
        }
        let caller = AiCaller {
            scope,
            username,
        };
        // Fail fast (before retrieval) when AI is off or paused for this user.
        self.ai.router().route(&caller.username)?;
        self.ai.budget().check(&caller).await?;
        let chunks: Vec<(Note, crate::chunk::ChunkDraft, String)> = match &self.retriever {
            Some(r) => match r.ask_chunks(&scope, question, self.config.top_k, folder).await {
                Ok(ranked) => ranked
                    .into_iter()
                    .map(|rc| {
                        let c = rc.chunk;
                        let draft = crate::chunk::ChunkDraft {
                            ord: usize::try_from(c.ord).unwrap_or(0),
                            heading_path: c.heading_path,
                            start: usize::try_from(c.start_offset).unwrap_or(0),
                            end: usize::try_from(c.end_offset).unwrap_or(0),
                            anchor_len: usize::try_from(c.anchor_len).unwrap_or(0),
                            block_id: c.block_id,
                            token_count: 0,
                            text: c.text,
                        };
                        (rc.note, draft, c.content_hash)
                    })
                    .collect(),
                Err(RetrievalError::Unavailable(e)) => {
                    tracing::warn!(error = %e, "ask: embeddings unavailable, keyword retrieval only");
                    self.keyword_chunks(&scope, question, folder).await?
                }
                Err(RetrievalError::EmptyQuery) => return Err(AskError::EmptyQuestion),
                Err(RetrievalError::Store(e)) => return Err(AskError::Internal(e.to_string())),
            },
            None => self.keyword_chunks(&scope, question, folder).await?,
        };
        let sources = self.sources(&scope, chunks).await;
        let tz = self.user_tz(&scope).await;
        let now = self.clock.now();
        let asked_at = tz.from_utc_datetime(&now.naive_utc()).to_rfc3339();
        let input = Input {
            question: QuestionInput {
                text: question,
                asked_at,
            },
            sources: sources
                .iter()
                .map(|s| SourceInput {
                    reference: &s.reference,
                    note_title: &s.title,
                    created: s.created.with_timezone(&tz).to_rfc3339(),
                    text: &s.text,
                })
                .collect(),
        };
        let prompt = prompts::latest(ids::ASK)
            .ok_or_else(|| AskError::Internal("ask prompt missing".into()))?;
        let req = prompt
            .chat_request(caller, &input, self.config.max_tokens)
            .map_err(|e| AskError::Internal(e.to_string()))?;
        let stream = self.ai.stream(req).await?;
        Ok(AskRun {
            engine: self.clone(),
            scope,
            sources,
            stream,
        })
    }
}

impl AskRun {
    /// The sources given to the model.
    pub fn sources(&self) -> &[Source] {
        &self.sources
    }

    /// Streams the answer to `emit` (token batches), then resolves and emits the citations
    /// and `Done`; on failure emits `Failed` and stops.
    pub async fn run(self, mut emit: impl FnMut(AskEvent) + Send) {
        let AskRun {
            engine,
            scope,
            sources,
            stream,
        } = self;
        let mut answer = String::new();
        let mut batches = stream.ready_chunks(engine.config.batch.max(1));
        let mut done = false;
        while let Some(batch) = batches.next().await {
            let mut text = String::new();
            for item in batch {
                match item {
                    Ok(StreamEvent::Text(t)) => text.push_str(&t),
                    Ok(StreamEvent::Done { .. }) => done = true,
                    Err(e) => {
                        if !text.is_empty() {
                            answer.push_str(&text);
                            emit(AskEvent::Tokens(text));
                        }
                        emit(AskEvent::Failed(e.into()));
                        return;
                    }
                }
            }
            if !text.is_empty() {
                answer.push_str(&text);
                emit(AskEvent::Tokens(text));
            }
            if done {
                break;
            }
        }
        if !done {
            emit(AskEvent::Failed(AskError::Unavailable(
                "the answer stream ended early".into(),
            )));
            return;
        }
        match engine.resolve(&scope, &sources, &answer).await {
            Ok((citations, final_answer)) => {
                for c in citations {
                    emit(AskEvent::Citation(c));
                }
                emit(AskEvent::Done {
                    answer: final_answer,
                });
            }
            Err(e) => emit(AskEvent::Failed(e)),
        }
    }
}

impl AskEngine {
    /// Resolves the answer's citations: appends the new block IDs it cites (one commit) and
    /// returns the citations and the final answer text.
    async fn resolve(
        &self,
        scope: &UserScope,
        sources: &[Source],
        answer: &str,
    ) -> Result<(Vec<Citation>, String), AskError> {
        let mut cited: Vec<(&str, &Source)> = Vec::new();
        for r in ask_citations(answer) {
            if cited.iter().any(|(x, _)| *x == r) {
                continue;
            }
            if let Some(s) = sources.iter().find(|s| s.reference == r) {
                cited.push((r, s));
            }
        }
        let requests: Vec<(usize, CiteRequest)> = cited
            .iter()
            .enumerate()
            .filter_map(|(i, (_, s))| {
                s.new_id.as_ref().map(|id| {
                    (
                        i,
                        CiteRequest {
                            note_id: s.note_id,
                            version: s.version.clone(),
                            block_start: s.block_start,
                            block_text: s.block_text.clone(),
                            block_id: id.clone(),
                        },
                    )
                })
            })
            .collect();
        let mut outcomes: BTreeMap<usize, CiteOutcome> = BTreeMap::new();
        if !requests.is_empty() {
            let (idx, reqs): (Vec<usize>, Vec<CiteRequest>) = requests.into_iter().unzip();
            let results = self
                .vault
                .ai_cite_blocks(scope, reqs, ASK_JOB.to_owned())
                .await
                .map_err(|e| AskError::Internal(e.to_string()))?;
            outcomes.extend(idx.into_iter().zip(results));
        }
        let mut citations = Vec::new();
        let mut targets = BTreeMap::new();
        for (i, (r, s)) in cited.iter().enumerate() {
            let block_id = match outcomes.get(&i) {
                Some(CiteOutcome::Appended) => s.new_id.clone(),
                Some(CiteOutcome::Existing(id)) => Some(id.clone()),
                Some(CiteOutcome::Missing) => None,
                None => s.existing_id.clone(),
            };
            let target = match &block_id {
                Some(b) => format!("{}#^{b}", s.link),
                None => s.link.clone(),
            };
            targets.insert((*r).to_owned(), target.clone());
            citations.push(Citation {
                index: u32::try_from(i + 1).unwrap_or(u32::MAX),
                reference: (*r).to_owned(),
                note_id: s.note_id,
                path: s.path.clone(),
                title: s.title.clone(),
                block_id,
                target,
            });
        }
        Ok((citations, rewrite_answer(answer, &targets)))
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn answers_keep_resolved_refs_and_drop_unknown_ones() {
        let mut t = BTreeMap::new();
        t.insert("Call#^ask-000001".to_owned(), "Call#^ask-000001".to_owned());
        t.insert("Pricing#^p1".to_owned(), "Pricing".to_owned());
        assert_eq!(
            rewrite_answer(
                "Weekly [[Call#^ask-000001]]. Cap 5% [[Pricing#^p1]] [[Made up]].",
                &t
            ),
            "Weekly [[Call#^ask-000001]]. Cap 5% [[Pricing]] Made up."
        );
    }

    #[test]
    fn saved_answers_quote_the_question_and_list_sources_once() {
        let c = |i: u32, target: &str| Citation {
            index: i,
            reference: target.to_owned(),
            note_id: NoteId::from_ulid(ulid::Ulid::nil()),
            path: "notes/Call.md".into(),
            title: "Call".into(),
            block_id: None,
            target: target.to_owned(),
        };
        assert_eq!(
            note_content(
                "What did Acme want?",
                "Weekly invoicing [[Call#^a1]].\n",
                &[c(1, "Call#^a1"), c(2, "Call#^a1")]
            ),
            "> What did Acme want?\n\nWeekly invoicing [[Call#^a1]].\n\n## Sources\n\n- [[Call#^a1]]\n"
        );
        assert_eq!(note_content("Q", "No answer.", &[]), "> Q\n\nNo answer.\n");
    }

    #[test]
    fn file_stems_drop_folders_and_extension() {
        assert_eq!(file_stem("notes/Call 2026.md"), "Call 2026");
        assert_eq!(file_stem("Top.md"), "Top");
    }
}
