//! The implementation behind every [`super::Command`] leaf. RFC 148 §2:
//! "behaviour does not change" — each function here does exactly what its
//! pre-148 `&[String]`-scanning counterpart did, with the parsed values
//! handed in directly rather than re-parsed. `serve`/`serve_dev` moved
//! here from `main.rs` (RFC 148 cycle O2a) so the dispatch in `cli.rs` can
//! reach them; `main.rs` itself is now a thin shell around
//! [`super::run`].

use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};

use crate::database::PreMigrationBackup;
use crate::{backup, config::Config};

mod templates;

/// `sui-id config sample [--production]` / the pre-148 `--print-sample-config`.
/// Both call this, so their stdout is byte-identical by construction.
pub(crate) fn config_sample(production: bool) -> Result<()> {
    if production {
        print!("{}", templates::production_toml());
    } else {
        print!("{}", templates::local_trial_toml());
    }
    Ok(())
}

/// `sui-id init [--production] [--output PATH]` (RFC 148 §4). Writes
/// exactly what `sui-id config sample [--production]` prints (the same
/// two functions), and **never overwrites**: `create_new` is the only way
/// the destination is opened, so an existing file is refused, not
/// replaced, before anything is written.
pub(crate) fn run_init(output: &Path, production: bool) -> Result<()> {
    use std::io::Write as _;

    let text = if production {
        templates::production_toml().to_string()
    } else {
        templates::local_trial_toml()
    };

    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                anyhow::anyhow!(
                    "refusing to overwrite the existing configuration at {}",
                    output.display()
                )
            } else {
                anyhow::Error::new(e).context(format!("creating {}", output.display()))
            }
        })?;
    file.write_all(text.as_bytes())
        .with_context(|| format!("writing {}", output.display()))?;

    let profile = if production {
        "production template"
    } else {
        "local trial"
    };
    eprintln!("wrote {} ({profile}).", output.display());
    eprintln!();
    eprintln!("Next:  sui-id serve      then open the setup link it prints.");
    eprintln!(
        "Help:  sui-id help       Reference: {}",
        sui_id_web::github_doc_url!("docs/src/reference/configuration.md")
    );
    Ok(())
}

/// `sui-id config check`. Loads and validates the configuration, opens no
/// database, and changes nothing. `Config::load` already runs `validate`.
pub(crate) fn config_check(path: &Path) -> Result<()> {
    Config::load(path).with_context(|| format!("loading config from {}", path.display()))?;
    println!("{} is valid.", path.display());
    Ok(())
}

pub(crate) fn run_backup(config_path: &Path, to: &Path, encrypt: bool) -> Result<()> {
    let cfg = Config::load(config_path)
        .with_context(|| format!("loading config from {}", config_path.display()))?;
    let opts = if encrypt {
        let pass = read_passphrase("Encryption passphrase", true)?;
        backup::BackupOptions {
            passphrase: Some(pass),
        }
    } else {
        backup::BackupOptions::default()
    };
    backup::run_backup(&cfg, to, &opts)?;
    if opts.passphrase.is_some() {
        eprintln!(
            "encrypted backup written to {} (mode 0600). Store the passphrase \
             separately from the file — losing it makes the backup unrecoverable.",
            to.display()
        );
    } else {
        eprintln!(
            "backup written to {} (mode 0600). The archive contains the master key; \
             treat it as a secret. For transport over an untrusted channel, use --encrypt.",
            to.display()
        );
    }
    Ok(())
}

pub(crate) fn run_restore(
    config_path: &Path,
    from: &Path,
    force: bool,
    decrypt: bool,
) -> Result<()> {
    let cfg = Config::load(config_path)
        .with_context(|| format!("loading config from {}", config_path.display()))?;

    // RFC 100 `0.94.0` P9 §2: the same liveness lock a rotation takes,
    // exclusive, always -- not only with `--force`. Taken before the
    // passphrase prompt below and before the source is read, so a
    // refusal is the first thing read, not something an operator meets
    // after typing a passphrase (P7's own review lesson, applied here).
    //
    // P9 review: on a fresh host the key file's directory may not exist
    // yet, and the lock file lives in it. The store's restore creates that
    // directory anyway (`backup::ops::run_restore`); creating it here first
    // keeps a fresh-host restore -- the case `--force` exists for --
    // working. It creates nothing the restore would not have created.
    if let Some(parent) = cfg.storage.key_file.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {} for the restore", parent.display()))?;
    }
    let adapter = crate::key_adapter::default_adapter();
    let paths = crate::rotate::preconditions::RotationPaths::for_key_file(&cfg.storage.key_file);
    let _lock = crate::rotate::preconditions::take_exclusive_lock(
        adapter.as_ref(),
        &paths,
        "restore.lock",
        crate::rotate::preconditions::LockAction::Restore,
    )?;

    let passphrase = if decrypt {
        Some(read_passphrase("Decryption passphrase", false)?)
    } else {
        None
    };
    backup::run_restore(&cfg, from, &backup::RestoreOptions { force, passphrase })?;
    eprintln!(
        "restored from {} into {} and {}",
        from.display(),
        cfg.storage.db_path.display(),
        cfg.storage.key_file.display()
    );
    Ok(())
}

pub(crate) fn run_verify_backup(from: &Path, decrypt: bool) -> Result<()> {
    let passphrase = if decrypt {
        Some(read_passphrase("Decryption passphrase", false)?)
    } else {
        None
    };
    let report = backup::run_verify(from, passphrase.as_deref())?;
    println!("Format version: {}", report.manifest.format_version);
    println!("sui-id version: {}", report.manifest.sui_id_version);
    println!("Schema version: {}", report.manifest.schema_version);
    println!("Created at:     {}", report.manifest.created_at);
    println!("Hostname:       {}", report.manifest.hostname);
    println!("Issuer:         {}", report.manifest.issuer);
    println!("Encrypted:      {}", report.encrypted);
    println!("Tar size:       {} bytes", report.tar_bytes);
    println!("Database size:  {} bytes", report.db_bytes);
    println!(
        "Master key:     {}",
        if report.key_present {
            "present"
        } else {
            "MISSING"
        }
    );
    println!();
    println!("✓ SQLite integrity check passed");
    if report.encrypted {
        println!("✓ Decrypted with provided passphrase");
    }
    Ok(())
}

/// `sui-id upgrade preview` (RFC 142 stage 4): what this build would do to
/// the configured database, without opening it for writing and without
/// resolving a master key.
pub(crate) fn run_preview_upgrade(config_path: &Path) -> Result<()> {
    let cfg = Config::load(config_path)
        .with_context(|| format!("loading config from {}", config_path.display()))?;
    let path = &cfg.storage.db_path;

    let outcome = crate::database::preview(path)?;
    let free_space = matches!(outcome, sui_id_store::migrations::Preview::Pending { .. })
        .then(|| sui_id_store::backup::check_free_space(path));
    print!(
        "{}",
        render_preview_upgrade(
            env!("CARGO_PKG_VERSION"),
            path,
            &outcome,
            free_space.as_ref()
        )
    );
    Ok(())
}

