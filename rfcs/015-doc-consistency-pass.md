# RFC 015 — Documentation and module-comment consistency pass

**Status.** Proposed
**Priority.** Medium. Maintainability / onboarding correctness.
No security or functional impact.
**Tracks.** v0.29.3 codebase review — medium-priority finding #7.
**Touches.** `README.md`, `crates/sui-id/src/handlers/settings.rs`
(module-level comment), assorted docs under `docs/`. No code
behaviour changes.

## Summary

The v0.29.3 codebase review identified three documentation-vs-code
mismatches that mislead readers without breaking anything. They
sit in the same kind-of-bug bucket: someone reading the project's
own docs to understand the project gets a slightly wrong picture.
None is hard to fix; collectively they're worth a single pass with
attention rather than five drive-by edits.

This is the smallest RFC in the v0.29.3-review series. It exists
because (a) the maintainer asked for the review's medium-priority
items to be RFC-tracked alongside the high-priority ones, and (b)
the fixes touch enough surface that bundling them is clearer than
scattered patches.

## What's actually wrong

### 1. README references stale paths

`README.md` mentions `crates/sui-id-bin/src/router.rs`.
The crate was renamed `sui-id-bin` → `sui-id` somewhere around
v0.20.x, and the current path is `crates/sui-id/src/router.rs`.
A reader following the link gets a 404 (post-v0.29.2's link
strategy change to absolute GitHub URLs makes this 404 louder
than it used to be).

The README also references `PUBLISHING.md` and `TERMS_OF_USE.md`,
neither of which exists in the current repo. The expectation
from the project's own README is that these documents exist;
they don't.

### 2. `settings.rs` module-comment lies about its scope

```rust
// crates/sui-id/src/handlers/settings.rs

//! Read-only settings overview.
//!
//! Renders the current server configuration to the admin panel.
//! Editable fields live in dedicated handlers (e.g. /admin/clients).
```

The actual handler accepts POST for `default_lang`,
`idle_session_timeout_secs`, `max_concurrent_sessions`, and
SMTP config. The comment is left over from a prior iteration
where settings really were read-only. A maintainer reading the
file is told "this is read-only" and then sees a POST handler
20 lines down — surprise is the wrong reaction to a module
comment.

### 3. `docs/operators.md` (and adjacent docs) drift

The codebase-review fingered the README; spot-checks during
this RFC's drafting suggest `docs/operators.md` and
`docs/integrators.md` carry similar lower-grade drift —
references to defaults that have changed, mentions of CLI
subcommands by their pre-v0.26.x names, etc. Severity: low,
but worth catching while we're here.

## Design

This is a documentation pass, not a feature. The "design"
section is mostly a checklist.

### README pass

- [ ] Replace `crates/sui-id-bin/src/router.rs` references with
      `crates/sui-id/src/router.rs`.
- [ ] Audit every relative path in the README (post-v0.29.2 they
      should all be absolute GitHub URLs anyway, but the
      hostname inside those URLs needs to match repo paths).
- [ ] Decide what `PUBLISHING.md` and `TERMS_OF_USE.md` references
      were meant to point at, and either:
      - point at the current equivalents (`docs/deployment.md`
        for publishing-the-binary kind of guidance? `LICENSE` /
        `NOTICE` for terms of use?), or
      - remove the references if there's no equivalent.

### settings.rs comment pass

Update the module-level comment to describe what the handler
actually does:

```rust
//! Server-settings admin tab.
//!
//! Renders the current server configuration and accepts updates
//! for the editable fields: default language, idle session
//! timeout, max concurrent sessions, and SMTP configuration. The
//! HIBP mode setting is managed here too once RFC 003's admin UI
//! lands. Read-only sections show non-editable settings (issuer,
//! storage path, master-key fingerprint, ...).
```

The comment update is the entire change; the handler's behaviour
is correct already.

### docs/ pass

A spot-check of `docs/deployment.md`, `docs/operators.md`,
`docs/integrators.md`, `docs/threat-model.md` for:

- CLI subcommand names (verify against `crates/sui-id/src/main.rs`'s
  current `clap` definitions).
- Default values (verify against the constants in
  `crates/sui-id/src/config.rs`).
- Cross-references (any link or path mention should point at
  something that exists in the current tree).

Discrepancies get fixed in-place. If a doc section is materially
out of date in a way that needs more than a paragraph rewrite,
flag it as a follow-up issue rather than expanding this RFC's
scope.

## Tests

There is no automated test for documentation correctness. The
verification is a manual pass:

- For each link in the README, click it (or `curl -I` it) and
  confirm it 200s.
- For each named file referenced, `git ls-files | grep` for
  it and confirm it exists.
- For each CLI subcommand named in docs, run `sui-id
  <subcommand> --help` and confirm the docs match the help
  text.
- For each default value documented, grep for it in
  `crates/sui-id/src/config.rs`.

This is a one-time effort tied to the RFC's implementation
pass. Recommend it not become a recurring CI check — false
positive rate would be high (e.g., legitimate changes to
defaults trigger doc-drift CI failures every time) and the
return on investment is low for a project at sui-id's scale.

## Multiple implementation steps

The work is small enough to land as a single PR. If preferred:

1. README pass.
2. `settings.rs` comment + a sweep of any other module-level
   comments that have similarly drifted.
3. `docs/*` pass.

Each is a self-contained change; none depends on the others.

## Security considerations

None. Documentation accuracy is a maintainability concern, not
a security one. The closest argument is that *bad* docs could
mislead an operator into a misconfiguration with security
consequences (e.g., docs say "default `hibp_mode = warn`" when
it's actually `off`), but the spot-checks above are designed
to catch exactly that case.

## Open questions

- **Should `PUBLISHING.md` and `TERMS_OF_USE.md` be created
  rather than removed from the README?** Recommend remove. The
  content those files would hold is already split between
  `LICENSE`, `NOTICE`, and `docs/deployment.md`. Adding new
  top-level docs would just create more drift surface.
- **Should we add a CONTRIBUTING-style "documentation
  conventions" doc?** Recommend no for this RFC. If the same
  drift problem recurs, revisit; for now the scale doesn't
  justify the meta-doc.
- **Is there value in checking the docs as part of CI?**
  Recommend no, for the reasons above. A periodic manual
  audit (e.g., per major release) is the right cadence for a
  project at this scale.
