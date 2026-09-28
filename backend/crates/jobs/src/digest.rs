//! The weekly `digest` job (PLAN §9.2): `_ai/digests/YYYY-Www.md` summarising the week's new
//! notes, the open items of entity pages and the contradictions the AI found, every bullet
//! cited. The week is the seven days before the run (in the user's time zone); the note is
//! named after the ISO week of the period's last day. Rerunning a week replaces the note's
//! body; nothing is written for a week without new notes, open items or contradictions. One
//! `ai: digest _ai/digests/<week>.md` commit.

use std::collections::BTreeMap;
use std::sync::Arc;

use chrono::{DateTime, Datelike, Duration, NaiveDate, TimeZone, Utc};
use chrono_tz::Tz;
use serde::Serialize;
use strata_ai::outputs::{CitedItem, Digest};
use strata_ai::prompts::{self, ids};
use strata_ai::{AiCaller, AiError, AiService};
use strata_common::{Clock, NoteId};
use strata_index::{AppDb, UserScope};
use strata_vault::VaultService;
use strata_vault::ops::ai_apply::{AiApplied, AiChangeSet, AiNoteWrite, BlockIdRequest};
use vault_format::{Document, PathIndex, Resolution, WikiLink};

use crate::handler::{JobClass, JobContext, JobError, JobHandler};
use crate::pipeline::{BlockInput, SourceNote};

/// Job kind `digest`.
pub const DIGEST: &str = "digest";
/// New notes given to the model.
pub const MAX_NOTES: usize = 40;
/// Blocks per new note.
pub const MAX_BLOCKS: usize = 8;

/// Output token limit.
const MAX_TOKENS: u32 = 3000;

/// The period of a digest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PeriodInput {
    /// First day.
    pub start: String,
    /// Last day.
    pub end: String,
    /// ISO week of the last day (`2026-W39`).
    pub week: String,
}

/// A new note in a digest prompt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DigestNote {
    /// ID.
    pub id: String,
    /// Title.
    pub title: String,
    /// `created`.
    pub created: String,
    /// Sidecar summary.
    pub summary: Option<String>,
    /// Key blocks.
    pub blocks: Vec<BlockInput>,
}

/// A citation in a digest prompt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CitationInput {
    /// Note.
    pub note_id: String,
    /// Block.
    pub block_id: String,
}

/// An open item in a digest prompt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OpenItemInput {
    /// Entity.
    pub entity_name: String,
    /// Text.
    pub text: String,
    /// Where it comes from.
    pub citation: CitationInput,
}

/// One side of a contradiction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SideInput {
    /// Note.
    pub note_id: String,
    /// Title.
    pub title: String,
}

/// A contradiction in a digest prompt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ContradictionInput {
    /// Source.
    pub source: SideInput,
    /// Target.
    pub target: SideInput,
    /// The AI's reason.
    pub reason: String,
}

/// The `digest` prompt input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DigestInput {
    /// Period.
    pub period: PeriodInput,
    /// New notes.
    pub new_notes: Vec<DigestNote>,
    /// Open items.
    pub open_items: Vec<OpenItemInput>,
    /// Contradictions.
    pub contradictions: Vec<ContradictionInput>,
}

/// The digest period ending on the day before `today`: `(start, end, "YYYY-Www")`.
pub fn period(today: NaiveDate) -> (NaiveDate, NaiveDate, String) {
    let end = today - Duration::days(1);
    let start = end - Duration::days(6);
    let w = end.iso_week();
    (start, end, format!("{}-W{:02}", w.year(), w.week()))
}

/// The `digest` job.
#[derive(Debug, Clone)]
pub struct DigestHandler {
    db: AppDb,
    vault: VaultService,
    ai: Arc<AiService>,
    clock: Arc<dyn Clock>,
    default_tz: Tz,
}

impl DigestHandler {
    /// The handler; days are counted in the user's `timezone` setting, else `default_tz`.
    pub fn new(
        db: AppDb,
        vault: VaultService,
        ai: Arc<AiService>,
        clock: Arc<dyn Clock>,
        default_tz: Tz,
    ) -> Self {
        Self {
            db,
            vault,
            ai,
            clock,
            default_tz,
        }
    }

