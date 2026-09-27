//! Heading-aware chunking of a note body for embeddings and citations (PLAN §9.1b, §9.2
//! `embed`, §9.5).
//!
//! Units are the body's citable blocks from `vault-format` ([`vault_format::body::analyze`]:
//! paragraphs, list items, quotes, tables, code, footnotes; headings only delimit sections).
//! Consecutive units are packed greedily into chunks of about [`MIN_TOKENS`]–[`MAX_TOKENS`]
//! estimated tokens:
//!
//! - a chunk never exceeds [`MAX_TOKENS`] unless a single unit does;
//! - a new section (the unit's heading path differs) starts a new chunk once the current one
//!   has at least [`MIN_TOKENS`], so small sections merge but a full chunk never straddles a
//!   heading;
//! - a unit larger than [`MAX_TOKENS`] is split at line, then sentence, then word boundaries
//!   into pieces that each become a chunk.
//!
//! A chunk is cited through its first block (`[[Note#^id]]`): [`ChunkDraft::block_id`] is that
//! block's ID when it has one, and its text is the first [`ChunkDraft::anchor_len`] bytes of
//! [`ChunkDraft::text`] (0 for a piece of a split block, which is found by offset instead).
//! A body without any block (empty, or headings only) yields one chunk with empty text, so
//! every note still gets a vector from its title.

use vault_format::body::{self, BlockKind};

/// A chunk grows to at least this many estimated tokens before a heading ends it.
pub const MIN_TOKENS: usize = 300;
/// A chunk holds at most this many estimated tokens (unless one unit is larger and splits).
pub const MAX_TOKENS: usize = 500;

/// One chunk of a body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkDraft {
    /// Position in the note (0-based).
    pub ord: usize,
    /// Headings enclosing the first unit, joined with `/` (like `blocks.heading_path`).
    pub heading_path: String,
    /// Body byte offset where the chunk's first unit starts.
    pub start: usize,
    /// Body byte offset where its last unit ends.
    pub end: usize,
    /// Byte length of the first block's text at the start of `text` (0: a split piece).
    pub anchor_len: usize,
    /// The first block's existing `^id`.
    pub block_id: Option<String>,
    /// The units' texts joined with a blank line (block IDs excluded).
    pub text: String,
    /// Estimated tokens of `text`.
    pub token_count: usize,
}

impl ChunkDraft {
    /// What is embedded: the note title and heading path give the chunk its context.
    pub fn embed_input(&self, title: &str) -> String {
        let mut s = String::with_capacity(title.len() + self.heading_path.len() + self.text.len() + 2);
        s.push_str(title);
        if !self.heading_path.is_empty() {
            s.push('\n');
            s.push_str(&self.heading_path);
        }
        if !self.text.is_empty() {
            s.push('\n');
            s.push_str(&self.text);
        }
        s
    }
}

/// Estimated tokens of `text` for a multilingual subword tokenizer: per word, one token per
/// four characters of ASCII and per three characters otherwise (at least one per word).
/// Deterministic; only chunk sizes depend on it.
pub fn estimate_tokens(text: &str) -> usize {
    text.split_whitespace()
        .map(|w| {
            let (ascii, other) = w.chars().fold((0usize, 0usize), |(a, o), c| {
                if c.is_ascii() { (a + 1, o) } else { (a, o + 1) }
            });
            (ascii.div_ceil(4) + other.div_ceil(3)).max(1)
        })
        .sum()
}

#[derive(Debug, Clone)]
struct Unit {
    start: usize,
    end: usize,
    heading_path: String,
    block_id: Option<String>,
    /// Whole block (anchorable by its text) or a piece of a split block.
    whole: bool,
    tokens: usize,
}

