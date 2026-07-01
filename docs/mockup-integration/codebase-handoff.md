# sui-id v0.48.4 — UI/UX Mockup Integration HANDOFF

**Purpose.** A UI/UX mockup has reached near-production-ready maturity. This
document is the **detailed birds-eye view** an architect needs to plan and
design its integration into the current sui-id codebase. It captures the
current rendering stack, the design-system surface, the i18n / state /
auth contracts the mockup must respect, the CI guardrails that will
arbitrate any integration PR, and the open questions the architect should
resolve as part of the integration design.

**Out of scope.** The HANDOFF does *not* dictate integration choices or
prescribe a particular implementation path. It describes the **terrain**
so the architect can choose a route.

**Format.** English Markdown. Section anchors are stable; cross-references
inside this document use them.

---

## 1. Project at a glance

`sui-id` is a **self-hostable, single-realm OIDC provider written in
Rust 2024**. The codebase has been hardening through a structured release
arc (Phase A → F) for the better part of the year; it is currently in a
**verification phase** ahead of any v1.0 designation. (The project owner
has stated explicitly: no v1.0 candidate tag — rc, pre, beta — is
scheduled until sufficient soak, external review, and integration
verification.)

- **Current version**: `v0.48.4`
- **Workspace edition**: 2024 (`rust-version = "1.91"`)
- **License**: Apache-2.0, author `nabbisen`
- **Backing store**: SQLite (single file), encrypted with ChaCha20-Poly1305
- **HTTP**: axum 0.8 + tower-http 0.6
- **Rendering**: Leptos SSR (server-rendered to a single HTML string)
- **OIDC surface**: full code+PKCE, refresh tokens, userinfo, JWKS,
  RP-initiated logout, OIDC discovery
- **Admin / self-service**: full Leptos-rendered web UI under
  `/admin/*` and `/me/security/*`
- **i18n**: three locale tables (Ja / En / Zh), ~620 string keys each;
  Zh has been intentionally hidden from the setup-wizard picker
  (see §7 "Known scope limits")

## 2. Workspace layout

```
sui-id/
├── Cargo.toml                     # virtual workspace
├── CHANGELOG.md                   # release notes (extensive history)
├── ROADMAP.md                     # phase + status
├── README.md                      # short
├── PUBLISHING.md
├── TERMS_OF_USE.md
├── sui-id.example.toml            # config example
├── crates/
│   ├── sui-id/                    # binary + handlers + router + assets
│   ├── sui-id-core/               # use-case layer, OIDC engine, state machines
│   ├── sui-id-i18n/               # 3 locale string tables (one file each)
│   ├── sui-id-shared/             # shared IDs, AuthMethod enum, DTOs
│   ├── sui-id-store/              # SQLite repositories, encryption, models
│   └── sui-id-web/                # ← Leptos SSR layer (UI lives here)
├── docs/                          # mdbook-compatible
│   ├── src/
│   │   ├── SUMMARY.md
│   │   ├── introduction.md
│   │   ├── getting-started/{quick-start,overview,faq}.md
│   │   ├── guides/{operators,deployment,upgrade,dangerous-operations}.md
│   │   ├── reference/{configuration,oidc-api,audit-events}.md
│   │   └── contributing/{architecture,local-dev,state-contract,translators}.md
│   └── ui-ux-contracts.md         # ← frozen cross-cutting UI/UX contracts
└── rfcs/
    ├── README.md
    ├── done/                       # 60+ implemented RFCs
    └── proposed/                   # open (post-1.0 candidates)
```

The architect's primary surface for mockup integration is **`crates/sui-id-web/`**.

## 3. The rendering stack (sui-id-web)

### 3.1 Crate layout

