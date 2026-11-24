//! RFC 148: the one command registry. This `clap` derive tree *is* the
//! list — the dispatch below is an exhaustive `match` on [`Command`], so a
//! command without a dispatch arm does not compile, and [`help`] /
//! [`reference`] render from the same tree via [`clap::CommandFactory`]
//! rather than a hand-kept second list (RFC 107's own defect).
//!
//! Every pre-148 form is kept, hidden from help, as either a separate
//! top-level [`Command`] variant (`restore`, `verify-backup`,
//! `preview-upgrade` — distinct words from the new tree, no collision) or
//! a sibling variant inside the group its new form nests under (`backup`'s
//! old flat `--to PATH` form on [`BackupArgs`] itself; the nine old
//! `admin <subaction>` names inside [`AdminCommand`]). Every such path
//! prints one notice to stderr (never stdout) before doing the old form's
//! job through the same handler the new form calls — see [`run`].

use anyhow::{Context, Result};
use clap::{CommandFactory, Parser, Subcommand};
use std::path::PathBuf;

mod first_run;
mod handlers;
pub mod help;
pub mod reference;

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests;

/// `--config PATH` and `--no-backup` are global (RFC 148 §1: `--config`
/// "stays one global option, valid on every command that reads the
/// configuration"); every other flag belongs to the leaf that reads it.
/// `disable_help_flag`/`disable_version_flag`: RFC 148 §2 lists
/// `--version`/`-V`/`--help`/`-h` as *aliases* that now print a notice,
/// which clap's own auto-generated flags cannot do — so they are plain
/// hidden fields below instead, and this struct's own `--help`/`--version`
/// are never reached. Per-subcommand `--help` (e.g. `sui-id backup
/// --help`) is untouched: this setting is not inherited by children.
#[derive(Parser, Debug)]
#[command(
    name = "sui-id",
    about = "Self-hosted OpenID Connect provider.",
    disable_help_flag = true,
    disable_help_subcommand = true,
    disable_version_flag = true
)]
pub struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// Path to the TOML configuration file.
    #[arg(long, global = true, value_name = "PATH")]
    config: Option<PathBuf>,

    /// Skip the automatic pre-migration database snapshot (RFC 142 stage
    /// 5). Applies to the server's own start and to any command that may
    /// migrate the database.
    #[arg(long, global = true)]
    no_backup: bool,

    // ── Pre-148 top-level mode flags, hidden, kept as aliases (§2) ──────
    #[arg(long = "print-sample-config", hide = true)]
    print_sample_config: bool,
    #[arg(long = "dev", hide = true)]
    dev_flag: bool,
    #[arg(long = "dev-bind", hide = true)]
    dev_bind: Option<String>,
    #[arg(long = "dev-db", hide = true)]
    dev_db: Option<PathBuf>,
    #[arg(long = "dev-seed", hide = true)]
    dev_seed: Option<PathBuf>,
    #[arg(long = "dev-admin-password", hide = true)]
    dev_admin_password: Option<String>,
    #[arg(long = "dev-client-secret", hide = true)]
    dev_client_secret: Option<String>,
    #[arg(long = "version", short = 'V', hide = true)]
    version_flag: bool,
    /// `global`: `disable_help_flag` above is inherited by every
    /// subcommand, so without this no command would accept `--help`.
    /// The root's is the pre-148 form; a subcommand's prints its own help.
    #[arg(long = "help", short = 'h', hide = true, global = true)]
    help_flag: bool,
}

