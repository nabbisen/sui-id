# Upgrade guide

## General procedure

1. **Back up** before upgrading:
   ```bash
   sui-id backup create --config sui-id.toml --to sui-id-pre-upgrade.tar
   ```
   Do not use `cp`: the database runs in WAL mode, so a copy of the
   `.sqlite` file can miss committed data.

2. **Stop** the running instance (SIGTERM; sui-id finishes in-flight requests
   before exiting).

3. **Replace** the binary with the new version.

4. **Preview**, with the new binary, before starting it:
   ```bash
   sui-id upgrade preview --config sui-id.toml
   ```
   Read which migrations it lists, and confirm it reports `Nothing was
   changed.` It opens nothing for writing, needs no master key, and prints
   the same refusal line startup would if the database is not one this
   build can use, so you learn that here rather than from a service that
   will not start.

5. **Start** the new binary. Migrations run automatically on startup. If a
   migration will actually run, starting also takes an automatic snapshot
   of the database first (RFC 142 stage 5) — see "The pre-migration
   snapshot" below. Check stderr for any migration errors.

6. **Verify** — open the admin panel and confirm the version shown in
   Settings → Advanced.

## Migration behaviour

sui-id runs database migrations forward-only on startup. There is no
`down` migration. The schema version is recorded in the database, and each
migration runs once.

**You can upgrade straight to the current release from any earlier
one.** No upgrade has to pass through an intermediate version first,
however far behind you are — skip as many releases as you like in one
step. Before every release, a database populated at every schema version
this project has ever shipped is brought forward to that release and
checked to still hold its data, unchanged.

What that covers is the records the database keeps for you: accounts and
their MFA enrolments, credentials, clients, signing keys, scope
definitions, server and SMTP settings, consents, federation providers
and links, and revoked tokens. Short-lived state — an authorization code
or a WebAuthn ceremony in flight — is not covered, and some upgrades
discard it. Configuration-file keys and CLI flags can also change
between releases; the `CHANGELOG` records those, and is worth reading
before upgrading across more than one or two of them.

## Version-specific notes

### v0.93.x

**One migration runs, `0052_master_key_sentinel`.** Starting takes an
automatic pre-migration snapshot first, beside the database file (see
"The pre-migration snapshot" below).

**The master key is now checked at every start.** A key that does not
open the existing database is refused:

> the master key in `<path>` (fingerprint `<fp>`) does not open this
> database, whose sentinel was written under fingerprint `<fp>`;
> restore the matching key file, or set SUI_ID_MASTER_KEY to it.

and so is a missing key file over existing data, instead of a new key
being generated:

> no master key file at `<path>`, but this database holds data sealed
> under a master key (fingerprint `<fp>`). Generating a new key would
> make it unreadable. Restore the key file.

The first start after upgrading writes the sentinel. An operator who
has kept the right key file notices nothing; one who hasn't is refused
at start, which is the point.

**The command line is grouped by task** (RFC 148); every earlier form
still works and prints a one-line notice naming its new form — for
example `restore` → `backup restore`, `admin unlock-user` → `admin user
unlock`, `admin issue-recovery-link` → `admin user recovery-link`.
**A usage error (an unknown option, or a missing required value) now
exits with status 2, not 1** — a script that tells a usage error from a
real failure by the exit code must change to test for 2. `sui-id init`
writes a starting configuration (`--production` for the production
template).

**No configuration change.**

**Federation.** A federation provider whose DNS answer carries more
than 8 addresses, up to 32, now passes *Check and enable*. Google's
signing-key host answers with 16.
**A recovery link is now refused for a user linked to a federation
provider, and password sign-in refuses such a user** (RFC 103 T10). If
you issued a recovery link to a federated user before this release,
check the audit log for `user.recovery_link.issued`: that user may hold
a local password.

For anyone building against the crates:

