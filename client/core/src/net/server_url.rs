//! The server address (owner decisions "App ID, server address, HTTPS first" and "No server
//! address in the UI"): every account of an install talks to one server, fixed at build time
//! (`STRATA_SERVER_URL`) and never shown on the sign-in or sign-up forms. It must be an
//! `https://` address; plain `http://` is accepted only for this device (`localhost`,
//! `127.0.0.1`, `::1`) in a debug build, for SSH-tunnel testing. Anything else is a
//! misconfigured build, which the app reports instead of starting.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use crate::error::CoreError;

/// The `field` of a [`CoreError::MisconfiguredBuild`] failure as Dart sees it.
pub const FIELD: &str = "server_url";
/// Reason: the build has no server address (unset or blank).
pub const MISSING: &str = "missing";
/// Reason: the address is not `https://<host>` (no scheme, another scheme, or no host).
pub const NOT_HTTPS: &str = "not_https";
/// Reason: a plain-`http://` address to a host other than this device, or any plain-`http://`
/// address in a release build.
pub const INSECURE_HTTP: &str = "insecure_http";

/// The build's server address as the core uses it (surrounding spaces and trailing `/`
/// removed), or [`CoreError::MisconfiguredBuild`] with [`MISSING`], [`NOT_HTTPS`] or
/// [`INSECURE_HTTP`]. `release` is whether this is a release build, where only `https://` is
/// accepted.
pub fn fixed(raw: &str, release: bool) -> Result<String, CoreError> {
    let url = raw.trim().trim_end_matches('/');
    if url.is_empty() {
        return Err(misconfigured(MISSING));
    }
    let Some((scheme, rest)) = url.split_once("://") else {
        return Err(misconfigured(NOT_HTTPS));
    };
    if authority(rest).is_empty() {
        return Err(misconfigured(NOT_HTTPS));
    }
    if scheme.eq_ignore_ascii_case("https") {
        Ok(url.to_owned())
    } else if scheme.eq_ignore_ascii_case("http") {
        if !release && is_loopback(rest) {
            Ok(url.to_owned())
        } else {
            Err(misconfigured(INSECURE_HTTP))
        }
    } else {
        Err(misconfigured(NOT_HTTPS))
    }
}

fn misconfigured(reason: &str) -> CoreError {
    CoreError::MisconfiguredBuild {
        reason: reason.to_owned(),
    }
}

/// The authority part of a URL (`rest` after `scheme://`).
fn authority(rest: &str) -> &str {
    rest.split(['/', '?', '#']).next().unwrap_or_default()
}

/// Whether the authority part of a URL (`rest` after `scheme://`) names this device.
fn is_loopback(rest: &str) -> bool {
    let authority = authority(rest);
    let host_port = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
    let host = match host_port.strip_prefix('[') {
        // `[::1]:8080`: the bracketed IPv6 literal.
        Some(v6) => v6.split(']').next().unwrap_or_default(),
        None => host_port.split(':').next().unwrap_or_default(),
    };
    host.eq_ignore_ascii_case("localhost")
        || host.parse::<IpAddr>().is_ok_and(|ip| {
            ip == IpAddr::V4(Ipv4Addr::LOCALHOST) || ip == IpAddr::V6(Ipv6Addr::LOCALHOST)
        })
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    fn refused(reason: &str) -> Result<String, CoreError> {
        Err(CoreError::MisconfiguredBuild {
            reason: reason.to_owned(),
        })
    }

    #[test]
    fn https_is_accepted_and_normalised_in_every_build() {
        for release in [true, false] {
            for (url, stored) in [
                (
                    "https://strata-ai.duckdns.org",
                    "https://strata-ai.duckdns.org",
                ),
                (
                    "  https://strata-ai.duckdns.org/// ",
                    "https://strata-ai.duckdns.org",
                ),
                (
                    "HTTPS://Strata.Example:8443/api",
                    "HTTPS://Strata.Example:8443/api",
                ),
                ("https://187.124.33.153", "https://187.124.33.153"),
                ("https://127.0.0.1:8443", "https://127.0.0.1:8443"),
            ] {
                assert_eq!(fixed(url, release), Ok(stored.to_owned()), "{url}");
            }
        }
    }

    #[test]
    fn a_blank_address_is_missing() {
        for release in [true, false] {
            for url in ["", "   ", "/", " // "] {
                assert_eq!(fixed(url, release), refused(MISSING), "{url:?}");
            }
        }
    }

    #[test]
    fn plain_http_to_another_host_is_refused_in_every_build() {
        for release in [true, false] {
            for url in [
                "http://strata-ai.duckdns.org",
                "http://187.124.33.153",
                "http://187.124.33.153:8080/",
                "HTTP://strata.example",
                " http://strata.example ",
                "http://localhost.strata.example",
                "http://127.0.0.1.nip.io",
                "http://127.0.0.2",
                "http://[::2]:8080",
                "http://[::ffff:127.0.0.1]",
                "http://localhost@strata.example",
                "http://user:pw@strata.example:80",
                "http://0.0.0.0",
            ] {
                assert_eq!(fixed(url, release), refused(INSECURE_HTTP), "{url}");
            }
        }
    }

    #[test]
    fn plain_http_to_this_device_is_accepted_in_debug_builds_only() {
        for (url, stored) in [
            ("http://127.0.0.1", "http://127.0.0.1"),
            ("http://127.0.0.1:8080/", "http://127.0.0.1:8080"),
            ("http://localhost:8080", "http://localhost:8080"),
            ("http://LOCALHOST", "http://LOCALHOST"),
            ("http://[::1]:8080", "http://[::1]:8080"),
            ("http://[::1]", "http://[::1]"),
            ("http://[0:0:0:0:0:0:0:1]:9", "http://[0:0:0:0:0:0:0:1]:9"),
            (
                "http://user@127.0.0.1:8080/x?y#z",
                "http://user@127.0.0.1:8080/x?y#z",
            ),
        ] {
            assert_eq!(fixed(url, false), Ok(stored.to_owned()), "{url}");
            assert_eq!(fixed(url, true), refused(INSECURE_HTTP), "{url}");
        }
    }

    #[test]
    fn other_forms_are_not_https() {
        for release in [true, false] {
            for url in [
                "strata-ai.duckdns.org",
                "127.0.0.1:8080",
                "ftp://strata-ai.duckdns.org",
                "wss://strata-ai.duckdns.org",
                "https://",
                "https:///path",
                "http://",
            ] {
                assert_eq!(fixed(url, release), refused(NOT_HTTPS), "{url}");
            }
        }
    }

    #[test]
    fn refusal_reaches_dart_as_misconfigured_build() {
        let Err(e) = fixed("http://strata.example", true) else {
            panic!("refused");
        };
        assert_eq!(
            crate::view::model::CoreFailure::from(e),
            crate::view::model::CoreFailure {
                code: "misconfigured_build".to_owned(),
                message_key: "error.misconfigured_build".to_owned(),
                field: Some("server_url".to_owned()),
                reason: Some("insecure_http".to_owned()),
                count: None,
                status: None,
            }
        );
    }
}