/// The exact text of `upgrade preview`'s report (RFC 142 stage 4 §2, RFC
/// 142 stage 5 §6). A pure function of its inputs.
fn render_preview_upgrade(
    version: &str,
    path: &Path,
    outcome: &sui_id_store::migrations::Preview,
    free_space: Option<&sui_id_store::backup::FreeSpaceCheck>,
) -> String {
    use std::fmt::Write as _;
    use sui_id_store::migrations::{MAX_SCHEMA_VERSION, Preview};

    let mut out = format!("sui-id {version}: upgrade preview for {}\n", path.display());
    match outcome {
        Preview::NoFile => {
            let _ = writeln!(out, "  No database exists at {} yet.", path.display());
            let _ = writeln!(
                out,
                "  Starting this sui-id creates its schema and runs all {MAX_SCHEMA_VERSION} \
                 migrations. Nothing was changed."
            );
        }
        Preview::Fresh => {
            let _ = writeln!(out, "  The database at {} is empty.", path.display());
            let _ = writeln!(
                out,
                "  Starting this sui-id creates its schema and runs all {MAX_SCHEMA_VERSION} \
                 migrations. Nothing was changed."
            );
        }
        Preview::AtHead => {
            let _ = writeln!(
                out,
                "  The database is already at schema version {MAX_SCHEMA_VERSION}. Starting \
                 this sui-id runs no migrations. Nothing was changed."
            );
        }
        Preview::Pending {
            current,
            last_migrated_by,
            migrations,
        } => {
            let stamp = match last_migrated_by {
                Some(v) => format!("last migrated by sui-id {v}"),
                None => "the release that last migrated it is not recorded".to_string(),
            };
            let _ = writeln!(out, "  Schema version now:      {current} ({stamp})");
            let _ = writeln!(out, "  This sui-id migrates to: {MAX_SCHEMA_VERSION}");
            let _ = writeln!(
                out,
                "  {} migrations would run when this sui-id starts:",
                migrations.len()
            );
            for m in migrations {
                let _ = writeln!(out, "    {}", m.name);
            }
            if let Some(check) = free_space {
                let _ = writeln!(out, "  {}", crate::database::backup_prediction_line(check));
            }
            let _ = writeln!(
                out,
                "  Nothing was changed. Back up before starting this sui-id: \
                 docs/src/guides/upgrade.md"
            );
        }
    }
    out
}

/// `sui-id admin user unlock --username NAME`.
///
/// Clears `failed_login_count` and `locked_until` on a user without
/// requiring a password verification — the operator's own authority,
/// witnessed by their access to the host's master key material, is what
/// authorises this.
pub(crate) async fn run_admin_unlock_user(
    config_path: &Path,
    policy: PreMigrationBackup,
    username: &str,
) -> Result<()> {
    let cfg = Config::load(config_path)
        .with_context(|| format!("loading config from {}", config_path.display()))?;
    let (db, _key_origin, _db_lock) =
        crate::database::open_checked(&cfg.storage.db_path, &cfg.storage.key_file, policy)
            .await
            .context("loading master key and opening database")?;

    let user = sui_id_store::repos::users::find_by_username(&db, username)
        .await
        .with_context(|| format!("looking up user {username:?}"))?;
    sui_id_store::commands::admin_unlock_user(&db, user.id)
        .await
        .context("clearing lockout")?;
    eprintln!("unlocked {username} (id={})", user.id);
    Ok(())
}

/// `sui-id admin user reset-mfa --username NAME --reason TEXT`.
///
/// Removes every MFA factor (TOTP and passkeys) for one user, on the
/// operator's authority (RFC 103 D12). Sessions are left alone, as on the
/// web path.
pub(crate) async fn run_admin_reset_mfa(
    config_path: &Path,
    policy: PreMigrationBackup,
    username: &str,
    reason: &str,
) -> Result<()> {
    if reason.trim().is_empty() {
        bail!("admin user reset-mfa requires a non-empty --reason TEXT");
    }
    let cfg = Config::load(config_path)
        .with_context(|| format!("loading config from {}", config_path.display()))?;
    let (db, _key_origin, _db_lock) =
        crate::database::open_checked(&cfg.storage.db_path, &cfg.storage.key_file, policy)
            .await
            .context("loading master key and opening database")?;

    match sui_id_core::admin::operator_reset_mfa(&db, username, reason).await {
        Ok((user_id, report)) => {
            eprintln!(
                "reset MFA for {username} (id={user_id}): totp {}, {} passkey(s) removed",
                if report.totp_removed {
                    "removed"
                } else {
                    "absent"
                },
                report.passkeys_removed
            );
            Ok(())
        }
        Err(sui_id_core::errors::CoreError::NotFound) => {
            bail!("no active user named {username:?} (unknown or deleted); nothing was changed")
        }
        Err(e) => Err(anyhow::anyhow!(e)).context("resetting MFA; nothing was changed"),
    }
}

/// `sui-id admin user recovery-link --username NAME --reason TEXT`.
///
/// Issues a single-use, 30-minute account-recovery link for one local
/// user, on the operator's authority (RFC 103 D2, D12). The link and the
/// token text are printed to **stdout** and nowhere else, once.
pub(crate) async fn run_admin_issue_recovery_link(
    config_path: &Path,
    policy: PreMigrationBackup,
    username: &str,
    reason: &str,
) -> Result<()> {
    let cfg = Config::load(config_path)
        .with_context(|| format!("loading config from {}", config_path.display()))?;
    let (db, _key_origin, _db_lock) =
        crate::database::open_checked(&cfg.storage.db_path, &cfg.storage.key_file, policy)
            .await
            .context("loading master key and opening database")?;
    let clock: sui_id_core::time::SharedClock = std::sync::Arc::new(sui_id_core::time::SystemClock);

    match sui_id_core::recovery_link::issue_as_operator(&db, &clock, username, reason).await {
        Ok((user_id, link)) => {
            let url = sui_id_core::recovery_link::completion_url(&cfg.server.issuer, &link.token);
            eprintln!(
                "issued a recovery link for {username} (id={user_id}); it expires at {} and \
                 invalidated {} earlier link(s)",
                link.expires_at
                    .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                link.invalidated
            );
            eprintln!(
                "deliver it through a channel that authenticates the person, and immediately: \
                 it is shown only once"
            );
            println!("Recovery link:");
            println!("{url}");
            println!("Token, if the link's fragment is dropped in transit:");
            println!("{}", link.token.expose());
            Ok(())
        }
        Err(sui_id_core::errors::CoreError::Store(sui_id_store::StoreError::RecoveryRefused(
            why,
        ))) => bail!("no recovery link was issued: {why}"),
        Err(e) => Err(anyhow::anyhow!(e)).context("issuing the recovery link; nothing was changed"),
    }
}

