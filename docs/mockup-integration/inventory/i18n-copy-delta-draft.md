# i18n Copy Delta Draft — Mockup ↔ Product

Phase-0 deliverable of [RFC-MI-000](../../../rfcs/done/RFC-MI-000-baseline-delta-inventory.md).
Generated against `sui-id-web-mockup v0.4.8/crates/sui-id-web/src/i18n.rs`
↔ `sui-id v0.49.0/crates/sui-id-i18n/src/strings.rs`.

This file is the input contract for **RFC-MI-040** (setup wizard
strings), **RFC-MI-041** (auth-surface strings), **RFC-MI-050**
(form-system strings), **RFC-MI-060** (self-service strings), and
**RFC-MI-070** (consent strings) — anywhere user-visible copy is
introduced or modified.

## Headline

```
Mockup keys:    426 total
Product keys:   598 total
Shared by name:  39  (~ 9% of mockup, ~ 7% of product)
Mockup-only:    382
Product-only:   559
```

The 382 "mockup-only" keys are **not all new strings**. The majority
are **renames of concepts the product already covers** under different
key names. The integration work for each is:

1. **Rename map.** Identify the equivalent product key. Update mockup
   markup to reference the product key. **No new string. No
   translation work.**
2. **Reword.** Identify the equivalent product key with similar
   semantics but different wording. Decide whether to update the
   product key or the mockup markup. (Default: keep the product key
   and reword in line with the mockup's tone.)
3. **Add.** Genuinely new copy with no product equivalent. Add a new
   key to the Strings struct and translate to all three locales.

## Cluster analysis (mockup-only keys by prefix)

The 382 mockup-only keys fall into 47 prefix clusters. The 20 largest:

| Prefix | Mockup-only count | Cluster category | RFC owner |
|---|---|---|---|
| `action_*` | 29 | RENAME — product uses `button_*` for the same concepts (e.g. `action_save` → `button_save`) | RFC-MI-050 |
| `users_*` | 19 | ADD-mostly — admin user management copy not covered by product's narrower vocabulary | RFC-MI-031 |
| `setup_*` | 19 | RENAME-mostly — product covers most setup copy under different names | RFC-MI-040 |
| `col_*` | 18 | RENAME — table column labels; product uses `*_col_*` (qualified by table) | RFC-MI-031 |
| `settings_*` | 17 | REWORD — both sides have setting labels; copy differs | (deferred — settings refactor) |
| `me_*` | 17 | REWORD — both sides have `/me/security` copy; tone differs | RFC-MI-060 |
| `impact_*` | 17 | ADD — mockup introduces the "impact summary" surface (RFC-MI-051) | RFC-MI-051 |
| `dashboard_*` | 16 | REWORD — both sides have dashboard copy; mockup is denser | RFC-MI-030 |
| `clients_*` | 14 | RENAME-mostly | RFC-MI-031 |
| `card_*` | 14 | ADD — mockup introduces several card titles for dashboard sections | RFC-MI-030 |
| `filter_*` | 12 | ADD — mockup introduces audit-log filter UI | RFC-MI-031 (deferred unless filter UI ships) |
| `mfa_*` | 11 | REWORD — both sides have MFA copy | RFC-MI-060 |
| `state_*` | 9 | RENAME — product uses `badge_*` (`state_active` → `badge_enabled`) | RFC-MI-031 |
| `config_*` | 9 | ADD — mockup mentions config file paths on dashboard | RFC-MI-030 |
| `dev_*` | 8 | ADD — mockup has explicit dev-mode disclosure copy | RFC-MI-040 |
| `chain_*` | 7 | ADD — mockup surfaces audit-chain-status copy | RFC-MI-031 |
| `admin_*` | 7 | REWORD — admin shell copy | RFC-MI-020 |
| `done_*` | 6 | RENAME — mockup setup-done copy | RFC-MI-040 |
| `confirm_*` | 6 | RENAME — product uses `confirm_screen_*` qualified by operation | RFC-MI-051 |
| `reauth_*` | 5 | RENAME — product calls step-up "step-up", mockup calls it "re-authentication" | RFC-MI-051 |

Remaining 27 clusters are < 5 keys each (mostly screen-specific
copy: `forgot_*`, `consent_*`, `error_*`, `stepup_*`, etc.).

## Specific rename map — Phase 0 sample (29 `action_*` keys)

This is the largest single cluster and the most mechanical to
resolve. The full 29-key map:

| Mockup key | Product key | Decision | Notes |
|---|---|---|---|
| `action_save` | `button_save` | rename | identity meaning |
| `action_cancel` | `button_cancel` | rename | identity meaning |
| `action_back` | `button_back` | rename | identity meaning |
| `action_next` | `button_continue` | rename | mockup says "Next", product says "Continue" — same role |
| `action_confirm` | `button_confirm` | rename | identity meaning |
| `action_create` | `button_create` | rename | identity meaning |
| `action_delete` | `button_delete` | rename | identity meaning |
| `action_edit` | (no product equivalent) | add | new for inline "Edit" buttons in mockup; product has used inline links so far |
| `action_view` | (no product equivalent) | add | mockup uses "View" affordances on cards; new key |
| `action_activate` | (no product equivalent) | add — but **gated**: `signing_key.activate` is in `danger-D3` (do-not-implement-yet) | RFC-MI-031 may defer this entirely |
| `action_publish` | (no product equivalent) | add — same gating as above (`signing_key.publish` is not adopted) | RFC-MI-031 |
| `action_retire` | (no product equivalent) | add — same gating | RFC-MI-031 |
| `action_user_resume` | (no product equivalent) | add — product's "resume" is a checkbox-like re-enable, not a button verb. Default: surface as `button_re_enable_user`. | RFC-MI-031 |
| `action_user_suspend` | `button_disable_user` | rename + reword — product uses "disable", mockup uses "suspend". **Default:** keep "disable" (product vocabulary is settled). | RFC-MI-031 |
| `action_user_delete` | (per-context `confirm_screen_delete_user_*` keys exist) | rename | RFC-MI-051 |
| `action_user_mfa_reset` | (per-context `confirm_screen_reset_mfa_*`) | rename | RFC-MI-051 |
| `action_user_force_logout` | (no product equivalent; danger-D1 default is to fold into disable) | **do-not-implement-yet** | RFC-MI-031 |
| `action_client_delete` | (per-context `confirm_screen_delete_client_*`) | rename | RFC-MI-051 |
| `action_client_secret_rotate` | (no key — inline button copy) | add | RFC-MI-031 |
| `action_signing_key_delete` | (per-context `confirm_screen_delete_signing_key_*`) | rename | RFC-MI-051 |
| `action_signing_key_publish` | (gated, see above) | do-not-implement-yet | — |
| `action_signing_key_retire` | (gated) | do-not-implement-yet | — |
| `action_me_mfa_disable` | (per-context confirm copy) | rename | RFC-MI-060 |
| `action_me_mfa_regen_recovery` | (per-context copy) | rename | RFC-MI-060 |
| `action_me_passkey_delete` | (per-context copy) | rename | RFC-MI-060 |
| `action_me_session_revoke` | (per-context copy) | rename | RFC-MI-060 |
| `action_me_sessions_revoke_all` | (per-context copy) | rename | RFC-MI-060 |
| `action_sessions_revoke_all` | (admin global — do-not-implement, see danger-D4) | do-not-implement-yet | — |
| `action_settings_update` | (no product equivalent — product does not have a "Review changes" button; updates are inline) | **default: do-not-implement-yet** unless settings-step-up adopted | RFC-MI-051 |

### Resolution for the action_* cluster

- **8 keys** → rename to existing `button_*` (mechanical edit in the
  screen-level RFC).
- **3 keys** → add new `button_*` keys (`button_edit`, `button_view`,
  `button_re_enable_user`).
- **13 keys** → resolved by per-context confirm-screen copy (already
  exists in the product as `confirm_screen_*`).
- **5 keys** → do-not-implement-yet (tied to gated dangerous
  actions).

**Net new strings from the `action_*` cluster: 3** (all simple verbs:
"Edit", "View", "Re-enable user").

## Genuinely new copy — `impact_*` cluster (17 keys, RFC-MI-051)

The "impact summary" is mockup-introduced UX: a structured paragraph
on the confirm screen that lists exactly what will change. The
product currently uses one prose sentence per `render_confirm_*`.

Net new strings: 17.

Sample (proposed keys, mockup as-is, requires translation):

| Proposed product key | Mockup key | English (mockup) | Translation status |
|---|---|---|---|
| `impact_section_label` | `impact_section_label` | "Impact summary" | ja, zh needed |
| `impact_users_disable` | `impact_user_suspend` | "The user will be unable to sign in until you re-enable them. Existing sessions are revoked immediately." | ja, zh needed |
| `impact_users_delete` | `impact_user_delete` | "All sessions, refresh tokens, MFA registrations, and audit references will be permanently removed. This cannot be undone." | ja, zh needed |
| `impact_clients_delete` | `impact_client_delete` | "All access tokens, refresh tokens, and authorization codes issued to this client will be revoked. Relying applications will lose access immediately." | ja, zh needed |
| `impact_signing_keys_delete` | `impact_signing_key_delete` | "Tokens signed with this key will no longer verify. Only retired keys past the retention period can be deleted." | ja, zh needed |
| `impact_me_mfa_disable` | `impact_me_mfa_disable` | "Two-factor authentication will be turned off. Your account will be protected by password alone." | ja, zh needed |
| `impact_me_passkey_delete` | `impact_me_passkey_delete` | "This passkey will be removed. You can register a new one anytime." | ja, zh needed |
| `impact_me_session_revoke` | `impact_me_session_revoke` | "The selected session will be signed out immediately." | ja, zh needed |
| `impact_me_sessions_revoke_all` | `impact_me_sessions_revoke_all` | "Every other session for your account will be signed out. Your current session is unaffected." | ja, zh needed |
| `impact_unknown_action` | `impact_unknown_action` | "No matching action found. Try the request again from the originating page." | ja, zh needed |
| `impact_severity_high_label` | `impact_severity_high` | "High impact" | ja, zh needed |
| `impact_severity_medium_label` | `impact_severity_medium` | "Medium impact" | ja, zh needed |
| `impact_severity_low_label` | `impact_severity_low` | "Low impact" | ja, zh needed |
| `impact_reversible_label` | `impact_reversible` | "Reversible" | ja, zh needed |
| `impact_irreversible_label` | `impact_irreversible` | "Irreversible" | ja, zh needed |
| `impact_audit_note_hint` | `impact_audit_note_hint` | "Add a short reason. This appears in the audit log." | ja, zh needed |
| `impact_audit_note_required_marker` | `impact_audit_note_marker` | "Required" | reuse `label_required` — no new key |

→ **16 net new keys** for RFC-MI-051 after deduplication.

Each requires:
- An entry in `crates/sui-id-i18n/src/strings.rs` (the canonical
  struct).
- A `&'static str` value in each of `locale/en.rs`, `locale/ja.rs`, `locale/zh_hans.rs`.
- **Anti-enumeration / security review**: the impact-summary copy
  must not leak information (e.g. existence of a refresh token
  family). The example above does — that is acceptable for the
  *operator's* confirm screens (admins know the system state). For
  user-side screens (`me_*`), copy must stay in user-visible terms.

## Anti-enumeration review markers

Per RFC-MI-000 §7 — every new visible string with security-review
implication is flagged. Across all 382 mockup-only keys:

| Category | Count | Where | Decision |
|---|---|---|---|
| **Anti-enumeration sensitive** (login, forgot-password, reset, MFA failure copy) | 7 mockup keys | `/login`, `/mfa`, `/forgot-password*` | **Must be reviewed by security reviewer before any change.** Defaults: keep product wording, which has already passed RFC 041 / RFC 044. RFC-MI-041 explicitly preserves anti-enumeration wording. |
| **Audit-row copy** | 18 mockup keys | `/admin/audit` rows | Must match the product's controlled audit-event vocabulary. RFC 060 (audit-note rollout) fixed the wording for v0.45+. No change without RFC. |
| **State-word vocabulary** | 9 mockup keys (`state_*`) | badges, status columns | RFC 044 froze the product's status vocabulary ("Active", "Disabled", "Pending", "Off", "In use", "Retired", "Published", "Healthy", "Unhealthy"). The mockup's `state_*` keys map onto the existing badge keys — **no new state word is permitted** without an RFC. |
| **OIDC consent scope copy** | 3 mockup keys (`consent_*`) | `/consent` | All three are already in the product (`consent_scope_openid`, `consent_scope_profile`, `consent_scope_email`). identity mapping. |
| **Setup token / security disclosure** | 5 mockup keys (`setup_token_*`, `setup_locked_*`) | `/setup`, startup banner | Already covered by product copy (v0.48.4's URL-parameter setup-token UX is preserved). |
| **Dev-mode banner** | 8 mockup keys (`dev_*`) | dev-mode chrome | Mostly absorbed by RFC 032 (the product's dev-mode banner). RFC-MI-040 confirms. |

## Translation completeness invariant

The product's `Locale::strings()` match is exhaustive — every key in
the `Strings` struct must have a value in every locale file. The
mockup's i18n module enforces the same invariant (Rust type system).
Therefore:

> **Every net-new key added by an MI RFC must include ja, en, and zh
> values, even if the Chinese surface is hidden from the
> setup-wizard picker (spec §11.10).** Hiding zh from the picker is
> a UX choice; the translation table stays complete.

The migration plan §11 leaves "Chinese locale visibility" as a
deferred decision (D-11). For the i18n delta inventory, **zh is in
scope for every new key.**

## Aggregate

Of 382 mockup-only keys:

- **~280 keys** → rename / reword onto existing product keys (no
  new translation work).
- **~50 keys** → screen-specific copy that exists in the product
  under a per-screen name (e.g. `dashboard_*` keys exist in both
  but reworded).
- **~40 keys** → genuinely new copy (`impact_*`, audit-filter
  copy if filter UI ships, dev-mode disclosure).
- **~12 keys** → do-not-implement-yet (tied to gated dangerous
  actions).

### Net new translation work, by phase

| RFC | Net new keys (est.) | Translation effort |
|---|---|---|
| RFC-MI-020 (Shell) | 0 — admin shell copy is already covered | none |
| RFC-MI-030 (Dashboard) | ~10 — new card titles, sparkline tooltips | small |
| RFC-MI-031 (Audit + tables) | ~12 — chain-status copy, filter UI if shipped | small-medium |
| RFC-MI-040 (Setup) | ~4 — gate-state copy (Closed / Locked / Allowed / AllowedDev) | small |
| RFC-MI-041 (Auth) | 0 — anti-enumeration wording preserved | none |
| RFC-MI-050 (Form system) | ~3 — `button_edit`, `button_view`, `button_re_enable_user` | trivial |
| RFC-MI-051 (Danger / confirm) | ~16 — impact-summary copy | medium |
| RFC-MI-060 (Self-service) | ~6 — recovery-codes-view copy that survives the fold | small |
| RFC-MI-070 (Consent) | ~2 — anti-phishing clarity copy | small |
| RFC-MI-080 (Regression / a11y) | ~5 — focus-state announcements, reduced-motion notes | small |
| **Total** | **~58 net new keys** | spread across phases |

**~58 net new keys × 3 locales = ~174 new translation entries.** This
is *much* smaller than the apparent 382-key delta because the mockup
key vocabulary is largely a renaming exercise on existing concepts.

## Acceptance criteria (Phase 0)

- [x] Every cluster of mockup-only keys is classified (rename /
  reword / add / do-not-implement).
- [x] The `action_*` cluster (largest pure-rename group) is mapped
  exhaustively.
- [x] The `impact_*` cluster (largest pure-add group) is mapped
  exhaustively.
- [x] Anti-enumeration / audit-row / state-word sensitive copy is
  flagged for security review.
- [x] Net new translation work is estimated by phase.
- [x] Locale completeness invariant (ja + en + zh on every key) is
  restated.

## Decisions surfaced

| ID | Subject | Default | RFC owner |
|---|---|---|---|
| **i18n-D1** | Mockup `state_active/suspended/...` vocabulary | Reject — preserve product's RFC 044 frozen vocabulary | RFC-MI-031 |
| **i18n-D2** | `user.suspend` wording ("suspend" vs "disable") | Keep product "disable" | RFC-MI-031 |
| **i18n-D3** | Settings step-up button ("Review changes") | Do not introduce — settings updates remain inline | RFC-MI-051 (depends on danger-D5) |
| **i18n-D4** | zh visibility | Out of scope for i18n delta; translate but keep hidden in picker per D-11 | — |
| **i18n-D5** | Audit filter copy (`filter_*` cluster) | Defer all 12 keys until filter UI is approved | RFC-MI-031 |
