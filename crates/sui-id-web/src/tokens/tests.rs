//! Contrast verification for the design tokens — RFC 144, N2a.
//!
//! This reads `crate::components::components_css()`, the stylesheet
//! every page actually receives — not only `TOKENS_CSS` — because the
//! contract is about what a component *composes*, not what the token
//! file declares in isolation. No token is excluded from it on a claim
//! about its use: that exclusion was the defect RFC 144 §1 found.
//!
//! **What a composed pair is**, for every rule in `components_css()`
//! (after every `@media` block is stripped — none of them sets `color:`
//! or `background:` today, confirmed by direct inspection before this
//! was written; a future one that does is caught the moment it also
//! sets a real foreground, since the walk below is unconditional):
//! - a rule that sets `color:` contributes (that colour, its own
//!   `background:`/`background-color:` if it sets one — if none, each
//!   of the four surfaces);
//! - a rule that sets `background:`/`background-color:` without
//!   `color:` contributes (`--fg-default`, it) **and** (`--fg-muted`,
//!   it), because inherited text of either kind sits on it;
//! - every colour in `components_css()` must be a `var()` reference to a
//!   declared custom property — a literal or `color-mix(…)` is a
//!   failure, because the gate cannot resolve the pair;
//! - a translucent token (`--state-hover`, `--state-active`) used as a
//!   fill is alpha-composited over each of the four surfaces, and the
//!   pair is checked on each result.

use std::collections::HashMap;

use super::TOKENS_CSS;

/// AAA text threshold (WCAG 1.4.6) — rule 1. No text in `components_css()`
/// today is large enough (24px, or 18.66px bold) to claim the 4.5:1
/// exemption; if a future rule is, the gate's own failure will say where.
const AAA_TEXT: f64 = 7.0;
/// Non-text UI-component threshold (WCAG 1.4.11) — rule 4.
const UI_COMPONENT: f64 = 3.0;
/// `prefers-contrast: more` control-boundary threshold — rule 5.
const UI_COMPONENT_HIGH: f64 = 4.5;

const SURFACES: &[&str] = &[
    "surface-default",
    "surface-elevated",
    "surface-subtle",
    "surface-sunken",
];

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

/// Alpha-composite an rgba colour over an opaque background.
fn composite((r, g, b, a): (u8, u8, u8, f64), bg: Rgb) -> Rgb {
    let blend = |fgc: u8, bgc: u8| (a * fgc as f64 + (1.0 - a) * bgc as f64).round() as u8;
    (blend(r, bg.0), blend(g, bg.1), blend(b, bg.2))
}

/// Parse every `--name: #RRGGBB;` declaration inside a CSS fragment.
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