/// Splits `text` (at body offset `base`) into pieces of at most `max` estimated tokens.
fn split_ranges(text: &str, base: usize, max: usize) -> Vec<(usize, usize)> {
    // Candidate cut points: after line ends, then after sentence ends, then after words.
    let mut pieces: Vec<(usize, usize)> = Vec::new();
    let mut start = 0;
    let mut last_good: Option<usize> = None;
    let bytes: Vec<(usize, char)> = text.char_indices().collect();
    let mut i = 0;
    while i < bytes.len() {
        let (pos, c) = bytes[i];
        let next = bytes.get(i + 1).map_or(text.len(), |(p, _)| *p);
        let boundary = c == '\n'
            || (matches!(c, '.' | '!' | '?' | '؟' | '۔' | '。')
                && text[next..].starts_with([' ', '\n']))
            || c.is_whitespace();
        if boundary {
            let cut = next;
            if estimate_tokens(&text[start..cut]) > max {
                let at = last_good.filter(|g| *g > start).unwrap_or(cut);
                pieces.push((start, at));
                start = at;
                last_good = None;
                // Re-examine from the new start.
                if at < cut {
                    continue_from(&bytes, at, &mut i);
                    continue;
                }
            } else {
                last_good = Some(cut);
            }
        }
        let _ = pos;
        i += 1;
    }
    if start < text.len() {
        pieces.push((start, text.len()));
    }
    pieces
        .into_iter()
        .map(|(s, e)| {
            // Trim whitespace at the piece edges.
            let piece = &text[s..e];
            let lead = piece.len() - piece.trim_start().len();
            let trail = piece.len() - piece.trim_end().len();
            (base + s + lead, base + e - trail)
        })
        .filter(|(s, e)| e > s)
        .collect()
}

fn continue_from(bytes: &[(usize, char)], at: usize, i: &mut usize) {
    *i = bytes.partition_point(|(p, _)| *p < at);
}

fn units(body_text: &str) -> Vec<Unit> {
    let analysis = body::analyze(body_text);
    let mut out = Vec::new();
    for b in &analysis.blocks {
        if b.kind == BlockKind::Heading {
            continue;
        }
        let text = &body_text[b.span.clone()];
        if text.trim().is_empty() {
            continue;
        }
        let heading_path = b.heading_path.join("/");
        let tokens = estimate_tokens(text);
        if tokens <= MAX_TOKENS {
            out.push(Unit {
                start: b.span.start,
                end: b.span.end,
                heading_path,
                block_id: b.id.as_ref().map(|i| i.id.clone()),
                whole: true,
                tokens,
            });
            continue;
        }
        for (i, (s, e)) in split_ranges(text, b.span.start, MAX_TOKENS)
            .into_iter()
            .enumerate()
        {
            out.push(Unit {
                start: s,
                end: e,
                heading_path: heading_path.clone(),
                block_id: b.id.as_ref().map(|id| id.id.clone()),
                whole: i == 0 && s == b.span.start && e == b.span.end,
                tokens: estimate_tokens(&body_text[s..e]),
            });
        }
    }
    out
}

