# CLI reference

Generated from `sui-id`'s own command registry (RFC 148 §5) by `sui-id generate-cli-reference`. Do not edit by hand — a test fails if this page differs from a fresh render, and the regeneration command is named in that test's own failure message.

`--config PATH` and `--no-backup` are global: valid before or after any command below.

## Getting started

### `sui-id setup` {#setup}

Create the first administrator (headless initialization)

```text
sui-id setup [OPTIONS] --admin-username <ADMIN_USERNAME>
```

Environment:
- `SUI_ID_ADMIN_PASSWORD`

Example:

```sh
sui-id setup --admin-username admin
```

### `sui-id serve` {#serve}

Run the HTTP server — the same as no command at all

```text
sui-id serve
```

Environment:
- `SUI_ID_MASTER_KEY`

Example:

```sh
sui-id --config sui-id.toml
```

## Running it

### `sui-id backup create` {#backup-create}

Write a tarball snapshot of the database, master key, and manifest

```text
sui-id backup create [OPTIONS] --to <TO>
```

Environment:
- `SUI_ID_MASTER_KEY`
- `SUI_ID_BACKUP_PASSPHRASE`

Example:

```sh
sui-id backup create --to ./sui-id.tar --encrypt
```

### `sui-id backup restore` {#backup-restore}

Restore a backup tarball into the configured storage paths

```text
sui-id backup restore [OPTIONS] --from <FROM>
```

Environment:
- `SUI_ID_MASTER_KEY`
- `SUI_ID_BACKUP_PASSPHRASE`

Example:

```sh
sui-id backup restore --from ./sui-id.tar
```

### `sui-id backup verify` {#backup-verify}

Read a backup file and report what it contains. Writes nothing

```text
sui-id backup verify [OPTIONS] --from <FROM>
```

Environment:
- `SUI_ID_BACKUP_PASSPHRASE`

Example:

```sh
sui-id backup verify --from ./sui-id.tar
```

### `sui-id upgrade preview` {#upgrade-preview}

Read-only. Report what this build would do to the configured database without opening it for writing

```text
sui-id upgrade preview
```

Example:

```sh
sui-id upgrade preview
```

### `sui-id config sample` {#config-sample}

Print a configuration to start from

```text
sui-id config sample [OPTIONS]
```

Example:

```sh
sui-id config sample > sui-id.toml
```

### `sui-id config check` {#config-check}

Validate a configuration and change nothing

```text
sui-id config check
```

Example:

```sh
sui-id config check --config sui-id.toml
```

## Administering

### `sui-id admin user unlock` {#admin-user-unlock}

Clear an account-lockout state for the given user

```text
sui-id admin user unlock --username <USERNAME>
```

Environment:
- `SUI_ID_MASTER_KEY`

Example:

```sh
sui-id admin user unlock --username alice
```

### `sui-id admin user reset-mfa` {#admin-user-reset-mfa}

Remove every MFA factor (TOTP and passkeys) for the given user

```text
sui-id admin user reset-mfa --username <USERNAME> --reason <REASON>
```

Environment:
- `SUI_ID_MASTER_KEY`

Example:

```sh
sui-id admin user reset-mfa --username alice --reason "lost device"
```

### `sui-id admin user recovery-link` {#admin-user-recovery-link}

Issue a single-use account-recovery link for a local user

```text
sui-id admin user recovery-link --username <USERNAME> --reason <REASON>
```

Environment:
- `SUI_ID_MASTER_KEY`

Example:

```sh
sui-id admin user recovery-link --username alice --reason "sole admin, lost MFA"
```

### `sui-id admin master-key rotate` {#admin-master-key-rotate}

Re-seal every encrypted column under a new master key. Runs OFFLINE: stop the server first and take a fresh backup

```text
sui-id admin master-key rotate [OPTIONS]
```

Environment:
- `SUI_ID_MASTER_KEY`

Example:

```sh
sui-id admin master-key rotate --generate-new-key --yes
```

### `sui-id admin metrics-token rotate` {#admin-metrics-token-rotate}

Generate a fresh metrics bearer token; the previous one stops working immediately

```text
sui-id admin metrics-token rotate
```

Environment:
- `SUI_ID_MASTER_KEY`

Example:

```sh
sui-id admin metrics-token rotate
```

### `sui-id admin registration-token issue` {#admin-registration-token-issue}

Issue an RFC 7591 initial access token for dynamic client registration

```text
sui-id admin registration-token issue [OPTIONS]
```

Environment:
- `SUI_ID_MASTER_KEY`

Example:

```sh
sui-id admin registration-token issue --max-uses 1
```

### `sui-id admin registration-token revoke` {#admin-registration-token-revoke}

Immediately revoke a client-registration token by id

```text
sui-id admin registration-token revoke --id <ID>
```

Environment:
- `SUI_ID_MASTER_KEY`

Example:

```sh
sui-id admin registration-token revoke --id 01jc6z9f0000000000000000r0
```

### `sui-id admin registration-token list` {#admin-registration-token-list}

List every client-registration token. Never prints a token or its hash

```text
sui-id admin registration-token list
```

Environment:
- `SUI_ID_MASTER_KEY`

Example:

```sh
sui-id admin registration-token list
```

### `sui-id admin registration-token legacy-candidates` {#admin-registration-token-legacy-candidates}

Read-only (RFC 095). List `clients` rows a historical bug may have mislabelled

```text
sui-id admin registration-token legacy-candidates
```

Environment:
- `SUI_ID_MASTER_KEY`

Example:

```sh
sui-id admin registration-token legacy-candidates
```

## Developing

### `sui-id dev` {#dev}

Run a local dev server for testing. MUST NOT be used in production

```text
sui-id dev [OPTIONS]
```

Example:

```sh
sui-id dev --bind 127.0.0.1:8801
```

## Help

### `sui-id help` {#help}

Show this help, or help for one command

```text
sui-id help [COMMAND]...
```

Example:

```sh
sui-id help backup create
```

### `sui-id version` {#version}

Print version information

```text
sui-id version
```

Example:

```sh
sui-id version
```
