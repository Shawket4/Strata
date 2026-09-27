//! Which completion the token before the caret asks for (`[[`, `[[Note#^`, `@`, `#`). Pure:
//! the view builder fills the items from the local database.

/// The token before the caret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    /// Nothing to complete.
    None,
    /// `[[query` (replace `start..end` = the query; `closed` when `]]` follows the caret).
    WikiLink {
        /// Query.
        query: String,
        /// Byte range to replace.
        start: usize,
        /// End.
        end: usize,
        /// `]]` follows the caret.
        closed: bool,
    },
    /// `[[Note#^query`.
    BlockRef {
        /// The note part.
        note: String,
        /// The block query.
        query: String,
        /// Byte range to replace (the block query).
        start: usize,
        /// End.
        end: usize,
        /// `]]` follows the caret.
        closed: bool,
    },
    /// `@query` (the range includes `@`).
    Mention {
        /// Query.
        query: String,
        /// Start (the `@`).
        start: usize,
        /// End.
        end: usize,
    },
    /// `#query` (the range is the query after `#`).
    Tag {
        /// Query.
        query: String,
        /// Start.
        start: usize,
        /// End.
        end: usize,
    },
}

fn tag_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '-' | '/')
}

/// The token ending at byte `cursor` of `text`.
pub fn token_at(text: &str, cursor: usize) -> Token {
    let mut cursor = cursor.min(text.len());
    while !text.is_char_boundary(cursor) {
        cursor -= 1;
    }
    let line_start = text[..cursor].rfind('\n').map_or(0, |i| i + 1);
    let before = &text[line_start..cursor];
    let closed = text[cursor..].starts_with("]]");
    // `[[…` not closed before the caret.
    if let Some(open) = before.rfind("[[")
        && !before[open..].contains("]]")
    {
        let q_start = line_start + open + 2;
        let q = &text[q_start..cursor];
        if q.contains('|') {
            return Token::None;
        }
        if let Some(hash) = q.find("#^") {
            return Token::BlockRef {
                note: q[..hash].to_owned(),
                query: q[hash + 2..].to_owned(),
                start: q_start + hash + 2,
                end: cursor,
                closed,
            };
        }
        if q.contains('#') {
            return Token::None;
        }
        return Token::WikiLink {
            query: q.to_owned(),
            start: q_start,
            end: cursor,
            closed,
        };
    }
    // `@query` / `#query`: the word before the caret.
    let word_start = before
        .char_indices()
        .rev()
        .find(|(_, c)| c.is_whitespace())
        .map_or(0, |(i, c)| i + c.len_utf8());
    let word = &before[word_start..];
    if let Some(q) = word.strip_prefix('@')
        && !q.contains('@')
    {
        return Token::Mention {
            query: q.to_owned(),
            start: line_start + word_start,
            end: cursor,
        };
    }
    if let Some(q) = word.strip_prefix('#')
        && q.chars().all(tag_char)
        // `# heading` is not a tag; `#` alone at the line start is (typing a tag).
        && !(word_start == 0 && q.is_empty() && text[cursor..].starts_with(' '))
    {
        return Token::Tag {
            query: q.to_owned(),
            start: line_start + word_start + 1,
            end: cursor,
        };
    }
    Token::None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at_end(text: &str) -> Token {
        token_at(text, text.len())
    }

    #[test]
    fn detects_tokens() {
        assert_eq!(
            at_end("See [[Acm"),
            Token::WikiLink {
                query: "Acm".into(),
                start: 6,
                end: 9,
                closed: false
            }
        );
        assert_eq!(
            token_at("See [[Acm]] now", 9),
            Token::WikiLink {
                query: "Acm".into(),
                start: 6,
                end: 9,
                closed: true
            }
        );
        assert_eq!(
            at_end("[[Call 2026#^a1"),
            Token::BlockRef {
                note: "Call 2026".into(),
                query: "a1".into(),
                start: 13,
                end: 15,
                closed: false
            }
        );
        assert_eq!(
            at_end("call @أحم"),
            Token::Mention {
                query: "أحم".into(),
                start: 5,
                end: 12
            }
        );
        assert_eq!(
            at_end("done #sal"),
            Token::Tag {
                query: "sal".into(),
                start: 6,
                end: 9
            }
        );
        assert_eq!(at_end("[[Done]] and"), Token::None);
        assert_eq!(at_end("[[Note|ali"), Token::None);
        assert_eq!(at_end("mail a@b"), Token::None);
        assert_eq!(at_end("plain"), Token::None);
        assert_eq!(token_at("# Heading", 1), Token::None);
    }
}
