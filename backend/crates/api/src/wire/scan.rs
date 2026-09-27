//! Structural pre-scan of a MessagePack document.
//!
//! Runs before `rmp-serde` touches the bytes, iteratively (no recursion), so hostile input
//! (depth bombs, huge declared lengths, trailing garbage, extension types, invalid UTF-8) is
//! rejected in linear time without allocating per declared length.

use rmp::Marker;

use super::DecodeLimits;

/// Why a document was rejected by [`scan`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Violation {
    /// The input is empty.
    #[error("the body is empty")]
    Empty,
    /// The input ends in the middle of a value.
    #[error("the MessagePack value is truncated")]
    Truncated,
    /// Bytes remain after the first complete value.
    #[error("{count} trailing bytes after the MessagePack value")]
    TrailingBytes {
        /// Number of bytes after the value.
        count: usize,
    },
    /// Containers nest deeper than allowed.
    #[error("nesting exceeds the maximum depth of {limit}")]
    DepthExceeded {
        /// The configured limit.
        limit: usize,
    },
    /// A string is longer than allowed.
    #[error("a string exceeds the maximum length of {limit} bytes")]
    StringTooLong {
        /// The configured limit.
        limit: u32,
    },
    /// A binary is longer than allowed.
    #[error("a binary value exceeds the maximum length of {limit} bytes")]
    BinaryTooLong {
        /// The configured limit.
        limit: u32,
    },
    /// An array has more elements than allowed.
    #[error("an array exceeds the maximum of {limit} elements")]
    ArrayTooLong {
        /// The configured limit.
        limit: u32,
    },
    /// A map has more entries than allowed.
    #[error("a map exceeds the maximum of {limit} entries")]
    MapTooLong {
        /// The configured limit.
        limit: u32,
    },
    /// A string is not valid UTF-8.
    #[error("a string is not valid UTF-8")]
    InvalidUtf8,
    /// A MessagePack extension type was used; Strata's wire format has none.
    #[error("MessagePack extension types are not allowed")]
    ExtensionNotAllowed,
    /// The reserved marker byte `0xc1` was found.
    #[error("invalid MessagePack marker")]
    InvalidMarker,
}

impl Violation {
    /// Stable machine-readable code, used in problem details (`errors[].code`).
    pub fn code(&self) -> &'static str {
        match self {
            Self::Empty => "empty_body",
            Self::Truncated => "truncated",
            Self::TrailingBytes { .. } => "trailing_bytes",
            Self::DepthExceeded { .. } => "depth_exceeded",
            Self::StringTooLong { .. } => "string_too_long",
            Self::BinaryTooLong { .. } => "binary_too_long",
            Self::ArrayTooLong { .. } => "array_too_long",
            Self::MapTooLong { .. } => "map_too_long",
            Self::InvalidUtf8 => "invalid_utf8",
            Self::ExtensionNotAllowed => "extension_not_allowed",
            Self::InvalidMarker => "invalid_marker",
        }
    }
}

