# CLI reference

Generated from `sui-id`'s own command registry (RFC 148 §5). Do not edit by hand — a test fails if this page differs from a fresh render; run it with `SUI_ID_REGENERATE_CLI_REFERENCE=1` set to write the fresh render instead of comparing.

`--config PATH` and `--no-backup` are global: valid before or after any command below.

## Getting started

### `sui-id init` {#init}

Write a starter configuration

```text
sui-id init [OPTIONS]
```

Example:

```sh
sui-id init --production --output /etc/sui-id/sui-id.toml
```

### `sui-id setup` {#setup}

Create the first admin

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

Run the HTTP server

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

Back up the database

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

Restore from a backup

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

Check a backup is valid

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

Preview a schema upgrade

```text
sui-id upgrade preview
```

Example:

```sh
sui-id upgrade preview
```

### `sui-id config sample` {#config-sample}

Print a starter configuration

```text
sui-id config sample [OPTIONS]
```

Example:

```sh
sui-id config sample > sui-id.toml
```

### `sui-id config check` {#config-check}

Validate a configuration

```text
sui-id config check
```

Example:

```sh
sui-id config check --config sui-id.toml
```

## Administering

### `sui-id admin user unlock` {#admin-user-unlock}

Clear an account lockout

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

Remove a user's MFA factors

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

Issue a recovery link

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

Rotate the master key

```text
sui-id admin master-key rotate [OPTIONS]
```

Environment:
- `SUI_ID_MASTER_KEY`

Example:

```sh
sui-id admin master-key rotate --generate-new-key --yes --discard-backup-once-verified --verification-reference "checked 2026-11-24"
```

### `sui-id admin master-key backup remove` {#admin-master-key-backup-remove}

Remove the kept old key

```text
sui-id admin master-key backup remove [OPTIONS]
```

Environment:
- `SUI_ID_MASTER_KEY`

Example:

```sh
sui-id admin master-key backup remove --confirm-retired-or-reencrypted
```

### `sui-id admin master-key backup renew` {#admin-master-key-backup-renew}

Renew a retained backup's review

```text
sui-id admin master-key backup renew --until <RFC3339_INSTANT> --custody-reference <TEXT>
```

Environment:
- `SUI_ID_MASTER_KEY`

Example:

```sh
sui-id admin master-key backup renew --until 2027-06-01T00:00:00Z --custody-reference "ops-team ticket 1234"
```

### `sui-id admin metrics-token rotate` {#admin-metrics-token-rotate}

Rotate the metrics token

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

Issue a registration token

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

Revoke a registration token

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

List registration tokens

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

List mislabelled clients

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

Run a local dev server

```text
sui-id dev [OPTIONS]
```

Example:

```sh
sui-id dev --bind 127.0.0.1:8801
```

## Help

### `sui-id help` {#help}

Show help

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