/// Parse every `--name: rgba(r, g, b, a);` declaration inside a CSS
/// fragment — the translucent interaction tokens, which `parse_hex_decls`
/// never sees.
fn parse_rgba_decls(block: &str) -> HashMap<String, (u8, u8, u8, f64)> {
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
        let Some(open) = after.find("rgba(") else {
            continue;
        };
        let Some(close) = after[open..].find(')') else {
            continue;
        };
        let inner = &after[open + 5..open + close];
        let parts: Vec<f64> = inner
            .split(',')
            .filter_map(|p| p.trim().parse().ok())
            .collect();
        if let [r, g, b, a] = parts[..] {
            map.insert(name, (r as u8, g as u8, b as u8, a));
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

/// One resolved mode: hex tokens and the rgba interaction tokens, both
/// fully overlaid on the light base, exactly as the cascade resolves
/// them in the browser.
struct Mode {
    hex: HashMap<String, Rgb>,
    rgba: HashMap<String, (u8, u8, u8, f64)>,
}

/// The three resolved mode maps: light, dark, auto-dark. Dark and
/// auto-dark inherit every token they do not override from the light
/// `:root` base.
fn resolved_modes() -> Vec<(&'static str, Mode)> {
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

    let light_block = slice_between(css, ":root {", "[data-theme=\"light\"]");
    let dark_block = slice_between(
        css,
        "[data-theme=\"dark\"] {",
        "@media (prefers-color-scheme: dark)",
    );
    let auto_block = slice_between(
        css,
        ":root:not([data-theme]) {",
        "@media (prefers-reduced-motion",
    );

    let light_hex = parse_hex_decls(light_block);
    let light_rgba = parse_rgba_decls(light_block);

    let overlay_hex = |over: &HashMap<String, Rgb>| {
        let mut m = light_hex.clone();
        for (k, v) in over {
            m.insert(k.clone(), *v);
        }
        m
    };
    let overlay_rgba = |over: &HashMap<String, (u8, u8, u8, f64)>| {
        let mut m = light_rgba.clone();
        for (k, v) in over {
            m.insert(k.clone(), *v);
        }
        m
    };

    vec![
        (
            "light",
            Mode {
                hex: light_hex.clone(),
                rgba: light_rgba.clone(),
            },
        ),
        (
            "dark",
            Mode {
                hex: overlay_hex(&parse_hex_decls(dark_block)),
                rgba: overlay_rgba(&parse_rgba_decls(dark_block)),
            },
        ),
        (
            "auto-dark",
            Mode {
                hex: overlay_hex(&parse_hex_decls(auto_block)),
                rgba: overlay_rgba(&parse_rgba_decls(auto_block)),
            },
        ),
    ]
}

/// `--border-control`'s two `prefers-contrast: more` override values —
/// rule 5. Parsed directly from `TOKENS_CSS`'s own new block, not
/// hand-copied, so a value edited there is reflected here automatically.
fn contrast_border_control_overrides() -> Vec<(&'static str, Rgb)> {
    let css = TOKENS_CSS;
    let block = slice_between(
        css,
        "@media (prefers-contrast: more) {",
        "/* Text selection contrast",
    );
    assert!(
        !block.is_empty(),
        "TOKENS_CSS is missing the prefers-contrast: more block"
    );
    let light_part = slice_between(block, ":root {", "[data-theme=\"dark\"]");
    let dark_part = slice_between(
        block,
        "[data-theme=\"dark\"] {",
        "@media (prefers-color-scheme",
    );
    let light = parse_hex_decls(light_part);
    let dark = parse_hex_decls(dark_part);
    vec![
        (
            "light-contrast",
            *light
                .get("border-control")
                .expect("prefers-contrast light override for --border-control"),
        ),
        (
            "dark-contrast",
            *dark
                .get("border-control")
                .expect("prefers-contrast dark override for --border-control"),
        ),
    ]
}

// ── Extracting composed pairs from components_css() ─────────────────────────

/// Remove every `/* ... */` comment before anything else parses the
/// stylesheet. A comment is prose, not CSS: a comment that mentions
/// `@media (...)` or a brace would otherwise be taken for a real block
/// (0.92.0's N2b hit exactly that, and N2a's G12 hit its sibling).
fn strip_css_comments(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut rest = css;
    while let Some(start) = rest.find("/*") {
        out.push_str(&rest[..start]);
        let end = rest[start + 2..]
            .find("*/")
            .expect("every CSS comment in components_css() is terminated");
        rest = &rest[start + 2 + end + 2..];
    }
    out.push_str(rest);
    out
}

/// Unwrap every `@media (...) { ... }` block (brace-matched): its rules
/// stay, and only the guard goes. A rule inside a media query is composed
/// whenever its condition holds, so every check covers it exactly as it
/// covers a top-level rule. (Stripping the blocks instead would hide a
/// colour set inside one from every check, the literal check included.)
fn unwrap_media_blocks(css: &str) -> String {
    let mut out = String::new();
    let mut rest = css;
    loop {
        match rest.find("@media") {
            None => {
                out.push_str(rest);
                break;
            }
            Some(at) => {
                out.push_str(&rest[..at]);
                let after_at = &rest[at..];
                let brace_rel = after_at.find('{').expect("media block has a brace");
                let bytes = after_at.as_bytes();
                let mut depth = 1i32;
                let mut idx = brace_rel + 1;
                while depth > 0 {
                    match bytes[idx] {
                        b'{' => depth += 1,
                        b'}' => depth -= 1,
                        _ => {}
                    }
                    idx += 1;
                }
                // The block's inner rules, without its closing brace; a
                // nested media block inside is unwrapped in turn.
                out.push_str(&unwrap_media_blocks(&after_at[brace_rel + 1..idx - 1]));
                rest = &after_at[idx..];
            }
        }
    }
    out
}

/// Every rule's declaration block (`{ ... }`'s inside), after media blocks
/// are unwrapped — no nested braces remain.
fn extract_rule_decls(css: &str) -> Vec<String> {
    let mut out = vec![];
    let mut rest = css;
    while let Some(open) = rest.find('{') {
        let after_open = &rest[open + 1..];
        let Some(close) = after_open.find('}') else {
            break;
        };
        out.push(after_open[..close].to_string());
        rest = &after_open[close + 1..];
    }
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Paint {
    /// A `var()` reference to a declared custom property.
    Token(String),
    /// `transparent`, `none`, or absent.
    None,
    /// Anything else: a hex/rgb literal, `color-mix(...)`, `currentColor`.
    Literal(String),
}

fn parse_paint(v: &str) -> Paint {
    let v = v.trim();
    if v.is_empty() || v == "transparent" || v == "none" {
        return Paint::None;
    }
    if let Some(inner) = v.strip_prefix("var(--").and_then(|s| s.strip_suffix(')')) {
        let name = inner.split(',').next().unwrap_or(inner).trim();
        return Paint::Token(name.to_string());
    }
    Paint::Literal(v.to_string())
}

/// Finds `prop:`'s value in a declaration block, without matching a
/// longer property that merely ends in `prop` (`color` must not match
/// inside `border-color` or `background-color`).
fn find_decl(decls: &str, prop: &str) -> Option<String> {
    let mut search_from = 0usize;
    while let Some(rel) = decls[search_from..].find(prop) {
        let pos = search_from + rel;
        let preceded_by_word_char =
            pos > 0 && matches!(decls.as_bytes()[pos - 1], b'-' | b'a'..=b'z' | b'A'..=b'Z');
        let after = &decls[pos + prop.len()..];
        let followed_by_colon = after.trim_start().starts_with(':');
        if !preceded_by_word_char && followed_by_colon {
            let colon_rel = after.find(':').unwrap();
            let value_start = pos + prop.len() + colon_rel + 1;
            let value_end = decls[value_start..]
                .find(';')
                .map_or(decls.len(), |semi| value_start + semi);
            return Some(decls[value_start..value_end].trim().to_string());
        }
        search_from = pos + prop.len();
    }
    None
}

fn find_background(decls: &str) -> Option<String> {
    find_decl(decls, "background").or_else(|| find_decl(decls, "background-color"))
}

/// Every `color:`/`background:`/`background-color:` value in
/// `components_css()` that is neither a `var()` reference nor
/// `transparent`/`none` — the "every colour must be a token" rule.
fn literal_colour_failures(rule_decls: &[String]) -> Vec<String> {
    let mut out = vec![];
    for decls in rule_decls {
        for (prop, raw) in [
            ("color", find_decl(decls, "color")),
            ("background", find_background(decls)),
        ] {
            if let Some(raw) = raw
                && let Paint::Literal(lit) = parse_paint(&raw)
            {
                out.push(format!("{prop}: {lit} -- in `{}`", decls.trim()));
            }
        }
    }
    out
}

/// One text colour over one fill, not yet resolved to a mode.
struct TextPair {
    fg_token: String,
    fill: Fill,
    from: String,
}

enum Fill {
    /// A specific background token (e.g. `accent-default`).
    Token(String),
    /// No background was set (or it was `transparent`/`none`): checked
    /// against each of the four neutral surfaces.
    Surface,
    /// A translucent interaction token: alpha-composited over each of
    /// the four neutral surfaces.
    Translucent(String),
}

fn is_translucent(token: &str) -> bool {
    token == "state-hover" || token == "state-active"
}

/// The composed pair(s) one rule contributes, per the gate's own three
/// rules (module doc comment).
fn collect_text_pairs(decls: &str) -> Vec<TextPair> {
    let color = find_decl(decls, "color").map(|v| parse_paint(&v));
    let bg = find_background(decls).map(|v| parse_paint(&v));
    let from = decls.trim().to_string();

    let fill_of = |bg: &Option<Paint>| -> Option<Fill> {
        match bg {
            Some(Paint::Token(name)) if is_translucent(name) => {
                Some(Fill::Translucent(name.clone()))
            }
            Some(Paint::Token(name)) => Some(Fill::Token(name.clone())),
            Some(Paint::None) | None => Some(Fill::Surface),
            Some(Paint::Literal(_)) => None, // already flagged by literal_colour_failures
        }
    };

    match color {
        Some(Paint::Token(fg)) => match fill_of(&bg) {
            Some(fill) => vec![TextPair {
                fg_token: fg,
                fill,
                from,
            }],
            None => vec![],
        },
        Some(Paint::Literal(_)) => vec![], // already flagged
        Some(Paint::None) | None => {
            // No colour of its own: inherited text of either weight
            // sits on whatever background this rule sets.
            match &bg {
                Some(Paint::Token(name)) => {
                    let fill_variant = |n: &str| {
                        if is_translucent(n) {
                            Fill::Translucent(n.to_string())
                        } else {
                            Fill::Token(n.to_string())
                        }
                    };
                    vec![
                        TextPair {
                            fg_token: "fg-default".into(),
                            fill: fill_variant(name),
                            from: from.clone(),
                        },
                        TextPair {
                            fg_token: "fg-muted".into(),
                            fill: fill_variant(name),
                            from,
                        },
                    ]
                }
                _ => vec![],
            }
        }
    }
}

fn resolve_fill(fill: &Fill, mode: &Mode) -> Vec<(String, Rgb)> {
    match fill {
        Fill::Token(name) => mode
            .hex
            .get(name)
            .map(|rgb| vec![(name.clone(), *rgb)])
            .unwrap_or_default(),
        Fill::Surface => SURFACES
            .iter()
            .filter_map(|s| mode.hex.get(*s).map(|rgb| (s.to_string(), *rgb)))
            .collect(),
        Fill::Translucent(name) => {
            let Some(&rgba) = mode.rgba.get(name) else {
                return vec![];
            };
            SURFACES
                .iter()
                .filter_map(|s| {
                    mode.hex
                        .get(*s)
                        .map(|bg| (format!("{name} over {s}"), composite(rgba, *bg)))
                })
                .collect()
        }
    }
}

fn components_rule_decls() -> Vec<String> {
    extract_rule_decls(&unwrap_media_blocks(&strip_css_comments(
        crate::components::components_css(),
    )))
}

/// Like [`extract_rule_decls`], but keeps each rule's selector text
/// alongside its declaration block — RFC 144 §4's 44px check needs to
/// know *which* rule it is looking at, not only what it declares.
fn extract_rules_with_selector(css: &str) -> Vec<(String, String)> {
    let mut out = vec![];
    let mut rest = css;
    while let Some(open) = rest.find('{') {
        let selector = rest[..open].trim().to_string();
        let after_open = &rest[open + 1..];
        let Some(close) = after_open.find('}') else {
            break;
        };
        out.push((selector, after_open[..close].to_string()));
        rest = &after_open[close + 1..];
    }
    out
}

/// Whether one comma-separated selector part names `named` (`"button"`,
/// `".button"`, …) — either exactly, or as a compound selector built on
/// it (`button:disabled`, `.button.secondary`), not merely a selector
/// that happens to start with the same letters (`.button__icon` does
/// not name `.button`).
fn selector_part_names(part: &str, named: &str) -> bool {
    match part.strip_prefix(named) {
        Some(rest) => rest.is_empty() || matches!(rest.chars().next(), Some('.' | ':' | '[' | ' ')),
        None => false,
    }
}

fn selector_names_any(selector: &str, named: &[&str]) -> bool {
    selector
        .split(',')
        .map(str::trim)
        .any(|part| named.iter().any(|n| selector_part_names(part, n)))
}

/// A CSS length (`"2.75rem"`, `"44px"`, `"auto"`) to its px-equivalent,
/// at the platform-default 16px root. `None` for `"auto"` — a rule that
/// sets no floor makes no claim to check.
fn length_to_px(value: &str) -> Option<f64> {
    if value == "auto" {
        return None;
    }
    if let Some(n) = value.strip_suffix("rem") {
        return n.trim().parse::<f64>().ok().map(|r| r * 16.0);
    }
    value.strip_suffix("px").and_then(|n| n.trim().parse().ok())
}

// ── Tests ─────────────────────────────────────────────────────────────────

/// Rule 1: every pair a component actually composes clears AAA (7:1).
/// This is the mechanism RFC 144 §1 widened the gate to, in place of the
/// symptom (a per-token exclusion).
#[test]
fn every_composed_pair_in_components_css_clears_aaa() {
    let rule_decls = components_rule_decls();
    let mut pairs = vec![];
    for decls in &rule_decls {
        pairs.extend(collect_text_pairs(decls));
    }

    let mut failures = vec![];
    for (mode_name, mode) in resolved_modes() {
        for pair in &pairs {
            // An unresolved token is a failure, never a skip: a pair the
            // gate cannot compute is a pair it has not checked.
            let Some(&fg_rgb) = mode.hex.get(&pair.fg_token) else {
                failures.push(format!(
                    "[{mode_name}] text token `{}` does not resolve to a colour -- from `{}`",
                    pair.fg_token, pair.from
                ));
                continue;
            };
            let fills = resolve_fill(&pair.fill, &mode);
            if fills.is_empty() {
                failures.push(format!(
                    "[{mode_name}] the background under `{}` does not resolve -- from `{}`",
                    pair.fg_token, pair.from
                ));
            }
            for (bg_desc, bg_rgb) in fills {
                let ratio = contrast(fg_rgb, bg_rgb);
                if ratio < AAA_TEXT {
                    failures.push(format!(
                        "[{mode_name}] {} on {bg_desc} = {ratio:.2}:1 (need >= {AAA_TEXT}) -- from `{}`",
                        pair.fg_token, pair.from
                    ));
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "composed pairs below WCAG AAA:\n{}",
        failures.join("\n")
    );
}

/// Every colour `components_css()` sets must be a `var()` reference to a
/// declared custom property — a literal or `color-mix(...)` is a failure,
/// because the gate cannot resolve the pair it forms.
#[test]
fn every_colour_in_components_css_is_a_token() {
    let failures = literal_colour_failures(&components_rule_decls());
    assert!(
        failures.is_empty(),
        "unresolvable colour(s) in components_css():\n{}",
        failures.join("\n")
    );
}

/// Rule 3: only `--accent-default` and `--danger-default` are ever used
/// as `color:` among the five semantic colours. Warning, success and
/// info are never text — only borders, icons and tints.
#[test]
fn only_accent_and_danger_are_ever_text_among_semantic_colours() {
    let mut failures = vec![];
    for decls in &components_rule_decls() {
        if let Some(raw) = find_decl(decls, "color")
            && let Paint::Token(name) = parse_paint(&raw)
            && matches!(
                name.as_str(),
                "warning-default" | "success-default" | "info-default"
            )
        {
            failures.push(format!("color: var(--{name}) -- in `{}`", decls.trim()));
        }
    }
    assert!(
        failures.is_empty(),
        "a non-text semantic colour is used as color: (rule 3):\n{}",
        failures.join("\n")
    );
}

/// `--fg-on-accent` on `--accent-default`/`--accent-emphasis`, and
/// `--fg-on-danger` on `--danger-default`/`--danger-emphasis`: the two
/// button-label pairs, named explicitly because they are the only two
/// inverse tokens this palette still has.
#[test]
fn button_label_pairs_clear_aaa() {
    let mut failures = vec![];
    for (mode_name, mode) in resolved_modes() {
        for (fg, bgs) in [
            ("fg-on-accent", ["accent-default", "accent-emphasis"]),
            ("fg-on-danger", ["danger-default", "danger-emphasis"]),
        ] {
            let Some(&fg_rgb) = mode.hex.get(fg) else {
                failures.push(format!("[{mode_name}] missing token {fg}"));
                continue;
            };
            for bg in bgs {
                let Some(&bg_rgb) = mode.hex.get(bg) else {
                    failures.push(format!("[{mode_name}] missing token {bg}"));
                    continue;
                };
                let ratio = contrast(fg_rgb, bg_rgb);
                if ratio < AAA_TEXT {
                    failures.push(format!(
                        "[{mode_name}] {fg} on {bg} = {ratio:.2}:1 (need >= {AAA_TEXT})"
                    ));
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "button label pairs below WCAG AAA:\n{}",
        failures.join("\n")
    );
}

/// Rule 4: every control boundary and the focus ring clear 3:1 against
/// every surface. Also covers the three status colours (non-text, per
/// rule 3), which carry the same 3:1 floor.
#[test]
fn control_boundaries_and_status_colours_clear_ui_component_contrast() {
    let mut failures = vec![];
    for (mode_name, mode) in resolved_modes() {
        for token in [
            "border-control",
            "state-focus",
            "warning-default",
            "success-default",
            "info-default",
        ] {
            let Some(&fg_rgb) = mode.hex.get(token) else {
                failures.push(format!("[{mode_name}] missing token {token}"));
                continue;
            };
            for surface in SURFACES {
                let Some(&bg_rgb) = mode.hex.get(*surface) else {
                    continue;
                };
                let ratio = contrast(fg_rgb, bg_rgb);
                if ratio < UI_COMPONENT {
                    failures.push(format!(
                        "[{mode_name}] {token} (non-text) on {surface} = {ratio:.2}:1 (need >= {UI_COMPONENT})"
                    ));
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "non-text pairs below UI-component contrast:\n{}",
        failures.join("\n")
    );
}

/// Rule 5: under `prefers-contrast: more`, `--border-control` clears
/// 4.5:1 against every surface, light and dark.
#[test]
fn border_control_clears_enhanced_contrast_under_prefers_contrast_more() {
    let modes = resolved_modes();
    let light_surfaces = &modes[0].1.hex;
    let dark_surfaces = &modes[1].1.hex;

    let mut failures = vec![];
    for (name, border_rgb) in contrast_border_control_overrides() {
        let surfaces = if name == "light-contrast" {
            light_surfaces
        } else {
            dark_surfaces
        };
        for surface in SURFACES {
            let Some(&bg_rgb) = surfaces.get(*surface) else {
                continue;
            };
            let ratio = contrast(border_rgb, bg_rgb);
            if ratio < UI_COMPONENT_HIGH {
                failures.push(format!(
                    "[{name}] border-control on {surface} = {ratio:.2}:1 (need >= {UI_COMPONENT_HIGH})"
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "prefers-contrast: more border-control below 4.5:1:\n{}",
        failures.join("\n")
    );
}

/// Closure's own "by the same gate": `prefers-contrast: more` and
/// `prefers-reduced-motion` both exist in `TOKENS_CSS`.
#[test]
fn prefers_contrast_and_reduced_motion_both_exist() {
    assert!(
        TOKENS_CSS.contains("@media (prefers-contrast: more)"),
        "TOKENS_CSS is missing the prefers-contrast: more block"
    );
    assert!(
        TOKENS_CSS.contains("@media (prefers-reduced-motion: reduce)"),
        "TOKENS_CSS is missing the prefers-reduced-motion block"
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
    let dark = &modes[1].1.hex;
    let auto = &modes[2].1.hex;
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

/// RFC 144 §3: the type scale, and the spacing scale that moves with
/// it, are expressed in `rem`, not `px`, so a reader's own browser
/// font-size setting is honoured, not only page zoom. No
/// `--font-size-*` or `--space-*` token, and no `font-size:`
/// declaration `components_css()` sets, is `px`. Border widths and the
/// 1px visually-hidden utility are not text or spacing, and are not
/// checked here.
#[test]
fn no_font_size_or_space_token_is_in_px() {
    let mut failures = vec![];
    for line in TOKENS_CSS.lines() {
        let line = line.trim();
        if (line.starts_with("--font-size-") || line.starts_with("--space-")) && line.contains("px")
        {
            failures.push(format!("token declared in px: `{line}`"));
        }
    }
    for decls in components_rule_decls() {
        if let Some(value) = find_decl(&decls, "font-size")
            && value.contains("px")
        {
            failures.push(format!("font-size: {value} -- in `{}`", decls.trim()));
        }
    }
    assert!(
        failures.is_empty(),
        "px font-size or spacing token found:\n{}",
        failures.join("\n")
    );
}

/// RFC 144 §4: every rule in `components_css()` whose selector names
/// `button`, `.button`, `input`, `select`, `.user-menu__item` or
/// `.theme-toggle__btn`, and that sets `min-height` or `height`, sets
/// at least 44px (2.75rem) — standalone controls reach the WCAG 2.5.5
/// (AAA) target size. `.link-button` is the one named exception: an
/// inline link in running text, which WCAG 2.5.8 exempts. `input[type="radio"]`
/// (N2e) is the other: the label around it is the real target, checked
/// instead by [`chosen_option_label_reaches_44px`].
#[test]
fn standalone_controls_reach_44px() {
    const NAMED: &[&str] = &[
        "button",
        ".button",
        "input",
        "select",
        ".user-menu__item",
        ".theme-toggle__btn",
    ];
    const EXEMPT: &[&str] = &[".link-button", "input[type=\"radio\"]"];
    let mut failures = vec![];
    for (selector, decls) in extract_rules_with_selector(&unwrap_media_blocks(&strip_css_comments(
        crate::components::components_css(),
    ))) {
        if !selector_names_any(&selector, NAMED) {
            continue;
        }
        if selector_names_any(&selector, EXEMPT) {
            continue;
        }
        for prop in ["min-height", "height"] {
            let Some(value) = find_decl(&decls, prop) else {
                continue;
            };
            match length_to_px(&value) {
                Some(px) if px + 1e-9 >= 44.0 => {}
                Some(px) => failures.push(format!(
                    "{selector} {{ {prop}: {value}; }} -- {px}px, below 44px"
                )),
                None => {} // "auto": no floor claimed
            }
        }
    }
    assert!(
        failures.is_empty(),
        "standalone control(s) below the 44px target size:\n{}",
        failures.join("\n")
    );
}

/// RFC 144 N2e §1: `components_css()` holds a `:has(input[type="radio"]:checked)`
/// rule for each of the three label classes a chosen option must be
/// visible on — a selector that exists is the only thing a removed
/// rule (the mutation this test is written against) can falsify; the
/// styling itself is covered by [`every_composed_pair_in_components_css_clears_aaa`].
#[test]
fn every_chosen_option_label_has_a_checked_rule() {
    let css = unwrap_media_blocks(&strip_css_comments(crate::components::components_css()));
    let mut missing = vec![];
    for label in [
        "label.row-gap2-center-clickable",
        "label.row.row-gap2-center",
        "label.card.clickable-block",
    ] {
        let found = extract_rules_with_selector(&css)
            .into_iter()
            .any(|(selector, _)| {
                selector.contains(":has(")
                    && selector.contains("checked")
                    && selector.split(',').map(str::trim).any(|part| {
                        part.starts_with(label) && part[label.len()..].starts_with(":has(")
                    })
            });
        if !found {
            missing.push(label);
        }
    }
    assert!(
        missing.is_empty(),
        "no `:has(input[type=\"radio\"]:checked)` rule for: {}",
        missing.join(", ")
    );
}

/// RFC 144 N2e §1: the three labels that hold a radio and nothing else
/// clickable (`label.row-gap2-center-clickable`,
/// `label.row.row-gap2-center`, `label.card.clickable-block`) are
/// themselves at least 44px (2.75rem) tall — the label is what a
/// pointer aims at, not the radio `standalone_controls_reach_44px`
/// exempts.
#[test]
fn chosen_option_label_reaches_44px() {
    const NAMED: &[&str] = &[
        "label.row-gap2-center-clickable",
        "label.row.row-gap2-center",
        "label.card.clickable-block",
    ];
    let mut seen: std::collections::HashSet<&str> = NAMED.iter().copied().collect();
    let mut failures = vec![];
    for (selector, decls) in extract_rules_with_selector(&unwrap_media_blocks(&strip_css_comments(
        crate::components::components_css(),
    ))) {
        if !selector_names_any(&selector, NAMED) {
            continue;
        }
        // `:has(input[type="radio"]:checked)` variants name the same
        // label prefix but style the chosen state, not the floor height;
        // only the bare label rule is this test's concern.
        if selector.contains(":has(") {
            continue;
        }
        for part in selector.split(',').map(str::trim) {
            seen.remove(part);
        }
        match find_decl(&decls, "min-height") {
            Some(value) => match length_to_px(&value) {
                Some(px) if px + 1e-9 >= 44.0 => {}
                Some(px) => failures.push(format!(
                    "{selector} {{ min-height: {value}; }} -- {px}px, below 44px"
                )),
                None => {} // "auto": no floor claimed
            },
            None => failures.push(format!("{selector}: no min-height rule at all")),
        }
    }
    for missing in &seen {
        failures.push(format!(
            "{missing}: no rule in components_css() names it at all"
        ));
    }
    assert!(
        failures.is_empty(),
        "chosen-option label(s) below the 44px target size:\n{}",
        failures.join("\n")
    );
}
