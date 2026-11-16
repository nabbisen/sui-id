//! Design tokens — light + dark palettes, spacing, typography, radii, shadows.
//!
//! Single source of truth for visual style. Every page consumes these via
//! CSS custom properties (variables) so themes can swap with a single
//! `[data-theme="dark"]` attribute on the document root.
//!
//! RFC 144, N2a: the palette is **B** (RFC 149 §6), built to the refresh's
//! five rules (R1-R5) — see
//! `rfcs/handoffs/144-legible-to-older-eyes/palette-b-tokens-2026-11-16.md`
//! for every value and the pair it was checked against. In short: all text
//! clears 7:1 (WCAG AAA) in both modes; only `--accent-default` and
//! `--danger-default` are ever used as `color:` among the semantic
//! colours (warning/success/info are border- and tint-only); every
//! control boundary and the focus ring clear 3:1; `prefers-contrast: more`
//! raises control borders to 4.5:1. The gate in `tokens/tests.rs` now
//! reads `components::components_css()`, the stylesheet every page
//! actually receives, not only the tokens below — no token is excused
//! from it on a claim about its use.
//!
//! Token naming follows the convention from the design memo:
//!   --surface-* / --fg-* / --accent-* / --border-* / --state-* / --semantic.

pub const TOKENS_CSS: &str = r#"
:root {
  color-scheme: light dark;

  /* Light mode (default) ------------------------------------------------- */

  /* Surface */
  --surface-default:  #F6F7F9;
  --surface-subtle:   #EEF0F3;
  --surface-elevated: #FFFFFF;
  --surface-inverse:  #23272E;
  --surface-sunken:   #E8EBEF;

  /* Foreground */
  --fg-default:    #1A1D23;
  --fg-muted:      #464E5C;
  --fg-subtle:     #818791;
  --fg-on-accent:  #FFFFFF;
  --fg-inverse:    #F6F7F9;

  /* Disabled controls — explicit AAA-compliant tokens, not opacity.
     fg-disabled on bg-disabled clears 7:1 in both modes. A disabled
     control must never be the sole carrier of a meaningful value
     (read-only values use static rows); see the contrast gate in
     tokens/tests.rs. */
  --fg-disabled:   #464E5C;
  --bg-disabled:   #EEF0F3;

  /* Accent — rule 3: only accent and danger are ever `color:` among the
     semantic colours (links, the current nav item, field errors). Also
     checked as text on its own tint (the current nav item, the pressed
     theme toggle, the active language, .badge--accent). */
  --accent-default:  #3342AA;
  --accent-emphasis: #262F96;
  --accent-subtle:   #EBEDF7;

  /* Semantic — full set per RFC 061 (v0.46.0), refreshed per RFC 144.
   * Every semantic colour has three slots:
   *   --{name}-default — the border/foreground tint
   *   --{name}-subtle  — the tinted background for cards/banners
   *   --fg-on-{name}   — the foreground when text sits ON a -default fill
   * Rule 3: warning, success and info are never text and never fills —
   * only borders, icons and tints (3:1). Danger is checked as text on
   * every surface, and (rule 3) never on its own tint — RFC 144 §1a
   * moves every tinted danger label to --fg-default. Button labels
   * (--fg-on-accent, --fg-on-danger) are the only two inverse tokens
   * that still exist: warning/success/info are never fills, so they
   * need no --fg-on-* of their own (removed by this RFC). */
  --danger-default:  #9D0D1B;
  --danger-emphasis: #870009;
  --danger-subtle:   #F6E8E9;
  --fg-on-danger:    #FFFFFF;

  --warning-default: #9D6600;
  --warning-subtle:  #F5F0E6;

  --success-default: #2E7D52;
  --success-subtle:  #EAF2EE;

  --info-default:    #2F77C0;
  --info-subtle:     #EAF1F9;

  /* Interaction */
  --state-hover:    rgba(0, 0, 0, 0.05);
  --state-active:   rgba(0, 0, 0, 0.10);
  --state-focus:    #3342AA;
  --state-disabled: rgba(26, 29, 35, 0.38);

  /* Border — RFC 144 §2: split into a control boundary (3:1, WCAG
   * 1.4.11) and a divider that may stay subtle for rules and table
   * lines. --border-control is palette B's own control value;
   * --border-default is the midpoint of --border-control and
   * --surface-elevated (sRGB, 50%) — a visible-but-quiet divider, not
   * held to 3:1. */
  --border-control: #83878F;
  --border-default: #C1C3C7;
  --border-muted:   #E2E5E9;
  --border-accent:  #3342AA;

  /* Dev-mode banner (RFC 144 §1): one value, both modes — the banner
   * looks the same regardless of theme. Replaces the literal colours
   * components/banners.rs used to set directly. */
  --devmode-bg:   #5A3D00;
  --devmode-fg:   #FFF8E1;
  --devmode-warn: #FFCDD2;

  /* Shadow (light) */
  --shadow-sm: 0 1px 2px rgba(0, 0, 0, 0.04);
  --shadow-md: 0 2px 8px rgba(0, 0, 0, 0.06);
  --shadow-lg: 0 8px 24px rgba(0, 0, 0, 0.08);

  /* ------- Mode-independent tokens ------- */

  /* Spacing scale (4px base, 6 steps) */
  --space-1: 4px;
  --space-2: 8px;
  --space-3: 16px;
  --space-4: 24px;
  --space-5: 32px;
  --space-6: 48px;

  /* Typography scale */
  --font-size-display:  28px;
  --font-size-h2:       22px;
  --font-size-h3:       18px;
  --font-size-body:     15px;
  --font-size-caption:  13px;

  --line-height-display: 1.3;
  --line-height-h2:      1.35;
  --line-height-h3:      1.4;
  --line-height-body:    1.6;
  --line-height-caption: 1.5;

  --font-weight-regular: 400;
  --font-weight-medium:  500;
  --font-weight-bold:    700;

  /* Font families.
     Strategy: system-ui only, no web-font assets. Each OS provides the
     best-tuned UI font for its environment. CJK glyphs are reached via
     the Unicode font fallback — Hiragino on macOS, Yu Gothic UI on
     Windows, Noto Sans CJK on Linux/Android — without us bundling
     anything. When v1 multilingual support lands, :lang() rules can
     pin per-script fonts on top of this stack without touching the
     binary. */
  --font-sans:
      system-ui, -apple-system, "Segoe UI",
      "Hiragino Sans", "Yu Gothic UI",
      "Noto Sans JP", "Noto Sans KR", "Noto Sans SC",
      sans-serif;
  --font-mono:
      ui-monospace, "SF Mono", "Cascadia Code",
      "JetBrains Mono", Consolas, Menlo, monospace;

  /* Radius */
  --radius-sm: 6px;
  --radius-md: 10px;
  --radius-lg: 16px;

  /* Border width */
  --border-width-default:  1px;
  --border-width-emphasis: 2px;

  /* Layout constants */
  --content-max-width: 64rem;
  --content-narrow-width: 28rem;

  /* Motion / animation durations.
     Used with transition properties throughout components. When the
     prefers-reduced-motion media query fires, components that reference
     these tokens inherit zero-duration automatically via the override
     block below — no per-component duplication. */
  --motion-instant: 0ms;
  --motion-fast:    100ms;
  --motion-base:    200ms;
  --motion-slow:    350ms;
  --motion-easing:  cubic-bezier(0.4, 0, 0.2, 1);

  /* Z-index scale — explicit names prevent "magic number" layering. */
  --z-below:    -1;
  --z-base:      0;
  --z-raised:   10;
  --z-overlay:  100;
  --z-dropdown: 200;
  --z-modal:    300;
  --z-toast:    400;
}

