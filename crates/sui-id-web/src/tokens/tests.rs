//! Contrast verification for the design tokens.
//!
//! This test validates the *design specification* — WCAG 2.1 AA: every
//! text-on-colour pair must clear 4.5:1 for normal text — directly
//! against the live `TOKENS_CSS` string, not against any hand-copied
//! table. It parses the three mode roots (`:root` light,
//! `[data-theme="dark"]`, and the `@media (prefers-color-scheme: dark)`
//! auto-dark block), resolves each mode by overlaying its overrides on
//! the light base, and recomputes the ratio for every pair.
//!
//! It is the executable form of the contrast contract from the UI/UX
//! handoff: a token edit that drops any text-on-colour pair below AA
//! fails the build. Decorative tokens that are intentionally below 4.5:1
//! (notably `fg-subtle`) are not text carriers and are excluded by
//! design — see the colour-palette source of record.

use std::collections::HashMap;

use super::TOKENS_CSS;

/// Normal-text AA threshold.
const AA_TEXT: f64 = 4.5;
/// Non-text UI-component threshold (focus ring, per WCAG 1.4.11).
const UI_COMPONENT: f64 = 3.0;

type Rgb = (u8, u8, u8);

/// sRGB channel (0..=1) to linear-light, per WCAG relative-luminance.
fn srgb_to_linear(c: f64) -> f64 {
    if c <= 0.03928 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// WCAG relative luminance of an sRGB colour.
fn luminance((r, g, b): Rgb) -> f64 {
    let lin = |v: u8| srgb_to_linear(v as f64 / 255.0);
    0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
}

/// WCAG contrast ratio between two colours (order-independent).
fn contrast(a: Rgb, b: Rgb) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

/// Parse every `--name: #RRGGBB;` declaration inside a CSS fragment into
/// a name→rgb map. Non-hex declarations (rgba veils, lengths) are
/// ignored — they are never text-on-colour carriers.
fn parse_hex_decls(block: &str) -> HashMap<String, Rgb> {
    let mut map = HashMap::new();
    for line in block.lines() {
        let line = line.trim();
        if !line.starts_with("--") {
            continue;
        }
        let Some(colon) = line.find(':') else {
            continue;
        };
        let name = line[2..colon].trim().to_string();
        let after = &line[colon..];
        let Some(hash) = after.find('#') else {
            continue;
        };
        let hex: String = after[hash + 1..].chars().take(6).collect();
        if hex.len() != 6 {
            continue;
        }
        let byte = |s: &str| u8::from_str_radix(s, 16).ok();
        if let (Some(r), Some(g), Some(b)) = (byte(&hex[0..2]), byte(&hex[2..4]), byte(&hex[4..6]))
        {
            map.insert(name, (r, g, b));
        }
    }
    map
}

/// Return the CSS text between `start` (exclusive) and the first `end`
/// after it (exclusive). Returns an empty slice if `start` is absent;
/// `resolved_modes` asserts each region marker is present first, so a
/// renamed/removed root surfaces there with a precise message.
fn slice_between<'a>(s: &'a str, start: &str, end: &str) -> &'a str {
    let Some(i) = s.find(start) else {
        return "";
    };
    let rest = &s[i + start.len()..];
    rest.find(end).map_or(rest, |j| &rest[..j])
}

/// The three resolved mode maps. Dark and auto-dark inherit every token
/// they do not override from the light `:root` base, exactly as the
/// cascade resolves them in the browser.
fn resolved_modes() -> Vec<(&'static str, HashMap<String, Rgb>)> {
    let css = TOKENS_CSS;
    for marker in [
        ":root {",
        "[data-theme=\"dark\"] {",
        ":root:not([data-theme]) {",
    ] {
        assert!(
            css.contains(marker),
            "token CSS missing region marker: {marker:?}"
        );
    }

    let light = parse_hex_decls(slice_between(css, ":root {", "[data-theme=\"light\"]"));
    let dark_over = parse_hex_decls(slice_between(
        css,
        "[data-theme=\"dark\"] {",
        "@media (prefers-color-scheme: dark)",
    ));
    let auto_over = parse_hex_decls(slice_between(
        css,
        ":root:not([data-theme]) {",
        "@media (prefers-reduced-motion",
    ));

    let overlay = |over: &HashMap<String, Rgb>| {
        let mut m = light.clone();
        for (k, v) in over {
            m.insert(k.clone(), *v);
        }
        m
    };

    vec![
        ("light", light.clone()),
        ("dark", overlay(&dark_over)),
        ("auto-dark", overlay(&auto_over)),
    ]
}

