#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::clone_on_copy,
    clippy::panic
)]
//! Tests for RFC 105 — audit-note escaping.
//!
//! The command-level behaviour (a real event, a forged reason) is pinned in
//! `commands/tests/runner/recovery.rs`; this file is the encoding itself: what
//! it does to hostile values, that it reverses exactly, and that no value can
//! introduce a pair boundary under any way of splitting a note.

#[cfg(test)]
mod note_encoding_tests {
    use crate::registry::{
        AuditAttributes, MAX_ATTRIBUTE_VALUE_BYTES, decode_note_value, encode_note_value,
        note_field, parse_note, render_note,
    };

    /// Values chosen to break a `key=value` splitter, one way each.
    fn hostile() -> Vec<(&'static str, String)> {
        vec![
            ("a space", "two words".into()),
            ("a tab", "a\tb".into()),
            ("a newline", "line one\nline two".into()),
            ("a carriage return", "a\r\nb".into()),
            ("a NUL", "a\0b".into()),
            ("an equals sign", "a=b".into()),
            ("only an equals sign", "=".into()),
            (
                "a value that looks like a pair",
                "step_up=fresh:totp:1 via=cli".into(),
            ),
            ("a value that looks like the next pair", "x via=cli".into()),
            ("a percent sign", "100%".into()),
            ("a value that looks like an escape", "%20 %3D %41".into()),
            ("a double quote", "say \"hi\"".into()),
            ("a backslash", "C:\\dir\\file".into()),
            ("a single quote", "it's".into()),
            ("no-break space", "a\u{00A0}b".into()),
            ("em space", "a\u{2003}b".into()),
            ("ideographic space", "a\u{3000}b".into()),
            ("line separator", "a\u{2028}b".into()),
            ("paragraph separator", "a\u{2029}b".into()),
            ("next line", "a\u{0085}b".into()),
            ("DEL", "a\u{007F}b".into()),
            ("the empty string", String::new()),
            ("only spaces", "   ".into()),
            ("non-ASCII text", "日本語のテキスト".into()),
            ("an emoji", "🔐 locked".into()),
            ("510 bytes of three-byte characters", "あ".repeat(170)),
            (
                "exactly 512 bytes of four-byte characters",
                "🔐".repeat(128),
            ),
            ("512 bytes of spaces", " ".repeat(MAX_ATTRIBUTE_VALUE_BYTES)),
            (
                "512 bytes of equals signs",
                "=".repeat(MAX_ATTRIBUTE_VALUE_BYTES),
            ),
        ]
    }

    fn note_for(value: &str) -> String {
        let attrs = AuditAttributes::builder()
            .attribute("first", "1")
            .attribute("reason", value)
            .attribute("last", "z")
            .build()
            .expect("attributes");
        render_note(&attrs).expect("a note")
    }

    #[test]
    fn every_hostile_value_round_trips_exactly() {
        for (label, value) in hostile() {
            let note = note_for(&value);
            assert_eq!(
                note_field(&note, "reason").as_deref(),
                Some(value.as_str()),
                "{label}: {note:?}"
            );
            assert_eq!(
                decode_note_value(&encode_note_value(&value)),
                value,
                "{label}"
            );
        }
    }

    #[test]
    fn no_hostile_value_introduces_a_pair_boundary_under_any_split() {
        for (label, value) in hostile() {
            let note = note_for(&value);
            // Split on the ASCII space the writer uses, and on Unicode
            // whitespace, which a careless reader might use: three tokens,
            // each starting with its own key, either way.
            for (how, tokens) in [
                ("ascii space", note.split(' ').collect::<Vec<_>>()),
                (
                    "unicode whitespace",
                    note.split_whitespace().collect::<Vec<_>>(),
                ),
            ] {
                assert_eq!(tokens.len(), 3, "{label} ({how}): {note:?}");
                assert!(tokens[0].starts_with("first="), "{label} ({how})");
                assert!(tokens[1].starts_with("reason="), "{label} ({how})");
                assert!(tokens[2].starts_with("last="), "{label} ({how})");
            }
            assert!(
                !note.contains('\n') && !note.contains('\r'),
                "{label}: a note is one line"
            );
            assert!(
                note.chars().all(|c| !c.is_control()),
                "{label}: a note carries no control character: {note:?}"
            );
            // The pairs are exactly the three that were written, once each.
            let keys: Vec<String> = parse_note(&note).into_iter().map(|(k, _)| k).collect();
            assert_eq!(keys, ["first", "reason", "last"], "{label}");
            // And the only `=` in the note are the three separators.
            assert_eq!(note.matches('=').count(), 3, "{label}: {note:?}");
        }
    }

