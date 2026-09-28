//! The server-address rule (owner decision "App ID, server address, HTTPS first"): accounts
//! talk to their server over HTTPS; plain `http://` is refused unless the host is the device
//! itself (`localhost`, `127.0.0.1`, `::1`), which stays allowed for SSH-tunnel testing.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use crate::error::CoreError;

/// The `InvalidInput` field of a refused server address.
pub const FIELD: &str = "server_url";
/// The `InvalidInput` reason of a plain-`http://` address to a host other than this device.
pub const INSECURE_HTTP: &str = "insecure_http";

/// `raw` as the core stores it (surrounding spaces and trailing `/` removed), or
/// [`CoreError::InvalidInput`] `server_url` / [`INSECURE_HTTP`] when it is a plain-`http://`
/// address of a host other than this device.
pub fn checked(raw: &str) -> Result<String, CoreError> {
    let url = raw.trim().trim_end_matches('/');
    match url.split_once("://") {
        Some((scheme, rest)) if scheme.eq_ignore_ascii_case("http") && !is_loopback(rest) => {
            Err(CoreError::invalid(FIELD, INSECURE_HTTP))
        }
        _ => Ok(url.to_owned()),
    }
}

/// The server address suggested on the sign-in and sign-up forms, from the build's
/// configuration (`STRATA_DEFAULT_SERVER`): `None` when unset or blank, else trimmed.
pub fn default_from_build(configured: Option<&str>) -> Option<String> {
    configured
        .map(|u| u.trim().trim_end_matches('/'))
        .filter(|u| !u.is_empty())
        .map(str::to_owned)
}

/// Whether the authority part of a URL (`rest` after `scheme://`) names this device.
fn is_loopback(rest: &str) -> bool {
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
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

    fn refused() -> Result<String, CoreError> {
        Err(CoreError::InvalidInput {
            field: "server_url".to_owned(),
            reason: "insecure_http".to_owned(),
        })
    }

    #[test]
    fn https_is_accepted_and_normalised() {
        assert_eq!(
            checked("https://strata.example"),
            Ok("https://strata.example".to_owned())
        );
        assert_eq!(
            checked("  https://strata.example/// "),
            Ok("https://strata.example".to_owned())
        );
        assert_eq!(
            checked("HTTPS://Strata.Example:8443/api"),
            Ok("HTTPS://Strata.Example:8443/api".to_owned())
        );
        assert_eq!(
            checked("https://187.124.33.153"),
            Ok("https://187.124.33.153".to_owned())
        );
    }

    #[test]
    fn plain_http_to_another_host_is_refused() {
        for url in [
            "http://strata.example",
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
            assert_eq!(checked(url), refused(), "{url}");
        }
    }

    #[test]
    fn plain_http_to_this_device_is_accepted() {
        for (url, stored) in [
            ("http://127.0.0.1", "http://127.0.0.1"),
            ("http://127.0.0.1:8080/", "http://127.0.0.1:8080"),
            ("http://localhost:8080", "http://localhost:8080"),
            ("http://LOCALHOST", "http://LOCALHOST"),
            ("http://[::1]:8080", "http://[::1]:8080"),
            ("http://[::1]", "http://[::1]"),
            ("http://[0:0:0:0:0:0:0:1]:9", "http://[0:0:0:0:0:0:0:1]:9"),
            ("http://user@127.0.0.1:8080/x?y#z", "http://user@127.0.0.1:8080/x?y#z"),
        ] {
            assert_eq!(checked(url), Ok(stored.to_owned()), "{url}");
        }
    }

    #[test]
    fn other_forms_are_left_to_the_network_layer() {
        // No scheme or another scheme: not an insecure address; the client reports it.
        assert_eq!(checked(""), Ok(String::new()));
        assert_eq!(checked("strata.example"), Ok("strata.example".to_owned()));
        assert_eq!(
            checked("ftp://strata.example"),
            Ok("ftp://strata.example".to_owned())
        );
    }

    #[test]
    fn default_from_build_drops_blank_values() {
        assert_eq!(default_from_build(None), None);
        assert_eq!(default_from_build(Some("")), None);
        assert_eq!(default_from_build(Some("  ")), None);
        assert_eq!(
            default_from_build(Some(" https://strata.example/ ")),
            Some("https://strata.example".to_owned())
        );
    }

    #[test]
    fn refusal_reaches_dart_as_invalid_input() {
        let Err(e) = checked("http://strata.example") else {
            panic!("refused");
        };
        assert_eq!(
            crate::view::model::CoreFailure::from(e),
            crate::view::model::CoreFailure {
                code: "invalid_input".to_owned(),
                message_key: "error.invalid_input".to_owned(),
                field: Some("server_url".to_owned()),
                reason: Some("insecure_http".to_owned()),
                count: None,
                status: None,
            }
        );
    }
}
