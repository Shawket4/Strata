//! Username and display-name rules (PLAN §15: usernames are normalised — case, Unicode
//! confusables — before uniqueness checks).
//!
//! 1. **NFKC case folding**: `NFKC(case_fold(NFKC(input)))`, the UAX #31 identifier form, so
//!    `Ahmed`, `ＡＨＭＥＤ` (fullwidth) and `ahmed` are one name.
//! 2. **Validation** on the folded form: 3–32 characters; letters, marks and digits of any
//!    script plus `.`, `_` and `-`; must start with a letter or digit.
//! 3. **Confusable skeleton** (UTS #39 §4, `unicode-security`): the key stored in
//!    `users.username_normalized` (unique). Two names with the same skeleton — `ahmed` and
//!    `аhmed` with a Cyrillic `а` — can't both exist, and a login matches by skeleton.
//!
//! The username as typed (trimmed) is kept for display.

use unicode_normalization::UnicodeNormalization;

/// Shortest username, in characters (after folding).
pub const MIN_USERNAME_CHARS: usize = 3;
/// Longest username, in characters (after folding).
pub const MAX_USERNAME_CHARS: usize = 32;
/// Longest display name, in characters.
pub const MAX_DISPLAY_NAME_CHARS: usize = 100;

/// Why a username or display name is not acceptable. Content-free.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum NameError {
    /// Too short or too long.
    #[error("usernames have 3 to 32 characters")]
    UsernameLength,
    /// A character outside letters, marks, digits, `.`, `_`, `-`.
    #[error("usernames may contain only letters, digits, '.', '_' and '-'")]
    UsernameCharacters,
    /// Must start with a letter or digit.
    #[error("usernames must start with a letter or digit")]
    UsernameStart,
    /// Empty, too long, or with control characters.
    #[error("display names have 1 to 100 characters and no control characters")]
    DisplayName,
}

/// A validated username.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Username {
    /// As entered (trimmed), for display.
    pub display: String,
    /// The confusable skeleton of the folded form: the uniqueness and lookup key.
    pub key: String,
}

/// NFKC case folding (`NFKC(case_fold(NFKC(s)))`).
pub fn fold(s: &str) -> String {
    let nfkc: String = s.nfkc().collect();
    caseless::default_case_fold_str(&nfkc).nfkc().collect()
}

/// The UTS #39 skeleton of the folded form of `s`.
pub fn skeleton(s: &str) -> String {
    unicode_security::skeleton(&fold(s)).collect()
}

/// Validates and normalises a username.
pub fn parse_username(input: &str) -> Result<Username, NameError> {
    let display = input.trim();
    let folded = fold(display);
    let len = folded.chars().count();
    if !(MIN_USERNAME_CHARS..=MAX_USERNAME_CHARS).contains(&len) {
        return Err(NameError::UsernameLength);
    }
    if !folded.chars().all(is_username_char) {
        return Err(NameError::UsernameCharacters);
    }
    if !folded.chars().next().is_some_and(char::is_alphanumeric) {
        return Err(NameError::UsernameStart);
    }
    Ok(Username {
        display: display.to_owned(),
        key: unicode_security::skeleton(&folded).collect(),
    })
}

/// The lookup key for a login attempt: the skeleton, without validation (so a malformed
/// name simply matches no account).
pub fn login_key(input: &str) -> String {
    skeleton(input.trim())
}

fn is_username_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '.' | '_' | '-') || is_mark(c)
}

/// Combining marks (Arabic harakat, Latin accents after NFKC) are allowed inside names.
fn is_mark(c: char) -> bool {
    use unicode_normalization::char::canonical_combining_class;
    canonical_combining_class(c) != 0
}

/// Validates a display name (trimmed; 1–100 characters; no control characters).
pub fn parse_display_name(input: &str) -> Result<String, NameError> {
    let trimmed = input.trim();
    let len = trimmed.chars().count();
    if len == 0 || len > MAX_DISPLAY_NAME_CHARS || trimmed.chars().any(char::is_control) {
        return Err(NameError::DisplayName);
    }
    Ok(trimmed.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn case_and_width_variants_share_a_key() {
        let a = parse_username("Ahmed").expect("valid");
        let b = parse_username("ＡＨＭＥＤ").expect("valid");
        let c = parse_username("  ahmed ").expect("valid");
        assert_eq!(a.key, b.key);
        assert_eq!(a.key, c.key);
        assert_eq!(a.display, "Ahmed");
        assert_eq!(c.display, "ahmed");
    }

    #[test]
    fn cyrillic_lookalikes_share_the_skeleton() {
        let latin = parse_username("ahmed").expect("valid");
        let mixed = parse_username("\u{0430}hmed").expect("valid"); // Cyrillic а
        assert_eq!(latin.key, mixed.key);
        // An all-Cyrillic spoof of "ace".
        let cyrillic = parse_username("\u{0430}\u{0441}\u{0435}").expect("valid");
        assert_eq!(cyrillic.key, parse_username("ace").expect("valid").key);
        assert_eq!(login_key("\u{0410}HMED"), latin.key);
    }

    #[test]
    fn distinct_names_keep_distinct_keys() {
        let a = parse_username("ahmed").expect("valid");
        let b = parse_username("ahmad").expect("valid");
        let arabic = parse_username("أحمد").expect("valid");
        assert_ne!(a.key, b.key);
        assert_ne!(a.key, arabic.key);
        assert_eq!(arabic.display, "أحمد");
    }

    #[test]
    fn rules_are_enforced() {
        assert_eq!(parse_username("ab"), Err(NameError::UsernameLength));
        assert_eq!(
            parse_username(&"a".repeat(33)),
            Err(NameError::UsernameLength)
        );
        assert_eq!(
            parse_username("bad name"),
            Err(NameError::UsernameCharacters)
        );
        assert_eq!(parse_username("a/b.c"), Err(NameError::UsernameCharacters));
        assert_eq!(parse_username("_ahmed"), Err(NameError::UsernameStart));
        assert!(parse_username("sam.ali-2_x").is_ok());
        assert!(parse_username("مُحَمَّد").is_ok());
    }

    #[test]
    fn display_names_are_trimmed_and_bounded() {
        assert_eq!(parse_display_name("  Ahmed  "), Ok("Ahmed".to_owned()));
        assert_eq!(parse_display_name("   "), Err(NameError::DisplayName));
        assert_eq!(parse_display_name("a\u{7}b"), Err(NameError::DisplayName));
        assert_eq!(
            parse_display_name(&"x".repeat(101)),
            Err(NameError::DisplayName)
        );
        assert_eq!(parse_display_name("أحمد سمير"), Ok("أحمد سمير".to_owned()));
    }
}