/// RFC 148 §1's tree, one variant per leaf, plus the hidden pre-148
/// aliases named in this module's own doc comment. `Serve` is the explicit
/// spelling of what bare `sui-id` already does (`main.rs` handles "no
/// command" before this match is reached, so `Serve` itself has no arm
/// here — seeing it is a programmer error, not a user-reachable state;
/// see `run`'s own comment at that match arm).
#[derive(Subcommand, Debug)]
enum Command {
    /// Write a configuration to start from.
    #[command(about = "Write a starter configuration")]
    Init {
        /// Write the production template instead of the local-trial
        /// profile.
        #[arg(long)]
        production: bool,
        /// Destination path. Refused if it already exists.
        #[arg(long, value_name = "PATH")]
        output: Option<PathBuf>,
    },
    /// Create the first administrator (headless initialization).
    #[command(about = "Create the first admin")]
    Setup {
        /// The new administrator's username.
        #[arg(long)]
        admin_username: String,
        /// The new administrator's email address.
        #[arg(long)]
        admin_email: Option<String>,
        /// The new administrator's display name.
        #[arg(long)]
        admin_display_name: Option<String>,
    },
    /// Run the HTTP server — the same as no command at all.
    #[command(about = "Run the HTTP server")]
    Serve,
    /// Back up, restore, or verify the database.
    Backup(BackupArgs),
    /// Preview a schema upgrade.
    Upgrade(UpgradeArgs),
    /// Print or validate a configuration.
    Config(ConfigArgs),
    /// Administer a running instance.
    Admin(AdminArgs),
    /// Run a local dev server for testing. MUST NOT be used in production.
    #[command(about = "Run a local dev server")]
    Dev {
        /// Listen address (default: 127.0.0.1:8801). A non-loopback
        /// address requires explicit 'yes' confirmation on stdin.
        #[arg(long)]
        bind: Option<String>,
        /// Pin the dev-mode database to a file path (default:
        /// in-memory). The file is truncated each restart.
        #[arg(long)]
        db: Option<PathBuf>,
        /// TOML file describing dev-mode admin / users / OIDC clients to
        /// seed. See samples/dev-seed.toml.
        #[arg(long)]
        seed: Option<PathBuf>,
        /// Override the dev-mode admin password (default: 'admin').
        #[arg(long)]
        admin_password: Option<String>,
        /// Override the first dev-mode client's secret (default:
        /// 'test-secret').
        #[arg(long)]
        client_secret: Option<String>,
    },
    /// Show this help, or help for one command.
    #[command(about = "Show help")]
    Help { command: Vec<String> },
    /// Print version information.
    #[command(about = "Print version information")]
    Version,

    // ── Pre-148 aliases: distinct top-level words, no collision ────────
    #[command(hide = true)]
    Restore {
        #[arg(long)]
        from: PathBuf,
        #[arg(long)]
        force: bool,
        #[arg(long)]
        decrypt: bool,
    },
    #[command(hide = true)]
    VerifyBackup {
        #[arg(long)]
        from: PathBuf,
        #[arg(long)]
        decrypt: bool,
    },
    #[command(hide = true)]
    PreviewUpgrade,
}

#[derive(clap::Args, Debug)]
struct BackupArgs {
    #[command(subcommand)]
    command: Option<BackupCommand>,
    /// Pre-148 flat form (`sui-id backup --to PATH [--encrypt]`), valid
    /// only when no subcommand is given.
    #[arg(long, hide = true)]
    to: Option<PathBuf>,
    #[arg(long, hide = true)]
    encrypt: bool,
}

#[derive(Subcommand, Debug)]
enum BackupCommand {
    /// Write a tarball snapshot of the database, master key, and manifest.
    #[command(about = "Back up the database")]
    Create {
        /// Output path for the backup tarball.
        #[arg(long)]
        to: PathBuf,
        /// Encrypt the backup with a passphrase (read from
        /// SUI_ID_BACKUP_PASSPHRASE, or prompted).
        #[arg(long)]
        encrypt: bool,
    },
    /// Restore a backup tarball into the configured storage paths.
    #[command(about = "Restore from a backup")]
    Restore {
        /// Input path for the backup tarball.
        #[arg(long)]
        from: PathBuf,
        /// Overwrite existing files at the configured storage paths.
        #[arg(long)]
        force: bool,
        /// Treat the input as an encrypted backup.
        #[arg(long)]
        decrypt: bool,
    },
    /// Read a backup file and report what it contains. Writes nothing.
    #[command(about = "Check a backup is valid")]
    Verify {
        /// Input path for the backup tarball.
        #[arg(long)]
        from: PathBuf,
        /// Treat the input as an encrypted backup.
        #[arg(long)]
        decrypt: bool,
    },
}

#[derive(clap::Args, Debug)]
struct UpgradeArgs {
    #[command(subcommand)]
    command: UpgradeCommand,
}