/// Checks that `input` holds exactly one well-formed MessagePack value within `limits`.
pub fn scan(input: &[u8], limits: &DecodeLimits) -> Result<(), Violation> {
    if input.is_empty() {
        return Err(Violation::Empty);
    }
    let mut cursor = Cursor { input, pos: 0 };
    // Remaining child count of each open container, innermost last.
    let mut open: Vec<u64> = Vec::new();
    loop {
        let marker = Marker::from_u8(cursor.byte()?);
        let children: u64 = match marker {
            Marker::FixPos(_) | Marker::FixNeg(_) | Marker::Null | Marker::True | Marker::False => {
                0
            }
            Marker::U8 | Marker::I8 => cursor.skip(1).map(|()| 0)?,
            Marker::U16 | Marker::I16 => cursor.skip(2).map(|()| 0)?,
            Marker::U32 | Marker::I32 | Marker::F32 => cursor.skip(4).map(|()| 0)?,
            Marker::U64 | Marker::I64 | Marker::F64 => cursor.skip(8).map(|()| 0)?,
            Marker::FixStr(n) => cursor.string(u32::from(n), limits).map(|()| 0)?,
            Marker::Str8 => {
                let n = cursor.len(1)?;
                cursor.string(n, limits).map(|()| 0)?
            }
            Marker::Str16 => {
                let n = cursor.len(2)?;
                cursor.string(n, limits).map(|()| 0)?
            }
            Marker::Str32 => {
                let n = cursor.len(4)?;
                cursor.string(n, limits).map(|()| 0)?
            }
            Marker::Bin8 | Marker::Bin16 | Marker::Bin32 => {
                let n = cursor.len(bin_width(marker))?;
                if n > limits.max_bin_len {
                    return Err(Violation::BinaryTooLong {
                        limit: limits.max_bin_len,
                    });
                }
                cursor.skip(n as usize).map(|()| 0)?
            }
            Marker::FixArray(n) => cursor.array(u32::from(n), limits)?,
            Marker::Array16 => {
                let n = cursor.len(2)?;
                cursor.array(n, limits)?
            }
            Marker::Array32 => {
                let n = cursor.len(4)?;
                cursor.array(n, limits)?
            }
            Marker::FixMap(n) => cursor.map(u32::from(n), limits)?,
            Marker::Map16 => {
                let n = cursor.len(2)?;
                cursor.map(n, limits)?
            }
            Marker::Map32 => {
                let n = cursor.len(4)?;
                cursor.map(n, limits)?
            }
            Marker::Ext8
            | Marker::Ext16
            | Marker::Ext32
            | Marker::FixExt1
            | Marker::FixExt2
            | Marker::FixExt4
            | Marker::FixExt8
            | Marker::FixExt16 => return Err(Violation::ExtensionNotAllowed),
            Marker::Reserved => return Err(Violation::InvalidMarker),
        };
        if children > 0 {
            if open.len() >= limits.max_depth {
                return Err(Violation::DepthExceeded {
                    limit: limits.max_depth,
                });
            }
            open.push(children);
            continue;
        }
        // A value completed: count it against its enclosing containers.
        loop {
            match open.last_mut() {
                None => {
                    let rest = input.len() - cursor.pos;
                    return if rest == 0 {
                        Ok(())
                    } else {
                        Err(Violation::TrailingBytes { count: rest })
                    };
                }
                Some(remaining) => {
                    *remaining -= 1;
                    if *remaining == 0 {
                        open.pop();
                    } else {
                        break;
                    }
                }
            }
        }
    }
}

fn bin_width(marker: Marker) -> usize {
    match marker {
        Marker::Bin8 => 1,
        Marker::Bin16 => 2,
        _ => 4,
    }
}

struct Cursor<'a> {
    input: &'a [u8],
    pos: usize,
}

