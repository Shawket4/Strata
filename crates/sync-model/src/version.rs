//! Versions: the content hash of a note (or of a task line), `sha256:<64 lowercase hex>`.
//!
//! The same spelling is used for `If-Match`, `base_version`, `notes.content_hash` and the
//! sidecar's `content_hash` (§6.5), so a version computed on the device equals the one the
//! server computes for the same bytes.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const PREFIX: &str = "sha256:";

/// A content-hash version.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Version(String);

/// A string that is not `sha256:` followed by 64 lowercase hex digits.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid version {0:?}: expected `sha256:` and 64 lowercase hex digits")]
pub struct InvalidVersion(pub String);

impl Version {
    /// The version of `bytes` (a file's exact bytes, BOM and line endings included).
    pub fn of(bytes: &[u8]) -> Self {
        Self(format!("{PREFIX}{}", hex::encode(Sha256::digest(bytes))))
    }

    /// The version of a UTF-8 text.
    pub fn of_text(text: &str) -> Self {
        Self::of(text.as_bytes())
    }

    /// The canonical string.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether `text` has this version.
    pub fn matches(&self, text: &str) -> bool {
        *self == Self::of_text(text)
    }
}

impl FromStr for Version {
    type Err = InvalidVersion;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let hex = s
            .strip_prefix(PREFIX)
            .ok_or_else(|| InvalidVersion(s.to_owned()))?;
        if hex.len() == 64 && hex.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
            Ok(Self(s.to_owned()))
        } else {
            Err(InvalidVersion(s.to_owned()))
        }
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for Version {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Version {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_exact_bytes() {
        assert_eq!(
            Version::of(b"").as_str(),
            "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_ne!(Version::of_text("a\n"), Version::of_text("a\r\n"));
        assert!(Version::of_text("x").matches("x"));
    }

    #[test]
    fn parses_only_canonical_strings() {
        let v = Version::of_text("hello");
        assert_eq!(v.as_str().parse::<Version>(), Ok(v.clone()));
        for bad in [
            "",
            "sha256:",
            "md5:00",
            &v.as_str().to_uppercase(),
            &v.as_str()[..70],
        ] {
            assert_eq!(bad.parse::<Version>(), Err(InvalidVersion(bad.to_owned())));
        }
    }
}
