//! The default device name of the sign-in and sign-up forms (PLAN §12.7), chosen from the raw
//! facts the app shell reads from the platform ([`DeviceFacts`]). Dart only gathers the facts;
//! picking, cleaning and formatting happen here (L15). The user can still edit the name.
//!
//! Per platform, the first usable fact wins:
//! - Android: the device name (`Settings.Global.DEVICE_NAME`, usually the marketing name),
//!   then manufacturer + model ("Samsung SM-S921B"), then the host name;
//! - iOS: the device name unless it is only the generic model ("iPhone"), then the commercial
//!   model name ("iPhone 16 Pro"), then the model, then the host name;
//! - macOS: the computer name, then the host name, then the model name;
//! - Windows and Linux: the host name, then the device name.
//!
//! A host name of `localhost` (Android's) is not a name; `.local`-style suffixes are dropped.
//! When nothing is usable, the platform's generic name is the default.

use crate::view::model::{DeviceFacts, Platform};

/// The server's limit for device names (the same as display names: 1 to 100 characters).
pub const MAX_DEVICE_NAME_CHARS: usize = 100;

/// Host-name suffixes that only say "this local network".
const LOCAL_SUFFIXES: [&str; 4] = [".localdomain", ".local", ".lan", ".home"];

/// The default device name for this platform and these facts: never empty, trimmed, single
/// spaced, without control characters and at most [`MAX_DEVICE_NAME_CHARS`] characters.
pub fn default_device_name(platform: Platform, facts: &DeviceFacts) -> String {
    let device = clean(&facts.device_name);
    let host = host_name(&facts.host_name);
    let chosen = match platform {
        Platform::Android => device
            .filter(|d| !same(d, &facts.model))
            .or_else(|| manufacturer_and_model(&facts.manufacturer, &facts.model))
            .or(host),
        Platform::Ios => device
            .filter(|d| !same(d, &facts.model))
            .or_else(|| clean(&facts.model_name))
            .or_else(|| clean(&facts.model))
            .or(host),
        Platform::Macos => device.or(host).or_else(|| clean(&facts.model_name)),
        Platform::Windows | Platform::Linux => host.or(device),
    };
    chosen.unwrap_or_else(|| generic_name(platform).to_owned())
}

/// The name when the platform tells nothing usable.
fn generic_name(platform: Platform) -> &'static str {
    match platform {
        Platform::Android => "Android device",
        Platform::Ios => "iOS device",
        Platform::Macos => "Mac",
        Platform::Windows => "Windows PC",
        Platform::Linux => "Linux PC",
    }
}

/// `Samsung SM-S921B`: the manufacturer, capitalised when the platform reports it in lower
/// case, before the model, unless the model already starts with it (`Pixel 8` by `Google`
/// becomes `Google Pixel 8`, `OnePlus 12` by `oneplus` stays `OnePlus 12`). `None` without a
/// model.
fn manufacturer_and_model(manufacturer: &str, model: &str) -> Option<String> {
    let model = clean(model)?;
    let Some(maker) = clean(manufacturer).filter(|m| !m.eq_ignore_ascii_case("unknown")) else {
        return Some(model);
    };
    if starts_with_word(&model, &maker) {
        return Some(model);
    }
    clean(&format!("{} {model}", capitalised(&maker)))
}

/// `word` in upper case first when it is all lower case (`samsung` → `Samsung`); mixed-case
/// names (`OnePlus`, `HMD Global`) are kept as the maker writes them.
fn capitalised(word: &str) -> String {
    if word.chars().any(char::is_uppercase) {
        return word.to_owned();
    }
    let mut chars = word.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

/// Whether `text` starts with `prefix` as a whole word, ignoring case.
fn starts_with_word(text: &str, prefix: &str) -> bool {
    let text = text.to_lowercase();
    let prefix = prefix.to_lowercase();
    text.strip_prefix(&prefix)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with(' '))
}

/// Whether two facts name the same thing, ignoring case and surrounding space.
fn same(a: &str, b: &str) -> bool {
    clean(b).is_some_and(|b| a.to_lowercase() == b.to_lowercase())
}

/// A host name that names this device: cleaned, without a local-network suffix, and not
/// `localhost`.
fn host_name(raw: &str) -> Option<String> {
    let mut host = clean(raw)?;
    for suffix in LOCAL_SUFFIXES {
        let cut = host.len().checked_sub(suffix.len());
        if let Some(cut) = cut
            && host.is_char_boundary(cut)
            && host[cut..].eq_ignore_ascii_case(suffix)
        {
            host.truncate(cut);
            break;
        }
    }
    let host = clean(&host)?;
    let local = ["localhost", "127.0.0.1", "::1"];
    (!local.iter().any(|l| host.eq_ignore_ascii_case(l))).then_some(host)
}