```
crates/sui-id-web/src/
├── lib.rs                       49 LOC — re-exports the public render_*
│                                         + UI Data structs
├── tokens.rs                   298 LOC — CSS custom properties
│                                         (--space-*, --fg-*, --accent-*, etc.)
├── components.rs              1094 LOC — component CSS + utility classes
│                                         + status_badge component
├── layout.rs                   232 LOC — Shell, AuthShell, Nav, ThemeToggle,
│                                         Footer (the page chrome)
├── pages.rs                     48 LOC — umbrella that re-exports submodules
└── pages/
    ├── common.rs                    — empty_state, table_empty_row helpers
    ├── audit.rs               5.0K
    ├── auth.rs                 19K  — login, MFA challenge, MFA setup,
    │                                  password change, step-up, forgot/reset
    ├── clients.rs              16K  — OAuth client CRUD screens
    ├── confirm.rs              13K  — RFC 030 dangerous-op confirmation screens
    ├── dashboard.rs            16K  — admin overview with sparkline + recent
    │                                  events
    ├── error.rs               1.8K  — 401/404/429/500 error page
    ├── me_security.rs         1.4K  — umbrella for the tab-split self-service
    │                                  surface (RFC 040)
    ├── me_security/
    │   ├── overview.rs         (76)
    │   ├── mfa.rs             (125)
    │   ├── sessions.rs        (109)
    │   ├── passkey.rs         (126)
    │   ├── language.rs         (78)
    │   └── security.rs        (265)  ← password change inside the tab shell
    ├── oidc.rs                2.6K  — OIDC consent screen
    ├── settings.rs            2.1K  — umbrella for /admin/settings tabs
    ├── settings/
    │   ├── basic.rs           (142)
    │   ├── authentication.rs  (117)
    │   ├── email.rs           (146)
    │   ├── security.rs        (153)
    │   ├── logs.rs            (106)
    │   └── other.rs           (105)
    ├── setup.rs                14K  — 5-step setup wizard pages
    ├── signing_keys.rs        4.8K
    └── users.rs                15K  — list + detail + creation form
```

Every `.rs` file in `crates/` is under the project spec's 500-LOC
**recommended** ceiling for the screen domains (the larger files
above — `auth.rs`, `clients.rs`, `users.rs`, `dashboard.rs`,
`setup.rs`, `confirm.rs` — are flat single-domain files that resist
useful splitting). The split policy was the v0.47.0–v0.48.0 Phase F
arc; the architect should respect that ceiling for any new files
introduced by the mockup.

### 3.2 The render-string pattern

Handlers return rendered HTML as a `String`. The architectural
shape is:

```rust
// In handlers/admin/dashboard.rs:
pub async fn dashboard(...) -> Result<Response, HttpError> {
    let data: DashboardData = ...build from DB ...;
    let body = sui_id_web::render_dashboard(data, flash, dev_mode, lang);
    Ok(Html(body).into_response())
}
```

```rust
// In sui-id-web::pages::dashboard:
pub fn render_dashboard(
    data: DashboardData,
    flash: Option<Flash>,
    dev_mode: bool,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || view! { ... })
}
```

**Properties of this pattern that matter for the mockup integration:**

- The boundary between **handler-side data assembly** and
  **rendering-side presentation** is a Rust struct (`DashboardData`,
  `UserDetailData`, `MeShellData`, etc., all defined in
  `crates/sui-id-web/src/pages/*.rs`).
- The `render` helper is a thin wrapper around Leptos' SSR machinery
  that returns a complete HTML document (`<!doctype html>` + full
  `<html>` tree). No hydration: there is no Wasm bundle; client-side
  interactivity comes from three small hand-written `.js` files
  served at `/static/*`.
- The data structs are **owned** (no lifetimes); they cross the
  handler→render boundary by value. This matters because Leptos'
  `view!` closure needs `'static` captures.

### 3.3 Top-level layout: Shell vs AuthShell

There are exactly **two** page shells (`crates/sui-id-web/src/layout.rs`):

| Shell       | Used by                                                  |
|-------------|----------------------------------------------------------|
| `Shell`     | All authenticated admin screens (`/admin/*`, `/me/security/*`) |
| `AuthShell` | Login, MFA challenge, password change, reset, setup wizard, error pages |

`Shell` renders the app header (with brand + admin nav row + sign-out
button) and the app footer (tagline + a11y badges + theme toggle +
version). `AuthShell` is a stripped-down variant: brand only,
centred narrow card body, same footer. Both shells:

- Set the `<html lang>` and `<html dir>` attributes from the
  passed-in `Locale`.
- Inline the `tokens.rs` + `components.rs` CSS into `<style>` so
  there is no separate stylesheet network request.
- Load three external scripts with `defer`:
  `/static/theme-init.js`, `/static/copy.js`,
  `/static/logout-csrf.js` (the latter only via `Shell`).

The `<title>` is `{page-title} · sui-id`.

### 3.4 The 28 `render_*` public entry points

