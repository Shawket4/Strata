//! Appending block IDs — the only automated edit Strata makes to a body (PLAN §6.3). The prose
//! is never changed: the ID is added after the block's last character (or on its own line for
//! structured blocks, as Obsidian requires).

use crate::body::{self, Block, BlockKind};
use crate::line::LineEnding;

/// Why a block ID could not be appended.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BlockIdError {
    /// IDs must match `[a-z0-9-]+`.
    #[error("invalid block id `{0}`: use lowercase letters, digits and `-`")]
    InvalidId(String),
    /// No block starts at the given offset.
    #[error("no block starts at byte {0}")]
    NoBlockAt(usize),
    /// The block already has an ID.
    #[error("block already has id `{0}`")]
    AlreadyHasId(String),
    /// Headings are cited with `#Heading` links, not block IDs.
    #[error("headings cannot carry block ids")]
    Heading,
    /// The ID is already used by another block of the note.
    #[error("block id `{0}` is already used in this note")]
    Duplicate(String),
}

/// Whether `id` is a valid block ID to write (`[a-z0-9-]+`).
pub fn is_valid_block_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// The result of appending an ID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Appended {
    /// The new body.
    pub body: String,
    /// Byte offset where text was inserted (in the old body).
    pub at: usize,
    /// The inserted text.
    pub inserted: String,
}

/// Appends `^id` to the block starting at byte `block_start` of `body` (a start offset from
/// [`crate::body::analyze`]). Paragraphs, list items and footnotes get ` ^id` at the end of
/// their last line; quotes, tables, code and HTML blocks get a blank line and `^id` after
/// them.
pub fn append_block_id(body: &str, block_start: usize, id: &str) -> Result<Appended, BlockIdError> {
    if !is_valid_block_id(id) {
        return Err(BlockIdError::InvalidId(id.to_owned()));
    }
    let analysis = body::analyze(body);
    if analysis.block_by_id(id).is_some() {
        return Err(BlockIdError::Duplicate(id.to_owned()));
    }
    let block = analysis
        .blocks
        .iter()
        .find(|b| b.span.start == block_start)
        .ok_or(BlockIdError::NoBlockAt(block_start))?;
    append_to(body, block, id)
}

/// Appends `^id` to `block` (which must come from analysing `body`).
pub fn append_to(body: &str, block: &Block, id: &str) -> Result<Appended, BlockIdError> {
    if !is_valid_block_id(id) {
        return Err(BlockIdError::InvalidId(id.to_owned()));
    }
    if let Some(existing) = &block.id {
        return Err(BlockIdError::AlreadyHasId(existing.id.clone()));
    }
    if block.kind == BlockKind::Heading {
        return Err(BlockIdError::Heading);
    }
    let at = block.span.end;
    let inserted = if block.kind.id_on_own_line() {
        let eol = LineEnding::detect(body).as_str();
        format!("{eol}{eol}^{id}")
    } else {
        format!(" ^{id}")
    };
    let mut out = String::with_capacity(body.len() + inserted.len());
    out.push_str(&body[..at]);
    out.push_str(&inserted);
    out.push_str(&body[at..]);
    Ok(Appended {
        body: out,
        at,
        inserted,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_to_paragraph_without_touching_prose() {
        let body = "## H\n\nPrefers weekly invoicing.  \nSecond line\n\nNext\n";
        let a = body::analyze(body);
        let start = a.blocks[1].span.start;
        let r = append_block_id(body, start, "a1b2");
        assert_eq!(
            r.map(|r| r.body),
            Ok("## H\n\nPrefers weekly invoicing.  \nSecond line ^a1b2\n\nNext\n".to_owned())
        );
    }

    #[test]
    fn appends_to_list_item_before_nested_list() {
        let body = "- parent\n  - child\n";
        let r = append_block_id(body, 0, "p");
        assert_eq!(r.map(|r| r.body), Ok("- parent ^p\n  - child\n".to_owned()));
    }

    #[test]
    fn appends_after_structured_blocks() {
        let body = "> quote\r\n> more\r\n\r\nafter\r\n";
        let r = append_block_id(body, 0, "q");
        assert_eq!(
            r.map(|r| r.body),
            Ok("> quote\r\n> more\r\n\r\n^q\r\n\r\nafter\r\n".to_owned())
        );
        let body = "| a |\n|---|\n| 1 |";
        let r = append_block_id(body, 0, "t");
        let new = r.map(|r| r.body).unwrap_or_default();
        assert_eq!(new, "| a |\n|---|\n| 1 |\n\n^t");
        assert_eq!(
            body::analyze(&new).blocks[0]
                .id
                .as_ref()
                .map(|b| b.id.as_str()),
            Some("t")
        );
    }

    #[test]
    fn errors() {
        let body = "# H\n\npara ^x\n\nother\n";
        assert_eq!(
            append_block_id(body, 0, "Bad"),
            Err(BlockIdError::InvalidId("Bad".into()))
        );
        assert_eq!(append_block_id(body, 0, "y"), Err(BlockIdError::Heading));
        assert_eq!(
            append_block_id(body, 5, "y"),
            Err(BlockIdError::AlreadyHasId("x".into()))
        );
        assert_eq!(
            append_block_id(body, 2, "y"),
            Err(BlockIdError::NoBlockAt(2))
        );
        assert_eq!(
            append_block_id(body, 14, "x"),
            Err(BlockIdError::Duplicate("x".into()))
        );
        assert_eq!(
            append_block_id(body, 14, "y").map(|r| r.body),
            Ok("# H\n\npara ^x\n\nother ^y\n".into())
        );
    }

    #[test]
    fn appended_id_is_detected() {
        let body = "text with [[link]] and `code`\n";
        let r = append_block_id(body, 0, "c1d2")
            .map(|r| r.body)
            .unwrap_or_default();
        let a = body::analyze(&r);
        assert_eq!(a.block_by_id("c1d2").map(|b| b.span.clone()), Some(0..35));
    }
}
