#![allow(clippy::unwrap_used)]
//! Unit tests for `sui-id-i18n`.

use crate::{Locale, STRINGS_JA, negotiate_from_accept_language};

#[test]
fn parse_round_trip() {
    // All locales that appear in ALL plus the zh variants.
    for &loc in Locale::ALL {
        assert_eq!(Locale::parse(loc.tag()), Some(loc));
    }
    // zh variants are not in ALL but must round-trip.
    assert_eq!(Locale::parse(Locale::ZhHans.tag()), Some(Locale::ZhHans));
    assert_eq!(Locale::parse(Locale::ZhHant.tag()), Some(Locale::ZhHant));
}

#[test]
fn parse_tolerates_region_suffix() {
    assert_eq!(Locale::parse("en-US"), Some(Locale::En));
    assert_eq!(Locale::parse("ja_JP"), Some(Locale::Ja));
    assert_eq!(Locale::parse("EN"), Some(Locale::En));
}

#[test]
fn parse_zh_variants() {
    // Simplified
    assert_eq!(Locale::parse("zh"), Some(Locale::ZhHans)); // legacy / bare
    assert_eq!(Locale::parse("zh-Hans"), Some(Locale::ZhHans));
    assert_eq!(Locale::parse("zh-CN"), Some(Locale::ZhHans));
    assert_eq!(Locale::parse("zh-SG"), Some(Locale::ZhHans));
    // Traditional
    assert_eq!(Locale::parse("zh-Hant"), Some(Locale::ZhHant));
    assert_eq!(Locale::parse("zh-TW"), Some(Locale::ZhHant));
    assert_eq!(Locale::parse("zh-HK"), Some(Locale::ZhHant));
    assert_eq!(Locale::parse("zh-MO"), Some(Locale::ZhHant));
}

#[test]
fn parse_unknown_returns_none() {
    assert_eq!(Locale::parse(""), None);
    assert_eq!(Locale::parse("xyz"), None);
    assert_eq!(Locale::parse("fr-FR"), None);
}

#[test]
fn negotiate_picks_first_recognised() {
    assert_eq!(
        negotiate_from_accept_language("fr;q=1, en;q=0.5"),
        Some(Locale::En)
    );
    assert_eq!(negotiate_from_accept_language("ja, en"), Some(Locale::Ja));
    assert_eq!(negotiate_from_accept_language(""), None);
    assert_eq!(
        negotiate_from_accept_language("zh, fr"),
        Some(Locale::ZhHans)
    );
    assert_eq!(
        negotiate_from_accept_language("zh-TW, zh"),
        Some(Locale::ZhHant)
    );
}

#[test]
fn each_locale_in_all_has_strings() {
    for &loc in Locale::ALL {
        let s = loc.strings();
        assert!(!s.button_save.is_empty(), "{loc:?}.button_save empty");
        assert!(!s.login_title.is_empty(), "{loc:?}.login_title empty");
    }
}

#[test]
fn zh_hans_strings_are_non_empty() {
    let s = Locale::ZhHans.strings();
    assert!(!s.button_save.is_empty());
    assert!(!s.login_title.is_empty());
}

#[test]
fn native_names_are_in_their_own_script() {
    assert!(STRINGS_JA.button_save.chars().any(|c| c >= '\u{3040}'));
    assert!(Locale::Ja.native_name().contains("日本語"));
    assert!(Locale::En.native_name().is_ascii());
    assert!(Locale::ZhHans.native_name().contains("简体"));
    assert!(Locale::ZhHant.native_name().contains("繁體"));
}

#[test]
fn locale_native_names_in_strings_tables() {
    // Every locale's strings table carries names for both zh variants.
    for &loc in &[Locale::Ja, Locale::En, Locale::ZhHans] {
        let s = loc.strings();
        assert!(
            !s.locale_native_zh_hans.is_empty(),
            "{loc:?}.locale_native_zh_hans empty"
        );
        assert!(
            !s.locale_native_zh_hant.is_empty(),
            "{loc:?}.locale_native_zh_hant empty"
        );
    }
}

#[test]
fn serde_round_trip() {
    use serde_json;
    for &loc in &[Locale::Ja, Locale::En, Locale::ZhHans, Locale::ZhHant] {
        let json = serde_json::to_string(&loc).unwrap();
        let back: Locale = serde_json::from_str(&json).unwrap();
        assert_eq!(back, loc, "serde round-trip failed for {loc:?}");
    }
}

#[test]
fn serde_legacy_zh_deserialises_to_zh_hans() {
    // Stored preferences written before the Simplified/Traditional split
    // used `"zh"`. They must continue to deserialise correctly.
    let back: Locale = serde_json::from_str("\"zh\"").unwrap();
    assert_eq!(back, Locale::ZhHans);
}