/// Chunks `body_text` (a note body, frontmatter excluded).
pub fn chunk_body(body_text: &str) -> Vec<ChunkDraft> {
    let mut chunks: Vec<ChunkDraft> = Vec::new();
    let mut current: Vec<Unit> = Vec::new();
    let mut tokens = 0;
    let flush = |current: &mut Vec<Unit>, tokens: &mut usize, chunks: &mut Vec<ChunkDraft>| {
        if current.is_empty() {
            return;
        }
        let first = &current[0];
        let texts: Vec<&str> = current.iter().map(|u| &body_text[u.start..u.end]).collect();
        let text = texts.join("\n\n");
        chunks.push(ChunkDraft {
            ord: chunks.len(),
            heading_path: first.heading_path.clone(),
            start: first.start,
            end: current.last().map_or(first.end, |u| u.end),
            anchor_len: if first.whole { first.end - first.start } else { 0 },
            block_id: first.block_id.clone(),
            token_count: estimate_tokens(&text),
            text,
        });
        current.clear();
        *tokens = 0;
    };
    for unit in units(body_text) {
        let new_section = current
            .last()
            .is_some_and(|u| u.heading_path != unit.heading_path);
        let piece = !unit.whole;
        if !current.is_empty()
            && (tokens + unit.tokens > MAX_TOKENS || (new_section && tokens >= MIN_TOKENS) || piece)
        {
            flush(&mut current, &mut tokens, &mut chunks);
        }
        tokens += unit.tokens;
        current.push(unit);
        if piece {
            flush(&mut current, &mut tokens, &mut chunks);
        }
    }
    flush(&mut current, &mut tokens, &mut chunks);
    if chunks.is_empty() {
        chunks.push(ChunkDraft {
            ord: 0,
            heading_path: String::new(),
            start: 0,
            end: 0,
            anchor_len: 0,
            block_id: None,
            text: String::new(),
            token_count: 0,
        });
    }
    chunks
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn token_estimates_count_words_by_script() {
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("a bb ccccc"), 1 + 1 + 2);
        // "عقد" (3 letters) = 1, "وطنية" (5) = 2, "safe" = 1.
        assert_eq!(estimate_tokens("عقد وطنية safe"), 4);
    }

    #[test]
    fn empty_and_heading_only_bodies_give_one_empty_chunk() {
        let empty = ChunkDraft {
            ord: 0,
            heading_path: String::new(),
            start: 0,
            end: 0,
            anchor_len: 0,
            block_id: None,
            text: String::new(),
            token_count: 0,
        };
        assert_eq!(chunk_body(""), vec![empty.clone()]);
        assert_eq!(chunk_body("# Only\n\n## Headings\n"), vec![empty.clone()]);
        assert_eq!(empty.embed_input("Title"), "Title");
    }

    #[test]
    fn small_sections_merge_and_keep_the_first_block_as_anchor() {
        let body = "Intro line ^intro\n\n## Pricing\n\nWeekly invoicing.\n\n- item one\n";
        let chunks = chunk_body(body);
        assert_eq!(chunks.len(), 1);
        let c = &chunks[0];
        assert_eq!(c.text, "Intro line\n\nWeekly invoicing.\n\n- item one");
        assert_eq!((c.start, c.anchor_len), (0, "Intro line".len()));
        assert_eq!(&body[c.start..c.start + c.anchor_len], "Intro line");
        assert_eq!(c.block_id.as_deref(), Some("intro"));
        assert_eq!(c.heading_path, "");
        assert_eq!(c.end, body.find("- item one").expect("item") + "- item one".len());
        assert_eq!(
            c.embed_input("Acme"),
            "Acme\nIntro line\n\nWeekly invoicing.\n\n- item one"
        );
    }

    /// A paragraph of `n` ASCII words of four letters (one token each).
    fn para(word: &str, n: usize) -> String {
        vec![word; n].join(" ")
    }

    #[test]
    fn full_chunks_end_at_headings_and_never_exceed_the_maximum() {
        let a = para("aaaa", 200);
        let b = para("bbbb", 200);
        let c = para("cccc", 150);
        let d = para("dddd", 50);
        let body = format!("# One\n\n{a}\n\n{b}\n\n{c}\n\n## Two\n\n{d}\n");
        let chunks = chunk_body(&body);
        let summary: Vec<(usize, &str, usize)> = chunks
            .iter()
            .map(|c| (c.ord, c.heading_path.as_str(), c.token_count))
            .collect();
        // 200 + 200 = 400; + 150 > 500 → new chunk; c (150) < 300 so section Two (50) merges.
        assert_eq!(summary, vec![(0, "One", 400), (1, "One", 200)]);
        assert_eq!(chunks[1].text, format!("{c}\n\n{d}"));
        assert!(chunks.iter().all(|c| c.token_count <= MAX_TOKENS));

        let body = format!("# One\n\n{a}\n\n{b}\n\n## Two\n\n{d}\n");
        let summary: Vec<(&str, usize)> = chunk_body(&body)
            .iter()
            .map(|c| (c.heading_path.as_str(), c.token_count))
            .collect();
        // 400 ≥ 300 at the heading: Two starts its own chunk.
        assert_eq!(summary, vec![("One", 400), ("One/Two", 50)]);
    }

    #[test]
    fn oversized_blocks_split_into_pieces_cited_by_offset() {
        let big = para("word", 1200);
        let body = format!("Lead ^lead\n\n{big} ^big\n");
        let chunks = chunk_body(&body);
        let summary: Vec<(usize, usize, Option<&str>)> = chunks
            .iter()
            .map(|c| (c.token_count, c.anchor_len, c.block_id.as_deref()))
            .collect();
        assert_eq!(
            summary,
            vec![
                (1, 4, Some("lead")),
                (500, 0, Some("big")),
                (500, 0, Some("big")),
                (200, 0, Some("big"))
            ]
        );
        // Pieces tile the block without overlap, at word boundaries.
        let start = body.find("word").expect("big");
        assert_eq!(chunks[1].start, start);
        assert_eq!(chunks[2].start, chunks[1].end + 1);
        assert!(chunks[1..].iter().all(|c| c.text.split(' ').all(|w| w == "word")));
    }

    #[test]
    fn arabic_and_mixed_notes_chunk_by_blocks_with_their_ids() {
        let body = "## العقد\n\nعقد وطنية في الخزنة ^a1\n\nThe contract is in the safe. ^b2\n";
        let chunks = chunk_body(body);
        assert_eq!(chunks.len(), 1);
        assert_eq!(
            (chunks[0].heading_path.as_str(), chunks[0].block_id.as_deref()),
            ("العقد", Some("a1"))
        );
        assert_eq!(
            &body[chunks[0].start..chunks[0].start + chunks[0].anchor_len],
            "عقد وطنية في الخزنة"
        );
    }
}
