//! Structural limits applied to every decoded MessagePack body and frame (PLAN §7.7, §15).

/// Limits checked by [`super::scan`] before any value is materialised.
///
/// All lengths are counts from the MessagePack header: bytes for strings and binaries,
/// elements for arrays, entries (key/value pairs) for maps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecodeLimits {
    /// Maximum number of nested containers (arrays/maps). A top-level map has depth 1.
    pub max_depth: usize,
    /// Maximum string length in bytes.
    pub max_str_len: u32,
    /// Maximum binary length in bytes.
    pub max_bin_len: u32,
    /// Maximum number of array elements.
    pub max_array_len: u32,
    /// Maximum number of map entries.
    pub max_map_len: u32,
}

impl DecodeLimits {
    /// Default nesting limit.
    pub const DEFAULT_MAX_DEPTH: usize = 64;
}

impl Default for DecodeLimits {
    fn default() -> Self {
        Self {
            max_depth: Self::DEFAULT_MAX_DEPTH,
            max_str_len: 1024 * 1024,
            max_bin_len: 1024 * 1024,
            max_array_len: 10_000,
            max_map_len: 1_000,
        }
    }
}