#[derive(Subcommand, Debug)]
enum UpgradeCommand {
    /// Read-only. Report what this build would do to the configured
    /// database without opening it for writing.
    #[command(about = "Preview a schema upgrade")]
    Preview,
}

#[derive(clap::Args, Debug)]
struct ConfigArgs {
    #[command(subcommand)]
    command: ConfigCommand,
}

#[derive(Subcommand, Debug)]
enum ConfigCommand {
    /// Print a configuration to start from.
    #[command(about = "Print a starter configuration")]
    Sample {
        /// Print the production template (`sui-id.example.toml`) instead
        /// of the local-trial sample.
        #[arg(long)]
        production: bool,
    },
    /// Validate a configuration and change nothing.
    #[command(about = "Validate a configuration")]
    Check,
}

#[derive(clap::Args, Debug)]
struct AdminArgs {
    #[command(subcommand)]
    command: AdminCommand,
}

#[derive(Subcommand, Debug)]
enum AdminCommand {
    /// Manage an individual user's lockout, MFA, or recovery.
    User(AdminUserArgs),
    /// Rotate the master key.
    MasterKey(AdminMasterKeyArgs),
    /// Rotate the metrics bearer token.
    MetricsToken(AdminMetricsTokenArgs),
    /// Manage dynamic-client-registration tokens.
    RegistrationToken(AdminRegistrationTokenArgs),

    // ── Pre-148 aliases: same depth as the new nouns above ──────────────
    #[command(hide = true)]
    UnlockUser {
        #[arg(long)]
        username: String,
    },
    #[command(hide = true)]
    ResetMfa {
        #[arg(long)]
        username: String,
        #[arg(long)]
        reason: String,
    },
    #[command(hide = true)]
    IssueRecoveryLink {
        #[arg(long)]
        username: String,
        #[arg(long)]
        reason: String,
    },
    #[command(hide = true)]
    RotateKey {
        #[arg(long, conflicts_with = "new_key")]
        generate_new_key: bool,
        #[arg(long)]
        new_key: Option<PathBuf>,
        #[arg(long, short = 'y')]
        yes: bool,
        #[arg(long)]
        check: bool,
        #[arg(long, value_name = "RFC3339_INSTANT", conflicts_with_all = ["remove_backup_after_verified", "verification_reference"])]
        retain_backup_until: Option<String>,
        #[arg(long, value_name = "TEXT", requires = "retain_backup_until")]
        custody_reference: Option<String>,
        #[arg(long, conflicts_with_all = ["retain_backup_until", "custody_reference"])]
        remove_backup_after_verified: bool,
        #[arg(long, value_name = "TEXT", requires = "remove_backup_after_verified")]
        verification_reference: Option<String>,
    },
    #[command(hide = true)]
    RotateMetricsToken,
    #[command(hide = true)]
    IssueRegistrationToken {
        #[arg(long, default_value_t = 1)]
        max_uses: i64,
        #[arg(long)]
        note: Option<String>,
    },
    #[command(hide = true)]
    RevokeRegistrationToken {
        #[arg(long)]
        id: String,
    },
    #[command(hide = true)]
    ListRegistrationTokens,
    #[command(hide = true)]
    LegacyDynamicCandidates,
}

#[derive(clap::Args, Debug)]
struct AdminUserArgs {
    #[command(subcommand)]
    command: AdminUserCommand,
}

#[derive(Subcommand, Debug)]
enum AdminUserCommand {
    /// Clear an account-lockout state for the given user.
    #[command(about = "Clear an account lockout")]
    Unlock {
        /// The user's username.
        #[arg(long)]
        username: String,
    },
    /// Remove every MFA factor (TOTP and passkeys) for the given user.
    #[command(about = "Remove a user's MFA factors")]
    ResetMfa {
        /// The user's username.
        #[arg(long)]
        username: String,
        /// Why, recorded in the audit log.
        #[arg(long)]
        reason: String,
    },
    /// Issue a single-use account-recovery link for a local user.
    #[command(about = "Issue a recovery link")]
    RecoveryLink {
        /// The user's username.
        #[arg(long)]
        username: String,
        /// Why, recorded in the audit log.
        #[arg(long)]
        reason: String,
    },
}

#[derive(clap::Args, Debug)]
struct AdminMasterKeyArgs {
    #[command(subcommand)]
    command: AdminMasterKeyCommand,
}