/// `sui-id admin master-key rotate [--generate-new-key | --new-key PATH]
/// [--yes] [--check]`. Mutual exclusion between the two key-source flags
/// is enforced by clap (`conflicts_with`) before this runs.
///
/// Re-seal every encrypted column in the database under a new 32-byte
/// XChaCha20-Poly1305 master key. Runs offline: the caller is expected to
/// have stopped the server first, taken a backup of both the DB and the
/// current key file, and only then to invoke this. See the pre-148
/// `run_admin_rotate_key`'s own doc history for the full step-by-step
/// durability rationale (RFC 094, RFC 100 stage 0/1) — unchanged here.
///
/// RFC 100 stage 2: `--check` runs only `rotate::preconditions::check`
/// (P1-P4) and reports; it opens no database for writing, prompts for
/// nothing, and creates, removes or renames no file. A real rotation
/// runs the identical check after the confirmation prompt, then P5's
/// filesystem probe, before anything today's sequence does.
/// The four raw `--retain-backup-until`/`--custody-reference`/
/// `--discard-backup-once-verified`/`--verification-reference` flags,
/// bundled so `cli.rs`'s two call sites (the noun-first command and the
/// pre-148 `rotate-key` alias) pass them identically. [`Self::into_disposition`]
/// is the one place they become a typed
/// [`sui_id_store::master_key_rotation::RotationDisposition`].
pub(crate) struct CustodyChoiceArgs {
    pub(crate) retain_backup_until: Option<String>,
    pub(crate) custody_reference: Option<String>,
    pub(crate) discard_backup_once_verified: bool,
    pub(crate) verification_reference: Option<String>,
}