`sui-id-web::lib.rs` re-exports exactly these functions; this is the
complete public surface that handlers call:

```
render_audit          render_consent             render_setup_admin
render_clients        render_dashboard           render_setup_done
render_client_edit    render_error               render_setup_hibp
render_confirm_*      render_forgot_password     render_setup_lang
render_login          render_forgot_password_sent render_setup_welcome
render_mfa_challenge  render_password_change     render_signing_keys
render_mfa_setup      render_reset_password      render_user_detail
render_signing_keys   render_reset_password_invalid render_users
render_step_up
```

Plus tab-aware `render_me_overview`, `render_me_mfa`,
`render_me_sessions`, `render_me_passkey`, `render_me_language`,
`render_me_security` (the password-change tab), and
`render_settings_*` for each settings tab.

**For mockup integration**: every screen the mockup defines maps to
exactly one of these functions, or motivates adding a new one. The
naming convention is `render_<page-stem>`; new files in `pages/`
typically expose one `render_*` + one `*Data` struct.

## 4. Design system

### 4.1 Tokens (`tokens.rs`)

A small, deliberately-restricted CSS custom property vocabulary
(RFC 049 freezes it; CI gate `css-tokens` will reject typos and
references to undefined tokens). Categories:

- **Spacing**: `--space-1` through `--space-6` (8 / 12 / 16 / 24 /
  32 / 48 px).
- **Foreground**: `--fg-default`, `--fg-muted`, `--fg-on-accent`.
- **Surface**: `--surface-default`, `--surface-subtle`, `--surface-elevated`.
- **Accent**: `--accent-default`, `--accent-subtle`.
- **Semantic palette** (RFC 061): `--danger-default`, `--danger-subtle`,
  `--fg-on-danger`; same triple for `warning`, `success`, `info`. CI gate
  `semantic-palette-parity` ensures all 12 names are defined in
  light / dark / auto-dark modes.
- **Border / radius / state**: `--border-muted`, `--border-strong`,
  `--border-width-default`, `--radius-sm`, `--radius-md`, `--state-hover`,
  `--state-active`.
- **Typography**: `--font-size-caption`, `--font-size-body`, `--font-size-h1`
  through `h4`, `--font-weight-medium`, `--font-family-system`.
- **Layout**: `--content-max-width` (64rem), `--content-narrow-width`
  (28rem).

The mockup will need to either **map its design tokens onto this
vocabulary** or **propose extensions through a new RFC** that goes
through the existing token-freeze review process.

The mode logic: `[data-theme]` attribute on `<html>` selects light vs
dark. `theme-init.js` sets it from `localStorage`; absence means
"system" (the `:root:not([data-theme])` selector picks up
`prefers-color-scheme`).

`::selection { background: var(--accent-default); color: var(--fg-on-accent) }`
(v0.48.2 fix).

### 4.2 Components (`components.rs`)

A single 1094-LOC file containing one giant raw-string constant
(`COMPONENTS_CSS`) plus exactly one rendered Rust component
(`status_badge` with a `StatusKind` enum).

The CSS is organised in declared families:

- App chrome: `.app-header`, `.app-nav`, `.app-nav__link`,
  `.app-main`, `.app-footer`, `.app-footer__a11y`, `.theme-toggle`.
- Auth surface: `.auth-card`.
- Cards: `.card`, `.card__title`, `.card__action`.
- Forms: `.field`, `.field__label`, `.field__hint`, `.field--required`,
  inputs by type.
- Tables: `table`, `thead th`, `tbody td`, `.table-wrap`,
  `.cell-wrap` (v0.48.2 opt-out for wrapping cells).
- Buttons: `.button`, `.button.secondary`, `.button.danger`,
  `.button.ghost`.
- Banners + flash: `.banner`, `.banner--success`, `.banner--warning`,
  `.banner--danger`, `.banner--info`.
- Badges: `.badge`, `.badge--ok`, `.badge--danger`, `.badge--info`,
  `.badge--muted`.
- Layout primitives: `.stack`, `.stack-tight`, `.row`, `.grid-cards`.
- Confirmation screens (RFC 030): `.confirm-shell`, `.confirm-action`.
- Empty states: `.empty-state`, `.empty-state__icon`, `.empty-state__action`.
- Dashboard primitives: `.sparkline`, `.recent-event-list`.
- Setup wizard: `.setup-lang-picker`, `.setup-step-indicator`.
- Tabs: `.me-tabs`, `.me-tabs__link`.