#[derive(Subcommand, Debug)]
enum AdminMasterKeyCommand {
    /// Re-seal every encrypted column under a new master key. Runs
    /// OFFLINE: stop the server first and take a fresh backup.
    #[command(about = "Rotate the master key")]
    Rotate {
        /// Mint a fresh key from the OS RNG (default if neither this nor
        /// --new-key is given).
        #[arg(long, conflicts_with = "new_key")]
        generate_new_key: bool,
        /// Use an already-prepared new key file (e.g. from an
        /// HSM-backed workflow) instead of generating one.
        #[arg(long)]
        new_key: Option<PathBuf>,
        /// Skip the interactive confirmation prompt.
        #[arg(long, short = 'y')]
        yes: bool,
        /// Check whether a rotation could start, and change nothing.
        #[arg(long)]
        check: bool,
        /// RFC 100 old-key custody: keep the pre-rotation backup of the
        /// old key until this RFC 3339 instant (must be in the future),
        /// under --custody-reference. Exactly one of this or
        /// --remove-backup-after-verified is required (--check needs
        /// neither).
        #[arg(long, value_name = "RFC3339_INSTANT", conflicts_with_all = ["remove_backup_after_verified", "verification_reference"])]
        retain_backup_until: Option<String>,
        /// Who is accountable for reviewing the retained backup (RFC
        /// 100 old-key custody) -- required with --retain-backup-until.
        #[arg(long, value_name = "TEXT", requires = "retain_backup_until")]
        custody_reference: Option<String>,
        /// RFC 100 old-key custody: remove the pre-rotation backup of
        /// the old key once verified, citing --verification-reference.
        /// Exactly one of this or --retain-backup-until is required
        /// (--check needs neither).
        #[arg(long, conflicts_with_all = ["retain_backup_until", "custody_reference"])]
        remove_backup_after_verified: bool,
        /// How the backup was verified (RFC 100 old-key custody) --
        /// required with --remove-backup-after-verified.
        #[arg(long, value_name = "TEXT", requires = "remove_backup_after_verified")]
        verification_reference: Option<String>,
    },
}

#[derive(clap::Args, Debug)]
struct AdminMetricsTokenArgs {
    #[command(subcommand)]
    command: AdminMetricsTokenCommand,
}

#[derive(Subcommand, Debug)]
enum AdminMetricsTokenCommand {
    /// Generate a fresh metrics bearer token; the previous one stops
    /// working immediately.
    #[command(about = "Rotate the metrics token")]
    Rotate,
}

#[derive(clap::Args, Debug)]
struct AdminRegistrationTokenArgs {
    #[command(subcommand)]
    command: AdminRegistrationTokenCommand,
}

#[derive(Subcommand, Debug)]
enum AdminRegistrationTokenCommand {
    /// Issue an RFC 7591 initial access token for dynamic client
    /// registration.
    #[command(about = "Issue a registration token")]
    Issue {
        /// How many times the token may be redeemed (default: 1).
        #[arg(long, default_value_t = 1)]
        max_uses: i64,
        /// An operator note stored alongside the token.
        #[arg(long)]
        note: Option<String>,
    },
    /// Immediately revoke a client-registration token by id.
    #[command(about = "Revoke a registration token")]
    Revoke {
        /// The registration token's id.
        #[arg(long)]
        id: String,
    },
    /// List every client-registration token. Never prints a token or its
    /// hash.
    #[command(about = "List registration tokens")]
    List,
    /// Read-only (RFC 095). List `clients` rows a historical bug may have
    /// mislabelled.
    #[command(about = "List mislabelled clients")]
    LegacyCandidates,
}

/// One notice line (RFC 148 §2), stderr only, never stdout — so a piped
/// alias such as `sui-id --print-sample-config > sui-id.toml` writes
/// exactly what it writes today.
fn alias_notice(old: &str, new: &str) {
    eprintln!("note: `sui-id {old}` is now `sui-id {new}`; the old form still works.");
}

fn config_path(cli: &Cli) -> PathBuf {
    cli.config
        .clone()
        .unwrap_or_else(|| PathBuf::from("./sui-id.toml"))
}

