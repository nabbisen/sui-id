//! Fuzz target: Accept-Language header parsing (RFC 084).
//!
//! Invariants asserted:
//! - P1 (no panic): any byte sequence fed to `negotiate_from_accept_language` must not panic.
//! - P3 (round-trip): if the function returns `Some(locale)`, then
//!   `Locale::parse(locale.tag())` must return `Some(locale)` — the resolved
//!   locale is self-consistently parseable.

#![no_main]

use libfuzzer_sys::fuzz_target;
use sui_id_i18n::{Locale, negotiate_from_accept_language};

fuzz_target!(|data: &[u8]| {
    // libFuzzer may supply arbitrary bytes; skip if not valid UTF-8
    // (the function takes &str; invalid UTF-8 is a pre-condition violation
    // the HTTP layer catches before we're called).
    let Ok(header) = std::str::from_utf8(data) else {
        return;
    };

    // P1: must not panic.
    let result = negotiate_from_accept_language(header);

    // P3: round-trip coherence.
    if let Some(locale) = result {
        let reparsed = Locale::parse(locale.tag());
        assert_eq!(
            reparsed,
            Some(locale),
            "P3: negotiate returned locale whose tag does not round-trip: tag={:?}",
            locale.tag()
        );
    }
});
