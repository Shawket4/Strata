//! MessagePack wire layer (PLAN §7.7, L21).
//!
//! - [`MsgPack`]: request extractor (content type, body limit, bounded decoding) and response
//!   type (structs encoded as maps with field names).
//! - [`Problem`]: RFC 7807 problem details carried as `application/problem+msgpack`.
//! - [`negotiate`]: `Accept` handling (406 when MessagePack is not acceptable).
//! - [`ws`]: WebSocket framing for streams (D24).
//!
//! Conventions are documented in `docs/WIRE_FORMAT.md`.

mod binary;
mod codec;
mod extract;
mod limits;
pub mod negotiate;
mod problem;
mod respond;
mod scan;
pub mod ws;

pub use binary::Binary;
pub use codec::{DecodeError, decode, encode};
pub use extract::{MsgPack, MsgPackConfig};
pub use limits::DecodeLimits;
pub use problem::{
    DuplicateCandidate, MatchLevel, Problem, ProblemDetails, ProblemExtensions, ProblemFieldError,
    ProblemType, decode_error_codes,
};
pub use scan::{Violation, scan};

/// Media type of every request and response body.
///
/// PLAN §7.7 names `application/msgpack`; the IANA-registered type is `application/vnd.msgpack`.
/// Which one Strata uses is an open owner decision (see `docs/WIRE_FORMAT.md`); every use in
/// the server, contract, generator and client goes through this constant (and
/// [`PROBLEM_MSGPACK`]) so switching is a one-line change plus regeneration.
pub const MSGPACK: &str = "application/msgpack";

/// Media type of error bodies: RFC 7807 problem details encoded as MessagePack.
pub const PROBLEM_MSGPACK: &str = "application/problem+msgpack";

/// Media type of vault export/import bodies (§7.6).
pub const ZIP: &str = "application/zip";