/// `--no-backup` is global (declared once on [`Cli`]); every caller that
/// reaches [`crate::database::open`] asks it the same way.
fn backup_policy(cli: &Cli) -> crate::database::PreMigrationBackup {
    if cli.no_backup {
        crate::database::PreMigrationBackup::Declined
    } else {
        crate::database::PreMigrationBackup::Take
    }
}

/// Parsed from `std::env::args()` by the caller (`main.rs`), so a test can
/// pass a synthetic argv instead. `--version`/`--help` win over a parse
/// error from the rest of the line, same as today (`main.rs` used to check
/// them before ever walking argv for a subcommand) — checked here before
/// `Cli::try_parse_from` can reject an otherwise-incomplete line.
/// `Some(path)` when `--help` (or `-h`) was given *on a subcommand*, with
/// that subcommand's path; `None` otherwise, including the root's own
/// `--help`, which is the pre-148 form `run` handles with its notice.
fn help_requested_on_a_command(args: &[String]) -> Option<Vec<String>> {
    fn relax(cmd: clap::Command) -> clap::Command {
        cmd.subcommand_required(false)
            .arg_required_else_help(false)
            .mut_args(|a| a.required(false))
            .mut_subcommands(relax)
    }
    let relaxed = relax(<Cli as clap::CommandFactory>::command());
    let matches = relaxed.try_get_matches_from(args).ok()?;
    let mut path = Vec::new();
    let mut level = &matches;
    while let Some((name, sub)) = level.subcommand() {
        path.push(name.to_string());
        level = sub;
    }
    (!path.is_empty() && level.get_flag("help_flag")).then_some(path)
}