impl CustodyChoiceArgs {
    /// RFC 100 `0.94.0` P2 §2: exactly one of the two forms is
    /// required. `clap`'s own `conflicts_with_all`/`requires` already
    /// make a *mixed* choice (both forms, or a reference without its
    /// own flag) a usage error before this ever runs; this catches the
    /// one shape clap cannot: *missing* (neither flag at all), and
    /// `--retain-backup-until`'s own instant being malformed or not in
    /// the future. Prints the problem and exits **2**, the same code
    /// clap itself uses for a usage error -- never returned as an
    /// ordinary `Result::Err` (exit 1).
    fn into_disposition(self) -> sui_id_store::master_key_rotation::RotationDisposition {
        use sui_id_store::master_key_rotation::RotationDisposition;
        let usage_error = |line: String| -> ! {
            eprintln!(
                "refusing to rotate: {line}\n\nThe operator must choose exactly one of:\n  \
                 --retain-backup-until <RFC3339_INSTANT> --custody-reference <TEXT>\n  \
                 --discard-backup-once-verified --verification-reference <TEXT>"
            );
            std::process::exit(2);
        };
        match (self.retain_backup_until, self.discard_backup_once_verified) {
            (None, false) => usage_error(
                "no old-key custody choice was given -- every rotation requires one".to_owned(),
            ),
            (Some(review_at), false) => {
                let review_at = chrono::DateTime::parse_from_rfc3339(&review_at)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|e| {
                        usage_error(format!(
                            "--retain-backup-until {review_at:?} is not a valid RFC 3339 instant: {e}"
                        ))
                    });
                if review_at <= chrono::Utc::now() {
                    usage_error(format!(
                        "--retain-backup-until {} is not in the future",
                        review_at.to_rfc3339()
                    ));
                }
                #[allow(clippy::expect_used)] // `requires = "retain_backup_until"` in cli.rs
                let custody_reference = self
                    .custody_reference
                    .expect("clap requires --custody-reference with --retain-backup-until");
                RotationDisposition::RetainUntil {
                    review_at,
                    custody_reference,
                }
            }
            (None, true) => {
                #[allow(clippy::expect_used)]
                // `requires = "discard_backup_once_verified"` in cli.rs
                let verification_reference = self.verification_reference.expect(
                    "clap requires --verification-reference with --discard-backup-once-verified",
                );
                RotationDisposition::RemoveAfterVerified {
                    verification_reference,
                }
            }
            (Some(_), true) => {
                // `conflicts_with_all` in cli.rs already refuses this
                // combination before this function is ever reached.
                unreachable!(
                    "clap refuses --retain-backup-until with --discard-backup-once-verified"
                )
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_admin_rotate_key(
    config_path: &Path,
    policy: PreMigrationBackup,
    generate_new_key: bool,
    new_key: Option<PathBuf>,
    yes: bool,
    check: bool,
    custody: CustodyChoiceArgs,
) -> Result<()> {
    let _ = generate_new_key; // the default when neither flag is given; no separate branch needed
    let cfg = Config::load(config_path)
        .with_context(|| format!("loading config from {}", config_path.display()))?;
    let provided_new_key_path = new_key;

    if check {
        return run_rotate_check(&cfg.storage.key_file, &cfg.storage.db_path);
    }
    // RFC 100 `0.94.0` P2 §2: resolved before anything else is touched,
    // so a missing or mixed custody choice refuses (exit 2) before the
    // lock, the confirmation prompt, or the pre-flight recovery pass
    // below.
    let disposition = custody.into_disposition();
    let adapter = crate::key_adapter::default_adapter();

    // RFC 112 D5: print a summary and ask for confirmation before the
    // database is even opened, so a refusal is the first thing read.
    crate::database::check(&cfg.storage.db_path)?;

    // RFC 100 `0.94.0` P2 §1, P7 §1: the liveness lock, exclusive against
    // a running server, any other sui-id command, or another rotation.
    // Taken non-blocking, and **before the summary** (P7 review), so a
    // refusal is the first thing read, not something an operator meets
    // after typing `yes`. It is held through the prompt.
    let old_path = cfg.storage.key_file.clone();
    let paths = crate::rotate::preconditions::RotationPaths::for_key_file(&old_path);
    let _lock =
        crate::rotate::preconditions::take_rotation_lock(adapter.as_ref(), &paths, "rotate.lock")?;

    eprintln!(
        "About to rotate the master key for the database at {}.",
        cfg.storage.db_path.display()
    );
    eprintln!("Old key file: {}", cfg.storage.key_file.display());
    if let Some(p) = &provided_new_key_path {
        eprintln!("New key:      {} (provided)", p.display());
    } else {
        eprintln!("New key:      generated by this command");
    }
    eprintln!(
        "This rotation is crash-safe: an interruption part-way is completed or undone \
         automatically, at the next start or rotation attempt."
    );
    eprintln!(
        "Make sure you have a fresh backup of both the DB and the old key file before \
         continuing."
    );

    if !yes {
        use std::io::{BufRead, Write};
        let mut stderr = std::io::stderr();
        write!(stderr, "Type 'yes' to continue: ").ok();
        stderr.flush().ok();
        let mut line = String::new();
        std::io::stdin()
            .lock()
            .read_line(&mut line)
            .context("reading confirmation from stdin")?;
        if line.trim() != "yes" {
            bail!("aborted (no 'yes' confirmation)");
        }
    }

    // RFC 100 stage 4 (P7 §1: moved here, under the exclusive lock taken
    // above, before the summary, closing what would otherwise be a gap
    // between this pass and the lock): this command resolves its key through
    // `keyring::resolve_checked` directly, not `database::open_checked`,
    // so it calls recovery explicitly -- before the precondition check
    // below, since it is the command most likely to meet its own
    // leftover, and a leftover recovery would clean must not trip P3's
    // own refusal first. The lock is already held exclusively, so this
    // does not take it again (a second `flock` open in this same
    // process would conflict with the first).
    crate::rotate::recovery::recover_and_report(
        adapter.as_ref(),
        &old_path,
        &cfg.storage.db_path,
        true,
    )?;

    // RFC 100 stage 2: P1-P4, gathered -- every failing precondition
    // named, not only the first. Nothing is created, removed or renamed
    // by this call.
    let (results, resolved) =
        crate::rotate::preconditions::check(adapter.as_ref(), &old_path, &cfg.storage.db_path);
    let failures: Vec<&str> = results
        .iter()
        .filter_map(|r| r.failure.as_deref())
        .collect();
    if !failures.is_empty() {
        bail!(failures.join("\n\n"));
    }
    #[allow(clippy::expect_used)] // every precondition passed, so `check` guarantees `Some`
    let resolved_old = resolved.expect("all preconditions passed, so a key was resolved");

    // P5: prove the filesystem actually supports what rotation needs,
    // before anything else -- a temporary probe file, removed either way.
    crate::rotate::preconditions::probe_filesystem(adapter.as_ref(), &old_path).await?;

    let old_key = resolved_old.key;
    let db = crate::database::open(&cfg.storage.db_path, dup_key(&old_key), policy)?;
    sui_id_store::master_key_check::ensure_sentinel_written(&db, db.key())
        .await
        .context("writing the master key sentinel")?;

    let (new_key, new_key_b64) = if let Some(path) = &provided_new_key_path {
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("reading --new-key file at {}", path.display()))?;
        let key = sui_id_store::crypto::MasterKey::from_base64(&raw)
            .context("--new-key file must contain a base64-encoded 32-byte key")?;
        let b64 = raw.trim().to_owned();
        (key, b64)
    } else {
        let key = sui_id_store::crypto::MasterKey::generate();
        let b64 = key.to_base64();
        (key, b64)
    };

    let outcome = match crate::rotate::rotate(
        adapter.as_ref(),
        &db,
        &cfg.storage.db_path,
        &old_path,
        &old_key,
        &new_key,
        &new_key_b64,
        disposition.clone(),
        policy,
    )
    .await
    {
        Err(crate::rotate::RotateError::NotDurable { phase, source }) => {
            return Err(anyhow::Error::new(source).context(format!(
                "{phase} committed, but could not be made durable (checkpoint busy) -- \
                 something else has the database open. Stop sui-id (rotation runs offline), \
                 free up whatever holds {} open, then either start sui-id or run this command \
                 again: recovery will finish the rotation automatically. Do not touch {}, {} or \
                 {} by hand",
                cfg.storage.db_path.display(),
                paths.workspace.display(),
                paths.next.display(),
                old_path.display(),
            )));
        }
        Err(crate::rotate::RotateError::Other(e)) => return Err(e),
        Ok(outcome) => outcome,
    };
    let report = outcome.report;

    // RFC 100 `0.94.0` P2 §1/§3 point 12: release the lock, THEN print
    // success -- success must never be printed while another process
    // could still observe the lock held for a rotation that, from its
    // own point of view, has not finished yet.
    drop(_lock);

    eprintln!();
    eprintln!("Rotation complete (rotation {}).", outcome.rotation_id);
    eprintln!("  signing_keys:               {}", report.signing_keys);
    eprintln!("  refresh_tokens:             {}", report.refresh_tokens);
    eprintln!("  user_totp (secrets):        {}", report.user_totp_secrets);
    eprintln!(
        "  user_totp (recovery codes): {}",
        report.user_totp_recovery_codes
    );
    eprintln!(
        "  user_webauthn_credentials:  {}",
        report.user_webauthn_credentials
    );
    eprintln!("  smtp_config:                {}", report.smtp_config);
    eprintln!("  total:                      {}", report.total());
    eprintln!();
    match disposition {
        sui_id_store::master_key_rotation::RotationDisposition::RemoveAfterVerified { .. } => {
            eprintln!(
                "The old key is kept at {}. When you have checked that nothing needs it, run \
                 `sui-id admin master-key backup remove --confirm-retired-or-reencrypted`. \
                 Until then, the next rotation refuses.",
                paths.backup.display()
            );
        }
        sui_id_store::master_key_rotation::RotationDisposition::RetainUntil {
            review_at,
            custody_reference,
        } => {
            eprintln!(
                "The old key is kept at {} until {} ({custody_reference}). After that date, \
                 sui-id warns at every start until you renew or remove it. Until it is gone, \
                 the next rotation refuses.",
                paths.backup.display(),
                review_at.to_rfc3339(),
            );
        }
    }
    eprintln!("New key file written to {}", old_path.display());
    eprintln!();
    eprintln!("Restart the server. The new key is in place.");
    Ok(())
}

fn dup_key(key: &sui_id_store::crypto::MasterKey) -> sui_id_store::crypto::MasterKey {
    use zeroize::Zeroize;
    let mut bytes = *key.as_bytes();
    let copy = sui_id_store::crypto::MasterKey::from_bytes(bytes);
    bytes.zeroize();
    copy
}

/// `sui-id admin master-key rotate --check`. RFC 100 stage 2 §2: P1-P4
/// only, read-only, creating nothing -- P5 (the filesystem probe) needs
/// a write, so it is not checked here, and this says so. Exits 0 if
/// every precondition passed, 1 otherwise, bypassing the generic
/// `Error: {err:?}` wrapper so the report is the only thing printed.
fn run_rotate_check(key_file: &Path, db_path: &Path) -> Result<()> {
    let adapter = crate::key_adapter::default_adapter();
    let (results, _resolved) =
        crate::rotate::preconditions::check(adapter.as_ref(), key_file, db_path);

    println!(
        "Rotation precondition check for the database at {}.",
        db_path.display()
    );
    println!();
    let all_passed = results.iter().all(|r| r.passed());
    for r in &results {
        let mark = if r.passed() { "ok" } else { "FAIL" };
        println!("  [{mark}] {:<2}  {}", r.id, r.label);
        if let Some(failure) = &r.failure {
            for line in failure.lines() {
                println!("         {line}");
            }
        }
    }
    println!();
    if let Some(line) = crate::rotate::backup::overdue_finding_line_from_paths(
        key_file,
        db_path,
        chrono::Utc::now(),
    ) {
        println!("{line}");
        println!();
    }
    println!(
        "not checked here: whether this filesystem supports the operations rotation needs; \
         a rotation probes that first, with a temporary file it removes."
    );
    println!();
    if all_passed {
        println!("All checked preconditions passed.");
        Ok(())
    } else {
        let failed = results.iter().filter(|r| !r.passed()).count();
        println!(
            "{failed} of {} checked precondition(s) failed. Rotation would refuse.",
            results.len()
        );
        std::process::exit(1);
    }
}

/// RFC 100 `0.94.0` P3 §2: `uid[:name]`, read from the process and
/// `/etc/passwd` -- never authenticated, which is why every caller
/// labels the resulting attribute `os_user`, never `operator`.
/// `rustix::process::getuid` (the `process` feature is already
/// enabled); `rustix` has no uid-to-name lookup, so the name comes
/// from reading `/etc/passwd` directly, with no new dependency. `None`
/// for the name (uid alone) if the file cannot be read, or has no
/// matching line -- never an error, since the uid alone is still a
/// true, useful fact.
fn os_user_identity() -> String {
    let uid = rustix::process::getuid().as_raw();
    let name = std::fs::read_to_string("/etc/passwd")
        .ok()
        .and_then(|contents| {
            contents.lines().find_map(|line| {
                let mut fields = line.split(':');
                let name = fields.next()?;
                let _password = fields.next()?;
                let line_uid: u32 = fields.next()?.parse().ok()?;
                (line_uid == uid).then(|| name.to_owned())
            })
        });
    match name {
        Some(name) => format!("{uid}:{name}"),
        None => uid.to_string(),
    }
}

/// `sui-id admin master-key backup remove --confirm-retired-or-reencrypted
/// [--verification-reference TEXT]`. RFC 100 `0.94.0` P3 §2: verifies
/// the kept pre-rotation backup against its own row, unlinks it, and
/// records who and when. "No secure-erasure claim" (RFC 100): said in
/// the command's own output, once.
pub(crate) async fn run_admin_backup_remove(
    config_path: &Path,
    policy: PreMigrationBackup,
    confirm_retired_or_reencrypted: bool,
    verification_reference: Option<String>,
) -> Result<()> {
    if !confirm_retired_or_reencrypted {
        bail!(
            "refusing: --confirm-retired-or-reencrypted is required -- state, explicitly, that \
             every copy of the database made before the rotation that kept this old key (not \
             a `backup create` tarball, which carries its own key) is retired or re-encrypted, \
             so this key is safe to remove, before this command unlinks it"
        );
    }
    let cfg = Config::load(config_path)
        .with_context(|| format!("loading config from {}", config_path.display()))?;
    let (db, _origin, _db_lock) =
        crate::database::open_checked(&cfg.storage.db_path, &cfg.storage.key_file, policy)
            .await
            .context("loading master key and opening database")?;
    let adapter = crate::key_adapter::default_adapter();
    let os_user = os_user_identity();

    match crate::rotate::backup::remove(
        adapter.as_ref(),
        &db,
        &cfg.storage.key_file,
        verification_reference,
        os_user,
    )
    .await?
    {
        crate::rotate::backup::RemovalOutcome::NothingToRemove => {
            eprintln!("Nothing to remove: no pre-rotation backup exists.");
        }
        crate::rotate::backup::RemovalOutcome::Removed {
            rotation_id,
            verification_reference,
        } => {
            eprintln!(
                "Removed the pre-rotation backup for rotation {rotation_id}, citing: \
                 {verification_reference}"
            );
            eprintln!("No secure-erasure claim: this is an ordinary file removal, not a wipe.");
        }
    }
    Ok(())
}

/// `sui-id admin master-key backup renew --until <RFC3339_INSTANT>
/// --custody-reference <TEXT>`. RFC 100 `0.94.0` P3 §2: for a
/// `RetainUntil` backup only -- moves `review_at` forward and records
/// the new reference, audited.
pub(crate) async fn run_admin_backup_renew(
    config_path: &Path,
    policy: PreMigrationBackup,
    until: String,
    custody_reference: String,
) -> Result<()> {
    let review_at = chrono::DateTime::parse_from_rfc3339(&until)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .with_context(|| format!("--until {until:?} is not a valid RFC 3339 instant"))?;
    let cfg = Config::load(config_path)
        .with_context(|| format!("loading config from {}", config_path.display()))?;
    let (db, _origin, _db_lock) =
        crate::database::open_checked(&cfg.storage.db_path, &cfg.storage.key_file, policy)
            .await
            .context("loading master key and opening database")?;
    sui_id_store::commands::renew_master_key_backup(&db, review_at, custody_reference.clone())
        .await
        .map_err(anyhow::Error::new)?;
    eprintln!(
        "Renewed: review by {}, custody reference: {custody_reference}",
        review_at.to_rfc3339()
    );
    Ok(())
}

/// `sui-id setup --admin-username NAME [--admin-email EMAIL]
/// [--admin-display-name NAME]`.
///
/// RFC 077: headless initialization. Creates the first administrator and
/// bootstraps a signing key without the GUI wizard.
pub(crate) async fn run_setup(
    config_path: &Path,
    policy: PreMigrationBackup,
    admin_username: &str,
    admin_email: Option<&str>,
    admin_display_name: Option<&str>,
) -> Result<()> {
    let username = if admin_username.trim().is_empty() {
        bail!("setup requires a non-empty --admin-username NAME");
    } else {
        admin_username
    };

    let cfg = Config::load(config_path)
        .with_context(|| format!("loading config from {}", config_path.display()))?;
    let (db, _key_origin, _db_lock) =
        crate::database::open_checked(&cfg.storage.db_path, &cfg.storage.key_file, policy)
            .await
            .context("loading master key and opening database")?;

    let clock = sui_id_core::time::system_clock();

    let env_password = std::env::var("SUI_ID_ADMIN_PASSWORD")
        .ok()
        .filter(|s| !s.is_empty());
    let (password, generated): (zeroize::Zeroizing<String>, bool) = match env_password {
        Some(p) => (zeroize::Zeroizing::new(p), false),
        None => (sui_id_core::setup::generate_admin_password(), true),
    };

    let created = sui_id_core::setup::create_initial_admin_headless(
        &db,
        &clock,
        username,
        &password,
        admin_display_name,
        admin_email,
    )
    .await
    .map_err(|e| match e {
        sui_id_core::errors::CoreError::AlreadyInitialized => {
            anyhow::anyhow!("this instance is already initialized; setup can only run once")
        }
        other => anyhow::anyhow!("{other}"),
    })?;

    let login_url = format!("{}/admin/login", cfg.server.issuer.trim_end_matches('/'));

    eprintln!("sui-id initialized (admin id={}).", created.user_id);
    println!();
    println!("============ INITIAL ADMIN CREDENTIALS ============");
    println!("  username: {}", created.username);
    if generated {
        println!("  password: {}", password.as_str());
    }
    println!("  login:    {}", login_url);
    println!("===================================================");
    println!();
    if generated {
        println!("This password is shown only once. Change it after first");
        println!("login at /me/security/password.");
    } else {
        println!("Password taken from SUI_ID_ADMIN_PASSWORD.");
    }
    Ok(())
}

/// Read a passphrase from stdin (interactive TTY) or from the
/// `SUI_ID_BACKUP_PASSPHRASE` environment variable (for cron and scripted
/// use). When `confirm` is true and we're on a TTY, the passphrase is
/// asked twice and rejected on mismatch.
pub(crate) fn read_passphrase(prompt: &str, confirm: bool) -> Result<String> {
    if let Ok(env) = std::env::var("SUI_ID_BACKUP_PASSPHRASE")
        && !env.is_empty()
    {
        return Ok(env);
    }
    use std::io::{BufRead, Write};
    let stdin = std::io::stdin();
    let mut stderr = std::io::stderr();
    write!(stderr, "{prompt}: ").ok();
    stderr.flush().ok();
    let mut first = String::new();
    stdin
        .lock()
        .read_line(&mut first)
        .context("reading passphrase")?;
    let first = first.trim_end_matches(['\r', '\n']).to_string();
    if first.is_empty() {
        bail!("passphrase must not be empty");
    }
    if confirm {
        write!(stderr, "{prompt} (again): ").ok();
        stderr.flush().ok();
        let mut second = String::new();
        stdin
            .lock()
            .read_line(&mut second)
            .context("reading passphrase confirmation")?;
        let second = second.trim_end_matches(['\r', '\n']).to_string();
        if first != second {
            bail!("passphrases did not match");
        }
    }
    Ok(first)
}

/// `sui-id admin metrics-token rotate`.
///
/// Generates a fresh 32-byte random metrics bearer token, stores its
/// SHA-256 hex hash, and prints the raw token once to stdout.
pub(crate) async fn run_admin_rotate_metrics_token(
    config_path: &Path,
    policy: PreMigrationBackup,
) -> Result<()> {
    use getrandom::fill as rand_fill;
    use sha2::{Digest, Sha256};

    let cfg = Config::load(config_path)
        .with_context(|| format!("loading config from {}", config_path.display()))?;
    let (db, _key_origin, _db_lock) =
        crate::database::open_checked(&cfg.storage.db_path, &cfg.storage.key_file, policy)
            .await
            .context("loading master key and opening database")?;

    let mut raw = [0u8; 32];
    rand_fill(&mut raw).context("generating random bytes")?;
    let token: String = raw.iter().map(|b| format!("{b:02x}")).collect::<String>();

    let hash = Sha256::digest(token.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();

    let now = sui_id_core::time::system_clock().now();
    sui_id_store::commands::rotate_metrics_token(&db, Some(hash.clone()), now)
        .await
        .context("updating metrics_token_hash")?;

    println!("New metrics bearer token (shown ONCE — save it now):");
    println!();
    println!("  {token}");
    println!();
    println!("Update your Prometheus scrape config:");
    println!("  authorization:");
    println!("    type: Bearer");
    println!("    credentials: {token}");
    println!();
    println!(
        "Old token (if any) is now invalid. Update scrape configs before the          next scrape interval."
    );

    Ok(())
}

/// `sui-id admin registration-token issue [--max-uses N] [--note TEXT]`.
///
/// Generates a fresh RFC 7591 initial access token, stores its SHA-256
/// hash, and prints the raw token once to stdout.
pub(crate) async fn run_admin_issue_registration_token(
    config_path: &Path,
    policy: PreMigrationBackup,
    max_uses: i64,
    note: Option<&str>,
) -> Result<()> {
    use sha2::{Digest, Sha256};
    use sui_id_shared::ids::RegistrationTokenId;

    let cfg = Config::load(config_path)
        .with_context(|| format!("loading config from {}", config_path.display()))?;
    let (db, _key_origin, _db_lock) =
        crate::database::open_checked(&cfg.storage.db_path, &cfg.storage.key_file, policy)
            .await
            .context("loading master key and opening database")?;

    let mut raw = [0u8; 32];
    getrandom::fill(&mut raw).context("generating random bytes")?;
    let token: String = raw.iter().map(|b| format!("{b:02x}")).collect();

    let hash: String = Sha256::digest(token.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();

    let now = sui_id_core::time::system_clock().now();
    let row = sui_id_store::repos::client_registration_token::RegistrationTokenRow {
        id: RegistrationTokenId::new(),
        token_hash: hash,
        max_uses,
        used_count: 0,
        expires_at: None,
        revoked_at: None,
        note: note.map(str::to_owned),
        created_at: now,
        updated_at: now,
    };

    sui_id_store::commands::issue_registration_token(&db, row.clone())
        .await
        .context("storing registration token")?;

    print!(
        "{}",
        render_issued_registration_token(&token, row.id, max_uses)
    );
    Ok(())
}

fn render_issued_registration_token(
    token: &str,
    id: sui_id_shared::ids::RegistrationTokenId,
    max_uses: i64,
) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    let w = &mut out;
    let _ = writeln!(
        w,
        "RFC 7591 initial access token (shown ONCE — save it now):"
    );
    let _ = writeln!(w);
    let _ = writeln!(w, "  {token}");
    let _ = writeln!(w);
    let _ = writeln!(
        w,
        "Id (not secret — record it to revoke this token later with"
    );
    let _ = writeln!(w, "`admin registration-token revoke --id ID`):");
    let _ = writeln!(w);
    let _ = writeln!(w, "  {id}");
    let _ = writeln!(w);
    let _ = writeln!(w, "Usage (max_uses = {max_uses}):");
    let _ = writeln!(w, "  curl -X POST https://your-sui-id/oauth2/register \\");
    let _ = writeln!(w, "    -H 'Authorization: Bearer {token}' \\");
    let _ = writeln!(w, "    -H 'Content-Type: application/json' \\");
    let _ = writeln!(
        w,
        "    -d '{{\"client_name\":\"My App\",\"redirect_uris\":[\"https://app.example/cb\"]}}'"
    );
    out
}

/// `sui-id admin registration-token revoke --id ID`.
pub(crate) async fn run_admin_revoke_registration_token(
    config_path: &Path,
    policy: PreMigrationBackup,
    id_str: &str,
) -> Result<()> {
    use sui_id_shared::ids::RegistrationTokenId;

    let id: RegistrationTokenId = id_str
        .parse()
        .with_context(|| format!("{id_str:?} is not a valid registration-token id"))?;

    let cfg = Config::load(config_path)
        .with_context(|| format!("loading config from {}", config_path.display()))?;
    let (db, _key_origin, _db_lock) =
        crate::database::open_checked(&cfg.storage.db_path, &cfg.storage.key_file, policy)
            .await
            .context("loading master key and opening database")?;

    let now = sui_id_core::time::system_clock().now();
    sui_id_store::commands::revoke_registration_token(&db, id, now)
        .await
        .with_context(|| format!("revoking registration token {id}"))?;

    println!("Registration token {id} revoked.");
    Ok(())
}

/// `sui-id admin registration-token list`.
pub(crate) async fn run_admin_list_registration_tokens(
    config_path: &Path,
    policy: PreMigrationBackup,
) -> Result<()> {
    let cfg = Config::load(config_path)
        .with_context(|| format!("loading config from {}", config_path.display()))?;
    let (db, _key_origin, _db_lock) =
        crate::database::open_checked(&cfg.storage.db_path, &cfg.storage.key_file, policy)
            .await
            .context("loading master key and opening database")?;

    let rows = sui_id_store::repos::client_registration_token::list(&db)
        .await
        .context("listing registration tokens")?;
    print!("{}", render_registration_token_list(&rows));
    Ok(())
}

fn render_registration_token_list(
    rows: &[sui_id_store::repos::client_registration_token::RegistrationTokenRow],
) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    let w = &mut out;

    if rows.is_empty() {
        let _ = writeln!(w, "No registration tokens.");
        return out;
    }

    for row in rows {
        let max_uses = if row.max_uses == 0 {
            "unlimited".to_string()
        } else {
            row.max_uses.to_string()
        };
        let expires_at = row
            .expires_at
            .map(|t| t.to_rfc3339())
            .unwrap_or_else(|| "never".to_string());
        let revoked = match row.revoked_at {
            Some(at) => format!("revoked {}", at.to_rfc3339()),
            None => "active".to_string(),
        };
        let _ = writeln!(
            w,
            "id={} created_at={} max_uses={} used_count={} expires_at={} {}",
            row.id,
            row.created_at.to_rfc3339(),
            max_uses,
            row.used_count,
            expires_at,
            revoked,
        );
    }
    out
}

const LEGACY_WINDOW_START_COMMIT: &str = "48946f6";
const LEGACY_WINDOW_START_DATE: &str = "2026-07-02";
const LEGACY_WINDOW_END_COMMIT: &str = "930513b";
const LEGACY_WINDOW_END_DATE: &str = "2026-10-02";

fn oldest_evidence_seq(
    report: &sui_id_store::repos::audit::LegacyDynamicCandidateReport,
) -> Option<i64> {
    report
        .candidates
        .iter()
        .map(|c| c.audit_seq)
        .chain(report.unresolved.iter().map(|u| u.audit_seq))
        .min()
}

fn render_legacy_dynamic_candidate_report(
    report: &sui_id_store::repos::audit::LegacyDynamicCandidateReport,
    chain: Option<(i64, i64, &sui_id_store::repos::audit::ChainVerifyReport)>,
) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    let w = &mut out;

    let _ = writeln!(
        w,
        "RFC 095 legacy dynamic-registration candidate report (read-only; nothing here writes to the database)."
    );
    let _ = writeln!(w);
    let _ = writeln!(
        w,
        "Historical ambiguity window: {LEGACY_WINDOW_START_COMMIT} ({LEGACY_WINDOW_START_DATE}) \
         through {LEGACY_WINDOW_END_COMMIT} ({LEGACY_WINDOW_END_DATE})."
    );
    let _ = writeln!(
        w,
        "Within that window, `clients::create` discarded `registered_via`, and the audit \
         row that would identify a dynamic registration was written best-effort: if that \
         write failed, the registration still succeeded and left no trace at all. A client \
         with no `client.dynamic_register` audit row is therefore not evidence of anything \
         — it is a gap this report cannot see, not evidence that nothing happened."
    );
    let _ = writeln!(w);

    if report.candidates.is_empty() {
        let _ = writeln!(
            w,
            "No legacy_unstamped_dynamic_candidate is visible in the evidence this report is \
             permitted to use. This does NOT mean no such client exists — the historical \
             best-effort audit cannot prove completeness, so no \"zero candidates\" claim is \
             being made. See the incompleteness note above."
        );
    } else {
        let n = report.candidates.len();
        let _ = writeln!(
            w,
            "{n} legacy_unstamped_dynamic_candidate(s) FOUND in the evidence available — not \
             necessarily {n} that exist. The same incompleteness applies here: a dynamically \
             registered client with no surviving audit row would not appear in this list \
             either."
        );
        let _ = writeln!(w);
        for c in &report.candidates {
            let _ = writeln!(
                w,
                "  legacy_unstamped_dynamic_candidate: client_id={} name={:?} \
                 created_at={} is_disabled={} is_deleted={} audit_seq={} audit_at={}",
                c.client_id,
                c.client_name,
                c.created_at.to_rfc3339(),
                c.is_disabled,
                c.is_deleted,
                c.audit_seq,
                c.audit_at.to_rfc3339(),
            );
        }
    }

    let _ = writeln!(w);
    let _ = writeln!(
        w,
        "Audit-chain verification for the evidence above (informational, read-only):"
    );
    match chain {
        Some((oldest, newest, chain)) => {
            let limit = newest - oldest + 1;
            let _ = writeln!(
                w,
                "  verified seq range [{oldest}, {newest}] or older ({limit} rows read back \
                 from the tail, which reaches at least this far -- further if `seq` has any \
                 gaps): checked={}, broken_at_seq={:?}, legacy_unhashed={}.",
                chain.checked, chain.broken_at_seq, chain.legacy_unhashed
            );
            if chain.legacy_unhashed != 0 {
                let _ = writeln!(
                    w,
                    "  legacy_unhashed is nonzero, which should not happen for this window \
                     (the hash chain predates `registered_via`) — this is a finding, not a \
                     formality."
                );
            }
            if chain.broken_at_seq.is_some() {
                let _ = writeln!(
                    w,
                    "  broken_at_seq is set — the chain itself may be compromised in or \
                     before this range. Treat every row in it, candidate or not, as unverified."
                );
            }
        }
        None => {
            let _ = writeln!(
                w,
                "  no client.dynamic_register evidence row exists to anchor a verification \
                 range to, so none was run for this report. This says nothing about the rest \
                 of the audit log."
            );
        }
    }

    if !report.unresolved.is_empty() {
        let _ = writeln!(w);
        let _ = writeln!(
            w,
            "client.dynamic_register audit rows that do not resolve to any client row ({}) — \
             more alarming than a candidate, not a candidate themselves:",
            report.unresolved.len()
        );
        for u in &report.unresolved {
            let _ = writeln!(
                w,
                "  unresolved: audit_seq={} audit_at={} target={:?}",
                u.audit_seq,
                u.audit_at.to_rfc3339(),
                u.target,
            );
        }
    }

    let _ = writeln!(w);
    let _ = writeln!(
        w,
        "This report lists; it disposes nothing. Before RFC 095 M3 closes, the owner must \
         durably record, for every candidate above, disablement, explicit legacy acceptance, \
         or a separately audited metadata correction."
    );

    out
}

/// `sui-id admin registration-token legacy-candidates`.
pub(crate) async fn run_admin_legacy_dynamic_candidates(
    config_path: &Path,
    policy: PreMigrationBackup,
) -> Result<()> {
    let cfg = Config::load(config_path)
        .with_context(|| format!("loading config from {}", config_path.display()))?;
    let (db, _key_origin, _db_lock) =
        crate::database::open_checked(&cfg.storage.db_path, &cfg.storage.key_file, policy)
            .await
            .context("loading master key and opening database")?;

    let report = sui_id_store::repos::audit::legacy_dynamic_candidates(&db)
        .await
        .context("querying legacy dynamic-registration candidates")?;

    let oldest = oldest_evidence_seq(&report);
    let chain_result = match (oldest, report.max_seq) {
        (Some(oldest), Some(newest)) => {
            let limit = newest - oldest + 1;
            let r = sui_id_store::repos::audit::verify_chain_tail(&db, limit)
                .await
                .context("verifying audit chain over the candidate range")?;
            Some((oldest, newest, r))
        }
        _ => None,
    };
    let chain_ref = chain_result.as_ref().map(|(o, n, r)| (*o, *n, r));

    print!(
        "{}",
        render_legacy_dynamic_candidate_report(&report, chain_ref)
    );

    Ok(())
}

pub(crate) async fn serve(config_path: &Path, policy: PreMigrationBackup) -> Result<()> {
    let cfg = Config::load(config_path)
        .with_context(|| format!("loading config from {}", config_path.display()))?;

    let startup = crate::startup::prepare(cfg, policy).await?;
    let router = crate::build_router(startup.state.clone());

    crate::gc::spawn(startup.state.clone());
    {
        let smtp = std::sync::Arc::new(sui_id_core::mail::SmtpMailSender::new(
            startup.state.db.clone(),
            String::from("sui-id"),
        ));
        let worker = sui_id_core::mail::outbox::OutboxWorker::new(
            startup.state.db.clone(),
            smtp,
            startup.state.clock.clone(),
            5, // idle_tick_secs
            5, // max_attempts
        );
        worker.spawn();
    }
    {
        let worker = sui_id_core::forgot_password::ForgotPasswordWorker::new(
            startup.state.db.clone(),
            startup.state.clock.clone(),
            startup.state.mailer.clone(),
            startup.state.issuer().to_owned(),
            std::time::Duration::from_secs(1),
        );
        worker.spawn();
    }
    {
        let db = startup.state.db.clone();
        tokio::spawn(async move {
            match sui_id_store::repos::refresh_tokens::backfill_token_hashes(&db).await {
                Ok(0) => {}
                Ok(n) => tracing::info!(rows = n, "backfill: populated refresh_token.token_hash"),
                Err(e) => tracing::warn!(error = %e, "backfill: refresh_token.token_hash failed"),
            }
        });
    }

    let addr: std::net::SocketAddr = startup
        .listen_addr
        .parse()
        .with_context(|| format!("invalid listen_addr {}", startup.listen_addr))?;
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("binding {addr}"))?;
    tracing::info!(%addr, "sui-id listening");

    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .context("running server")?;
    Ok(())
}