    async fn tz(&self, scope: &UserScope) -> Result<Tz, JobError> {
        let mut tx = self.db.begin(scope).await?;
        let v = strata_index::repo::settings::get_setting(&mut tx, "timezone").await?;
        tx.commit().await?;
        Ok(v.and_then(|b| rmp_serde::from_slice::<String>(&b).ok())
            .and_then(|n| n.parse::<Tz>().ok())
            .unwrap_or(self.default_tz))
    }
}

fn day_start(tz: Tz, d: NaiveDate) -> DateTime<Utc> {
    tz.from_local_datetime(&d.and_hms_opt(0, 0, 0).unwrap_or_default())
        .earliest()
        .map_or_else(|| d.and_hms_opt(0, 0, 0).unwrap_or_default().and_utc(), |t| {
            t.with_timezone(&Utc)
        })
}

/// Bullets of an AI section with the note and block of their first citation.
fn cited_bullets(section: &str) -> Vec<(String, WikiLink)> {
    section
        .lines()
        .filter_map(|l| l.strip_prefix("- "))
        .filter_map(|item| {
            let links = vault_format::wikilink::find_all(item);
            let first = links.into_iter().find(|l| !l.embed)?;
            let text = item
                .find("[[")
                .map_or(item, |i| &item[..i])
                .trim()
                .to_owned();
            Some((text, first))
        })
        .collect()
}