pub async fn run(args: Vec<String>) -> Result<()> {
    // No scan of the raw arguments (review fold-in, 0.93.0 O2a): `clap`
    // parses `--help` and `--version` like any other flag. A scan ahead of
    // it took `sui-id backup create --help` for the top-level form, and a
    // flag's *value* spelled `-h` for the help flag, which is RFC 107's bug
    // class. The root's own `--help`/`--version` are the pre-148 forms,
    // handled in the `None` arm below; a subcommand's `--help` is `clap`'s.
    // A subcommand's `--help` prints that command's own detail, with no
    // notice: it is not a pre-148 form. clap checks required arguments
    // before acting on an ordinary flag, so `sui-id backup create --help`
    // would fail on its missing `--to`. So a relaxed copy of the registry,
    // with nothing required, answers one question first: was help asked
    // for, and on which command? The path comes from clap's own matches,
    // so a flag's value is never taken for part of it.
    if let Some(path) = help_requested_on_a_command(&args) {
        if !help::print_help(&path) {
            std::process::exit(2);
        }
        return Ok(());
    }

    let mut cli = match Cli::try_parse_from(&args) {
        Ok(cli) => cli,
        Err(e) => e.exit(),
    };

    // Read before `cli.command` is matched by value below -- a struct
    // cannot be borrowed as a whole once any one of its fields has been
    // moved out of it, even to read an unrelated field.
    let cfg_path = config_path(&cli);
    let explicit_config = cli.config.is_some();
    let policy = backup_policy(&cli);
    let command = cli.command.take();

    match command {
        None => {
            if cli.version_flag {
                alias_notice("--version", "version");
                help::print_version();
                return Ok(());
            }
            if cli.help_flag {
                alias_notice("--help", "help");
                help::print_overview();
                return Ok(());
            }
            if cli.print_sample_config {
                alias_notice("--print-sample-config", "config sample");
                return handlers::config_sample(false);
            }
            if cli.dev_flag {
                alias_notice("--dev", "dev");
                return handlers::serve_dev(handlers::DevArgs {
                    bind: cli.dev_bind,
                    db: cli.dev_db,
                    seed: cli.dev_seed,
                    admin_password: cli.dev_admin_password,
                    client_secret: cli.dev_client_secret,
                })
                .await;
            }
            // Bare `sui-id [--config PATH]`: the documented service unit.
            // No notice (§3): the server should not gain a stderr line on
            // every start. RFC 148 §4 (O2b): a missing configuration is
            // a first-run conversation, not a bare refusal -- but only
            // in a terminal.
            first_run::serve_or_first_run(&cfg_path, explicit_config, policy).await
        }
        Some(Command::Serve) => {
            first_run::serve_or_first_run(&cfg_path, explicit_config, policy).await
        }
        Some(Command::Init { production, output }) => {
            let output = output.unwrap_or_else(|| PathBuf::from("./sui-id.toml"));
            handlers::run_init(&output, production)
        }
        Some(Command::Setup {
            admin_username,
            admin_email,
            admin_display_name,
        }) => {
            handlers::run_setup(
                &cfg_path,
                policy,
                &admin_username,
                admin_email.as_deref(),
                admin_display_name.as_deref(),
            )
            .await
        }
        Some(Command::Backup(BackupArgs {
            command: Some(BackupCommand::Create { to, encrypt }),
            ..
        })) => handlers::run_backup(&cfg_path, &to, encrypt),
        Some(Command::Backup(BackupArgs {
            command:
                Some(BackupCommand::Restore {
                    from,
                    force,
                    decrypt,
                }),
            ..
        })) => handlers::run_restore(&cfg_path, &from, force, decrypt),
        Some(Command::Backup(BackupArgs {
            command: Some(BackupCommand::Verify { from, decrypt }),
            ..
        })) => handlers::run_verify_backup(&from, decrypt),
        Some(Command::Backup(BackupArgs {
            command: None,
            to,
            encrypt,
        })) => {
            let to = to.context("backup requires --to PATH")?;
            alias_notice("backup --to PATH", "backup create --to PATH");
            handlers::run_backup(&cfg_path, &to, encrypt)
        }
        Some(Command::Upgrade(UpgradeArgs {
            command: UpgradeCommand::Preview,
        })) => handlers::run_preview_upgrade(&cfg_path),
        Some(Command::Config(ConfigArgs {
            command: ConfigCommand::Sample { production },
        })) => handlers::config_sample(production),
        Some(Command::Config(ConfigArgs {
            command: ConfigCommand::Check,
        })) => handlers::config_check(&cfg_path),
        Some(Command::Admin(AdminArgs { command })) => {
            dispatch_admin(&cfg_path, policy, command).await
        }
        Some(Command::Dev {
            bind,
            db,
            seed,
            admin_password,
            client_secret,
        }) => {
            handlers::serve_dev(handlers::DevArgs {
                bind,
                db,
                seed,
                admin_password,
                client_secret,
            })
            .await
        }
        Some(Command::Help { command }) => {
            if help::print_help(&command) {
                Ok(())
            } else {
                std::process::exit(2);
            }
        }
        Some(Command::Version) => {
            help::print_version();
            Ok(())
        }
        Some(Command::Restore {
            from,
            force,
            decrypt,
        }) => {
            alias_notice("restore", "backup restore");
            handlers::run_restore(&cfg_path, &from, force, decrypt)
        }
        Some(Command::VerifyBackup { from, decrypt }) => {
            alias_notice("verify-backup", "backup verify");
            handlers::run_verify_backup(&from, decrypt)
        }
        Some(Command::PreviewUpgrade) => {
            alias_notice("preview-upgrade", "upgrade preview");
            handlers::run_preview_upgrade(&cfg_path)
        }
    }
}