/// `sui-id dev` / the pre-148 `--dev` flag's arguments, unprefixed (the
/// `dev-` prefix on each flag existed only because `--dev` itself was a
/// top-level mode flag; under the new subcommand there is nothing left to
/// disambiguate).
pub(crate) struct DevArgs {
    pub(crate) bind: Option<String>,
    pub(crate) db: Option<PathBuf>,
    pub(crate) seed: Option<PathBuf>,
    pub(crate) admin_password: Option<String>,
    pub(crate) client_secret: Option<String>,
}

/// `sui-id dev` / pre-148 `--dev`. See the pre-148 `serve_dev`'s own doc
/// history for the three ways this diverges from `serve` (database,
/// config, seed) — unchanged here.
pub(crate) async fn serve_dev(dev: DevArgs) -> Result<()> {
    use crate::dev_mode;

    let DevArgs {
        bind,
        db: dev_db,
        seed: dev_seed_path,
        admin_password: dev_admin_password,
        client_secret: dev_client_secret,
    } = dev;
    let dev_bind = bind.unwrap_or_else(|| "127.0.0.1:8801".into());

    let host_part = dev_bind
        .rsplit_once(':')
        .map(|(h, _)| h)
        .unwrap_or(&dev_bind);
    let is_loopback = host_part.starts_with("127.")
        || host_part == "::1"
        || host_part == "[::1]"
        || host_part == "localhost";
    if !is_loopback {
        dev_mode::confirm_external_bind(&dev_bind)?;
    }

    let (seed, seed_source) = dev_mode::resolve_seed(
        dev_seed_path.as_deref(),
        dev_mode::DevFlagOverrides {
            admin_password: dev_admin_password,
            client_secret: dev_client_secret,
        },
    )?;

    let mut cfg = Config::sample();
    cfg.server.listen_addr = dev_bind.clone();
    cfg.server.issuer = format!("http://{dev_bind}");
    cfg.server.cookie_secure = false;
    cfg.log.access_log = true;
    cfg.storage.db_path = dev_db
        .as_deref()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from(":memory:"));

    let db = dev_mode::open_dev_db(dev_db.as_deref())?;
    let setup_token = {
        use base64ct::Encoding;
        let mut buf = [0u8; 24];
        #[allow(clippy::expect_used)]
        getrandom::fill(&mut buf).expect("system RNG unavailable");
        base64ct::Base64::encode_string(&buf)
    };
    dev_mode::print_dev_warnings(&dev_bind, &seed_source);
    let outcome = {
        let clock = sui_id_core::time::system_clock();
        dev_mode::apply_seed(&db, &clock, &setup_token, &seed).await?
    };
    dev_mode::print_seed_summary(&seed, &outcome, &dev_bind);
    let _ = outcome.admin_user_id;

    let _log_guard = crate::startup::init_tracing(&cfg.log);
    crate::startup::warn_if_no_tls_roots();

    let mailer: std::sync::Arc<dyn sui_id_core::mail::MailSender> = std::sync::Arc::new(
        sui_id_core::mail::SmtpMailSender::new(db.clone(), String::from("sui-id-dev.local")),
    );
    let hibp_client: std::sync::Arc<dyn sui_id_core::hibp::HibpClient> =
        std::sync::Arc::new(sui_id_core::hibp::HttpHibpClient::new());
    let caches = std::sync::Arc::new(sui_id_core::cache::Caches::new());
    let mut state = crate::AppState::new(db, cfg, setup_token, mailer, hibp_client, caches);
    state.is_dev_mode = true;

    let router = crate::build_router(state.clone());
    crate::gc::spawn(state.clone());
    {
        let worker = sui_id_core::forgot_password::ForgotPasswordWorker::new(
            state.db.clone(),
            state.clock.clone(),
            state.mailer.clone(),
            state.issuer().to_owned(),
            std::time::Duration::from_secs(1),
        );
        worker.spawn();
    }
    {
        let db = state.db.clone();
        tokio::spawn(async move {
            match sui_id_store::repos::refresh_tokens::backfill_token_hashes(&db).await {
                Ok(0) => {}
                Ok(n) => tracing::info!(rows = n, "backfill: populated refresh_token.token_hash"),
                Err(e) => tracing::warn!(error = %e, "backfill: refresh_token.token_hash failed"),
            }
        });
    }

    let addr: std::net::SocketAddr = dev_bind
        .parse()
        .with_context(|| format!("invalid --bind {dev_bind}"))?;
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("binding {addr}"))?;
    tracing::info!(%addr, "sui-id (dev mode) listening");

    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .context("running dev server")?;
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let term = async {
        if let Ok(mut s) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            s.recv().await;
        }
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = term => {},
    }
    tracing::info!("graceful shutdown initiated");
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
#[path = "handlers/tests.rs"]
mod tests;