impl Cursor<'_> {
    fn remaining(&self) -> usize {
        self.input.len() - self.pos
    }

    fn byte(&mut self) -> Result<u8, Violation> {
        let b = *self.input.get(self.pos).ok_or(Violation::Truncated)?;
        self.pos += 1;
        Ok(b)
    }

    fn skip(&mut self, n: usize) -> Result<(), Violation> {
        if self.remaining() < n {
            return Err(Violation::Truncated);
        }
        self.pos += n;
        Ok(())
    }

    /// Reads a big-endian length of `width` bytes.
    fn len(&mut self, width: usize) -> Result<u32, Violation> {
        let bytes = self
            .input
            .get(self.pos..self.pos + width)
            .ok_or(Violation::Truncated)?;
        self.pos += width;
        Ok(bytes.iter().fold(0u32, |acc, b| (acc << 8) | u32::from(*b)))
    }

    fn string(&mut self, n: u32, limits: &DecodeLimits) -> Result<(), Violation> {
        if n > limits.max_str_len {
            return Err(Violation::StringTooLong {
                limit: limits.max_str_len,
            });
        }
        let n = n as usize;
        let bytes = self
            .input
            .get(self.pos..self.pos + n)
            .ok_or(Violation::Truncated)?;
        std::str::from_utf8(bytes).map_err(|_| Violation::InvalidUtf8)?;
        self.pos += n;
        Ok(())
    }

    fn array(&self, n: u32, limits: &DecodeLimits) -> Result<u64, Violation> {
        if n > limits.max_array_len {
            return Err(Violation::ArrayTooLong {
                limit: limits.max_array_len,
            });
        }
        // Every element takes at least one byte.
        if u64::from(n) > self.remaining() as u64 {
            return Err(Violation::Truncated);
        }
        Ok(u64::from(n))
    }

    fn map(&self, n: u32, limits: &DecodeLimits) -> Result<u64, Violation> {
        if n > limits.max_map_len {
            return Err(Violation::MapTooLong {
                limit: limits.max_map_len,
            });
        }
        let children = u64::from(n) * 2;
        if children > self.remaining() as u64 {
            return Err(Violation::Truncated);
        }
        Ok(children)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limits() -> DecodeLimits {
        DecodeLimits {
            max_depth: 3,
            max_str_len: 4,
            max_bin_len: 2,
            max_array_len: 3,
            max_map_len: 2,
        }
    }

    #[test]
    fn accepts_scalars_and_nested_values_within_limits() {
        // {"a": [1, nil, true]}, "abcd", bin[2], -1, f64, u64, empty map/array
        let cases: [&[u8]; 8] = [
            &[0x81, 0xa1, b'a', 0x93, 0x01, 0xc0, 0xc3],
            &[0xa4, b'a', b'b', b'c', b'd'],
            &[0xc4, 0x02, 0xff, 0x00],
            &[0xff],
            &[0xcb, 0, 0, 0, 0, 0, 0, 0, 0],
            &[0xcf, 1, 2, 3, 4, 5, 6, 7, 8],
            &[0x80],
            &[0x91, 0x91, 0x90],
        ];
        for case in cases {
            assert_eq!(scan(case, &limits()), Ok(()), "{case:02x?}");
        }
    }

    #[test]
    fn rejects_each_violation_with_its_code() {
        let cases: [(&[u8], Violation, &str); 13] = [
            (&[], Violation::Empty, "empty_body"),
            (&[0x92, 0x01], Violation::Truncated, "truncated"),
            (&[0xcd, 0x01], Violation::Truncated, "truncated"),
            (
                &[0x01, 0x02, 0x03],
                Violation::TrailingBytes { count: 2 },
                "trailing_bytes",
            ),
            (
                &[0x91, 0x91, 0x91, 0x91, 0x01],
                Violation::DepthExceeded { limit: 3 },
                "depth_exceeded",
            ),
            (
                &[0xa5, b'a', b'b', b'c', b'd', b'e'],
                Violation::StringTooLong { limit: 4 },
                "string_too_long",
            ),
            (
                &[0xc4, 0x03, 1, 2, 3],
                Violation::BinaryTooLong { limit: 2 },
                "binary_too_long",
            ),
            (
                &[0x94, 1, 2, 3, 4],
                Violation::ArrayTooLong { limit: 3 },
                "array_too_long",
            ),
            (
                &[0x83, 1, 1, 2, 2, 3, 3],
                Violation::MapTooLong { limit: 2 },
                "map_too_long",
            ),
            (&[0xa2, 0xc3, 0x28], Violation::InvalidUtf8, "invalid_utf8"),
            (
                &[0xd6, 0xff, 0, 0, 0, 0],
                Violation::ExtensionNotAllowed,
                "extension_not_allowed",
            ),
            (&[0xc1], Violation::InvalidMarker, "invalid_marker"),
            // Declared 65535 elements but only one byte follows: rejected without allocation.
            (&[0xdc, 0x00, 0x02, 0x01], Violation::Truncated, "truncated"),
        ];
        for (input, expected, code) in cases {
            let err = scan(input, &limits()).expect_err("must be rejected");
            assert_eq!(err, expected, "{input:02x?}");
            assert_eq!(err.code(), code);
        }
    }

    #[test]
    fn depth_bomb_of_a_million_levels_is_rejected_at_the_limit() {
        let mut bomb = vec![0x91u8; 1_000_000];
        bomb.push(0x01);
        assert_eq!(
            scan(&bomb, &DecodeLimits::default()),
            Err(Violation::DepthExceeded { limit: 64 })
        );
    }

    #[test]
    fn depth_exactly_at_the_limit_is_accepted() {
        let mut doc = vec![0x91u8; 64];
        doc.push(0x01);
        assert_eq!(scan(&doc, &DecodeLimits::default()), Ok(()));
    }
}