async fn dispatch_admin(
    cfg_path: &std::path::Path,
    policy: crate::database::PreMigrationBackup,
    command: AdminCommand,
) -> Result<()> {
    match command {
        AdminCommand::User(AdminUserArgs { command }) => match command {
            AdminUserCommand::Unlock { username } => {
                handlers::run_admin_unlock_user(cfg_path, policy, &username).await
            }
            AdminUserCommand::ResetMfa { username, reason } => {
                handlers::run_admin_reset_mfa(cfg_path, policy, &username, &reason).await
            }
            AdminUserCommand::RecoveryLink { username, reason } => {
                handlers::run_admin_issue_recovery_link(cfg_path, policy, &username, &reason).await
            }
        },
        AdminCommand::MasterKey(AdminMasterKeyArgs {
            command:
                AdminMasterKeyCommand::Rotate {
                    generate_new_key,
                    new_key,
                    yes,
                    check,
                    retain_backup_until,
                    custody_reference,
                    remove_backup_after_verified,
                    verification_reference,
                },
        }) => {
            handlers::run_admin_rotate_key(
                cfg_path,
                policy,
                generate_new_key,
                new_key,
                yes,
                check,
                handlers::CustodyChoiceArgs {
                    retain_backup_until,
                    custody_reference,
                    remove_backup_after_verified,
                    verification_reference,
                },
            )
            .await
        }
        AdminCommand::MetricsToken(AdminMetricsTokenArgs {
            command: AdminMetricsTokenCommand::Rotate,
        }) => handlers::run_admin_rotate_metrics_token(cfg_path, policy).await,
        AdminCommand::RegistrationToken(AdminRegistrationTokenArgs { command }) => match command {
            AdminRegistrationTokenCommand::Issue { max_uses, note } => {
                handlers::run_admin_issue_registration_token(
                    cfg_path,
                    policy,
                    max_uses,
                    note.as_deref(),
                )
                .await
            }
            AdminRegistrationTokenCommand::Revoke { id } => {
                handlers::run_admin_revoke_registration_token(cfg_path, policy, &id).await
            }
            AdminRegistrationTokenCommand::List => {
                handlers::run_admin_list_registration_tokens(cfg_path, policy).await
            }
            AdminRegistrationTokenCommand::LegacyCandidates => {
                handlers::run_admin_legacy_dynamic_candidates(cfg_path, policy).await
            }
        },
        AdminCommand::UnlockUser { username } => {
            alias_notice("admin unlock-user", "admin user unlock");
            handlers::run_admin_unlock_user(cfg_path, policy, &username).await
        }
        AdminCommand::ResetMfa { username, reason } => {
            alias_notice("admin reset-mfa", "admin user reset-mfa");
            handlers::run_admin_reset_mfa(cfg_path, policy, &username, &reason).await
        }
        AdminCommand::IssueRecoveryLink { username, reason } => {
            alias_notice("admin issue-recovery-link", "admin user recovery-link");
            handlers::run_admin_issue_recovery_link(cfg_path, policy, &username, &reason).await
        }
        AdminCommand::RotateKey {
            generate_new_key,
            new_key,
            yes,
            check,
            retain_backup_until,
            custody_reference,
            remove_backup_after_verified,
            verification_reference,
        } => {
            alias_notice("admin rotate-key", "admin master-key rotate");
            handlers::run_admin_rotate_key(
                cfg_path,
                policy,
                generate_new_key,
                new_key,
                yes,
                check,
                handlers::CustodyChoiceArgs {
                    retain_backup_until,
                    custody_reference,
                    remove_backup_after_verified,
                    verification_reference,
                },
            )
            .await
        }
        AdminCommand::RotateMetricsToken => {
            alias_notice("admin rotate-metrics-token", "admin metrics-token rotate");
            handlers::run_admin_rotate_metrics_token(cfg_path, policy).await
        }
        AdminCommand::IssueRegistrationToken { max_uses, note } => {
            alias_notice(
                "admin issue-registration-token",
                "admin registration-token issue",
            );
            handlers::run_admin_issue_registration_token(
                cfg_path,
                policy,
                max_uses,
                note.as_deref(),
            )
            .await
        }
        AdminCommand::RevokeRegistrationToken { id } => {
            alias_notice(
                "admin revoke-registration-token",
                "admin registration-token revoke",
            );
            handlers::run_admin_revoke_registration_token(cfg_path, policy, &id).await
        }
        AdminCommand::ListRegistrationTokens => {
            alias_notice(
                "admin list-registration-tokens",
                "admin registration-token list",
            );
            handlers::run_admin_list_registration_tokens(cfg_path, policy).await
        }
        AdminCommand::LegacyDynamicCandidates => {
            alias_notice(
                "admin legacy-dynamic-candidates",
                "admin registration-token legacy-candidates",
            );
            handlers::run_admin_legacy_dynamic_candidates(cfg_path, policy).await
        }
    }
}

/// The `clap::Command` tree this binary's registry builds — the single
/// source [`help`] and [`reference`] both walk. Never used for parsing
/// itself (`Cli::try_parse_from` does that); only for introspection, so
/// help and the reference page cannot name a command the parser does not
/// also accept, or vice versa.
pub(crate) fn command_tree() -> clap::Command {
    Cli::command()
}