- `sui-id-i18n`: `Strings::confirm_delete_federation_provider_impact`
  is replaced by `confirm_delete_federation_provider_impact_zero` and
  `_suffix`; many fields were added (a federation refusal's own
  sentence, and the unlink page's sign-in-method and follow-up text).
- `sui-id-store`: `RecoveryTarget` gained `has_federation_link`;
  `RecoveryRefusal` gained `TargetFederated`.
- `sui-id-web`: `ConfirmScreenData` gained `impact_count` and
  `impact_link` (new `CountedImpact`/`ImpactLink` types).
- `sui-id`: `PreflightError` and `CheckAndEnableError` are no longer
  `Copy`, `Clone`, `PartialEq` or `Eq` (their new variants carry
  non-comparable payloads); `KeyFileOps` gained `create_dir`,
  `list_dir` and `remove_dir`; `KeyFileMetadata` is no longer `Copy`
  (it gained `is_dir` and `symlink_target`).

### v0.92.x

**No migration and no configuration change.** Upgrading from `0.91.x`
runs no migration, so no pre-migration snapshot is taken. `sui-id.example.toml`
gained comments naming keys that already existed; no key was added, renamed
or removed.

**The console looks different**: a new palette, larger text that follows
the browser's font size, the account menu (with sign-out) on every page.

For anyone building against the crates:

- `sui-id-web`: `Shell` takes `nav: Chrome` in place of `show_nav` and
  `admin_username`; `MeShellData` carries `role` in place of `is_admin`.
- `sui-id-core`: `CreateClientSpec` has a required `consent_policy`.
- `sui-id`: `CurrentAdmin` and `CurrentAdminOrAuditor` carry the username.

### v0.91.x

**No migration and no configuration change.** Upgrading from `0.90.x`
runs no migration, so no pre-migration snapshot is taken.

**`sui-id admin rotate-key` now refuses when the current key came from
`SUI_ID_MASTER_KEY`** — see the operator guide's [*Rotating the master
key*](operators.md#rotating-the-master-key).

**Rotation is now durable against a power loss, except between its two
renames** — see the operator guide's [*Recovering a rotation that
stopped
partway*](operators.md#recovering-a-rotation-that-stopped-partway-corrected-three-times-2026-11-03-this-section-used-to-end-there-is-no-half-rotated-state-to-recover-from-which-is-not-true-of-a-failure-after-the-re-seal-commits-2026-11-04-it-still-described-the-key-loss-window-that-0871-had-already-closed-2026-11-14-it-said-the-new-key-was-written-to-disk-when-until-that-date-nothing-had-actually-synced-it-there-and-it-did-not-yet-say-the-re-seal-itself-is-made-durable-before-the-key-files-move),
which says what to do and what not to do.

**`sui-id-store`'s public API loses `users::set_last_login`, and
`signing_keys::insert_with_plaintext` now needs the `test-support`
feature.** Relevant only if you build against the crate directly, not
through the `sui-id` binary.

### v0.90.x

**Federated sign-in through every configured provider stops on upgrade,
until an administrator checks and re-enables each one.** Users keep
their accounts and their federation links — nothing is deleted. Password
and passkey sign-in are unaffected.

**Why:** an earlier release had no durable way to prove a provider's
configuration had actually been checked against its upstream. Existing
provider rows lack enough durable evidence to infer their endpoint
origins, algorithm policy, authentication method, or a safe preflight
(RFC 096), so this release disables every provider on upgrade rather
than trust a configuration nobody has re-verified.

**Two migrations run** (`0050`, `0051`) from `0.89.x`, so starting the new
binary **takes the automatic pre-migration snapshot** first — see "The
pre-migration snapshot" below for where it lands, its name and mode, and
that it is never deleted automatically. Going back to `0.89.x` means
restoring that snapshot, exactly as "Rollback" below describes.

**Before upgrading**, check every `[[federation_providers]]` block
against the stricter rules startup now enforces — see the [configuration
reference](../reference/configuration.md#federation_providers-rfc-004)
and the [operator guide's "Configure a
provider"](operators.md#federated-sign-in-upstream-oidc). A
block that fails one of them stops sui-id from starting; the refusal
names the provider and the field. **Run the new binary's `sui-id upgrade preview --config sui-id.toml`
first** (General procedure, step 4): it loads the configuration with these
rules without starting the server or writing to the database, so a block
that would refuse to start is reported now, by provider and field.

**After upgrading**, for each provider: open **Federation providers** in
the admin UI (`/admin/federation-providers`), then press **Check and
enable**. If it is refused, the message on the page names the next
step — see the operator guide's ["What a refusal
means"](operators.md#federated-sign-in-upstream-oidc) for the
complete list.

**What else changed for an operator**, each covered in the operator
guide:
- Editing a trust field (`issuer`, `client_id`, `client_secret_env`,
  `scopes`, `provision_mode`, `allowed_origins`, or `id_token_algs`) in
  the config file now applies on the next restart, and disables the
  provider until it is checked again.
- Removing a `[[federation_providers]]` block from the config now
  disables the provider, keeping its existing links, instead of leaving
  it as it was.
- The login page now shows a "Sign in with …" button for each enabled
  provider — it never did before, even though this is what RFC 004
  described from the start.
- An administrator can now delete a provider (once it is disabled and no
  longer configured), and unlink one user from one provider.

### v0.89.x

**Upgrading from `0.88.x` runs no migration, so no pre-migration snapshot
is taken.** Don't look for one.

**A new way to install: the static binary.** Verify it **before** running
it — the checksum and the build-provenance attestation — exactly as the
[Quick Start's own download
section](../getting-started/quick-start.md#download-the-binary-no-toolchain-needed)
does; that is not repeated here.

**A host with no CA bundle (e.g. `ca-certificates`) now starts, with one
warning, instead of crashing.** The warning means HIBP breach-checks, SMTP,
LDAPS and federation sign-in cannot verify TLS until that bundle is
installed — see the [Quick Start's runtime
requirement](../getting-started/quick-start.md#download-the-binary-no-toolchain-needed)
for what to install.

### v0.88.x

**Upgrading to `0.88.0` from a release with pending migrations takes the
automatic pre-migration snapshot first** — see "The pre-migration
snapshot" above for where it lands, its name and mode, and that it is
never deleted automatically. If there is not enough free space beside
the database, the start now **refuses with exit code `1`, with nothing
changed**. Free some space and start again; start with `--no-backup` only
if you have taken a backup yourself (General procedure, step 1).

**Run `sui-id upgrade preview` first** (General procedure, step 4) — it
now also predicts whether this snapshot will be taken and whether there
is room for it, before you start the new binary.

**Upgrading from `0.87.2` itself runs no migration, so no snapshot is
taken.** If you are upgrading from an older release, see below for
whether a migration (and therefore a snapshot) applies to your
starting version.

### v0.83.x

**New admin subaction** (now `sui-id admin registration-token
legacy-candidates [--config PATH]`; RFC 095). Read-only — it writes nothing. If any client ever self-registered
through RFC 7591 dynamic registration while your instance was running a
build between commits `48946f6` (2026-07-02) and `930513b` (2026-10-02),
run this command once after upgrading. A bug in that window discarded the
`registered_via` column on write, so such a client's row may still read
`admin` today even though it was never created by an operator.

**An empty result is not an all-clear.** The only admissible evidence is an
audit-log row recorded at registration time, and that row's write was
best-effort in the affected window — if it failed, the registration
succeeded anyway and left nothing to find. The command says this
explicitly every time it runs; it does not print a bare "0 candidates" the
way a clean report normally would. Before RFC 095 M3 is considered closed,
every candidate the command does find must be disabled, explicitly
accepted as legacy, or have its metadata corrected under a separate audit —
this command only lists; it decides nothing.

**New migration (`0049`): `token_endpoint_auth_method`, `grant_types`,
`response_types` on `clients` (RFC 095 M3 stage 4a).** Nullable, no
default — **no authentication behaviour changes in this release.**
Dynamic registration now stores what it already validated; an
administrator-created client, and every client row that predates this
migration, is left with all three unrecorded (not a guessed default) and
is marked "Auth method unrecorded" wherever the admin UI already shows a
client's provenance. The migration logs how many existing rows it left
that way — check your startup log once after upgrading.

### v0.76.x

**New migrations (0033–0038).** All run automatically and are backwards-compatible
(`ADD COLUMN` with defaults, new tables):

| Migration | Table | What changed |
|---|---|---|
| 0033 | `server_settings` | `metrics_token_hash` column for Prometheus auth |
| 0034 | `users` | `source`, `external_stable_id` columns for LDAP shadow rows |
| 0035 | `clients` | `registered_via`, `logo_uri`, `homepage_uri`, `privacy_policy_uri`, `tos_uri` |
| 0036 | new: `scope_definition`, `client_registration_token` | Scope catalog (seeded) and RFC 7591 registration tokens |
| 0037 | new: `federation_provider` | Upstream OIDC IdP configurations (encrypted secrets) |
| 0038 | new: `federation_link` | Per-user upstream identity links |

**New routes.** If you run sui-id behind a strict allowlist firewall or WAF,
add these paths:

- `GET /metrics` — Prometheus metrics (only if `metrics_enabled = true`)
- `POST /oauth2/register` — RFC 7591 dynamic client registration
- `GET /auth/federated/{slug}/start` — Federation sign-in initiation
- `GET /auth/federated/callback` — Federation sign-in callback
- `GET /auth/federated/link` — Federation link-only flow

**New configuration sections.** Optional; existing deployments that do not
add them continue to work without change:

- `metrics_enabled` / `metrics_listen_addr` under `[server]` — enable Prometheus metrics endpoint
- `[[user_sources]]` — LDAP external user source
- `[[federation_providers]]` — upstream OIDC provider

**New CLI subcommands:**

- `sui-id admin rotate-metrics-token` — generate/rotate a Prometheus bearer token
- `sui-id admin issue-registration-token` — issue an RFC 7591 initial-access token

### v0.36.x

- Dangerous operations (user disable/delete, MFA reset, client delete,
  signing key delete) now require navigating to a confirmation screen
  rather than accepting a browser `confirm()` dialog.
- The admin panel now shows operator action prompts when SMTP is
  unconfigured, HIBP is off, or `cookie_secure` is false.
- A `--dev` banner is displayed on every page when running in dev mode.

### v0.34.x

- Migration 0024 adds a `locale` column to `email_outbox`. This is a
  backwards-compatible schema change.
- Chinese Simplified (`zh`) locale is now supported.

### v0.33.x

- Migration 0023 adds the `email_outbox` table for async mail delivery.
  The outbox worker starts automatically; no configuration change is needed.

## The pre-migration snapshot

Starting a binary that will actually run a migration (`upgrade preview`
reports `Pending`, not already at head, and not a fresh database) first
takes an automatic, plain `VACUUM INTO` snapshot beside the database,
named from the database's own file name and the versions it spans, for
example `sui-id.sqlite.pre-upgrade-schema-44-to-49-20261108T091500Z.sqlite`.
It is mode `0600`, never world-readable. Decline it with `--no-backup`
(still logged, so the decision is on the record).

**It is not encrypted, and that is not a new exposure**: it holds exactly
what the live database file already holds — the encrypted columns stay
sealed under the master key, which is never in it. Unlike `sui-id backup`
(step 1 above), it needs no passphrase and no key-file copy; a restore
needs only `sqlite3` and the file itself. It is never deleted
automatically — once the upgrade is verified (step 6), it is safe to
delete.

## Rollback

If a bad release is deployed, roll back to the pre-upgrade state with
either the step-1 backup or the automatic pre-migration snapshot above.
Either one takes the database back to the moment before the migration ran.
**Everything written after the new binary started is lost**: sign-ins and
sessions, users or applications created, settings changed, audit entries.
Roll back as soon as you know you need to.

1. Stop the new binary.
2. Restore the pre-upgrade state, either:
   - the step-1 backup: `sui-id backup restore --config sui-id.toml --from
     sui-id-pre-upgrade.tar --force`; or
   - the automatic snapshot: replace the database file with it (and
     remove any `-wal`/`-shm` file beside the live database first — the
     snapshot is a complete file, not a diff). The snapshot belongs to the
     user that ran the binary that took it, so if that was not the service
     user (an `admin` command run as root, for example), give it the
     database's usual owner before starting.
3. Start the previous binary.

An older binary **refuses to start** against a database that a newer binary has
migrated. It reads the recorded schema version before it touches the file, and
stops if that version is newer than it understands; **nothing is written to the
database** when it does. The recovery is to run the newer binary again, or to
restore the pre-upgrade backup, as above.

What the refusal looks like: one line on stderr, first and alone (`sui-id:
refusing to run: the database at … is at schema version 44, but this sui-id
0.78.0 understands up to 43 …`), naming the path, both versions and, when the
database recorded it, the release that migrated it, and exit code **65**.
The release is recorded since sui-id 0.79.0 (RFC 112); a database no build
from before that has migrated since shows no release name, both here and in
`upgrade preview`'s own output.
Nothing is written to the database. There is no override flag. A schema version
that cannot be read, or that is missing from a database that has tables, is
refused the same way. The full text, the exit codes and the systemd setting
(`RestartPreventExitStatus=65`) are in the
[deployment guide](deployment.md#11-upgrades).

**One binary version per database at a time**: the check is made when a binary
opens the database, so a newer binary migrating while an older one is already
running is not detected by the older one. Stop the old instance first.