/// `raw` with control characters as spaces, runs of white space collapsed, trimmed and cut to
/// [`MAX_DEVICE_NAME_CHARS`] characters; `None` when nothing is left.
fn clean(raw: &str) -> Option<String> {
    let spaced: String = raw
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let joined = spaced.split_whitespace().collect::<Vec<_>>().join(" ");
    let cut: String = joined.chars().take(MAX_DEVICE_NAME_CHARS).collect();
    let cut = cut.trim_end().to_owned();
    (!cut.is_empty()).then_some(cut)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn facts(
        device_name: &str,
        manufacturer: &str,
        model: &str,
        model_name: &str,
        host_name: &str,
    ) -> DeviceFacts {
        DeviceFacts {
            device_name: device_name.to_owned(),
            manufacturer: manufacturer.to_owned(),
            model: model.to_owned(),
            model_name: model_name.to_owned(),
            host_name: host_name.to_owned(),
        }
    }

    fn name(platform: Platform, f: &DeviceFacts) -> String {
        default_device_name(platform, f)
    }

    #[test]
    fn android_prefers_the_device_name_setting() {
        let f = facts("Galaxy S24", "samsung", "SM-S921B", "", "localhost");
        assert_eq!(name(Platform::Android, &f), "Galaxy S24");
    }

    #[test]
    fn android_without_a_device_name_is_manufacturer_and_model() {
        let f = facts("", "samsung", "SM-S921B", "", "localhost");
        assert_eq!(name(Platform::Android, &f), "Samsung SM-S921B");
        let f = facts("  ", "  xiaomi ", " 23127PN0CG ", "", "localhost");
        assert_eq!(name(Platform::Android, &f), "Xiaomi 23127PN0CG");
    }

    #[test]
    fn android_device_name_equal_to_the_model_is_not_preferred() {
        let f = facts("sm-s921b", "samsung", "SM-S921B", "", "localhost");
        assert_eq!(name(Platform::Android, &f), "Samsung SM-S921B");
    }

    #[test]
    fn android_keeps_mixed_case_makers_and_models_that_name_them() {
        let f = facts("", "OnePlus", "CPH2581", "", "localhost");
        assert_eq!(name(Platform::Android, &f), "OnePlus CPH2581");
        let f = facts("", "Google", "Pixel 8", "", "localhost");
        assert_eq!(name(Platform::Android, &f), "Google Pixel 8");
        let f = facts("", "oneplus", "OnePlus 12", "", "localhost");
        assert_eq!(name(Platform::Android, &f), "OnePlus 12");
        let f = facts("", "HMD Global", "Nokia G21", "", "");
        assert_eq!(name(Platform::Android, &f), "HMD Global Nokia G21");
    }

    #[test]
    fn android_model_without_maker_and_maker_without_model() {
        let f = facts("", "", "SM-S921B", "", "localhost");
        assert_eq!(name(Platform::Android, &f), "SM-S921B");
        let f = facts("", "unknown", "SM-S921B", "", "localhost");
        assert_eq!(name(Platform::Android, &f), "SM-S921B");
        let f = facts("", "samsung", "", "", "localhost");
        assert_eq!(name(Platform::Android, &f), "Android device");
        let f = facts("", "samsung", "", "", "tablet-7");
        assert_eq!(name(Platform::Android, &f), "tablet-7");
    }

    #[test]
    fn ios_uses_the_user_name_or_else_the_commercial_model() {
        let f = facts(
            "Shawket's iPhone",
            "",
            "iPhone",
            "iPhone 16 Pro",
            "localhost",
        );
        assert_eq!(name(Platform::Ios, &f), "Shawket's iPhone");
        // iOS 16+ without the entitlement reports the generic model as the name.
        let f = facts("iPhone", "", "iPhone", "iPhone 16 Pro", "localhost");
        assert_eq!(name(Platform::Ios, &f), "iPhone 16 Pro");
        let f = facts("iPad", "", "iPad", "", "");
        assert_eq!(name(Platform::Ios, &f), "iPad");
        let f = facts("", "", "", "", "");
        assert_eq!(name(Platform::Ios, &f), "iOS device");
    }

    #[test]
    fn macos_uses_the_computer_name_then_the_host() {
        let f = facts(
            "Shawket's MacBook Pro",
            "",
            "Mac16,2",
            "MacBook Pro",
            "shawket-mbp.local",
        );
        assert_eq!(name(Platform::Macos, &f), "Shawket's MacBook Pro");
        let f = facts("", "", "Mac16,2", "MacBook Pro", "shawket-mbp.local");
        assert_eq!(name(Platform::Macos, &f), "shawket-mbp");
        let f = facts("", "", "Mac16,2", "MacBook Pro", "localhost");
        assert_eq!(name(Platform::Macos, &f), "MacBook Pro");
        let f = facts("", "", "", "", "");
        assert_eq!(name(Platform::Macos, &f), "Mac");
    }

    #[test]
    fn windows_and_linux_use_the_host_name() {
        let f = facts("DESKTOP-7Q2", "", "", "", "desktop-7q2");
        assert_eq!(name(Platform::Windows, &f), "desktop-7q2");
        let f = facts("DESKTOP-7Q2", "", "", "", "");
        assert_eq!(name(Platform::Windows, &f), "DESKTOP-7Q2");
        let f = facts("", "", "", "", "shawket-laptop.localdomain");
        assert_eq!(name(Platform::Linux, &f), "shawket-laptop");
        let f = facts("", "", "", "", "localhost.localdomain");
        assert_eq!(name(Platform::Linux, &f), "Linux PC");
        let f = facts("", "", "", "", "");
        assert_eq!(name(Platform::Windows, &f), "Windows PC");
    }

    #[test]
    fn localhost_is_never_the_name() {
        for platform in [
            Platform::Android,
            Platform::Ios,
            Platform::Macos,
            Platform::Windows,
            Platform::Linux,
        ] {
            for host in [
                "localhost",
                " LOCALHOST ",
                "localhost.local",
                "127.0.0.1",
                "::1",
            ] {
                let f = facts("", "", "", "", host);
                assert_eq!(name(platform, &f), generic_name(platform), "{host}");
            }
        }
    }

    #[test]
    fn names_are_cleaned_and_cut_to_the_server_limit() {
        let f = facts("", "", "", "", " my\tlaptop\n\u{7}two ");
        assert_eq!(name(Platform::Linux, &f), "my laptop two");
        let long = "x".repeat(150);
        let f = facts("", "", "", "", &long);
        assert_eq!(name(Platform::Linux, &f), "x".repeat(100));
        let f = facts("", "", "", "", &format!("{} y", "x".repeat(99)));
        assert_eq!(name(Platform::Linux, &f), "x".repeat(99));
        let f = facts("", "", "", "", "جهاز شوكت");
        assert_eq!(name(Platform::Linux, &f), "جهاز شوكت");
    }

    #[test]
    fn capitalised_handles_non_ascii_and_empty() {
        assert_eq!(capitalised("été"), "Été");
        assert_eq!(capitalised(""), "");
        assert_eq!(capitalised("LGE"), "LGE");
    }

    #[test]
    fn the_production_environment_carries_the_default_name() {
        let config = crate::view::model::CoreConfig {
            app_data_dir: "/data/strata".to_owned(),
            platform: Platform::Android,
            device: facts("", "samsung", "SM-S921B", "", "localhost"),
            server_url: "https://strata.example.org".to_owned(),
            release_build: true,
        };
        let env = crate::session::CoreEnv::production(&config).expect("env");
        assert_eq!(env.default_device_name, "Samsung SM-S921B");
    }

    fn any_platform() -> impl Strategy<Value = Platform> {
        prop_oneof![
            Just(Platform::Android),
            Just(Platform::Ios),
            Just(Platform::Macos),
            Just(Platform::Windows),
            Just(Platform::Linux),
        ]
    }

    proptest! {
        #[test]
        fn the_default_is_always_a_valid_device_name(
            platform in any_platform(),
            device_name in any::<String>(),
            manufacturer in any::<String>(),
            model in any::<String>(),
            model_name in any::<String>(),
            host_name in any::<String>(),
        ) {
            let f = DeviceFacts { device_name, manufacturer, model, model_name, host_name };
            let n = default_device_name(platform, &f);
            prop_assert!(!n.is_empty());
            prop_assert!(n.chars().count() <= MAX_DEVICE_NAME_CHARS);
            prop_assert!(!n.chars().any(char::is_control));
            prop_assert_eq!(n.trim(), n.as_str());
            prop_assert!(!n.contains("  "));
            prop_assert!(!n.eq_ignore_ascii_case("localhost"));
        }
    }
}
