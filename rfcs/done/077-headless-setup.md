# RFC 077 — Headless setup (`sui-id setup` subcommand)

- Status: accepted
- Target version: v0.63.0
- Author: Claude (product owner: nuix)

## Problem

sui-id can only be initialized through the GUI setup wizard: the server
prints a one-time tokenised URL at first boot, the operator opens it in a
browser and fills in the admin account form. This blocks automation:
provisioning via Ansible, cloud-init, Docker entrypoints, or CI smoke
tests all require a human with a browser.

## Goals

1. Initialize a fresh sui-id instance entirely from the command line.
2. No regression in the security posture of the admin credential.
3. No new configuration files, flags on the *server* path, or setup
   complexity for operators who keep using the GUI wizard. The wizard is
   unchanged.

## Non-goals

- Interactive prompting (a TTY wizard). The point is automation.
- Enforcing the password change at login (the `must_change` flag is
  *recorded* but login-time enforcement is a future RFC).

## Design

### Command

```
sui-id setup --config <path> --admin-username <name>
             [--admin-email <email>] [--admin-display-name <name>]
```

Runs against the configured database directly (opens it via the same
`Config::load` → `keyring::resolve` → `Database::open` path as the
existing `backup`/`admin` subcommands; migrations run automatically).
Exits non-zero with a clear message if the instance is already
initialized.

### Admin password — security model

There is **no `--admin-password` flag.** Passwords on the command line
leak into shell history and `ps` output; this is a deliberate omission.

Resolution order:

1. **`SUI_ID_ADMIN_PASSWORD` environment variable**, if set and
   non-empty. Validated against `SecurityLevel::Standard` policy
   (min 12 chars). Intended for provisioning tools that already manage
   secrets (Ansible vault, Docker secrets via env). This matches the
   existing `SUI_ID_MASTER_KEY` env-secret convention.
2. **Otherwise: generate a random password** — 24 characters from
   `[A-Za-z0-9]` (≈143 bits of entropy) using the OS RNG (`getrandom`,
   the same primitive that generates signing keys). The password is
   printed **once** to stdout in a clearly delimited block, with an
   advisory to change it at `/me/security/password` after first login.
   The credential row is stored with `must_change: true` to record the
   intent durably.

stdout carries only the credential block (machine-capturable);
all diagnostics go to stderr.

### Why no setup token on the CLI path

The web wizard requires the boot-time token because the endpoint is
network-reachable: anyone who can reach the port before the operator
must not be able to claim the instance. The CLI path requires
filesystem access to the database and master key — an attacker with
that access already owns the instance. Requiring a token here would add
ceremony without adding security. (Same trust model as the existing
`admin unlock-user` subcommand.)

### Core changes

`sui-id-core/src/setup.rs`:

- Extract the body of `create_initial_admin` (user row, credential,
  signing-key bootstrap, `mark_initialized`, audit event) into a
  private helper.
- `create_initial_admin(...)` — unchanged public signature; performs
  the constant-time token check then delegates. The web wizard path is
  untouched.
- New `create_initial_admin_headless(db, clock, username, password,
  display_name, email, must_change: bool)` — skips the token check,
  passes `must_change` through to the credential row, delegates to the
  same helper. Audit event notes `setup.create_initial_admin` with a
  `headless` detail.
- New `generate_admin_password() -> Zeroizing<String>` — 24-char
  alphanumeric via `getrandom`, rejection-sampled to avoid modulo bias.

### Binary changes

- `cli.rs`: `run_setup_subcommand(&args)` following the existing
  subcommand pattern (own tokio runtime, `parse_config_path`,
  keyring resolve, `Database::open`).
- `main.rs`: dispatch `Some("setup")`.
- `print_help`: one new line in the subcommand table.
- `docs/src/reference/configuration.md`: subcommand table row.

### Output format (generated-password case)

```
sui-id initialized.

============ INITIAL ADMIN CREDENTIALS ============
  username: <name>
  password: <generated>
===================================================

This password is shown only once. Change it after first
login at /me/security/password.
```

(env-var case prints the same block minus the password line.)

## Failure modes

| Condition | Behaviour |
|---|---|
| Already initialized | exit 1, "already initialized" on stderr |
| `SUI_ID_ADMIN_PASSWORD` set but < 12 chars | exit 1, policy message |
| `--admin-username` missing/empty | exit 1, usage on stderr |
| DB/key file unreadable | exit 1, existing keyring/store error text |

## Tests

- `generate_admin_password`: length 24, alphabet `[A-Za-z0-9]`,
  two calls differ.
- `create_initial_admin_headless`: creates admin, marks initialized,
  second call returns `AlreadyInitialized`; `must_change` persisted
  as passed.
- Existing tokenised-path tests unchanged and still green.