    #[test]
    fn a_forged_pair_cannot_be_matched_by_a_substring_query() {
        // The operator guide's `LIKE '%step_up=fresh%'` queries.
        let note = note_for("x step_up=fresh:totp:1 via=cli");
        assert!(!note.contains("step_up="), "{note}");
        assert!(!note.contains("via=cli"), "{note}");
        assert!(
            note.contains("reason=x%20step_up%3Dfresh:totp:1%20via%3Dcli"),
            "{note}"
        );
    }

    #[test]
    fn the_encoding_touches_only_what_it_must() {
        // Ordinary values, non-ASCII text and the characters the fixed fields
        // use (`:`, `-`, `,`, `;`, `/`, `.`) are left as written.
        for plain in [
            "web",
            "fresh:totp:7",
            "2026-09-24T01:53:20Z",
            "caller,verified;by/phone.",
            "日本語",
            "🔐",
        ] {
            assert_eq!(encode_note_value(plain), plain);
        }
        assert_eq!(encode_note_value("a b"), "a%20b");
        assert_eq!(encode_note_value("a=b"), "a%3Db");
        assert_eq!(encode_note_value("a%b"), "a%25b");
        assert_eq!(encode_note_value("a\nb"), "a%0Ab");
        assert_eq!(encode_note_value("a\u{00A0}b"), "a%C2%A0b");
        assert_eq!(encode_note_value("a\0b"), "a%00b");
        assert_eq!(encode_note_value("a\u{007F}b"), "a%7Fb");
    }

    #[test]
    fn keys_are_never_encoded() {
        let attrs = AuditAttributes::builder()
            .attribute("expires_at", "1")
            .build()
            .expect("attributes");
        assert_eq!(render_note(&attrs).as_deref(), Some("expires_at=1"));
    }

    #[test]
    fn an_event_with_no_attributes_has_no_note() {
        let attrs = AuditAttributes::builder().build().expect("attributes");
        assert_eq!(render_note(&attrs), None);
    }

    // ── historical rows ────────────────────────────────────────────────────

    #[test]
    fn a_row_written_before_the_encoding_still_reads_last_occurrence_wins() {
        // U37 wrote the operator's reason first and unescaped. A forged pair can
        // only precede the real one, so the last occurrence is the recorded one.
        let old = "reason=x step_up=fresh:totp:1 via=cli invalidated=9 via=web expires_at=2026-01-01T00:00:00Z invalidated=0 step_up=not_applicable:system_principal";
        assert_eq!(note_field(old, "via").as_deref(), Some("web"));
        assert_eq!(note_field(old, "invalidated").as_deref(), Some("0"));
        assert_eq!(
            note_field(old, "step_up").as_deref(),
            Some("not_applicable:system_principal")
        );
    }

    #[test]
    fn a_historical_value_with_a_bare_percent_is_read_literally() {
        // `%` was never special before, so old text may contain a bare one.
        assert_eq!(decode_note_value("100%"), "100%");
        assert_eq!(decode_note_value("50%zz"), "50%zz");
        assert_eq!(decode_note_value("%"), "%");
        assert_eq!(decode_note_value("a%2"), "a%2");
        assert_eq!(
            note_field("reason=100% sure via=web", "via").as_deref(),
            Some("web")
        );
    }

    #[test]
    fn a_token_with_no_equals_sign_is_skipped() {
        assert_eq!(
            parse_note("free text step_up=x"),
            vec![("step_up".to_owned(), "x".to_owned())]
        );
    }

    mod property {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn any_value_round_trips_and_never_splits(value in "\\PC{0,120}|.{0,120}") {
                let note = note_for(&value);
                prop_assert_eq!(note_field(&note, "reason"), Some(value.clone()));
                prop_assert_eq!(note.split_whitespace().count(), 3);
                prop_assert_eq!(note.matches('=').count(), 3);
                prop_assert_eq!(decode_note_value(&encode_note_value(&value)), value);
            }
        }
    }
}