**v0.48.0+ utility-class layer** (RFC 067, CI gate `inline-style-bound`
at 20):

- Margin: `.mt-{1..5}`, `.mb-{0..4}`, `.ml-{1,2}`, `.mt-2-mb-0`.
- Gap: `.gap-{1,2,3}`, `.gap1-center`.
- Flex / alignment: `.center`, `.items-center`, `.items-end`,
  `.justify-end`, `.justify-between`, `.flex-1`, `.flex-0-auto`,
  `.row-gap2-center`, `.row-gap2-center-clickable`, `.row-gap3-center`.
- Widths: `.max-w-card` (36rem), `.max-w-narrow` (22rem), `.min-w-16rem`.
- Typography: `.text-caption`, `.text-small`, `.fw-medium`, `.fw-500`.
- Colour: `.color-accent`, `.color-danger`.
- Display: `.inline-el`, `.inline-block`.
- Patterned: `.kv-label-cell`, `.button-reset`, `.clickable-block`,
  `.radio-hint`, `.center-pad-4`, `.center-pad-6`, `.center-pad-6-muted`,
  `.ul-indent`.

**v0.48.2 responsive layer** — a single `@media (max-width: 768px)`
breakpoint reduces padding, makes the nav horizontally scrollable,
collapses the footer to a column, and reduces card padding. This is
the **only** media query in the codebase. Anything narrower than
~480px or wider than tablet still uses the desktop layout.

### 4.3 Layout components

`crates/sui-id-web/src/layout.rs` exposes exactly three Leptos
components and one helper:

| Item        | Props                                                    | Notes                                                              |
|-------------|----------------------------------------------------------|--------------------------------------------------------------------|
| `Shell`     | `title`, `lang`, `current` (nav highlight key), `show_nav`, `children` | Authenticated admin pages                                          |
| `AuthShell` | `title`, `lang`, `children`                              | Login, setup, error, password-change-outside-tab                   |
| `Footer`    | `lang` — internal                                        | Used by both shells                                                |
| `Nav`       | `lang`, `current`, `csrf_token` — internal               | Built inside `Shell`; `csrf_token` is currently always `""` (see Q5 in §10) |
| `ThemeToggle` | `lang` — internal                                      | Three buttons with `data-theme-value`                              |

Neither shell is generic. If the mockup introduces a third top-level
shell (e.g. for a wider dashboard layout, or for a marketing-style
landing surface), the architect should plan it as a sibling
component rather than parameterising `Shell` further.

## 5. Handler / state contract

### 5.1 Routing

`crates/sui-id/src/router.rs` (~290 LOC) registers ~80 routes. The
public OIDC routes (`/.well-known/*`, `/oauth2/*`) and admin routes
(`/admin/*`, `/me/security/*`) are co-located in one function. The
architect should not expect to add routes for the mockup without
landing them here.

Routes that need particular awareness during mockup integration:

- `GET /` — index router. Redirects to `/setup` if uninitialised,
  `/admin` if initialised. **Latent issue**: it always redirects
  initialised installations to `/admin` regardless of authentication
  state, which produced a 401 lock-out loop in v0.48.0 (fixed in
  v0.48.1 by redirecting `Unauthenticated` errors to `/admin/login`).
- `GET /setup?token=…` — wizard entry. The token is now a URL
  parameter (v0.48.4), not a text field.
- `GET /admin` — dashboard. Auth-gated by `CurrentAdmin` extractor.
- `GET /admin/login` — login form.
- `POST /admin/logout` — uses `/static/logout-csrf.js` to populate
  a hidden CSRF input from the cookie (workaround for not threading
  CSRF through `Shell`; see §10 Q5).
- `GET /me/security/{overview,mfa,sessions,passkeys,language,password}`
  — six tab pages (RFC 040). Each is a separate handler in
  `crates/sui-id/src/handlers/me_security/`.

### 5.2 Authentication / authorisation contracts

Two extractors define who is allowed where:

- `CurrentUser(UserId)` — has a valid `sui_id_session` cookie.
- `CurrentAdmin(UserId)` — is `CurrentUser` *and* the user row has
  `is_admin = 1`.

A failure of either yields `CoreError::Unauthenticated`. The HTML
representation of this error redirects to `/admin/login`
(v0.48.1 fix); the JSON representation returns HTTP 401 with a
proper error body.