#[async_trait::async_trait]
impl JobHandler for DigestHandler {
    fn kind(&self) -> &'static str {
        DIGEST
    }

    fn class(&self) -> JobClass {
        JobClass::Llm
    }

    #[allow(clippy::too_many_lines)] // one linear pass: gather, call, validate, render, write
    async fn run(&self, ctx: JobContext) -> Result<(), JobError> {
        let tz = self.tz(&ctx.scope).await?;
        let today = self.clock.now().with_timezone(&tz).date_naive();
        let (start, end, week) = period(today);
        let from = day_start(tz, start);
        let to = day_start(tz, end + Duration::days(1));
        let mut tx = self.db.begin(&ctx.scope).await?;
        let new_ids: Vec<(NoteId,)> = sqlx::query_as(
            "SELECT id FROM notes WHERE kind = 'note' AND NOT trashed \
               AND path NOT LIKE '\\_ai/%' \
               AND ((created >= $1 AND created < $2) OR (updated >= $1 AND updated < $2)) \
             ORDER BY created DESC, id DESC LIMIT $3",
        )
        .bind(from)
        .bind(to)
        .bind(i64::try_from(MAX_NOTES).unwrap_or(40))
        .fetch_all(tx.conn())
        .await?;
        let contradictions: Vec<(NoteId, NoteId, Option<String>)> = sqlx::query_as(
            "SELECT r.src_id, r.dst_id, r.reason FROM relations r \
             JOIN notes s ON s.user_id = r.user_id AND s.id = r.src_id AND NOT s.trashed \
             JOIN notes t ON t.user_id = r.user_id AND t.id = r.dst_id AND NOT t.trashed \
             WHERE r.type = 'contradicts' AND r.by = 'ai' AND r.created >= $1 AND r.created < $2 \
             ORDER BY r.created, r.src_id, r.dst_id",
        )
        .bind(from)
        .bind(to)
        .fetch_all(tx.conn())
        .await?;
        let entity_ids: Vec<(NoteId, String)> = sqlx::query_as(
            "SELECT e.note_id, e.display_name FROM entities e JOIN notes n \
               ON n.user_id = e.user_id AND n.id = e.note_id \
             WHERE NOT n.trashed AND e.kind IN ('person', 'company') \
             ORDER BY e.display_name, e.note_id",
        )
        .fetch_all(tx.conn())
        .await?;
        let paths: Vec<(String, NoteId, String)> =
            sqlx::query_as("SELECT path, id, title FROM notes WHERE NOT trashed")
                .fetch_all(tx.conn())
                .await?;
        tx.commit().await?;
        let index = PathIndex::new(paths.iter().map(|p| p.0.clone()));
        let by_path: BTreeMap<&str, (NoteId, &str)> = paths
            .iter()
            .map(|(p, i, t)| (p.as_str(), (*i, t.as_str())))
            .collect();
        let title_of = |id: NoteId| {
            paths
                .iter()
                .find(|p| p.1 == id)
                .map(|p| p.2.clone())
                .unwrap_or_default()
        };

        let mut notes: Vec<SourceNote> = Vec::new();
        for (id,) in new_ids {
            if let Some(n) = SourceNote::load(&self.vault, &ctx.scope, id).await? {
                notes.push(n);
            }
        }
        let mut open_items = Vec::new();
        for (id, name) in &entity_ids {
            let Ok(view) = self.vault.note(&ctx.scope, *id).await else {
                continue;
            };
            let doc = Document::parse(&view.content);
            let Some(section) =
                strata_vault::ops::entities::section_text(doc.body(), "Open items")
            else {
                continue;
            };
            for (text, link) in cited_bullets(&section) {
                let Resolution::Resolved(p) = index.resolve(&link.path, Some(&view.path)) else {
                    continue;
                };
                let Some((note, _)) = by_path.get(p.as_str()) else {
                    continue;
                };
                let block = match &link.anchor {
                    Some(vault_format::Anchor::Block(b)) => b.clone(),
                    _ => continue,
                };
                open_items.push(OpenItemInput {
                    entity_name: name.clone(),
                    text,
                    citation: CitationInput {
                        note_id: note.to_string(),
                        block_id: block,
                    },
                });
            }
        }
        let contradictions: Vec<ContradictionInput> = contradictions
            .into_iter()
            .map(|(s, t, reason)| ContradictionInput {
                source: SideInput {
                    note_id: s.to_string(),
                    title: title_of(s),
                },
                target: SideInput {
                    note_id: t.to_string(),
                    title: title_of(t),
                },
                reason: reason.unwrap_or_default(),
            })
            .collect();
        if notes.is_empty() && open_items.is_empty() && contradictions.is_empty() {
            return Ok(());
        }
        let input = DigestInput {
            period: PeriodInput {
                start: start.to_string(),
                end: end.to_string(),
                week: week.clone(),
            },
            new_notes: notes
                .iter()
                .map(|n| DigestNote {
                    id: n.id.to_string(),
                    title: n.title.clone(),
                    created: n.created_rfc3339(),
                    summary: n.sidecar.summary.clone(),
                    blocks: n.blocks.iter().take(MAX_BLOCKS).cloned().collect(),
                })
                .collect(),
            open_items: open_items.clone(),
            contradictions: contradictions.clone(),
        };
        let prompt = prompts::latest(ids::DIGEST)
            .ok_or_else(|| JobError::Fatal("digest prompt missing".into()))?;
        let caller = AiCaller {
            scope: ctx.scope,
            username: ctx.username.clone(),
        };
        let out = match self
            .ai
            .complete::<Digest>(caller, prompt, &input, MAX_TOKENS)
            .await
        {
            Ok(o) => o.value,
            Err(AiError::Disabled | AiError::ProviderNotConfigured(_)) => return Ok(()),
            Err(e) => return Err(e.into()),
        };
        // Citations: a block of a new note, an open item's source, or a contradicting note.
        let mut known: BTreeMap<(String, String), Option<NoteId>> = BTreeMap::new();
        let mut generated: BTreeMap<(NoteId, String), String> = BTreeMap::new();
        for n in &notes {
            for b in n.blocks.iter().take(MAX_BLOCKS) {
                known.insert((n.id.to_string(), b.block_id.clone()), Some(n.id));
                if let Some(t) = n.generated.get(&b.block_id) {
                    generated.insert((n.id, b.block_id.clone()), t.clone());
                }
            }
        }
        for o in &open_items {
            if let Ok(id) = o.citation.note_id.parse::<NoteId>() {
                known.insert(
                    (o.citation.note_id.clone(), o.citation.block_id.clone()),
                    Some(id),
                );
            }
        }
        let sides: BTreeMap<String, NoteId> = contradictions
            .iter()
            .flat_map(|c| [&c.source.note_id, &c.target.note_id])
            .filter_map(|i| Some((i.clone(), i.parse().ok()?)))
            .collect();
        let link_of = |id: NoteId| {
            paths
                .iter()
                .find(|p| p.1 == id)
                .map(|p| index.link_text_for(&p.0))
        };
        let mut blocks_needed: Vec<BlockIdRequest> = Vec::new();
        // A contradicting note may be cited as a whole, in the contradictions section only.
        let mut render = |items: &[CitedItem], sides_ok: bool| -> String {
            let mut out = String::new();
            for it in items {
                let mut cites = Vec::new();
                for c in &it.citations {
                    let key = (c.note_id.clone(), c.block_id.clone());
                    let cite = if let Some(Some(id)) = known.get(&key) {
                        if let Some(text) = generated.get(&(*id, c.block_id.clone())) {
                            blocks_needed.push(BlockIdRequest {
                                note: *id,
                                block_text: text.clone(),
                                id: c.block_id.clone(),
                            });
                        }
                        link_of(*id).map(|l| format!("[[{l}#^{}]]", c.block_id))
                    } else if let Some(id) = sides.get(&c.note_id).filter(|_| sides_ok) {
                        link_of(*id).map(|l| format!("[[{l}]]"))
                    } else {
                        None
                    };
                    if let Some(c) = cite
                        && !cites.contains(&c)
                    {
                        cites.push(c);
                    }
                }
                if cites.is_empty() {
                    continue;
                }
                let text = it.text.split_whitespace().collect::<Vec<_>>().join(" ");
                out.push_str(&format!("- {text} {}\n", cites.join(" ")));
            }
            out
        };
        let highlights = render(&out.highlights, false);
        let open = render(&out.open_questions, false);
        let contra = render(&out.contradictions, true);
        let mut body = format!("Week {week}: {start} – {end}.\n");
        for (heading, content) in [
            ("Highlights", highlights),
            ("Open questions", open),
            ("Contradictions", contra),
        ] {
            if !content.is_empty() {
                body.push_str(&format!("\n## {heading}\n{content}"));
            }
        }
        let path = format!("_ai/digests/{week}.md");
        let id = match self.vault.note_by_path(&ctx.scope, &path).await {
            Ok(v) => v.id,
            Err(strata_vault::VaultError::NotFound) => {
                NoteId::from_ulid(ctx.job.id.as_ulid())
            }
            Err(e) => return Err(e.into()),
        };
        let mut set = AiChangeSet::new(DIGEST, id);
        set.job_id = Some(ctx.job.id);
        set.ai_notes.push(AiNoteWrite {
            id,
            path,
            title: out.title.trim().to_owned(),
            body,
        });
        blocks_needed.sort_by(|a, b| (a.note, &a.id).cmp(&(b.note, &b.id)));
        blocks_needed.dedup();
        set.block_ids = blocks_needed;
        match self.vault.ai_apply(&ctx.scope, set).await? {
            AiApplied::Done { .. } | AiApplied::Stale => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_period_is_the_seven_days_before_the_run() {
        let d = |s: &str| NaiveDate::parse_from_str(s, "%Y-%m-%d").expect("date");
        assert_eq!(
            period(d("2026-09-28")),
            (d("2026-09-21"), d("2026-09-27"), "2026-W39".to_owned())
        );
        assert_eq!(
            period(d("2027-01-04")),
            (d("2026-12-28"), d("2027-01-03"), "2026-W53".to_owned())
        );
    }

    #[test]
    fn cited_bullets_keep_text_and_first_link() {
        let b = cited_bullets("- Owes the signed copy [[Call#^a1]] [[Other]]\n- uncited\n");
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].0, "Owes the signed copy");
        assert_eq!(b[0].1.path, "Call");
    }
}