[data-theme="light"] {
  color-scheme: light;
}

/* Dark mode --------------------------------------------------------------
   Activated by data-theme="dark" on <html>. The early inline script in
   layout.rs sets this synchronously so initial paint is correct. */
[data-theme="dark"] {
  color-scheme: dark;

  --surface-default:  #14171C;
  --surface-subtle:   #191D23;
  --surface-elevated: #1E232B;
  --surface-inverse:  #EEF1F5;
  --surface-sunken:   #101317;

  --fg-default:    #EEF1F5;
  --fg-muted:      #B8C1CE;
  --fg-subtle:     #666D78;
  --fg-on-accent:  #14171C;
  --fg-inverse:    #1A1D23;

  --fg-disabled:   #B8C1CE;
  --bg-disabled:   #191D23;

  --accent-default:  #AABBFF;
  --accent-emphasis: #BDCEFF;
  --accent-subtle:   #2A3040;

  --danger-default:  #FF8C8B;
  --danger-emphasis: #FFA7A5;
  --danger-subtle:   #342E35;
  --fg-on-danger:    #14171C;

  --warning-default: #E8B54A;
  --warning-subtle:  #32322E;

  --success-default: #5CC48C;
  --success-subtle:  #243335;

  --info-default:    #67C8E8;
  --info-subtle:     #25343E;

  --state-hover:    rgba(255, 255, 255, 0.06);
  --state-active:   rgba(255, 255, 255, 0.12);
  --state-focus:    #AABBFF;
  --state-disabled: rgba(238, 241, 245, 0.38);

  --border-control: #666D77;
  --border-default: #424851;
  --border-muted:   #262B33;
  --border-accent:  #AABBFF;

  --shadow-sm: 0 1px 2px rgba(0, 0, 0, 0.4);
  --shadow-md: 0 2px 8px rgba(0, 0, 0, 0.5);
  --shadow-lg: 0 8px 24px rgba(0, 0, 0, 0.6);
}