**For the mockup**: any new page must declare its auth requirement
through the extractor it takes. There is no global "require auth"
middleware; auth is per-handler.

### 5.3 CSRF model

`crates/sui-id/src/csrf.rs` issues a `sui_id_csrf` cookie (not
HttpOnly: pages need to read it for form submissions). The `enforce_csrf`
function called inside each POST handler validates that the form's
`_csrf` field matches the cookie. The cookie is renewed on every
authenticated render.

The architect should note: the current Shell does **not** thread the
CSRF value into the rendered form. Forms either include an
`<input type="hidden" name="_csrf" value="{token}">` populated by
the page-specific renderer (this is the common pattern), or — in
the case of the admin nav sign-out form — rely on
`/static/logout-csrf.js` to read the cookie and populate the input
client-side.

If the mockup introduces any new POST endpoint, the renderer must
pass the CSRF value into the form. The `Shell` component currently
accepts `csrf_token: String` but the call sites pass `""` (see Q5).

### 5.4 Flash messages

`Flash { kind: FlashKind, text: String }` is the standard
inter-request banner. Handlers build a `Flash` for warnings /
success messages and pass `Some(Flash)` to the render function;
pages display it through a `flash_banner` helper (defined per-page,
not globalised — minor inconsistency the architect may want to
address as part of the mockup integration).

`FlashKind` is `Success | Warn | Error | Info`. The banner styles
map to the four `.banner--*` CSS classes.

## 6. i18n architecture

### 6.1 Locales and tables

`crates/sui-id-i18n/` holds translation tables under `src/locale/`:

- `locale/en.rs` (~850 LOC) — English strings + formatters
- `locale/ja.rs` (~835 LOC) — Japanese strings + formatters
- `locale/zh_hans.rs` (~830 LOC) — Simplified Chinese strings + formatters
- `locale/zh_hant.rs` — Traditional Chinese stub (delegates to zh_hans)

Plus `strings.rs` (~870 LOC), the canonical `Strings` struct definition.
The structure is a ~620-field struct with `&'static str` per field; each
locale file constructs one literal-string instance. The
`Locale::strings()` method returns the right one based on enum variant.

**~620 string keys** today. Recent additions (v0.48.2):
`me_overview_label_mfa_totp`, `me_overview_label_passkeys`,
`me_overview_no_recent_events`, `setup_welcome_lang_picker_label`.

### 6.2 Locale resolution at request time

`RequestLocale` extractor (`crates/sui-id/src/handlers.rs::~L348`)
uses three tiers, in order:

1. The authenticated user's `preferred_lang` column (if any).
2. The `sui_id_lang` cookie (if set).
3. The `Accept-Language` header (parsed by
   `sui_id_i18n::negotiate_from_accept_language`).
4. `Locale::default()` = `Ja`.

The setup wizard adds an explicit language picker at the top of the
welcome screen (v0.48.2); the picker uses `?lang=xx` → `LANG_COOKIE`
set → PRG to a clean URL.

### 6.3 Direction

Currently all three locales are LTR; the `dir_attr` in `Shell` is
always `"ltr"`. The mockup does not need to handle RTL.

### 6.4 CI gates that constrain UI text

- **text-leaks**: `>t.foo<` patterns (Leptos treats bare identifiers
  outside `{}` as literal text). 48 sites leaked at v0.41.0; the
  invariant has held at 0 since v0.42.0.
- **per-screen-i18n-completeness**: spot-checks for hardcoded English
  in screens that should be fully localised.
- The mockup-integration plan should include how it will pass these
  gates by construction (e.g. always using `t.field_name` from the
  Strings table, never literal strings).

## 7. Known scope limits and intentional gaps

Reading the codebase, the architect will encounter the following
patterns that are intentional, not omissions:

- **No client-side framework / no Wasm**. Leptos is used in SSR-only
  mode. The three `.js` files (`theme-init.js`, `copy.js`,
  `logout-csrf.js`) are hand-written. The mockup cannot assume a
  React-style render tree, hydration, or virtual DOM.
- **No fonts loaded**. The font stack is system-only
  (`--font-family-system`); the mockup must work with whatever the
  OS provides.