/// Every pair where a coloured fill carries text. Order: (foreground,
/// background). `fg-on-{semantic}` is mode-specific (white in light,
/// dark ink in dark); the resolved map already reflects that.
const TEXT_PAIRS: &[(&str, &str)] = &[
    ("fg-default", "surface-default"),
    ("fg-muted", "surface-default"),
    ("fg-on-accent", "accent-default"),
    ("fg-on-accent", "accent-emphasis"), // primary-button hover fill
    ("fg-on-danger", "danger-default"),
    ("fg-on-warning", "warning-default"),
    ("fg-on-success", "success-default"),
    ("fg-on-info", "info-default"),
    ("fg-disabled", "bg-disabled"),
];

#[test]
fn text_on_colour_pairs_pass_wcag_aa_in_all_modes() {
    let mut failures = Vec::new();
    for (mode, map) in resolved_modes() {
        for (fg, bg) in TEXT_PAIRS {
            match (map.get(*fg), map.get(*bg)) {
                (Some(f), Some(b)) => {
                    let ratio = contrast(*f, *b);
                    if ratio < AA_TEXT {
                        failures.push(format!(
                            "[{mode}] {fg} on {bg} = {ratio:.2}:1 (need >= {AA_TEXT})"
                        ));
                    }
                }
                _ => failures.push(format!("[{mode}] missing token in pair {fg} / {bg}")),
            }
        }
    }
    assert!(
        failures.is_empty(),
        "text-on-colour pairs below WCAG AA:\n{}",
        failures.join("\n")
    );
}

#[test]
fn focus_ring_meets_ui_component_contrast() {
    // The 2px focus ring is a non-text UI component: WCAG 1.4.11 asks
    // for >= 3:1 against the adjacent surface.
    let mut failures = Vec::new();
    for (mode, map) in resolved_modes() {
        match (map.get("state-focus"), map.get("surface-default")) {
            (Some(ring), Some(surface)) => {
                let ratio = contrast(*ring, *surface);
                if ratio < UI_COMPONENT {
                    failures.push(format!(
                        "[{mode}] state-focus on surface-default = {ratio:.2}:1 (need >= {UI_COMPONENT})"
                    ));
                }
            }
            _ => failures.push(format!("[{mode}] missing state-focus or surface-default")),
        }
    }
    assert!(
        failures.is_empty(),
        "focus ring below UI-component contrast:\n{}",
        failures.join("\n")
    );
}

#[test]
fn dark_and_auto_dark_stay_in_lockstep() {
    // The colour-palette source of record requires the explicit dark
    // root and the prefers-color-scheme auto-dark root to carry
    // identical values for every text-relevant token; a drift between
    // them produces a theme that is correct only when a preference is
    // saved. Guard the whole resolved set, not just a sample.
    let modes = resolved_modes();
    let dark = &modes[1].1;
    let auto = &modes[2].1;
    let mut drift = Vec::new();
    for (name, dval) in dark {
        match auto.get(name) {
            Some(aval) if aval == dval => {}
            Some(aval) => drift.push(format!("--{name}: dark {dval:?} != auto-dark {aval:?}")),
            None => drift.push(format!("--{name}: present in dark, missing in auto-dark")),
        }
    }
    for name in auto.keys() {
        if !dark.contains_key(name) {
            drift.push(format!("--{name}: present in auto-dark, missing in dark"));
        }
    }
    assert!(
        drift.is_empty(),
        "dark / auto-dark roots have drifted:\n{}",
        drift.join("\n")
    );
}