/* Auto dark mode (when user has no explicit preference saved) ----------
   Only applies when data-theme is unset; explicit user choice always
   overrides. */
@media (prefers-color-scheme: dark) {
  :root:not([data-theme]) {
    --surface-default:  #14171C;
    --surface-subtle:   #191D23;
    --surface-elevated: #1E232B;
    --surface-inverse:  #EEF1F5;
    --surface-sunken:   #101317;

    --fg-default:    #EEF1F5;
    --fg-muted:      #B8C1CE;
    --fg-subtle:     #666D78;
    --fg-on-accent:  #14171C;
    --fg-inverse:    #1A1D23;

    --fg-disabled:   #B8C1CE;
    --bg-disabled:   #191D23;

    --accent-default:  #AABBFF;
    --accent-emphasis: #BDCEFF;
    --accent-subtle:   #2A3040;

    --danger-default:  #FF8C8B;
    --danger-emphasis: #FFA7A5;
    --danger-subtle:   #342E35;
    --fg-on-danger:    #14171C;

    --warning-default: #E8B54A;
    --warning-subtle:  #32322E;

    --success-default: #5CC48C;
    --success-subtle:  #243335;

    --info-default:    #67C8E8;
    --info-subtle:     #25343E;

    --state-hover:    rgba(255, 255, 255, 0.06);
    --state-active:   rgba(255, 255, 255, 0.12);
    --state-focus:    #AABBFF;
    --state-disabled: rgba(238, 241, 245, 0.38);

    --border-control: #666D77;
    --border-default: #424851;
    --border-muted:   #262B33;
    --border-accent:  #AABBFF;

    --shadow-sm: 0 1px 2px rgba(0, 0, 0, 0.4);
    --shadow-md: 0 2px 8px rgba(0, 0, 0, 0.5);
    --shadow-lg: 0 8px 24px rgba(0, 0, 0, 0.6);
  }
}

/* Motion: honour OS-level "Reduce motion" preference.
   All transitions/animations that reference the token vars above are
   automatically zeroed; components do not need individual overrides. */
@media (prefers-reduced-motion: reduce) {
  :root {
    --motion-fast: 0ms;
    --motion-base: 0ms;
    --motion-slow: 0ms;
  }
  *, *::before, *::after {
    animation-duration: 0.01ms !important;
    animation-iteration-count: 1 !important;
    transition-duration: 0.01ms !important;
  }
}

/* RFC 144 R5: honour OS-level "Increase contrast" preference by raising
   the control-boundary token to its high-contrast value (4.5:1) in
   every mode. Nested so the dark value applies whichever way dark mode
   was reached — an explicit [data-theme="dark"], or the auto-dark
   media query with no saved preference. */
@media (prefers-contrast: more) {
  :root {
    --border-control: #666A72;
  }
  [data-theme="dark"] {
    --border-control: #838A94;
  }
  @media (prefers-color-scheme: dark) {
    :root:not([data-theme]) {
      --border-control: #838A94;
    }
  }
}

/* Text selection contrast (v0.48.2 bug fix): use the saturated
   accent + on-accent foreground rather than the subtle accent
   wash. The previous wash (--accent-subtle) was close in
   luminance to --surface-default in light mode, so a selection
   was visible against text but not against the page itself —
   the highlight blended into the background. Switching to the
   saturated accent gives the selection a strong, unambiguous
   shape regardless of where on the page it falls. The gate in
   tokens/tests.rs verifies --fg-on-accent on --accent-default
   clears 7:1 in every mode, so this pair is never re-measured here. */
::selection {
  background-color: var(--accent-default);
  color: var(--fg-on-accent);
}
"#;

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests;