- **No image assets** except the favicon and an SVG logo (under
  `crates/sui-id/static/`). The mockup cannot rely on raster
  illustration; small inline-SVG components are the precedent
  (see `sparkline` rendering in `pages/dashboard.rs`).
- **No third-party CSS**. No Tailwind, no Bootstrap. The utility-class
  layer in `components.rs` is hand-rolled and bounded.
- **Two languages in the picker** (`日本語 / English`), even though
  `locale/zh_hans.rs` (Simplified Chinese) and the `zh-Hans` stub in
  the language-preference picker exist. The Chinese table is kept
  current but is not in `Locale::ALL` pending a full copy review.
  Traditional Chinese (`locale/zh_hant.rs`) is a stub awaiting a
  contributor.
- **One breakpoint** at 768 px. Tablet-and-narrower gets simplified
  layout; everything wider uses desktop.
- **No animations** beyond CSS transitions on hover. `prefers-reduced-motion`
  is honoured.
- **Files larger than 500 LOC** are not all split. The Phase F policy
  split the three large outliers (`pages.rs`, `handlers/admin.rs`,
  `handlers/me_security.rs`); ten other files (i18n string tables,
  sui-id-core state machines, `backup.rs`, `handlers/oidc.rs`)
  remain over the recommendation because splitting them would
  harm cohesion. The mockup should not motivate further large
  files; new files should aim for 200-300 LOC each.

## 8. CI invariants the integration must respect

Each of these runs on every PR in `.github/workflows/ci.yml`:

| Job                          | Established by | Check                                                          |
|------------------------------|----------------|----------------------------------------------------------------|
| `build + test`               | always         | `cargo build`, `cargo test` (228/228 today); warnings → error  |
| `fmt + clippy`               | always         | `cargo fmt --check`, `cargo clippy -D warnings`                |
| `text-leak invariants`       | RFC 048        | 0 occurrences of `>t\.[a-z_0-9]+<` in `crates/`                |
| `css-tokens`                 | RFC 049        | every `var(--name)` resolves to a token in tokens.rs / components.rs |
| `semantic-palette-parity`    | RFC 061        | 12 semantic tokens × 3 modes = 36 declarations                 |
| `inline-style-bound`         | RFC 067        | `style="…"` count in `pages/**.rs` ≤ 20 (currently 16)         |

The mockup integration cannot regress any of these. If the mockup
needs additional inline styles, they must be migrated to utility
classes (the existing approach) or motivate a new utility-class RFC.

## 9. UI/UX contracts (the existing rulebook)

`docs/ui-ux-contracts.md` is **the** cross-cutting contract document.
Every UI-touching RFC inherits it. The architect should read it in
full before designing the integration; key sections:

- **§1 Screen relation map** — five isolated streams: Uninitialised,
  Login, OIDC, Admin, Self-service. Mock screens belonging to one
  stream must not bleed into another's chrome.
- **§ Dangerous-operation confirmation pattern** (RFC 030) —
  destructive POSTs always traverse a `/admin/.../delete-confirm`
  GET first, never inline confirms. The confirm page takes a
  `ConfirmScreenData` and emits the standard `confirm-shell`
  layout.
- **State word vocabulary** (RFC 044) — controlled vocabulary for
  status labels: "Active", "Disabled", "Pending", "Off", "In use",
  "Retired", "Published", "Healthy", "Unhealthy". The
  `status_badge` component is the only renderer; new states require
  an RFC.
- **Audit-row copy** (RFC 046) — every row with an opaque ID gets
  a "Copy id" button; the per-row data carries `data-copy="..."`
  and `data-copy-done="..."`; the page-wide `copy.js` handles
  the rest.

## 10. Open questions the architect should resolve

These are integration-level decisions that the HANDOFF deliberately
leaves unanswered:

**Q1. Page-level vs component-level adoption.** Does the mockup
replace whole screens (preserving the render-function boundary) or
introduce a parallel component layer? Either is feasible; both have
costs. A whole-screen approach minimises new public API but loses
mockup component reusability; a component-layer approach is closer
to the mockup's likely source-of-truth but multiplies the
component surface.

**Q2. Token vocabulary delta.** What tokens does the mockup require
that the current `tokens.rs` doesn't provide? Each new token either
extends the freeze (with a new RFC for the freeze policy) or maps
onto an existing one. The architect should produce a *complete*
token-delta table as part of the design.

**Q3. Component CSS organisation.** Today `components.rs` is one
1094-LOC file. The mockup may motivate splitting it. If so: per
RFC 067 the inline-style discipline applies across the split, and
the file-size policy applies to each shard.

**Q4. Tab structure**. `/me/security/*` (6 tabs, RFC 040) and
`/admin/settings/*` (6 tabs) both use a hand-rolled tab pattern
(separate routes, server-rendered). The mockup may propose a
single tab-helper component. Constraint: tabs must remain
deep-linkable (each tab is a distinct URL).

**Q5. CSRF and Shell threading.** The current Shell does not
receive a per-request CSRF token; the sign-out form relies on
`/static/logout-csrf.js`. The "proper" fix (CSRF server-render
through every `render_*` call site) was deferred from v0.48.1.
The mockup integration is an opportunity to address it if the
shell layer is being touched anyway.

**Q6. Flash unification.** Each page currently defines its own
`flash_banner` helper. A genuine shared component
(`flash_banner`, defined once in `components.rs`) is a
sensible by-product of any integration; the architect should
either bundle it or note its non-inclusion.

**Q7. Responsive breakpoint policy.** Only one breakpoint at
768 px exists today. Does the mockup require a more granular
ladder (sm / md / lg / xl)? Each new breakpoint multiplies the
amount of code in `components.rs` and the testing surface; pick
the minimum that the mockup honestly requires.

**Q8. Asset pipeline.** Static JS lives at `crates/sui-id/static/`
and is served by an `include_dir!`-backed handler. If the mockup
introduces additional client-side behaviour (popovers, modals,
client-side validation), the architect should decide whether to
keep the "hand-written, defer-loaded" pattern or introduce a
build step. The current discipline is "no build step beyond
cargo" — breaking it is a significant policy choice.

**Q9. i18n key additions.** Every new piece of user-visible copy
in the mockup needs a key in the `Strings` struct + a value in each
file under `locale/` (`en.rs`, `ja.rs`, `zh_hans.rs`). The translation
effort scales linearly with the mockup's text density. The architect
should quantify this.

**Q10. Migration strategy.** Big-bang switch vs screen-by-screen
roll-in. The latter is the codebase's established pattern
(every UI RFC touched one screen group at a time). If big-bang
is preferred, the architect should justify it explicitly.

## 11. RFC + release process for mockup integration

The codebase has an established **lifecycle policy** in
`rfcs/000-rfc-lifecycle-policy.md`. The architect must follow it:

1. **One RFC per integration phase.** A mockup integration is too
   big for a single RFC. Decompose into stages — token-vocabulary
   extension, layout-shell adoption, screen-group migration, etc.
   Each is an RFC.
2. **Status flow**: Proposed → Implemented (file moves from
   `rfcs/proposed/` to `rfcs/done/` when its release tag ships).
3. **Versioning**: each release that ships an RFC is documented in
   both `CHANGELOG.md` (full prose) and `ROADMAP.md` (one-line row).
4. **Releases ship as `.tar.gz` archives** of the Cargo project
   structure, named `sui-id-v{X.Y.Z}.tar.gz`. The release process
   excludes `target/` and any nested release directories.
5. **No v1.0-* tag is scheduled.** The verification phase continues
   until external review, soak time, and integration verification
   are complete. Mockup integration is part of that arc; it does
   not justify a v1 tag by itself.

## 12. Where to start

Suggested orientation reading order for the architect:

1. `ROADMAP.md` — last ~3 release rows + the Status section.
2. `docs/ui-ux-contracts.md` — the cross-cutting contract.
3. `rfcs/done/049-css-token-vocabulary-freeze.md` — what's frozen
   in the token vocabulary and why.
4. `rfcs/done/061-semantic-palette-extension.md` — how the
   semantic palette was extended (the precedent for any new
   palette decisions the mockup requires).
5. `rfcs/done/067-inline-style-discipline.md` — the utility-class
   policy and the CI bound.
6. `crates/sui-id-web/src/layout.rs` — the page-shell entry points.
7. `crates/sui-id-web/src/pages/dashboard.rs` — a representative
   non-trivial page (dashboard with sparkline + recent events) to
   see the data-struct / render boundary in detail.
8. `crates/sui-id-web/src/pages/me_security/overview.rs` — a small
   tab page to see how the tab shell composes (`MeShellData`
   wrapping inner content).
9. `crates/sui-id/src/handlers/admin/dashboard.rs` — the matching
   handler, to see how data assembly maps to the render boundary.

After this orientation, the architect has the full surface in
hand and can write the first integration RFC.

---

## Appendix A — exact public render surface

For the architect's reference, the **complete** public render
surface as of v0.48.4 (re-exported in `crates/sui-id-web/src/lib.rs`):

```
pub use pages::{
    // Auth + login family
    render_login, render_mfa_challenge, render_mfa_setup,
    render_password_change, render_step_up,
    render_forgot_password, render_forgot_password_sent,
    render_reset_password, render_reset_password_invalid,

    // Admin family
    render_audit, render_clients, render_client_edit,
    render_dashboard, render_signing_keys, render_users,
    render_user_detail,

    // Dangerous-operation confirmations (RFC 030)
    render_confirm_disable_user, render_confirm_delete_user,
    render_confirm_reset_mfa, render_confirm_delete_client,
    render_confirm_delete_signing_key,

    // Self-service tab family (RFC 040)
    render_me_overview, render_me_mfa, render_me_sessions,
    render_me_passkey, render_me_language, render_me_security,

    // Settings tab family
    render_settings_basic, render_settings_authentication,
    render_settings_email, render_settings_security,
    render_settings_logs, render_settings_other,

    // Setup wizard
    render_setup_welcome, render_setup_admin, render_setup_lang,
    render_setup_hibp, render_setup_done,

    // OIDC + system
    render_consent, render_error,
};
```

## Appendix B — exact CI invariants snippet

(For copy into the integration-RFC's "Test plan" section.)

```
text-leaks:          grep -rEn '>t\.[a-z_0-9]+<' crates/ --include='*.rs'
                     → must return empty

css-tokens:          every `var(--name)` referenced in crates/ resolves
                     to a `--name:` declaration in tokens.rs or components.rs

semantic-palette:    for s in danger warning success info:
                         for slot in default subtle fg-on:
                             grep -cE "^[[:space:]]+--{s}-{slot}[[:space:]]*:" \
                               crates/sui-id-web/src/tokens.rs == 3

inline-style-bound:  grep -rEohn 'style="[^"]*"' crates/sui-id-web/src/pages/ \
                       --include='*.rs' | wc -l ≤ 20
```

## Appendix C — Verification-phase issue catalogue (post-v0.48.0)

Issues found by real-environment testing since the verification phase
opened. The architect should be aware of which are addressed vs open:

| ID | Issue | Resolved in | Notes |
|---:|---|---|---|
| 2 | CSP blocks inline `<script>` + `onclick=` | v0.48.1 | Externalised to `/static/*.js` |
| 3 | Sign-out redirect loop | v0.48.1 | Subsumed by #2 |
| 9 | 401 lock-out + "Back home" loops to /admin | v0.48.1 | `Unauthenticated` → `/admin/login` redirect |
| 1 | `::selection` invisible in light mode | v0.48.2 | `--accent-default` + `--fg-on-accent` |
| 5 | `/me/security/overview` hardcoded English labels | v0.48.2 | 3 new i18n keys × 3 locales |
| 4 | Setup wizard stuck in English | v0.48.2 | Explicit `日本語 / English` picker on welcome |
| 6 | Footer a11y labels look interactive but aren't | v0.48.2 | `<ul role="note">` passive badges |
| 7 | Title tagline too prominent | v0.48.2 | Caption-size + muted + opacity 0.75 |
| 8 | Mobile responsive: nav and tables vertical squish | v0.48.2 | First `@media (max-width: 768px)` |
| OIDC | ID token `email` claim missing | v0.48.3 | `IdTokenClaims.email` added |
| Setup-token UX | Token via text input | v0.48.4 | Now URL parameter |
| zh in setup picker | Misleading | v0.48.4 | Removed from picker (kept in strings) |

Known follow-ups (not yet scheduled to a release):

- `.cell-wrap` per-table annotations (so free-text columns wrap on
  narrow viewports while ID / timestamp columns stay single-line).
- `?return=` on the login redirect (requires open-redirect-safe URL
  validation).
- CSRF server-render through every `render_*` call site (so
  `logout-csrf.js` can be removed).

The mockup integration should not silently regress any of the
above resolutions.

---

*Generated for the v0.48.4 codebase. Refresh this HANDOFF if more
than two release cycles elapse before the integration starts.*
