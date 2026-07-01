//! Outbound mail.
//!
//! sui-id sends a tiny number of emails: forgot-password reset
//! links, password-change notifications. Both are direct,
//! transactional, low-volume — there is no batching, no campaign
//! engine, no template inheritance.
//!
//! # Architecture
//!
//! - [`MailSender`] is the trait every consumer of this module
//!   talks to. It hides whether we're going out over SMTP, into
//!   an in-memory test capture, or somewhere else.
//! - [`SmtpMailSender`] is the production implementation. It
//!   reads the live `smtp_config` row on every send (so config
//!   changes apply immediately), opens a TLS or STARTTLS
//!   connection through `wasm-smtp-tokio`, and emits a single
//!   message.
//! - [`InMemoryMailSender`] is the test implementation. It
//!   captures every send so e2e tests can assert "yes, the
//!   forgot-password handler tried to send an email; here are
//!   its contents".
//!
//! # What we do not do here
//!
//! - **No background queue.** Sends are inline: the HTTP handler
//!   awaits the result. The SMTP timeout is short (a few seconds);
//!   on failure we record an audit event and let the user-facing
//!   response continue. See the v0.22.0 CHANGELOG entry for
//!   rationale.
//! - **No templating engine.** The two messages we send are
//!   built with `mail-builder` directly. Adding a templating layer
//!   would buy nothing for two messages and would complicate the
//!   localisation story when we want to add it later.
//! - **No DKIM signing.** Out of scope for v0.22.0 — see the
//!   ROADMAP entry on email deliverability.

pub mod outbox;

use crate::errors::{CoreError, CoreResult};
use crate::time::SharedClock;
use mail_builder::MessageBuilder;
use sui_id_store::Database;
use sui_id_store::repos::smtp_config;
use tokio::sync::Mutex;
use wasm_smtp::SmtpClient;
use wasm_smtp_tokio::{TokioPlainTransport, TokioTlsTransport};

/// One outgoing email. `text_body` is required (a plain-text
/// fallback is always present); `html_body` is optional. When both
/// are present, `mail-builder` emits a `multipart/alternative`
/// body so well-behaved clients pick the variant they prefer.
#[derive(Debug, Clone)]
pub struct OutgoingMail {
    pub to: String,
    pub subject: String,
    pub text_body: String,
    pub html_body: Option<String>,
    /// Resolved recipient locale for the outbox worker (RFC 002 § C).
    /// `None` falls back to the server default at render time.
    pub locale: Option<sui_id_i18n::Locale>,
}

/// Outcome of a successful send. The caller folds this into the
/// audit log so post-mortem investigation can correlate sui-id's
/// "we sent this email at 14:07" with the relay's logs.
#[derive(Debug, Clone)]
pub struct MailSendOutcome {
    /// The address sui-id sent FROM (the SMTP envelope, not
    /// necessarily the visible `From:` header). Comes from
    /// `smtp_config.from_address`.
    pub from: String,
    /// Recipient. Single-addressed; we don't fan out from this
    /// layer.
    pub to: String,
    /// Message subject (mostly for logging, never the body).
    pub subject: String,
}

/// The two-method trait every mail consumer in core depends on.
/// Concrete implementations are object-safe; we hand around
/// `Arc<dyn MailSender>` from `AppState`.
pub trait MailSender: Send + Sync {
    /// Send one email. Implementations must not block longer than
    /// their internal timeout; the caller is awaiting this and the
    /// HTTP response is on the other side.
    fn send<'a>(
        &'a self,
        mail: OutgoingMail,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = CoreResult<MailSendOutcome>> + Send + 'a>>;
}

// ---------- production: SmtpMailSender ----------

/// Production `MailSender` backed by `wasm-smtp-tokio`.
///
/// Holds a reference to the database and reads the live
/// `smtp_config` row on every send. We deliberately don't cache
/// the config — the DB row is the source of truth, and config
/// changes apply immediately without restart (one of the reasons
/// we put SMTP config in the DB rather than `sui-id.toml`).
///
/// The master key is read through `Database::key()` at send time
/// rather than copied here: `MasterKey` is intentionally not
/// `Clone`, and pinning the live db reference is cleaner than
/// inventing a clone path.
pub struct SmtpMailSender {
    db: Database,
    /// EHLO hostname sent to the remote relay. Defaults to a
    /// reasonable placeholder; an operator who needs a specific
    /// EHLO name (some relays inspect it for SPF) can configure
    /// it here.
    ehlo_hostname: String,
}

impl SmtpMailSender {
    pub fn new(db: Database, ehlo_hostname: impl Into<String>) -> Self {
        Self {
            db,
            ehlo_hostname: ehlo_hostname.into(),
        }
    }

    /// One-shot connectivity check used by the `/admin/settings/email`
    /// "Test Connection" button. Goes through EHLO and AUTH (when
    /// credentials are configured) but does *not* send a message,
    /// so it's safe to run against a real production relay without
    /// generating bounce traffic.
    ///
    /// Returns `Ok(())` on success; on failure, the returned
    /// `CoreError` is a `BadRequest` whose message is the SMTP
    /// error chain. The handler displays this string to the admin
    /// directly — it's the kind of error an operator wants to see
    /// verbatim ("550 5.7.1 relay denied", "auth failed", etc).
    pub async fn test_connection(&self) -> CoreResult<()> {
        let cfg = smtp_config::get(&self.db)
            .await?
            .ok_or_else(|| CoreError::BadRequest("SMTP is not configured".into()))?;
        let password = smtp_config::decrypt_password(&cfg, self.db.key()).await?;
        run_smtp_session(&cfg, password.as_deref(), &self.ehlo_hostname, None)
            .await
            .map_err(|e| CoreError::BadRequest(format!("SMTP test failed: {e}")))?;
        Ok(())
    }
}

impl MailSender for SmtpMailSender {
    fn send<'a>(
        &'a self,
        mail: OutgoingMail,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = CoreResult<MailSendOutcome>> + Send + 'a>>
    {
        Box::pin(async move {
            let cfg = smtp_config::get(&self.db)
                .await?
                .ok_or_else(|| CoreError::BadRequest("SMTP is not configured".into()))?;
            if !cfg.enabled {
                return Err(CoreError::BadRequest("SMTP is disabled".into()));
            }
            let password = smtp_config::decrypt_password(&cfg, self.db.key()).await?;
            let from = cfg.from_address.clone();
            let subject = mail.subject.clone();
            let to_addr = mail.to.clone();
            run_smtp_session(&cfg, password.as_deref(), &self.ehlo_hostname, Some(&mail))
                .await
                .map_err(|e| {
                    tracing::warn!(error = %e, "SMTP send failed");
                    CoreError::BadRequest(format!("SMTP send failed: {e}"))
                })?;
            Ok(MailSendOutcome {
                from,
                to: to_addr,
                subject,
            })
        })
    }
}

/// The actual SMTP dance. Connects (implicit-TLS or STARTTLS),
/// authenticates if credentials are present, then either
/// `send_message` (when `mail` is `Some`) or just `quit`
/// (the "test connection" path).
///
/// Kept as a free function so both `send` and `test_connection`
/// share exactly one wire-level path.
async fn run_smtp_session(
    cfg: &sui_id_store::models::SmtpConfigRow,
    password: Option<&str>,
    ehlo_hostname: &str,
    mail: Option<&OutgoingMail>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use sui_id_store::models::SmtpTlsMode;

    match cfg.tls_mode {
        SmtpTlsMode::Implicit => {
            let transport =
                TokioTlsTransport::connect_implicit_tls(&cfg.host, cfg.port, &cfg.host).await?;
            let client = SmtpClient::connect(transport, ehlo_hostname).await?;
            authenticate_and_dispatch(client, cfg, password, mail).await?;
        }
        SmtpTlsMode::StartTls => {
            let transport = TokioPlainTransport::connect(&cfg.host, cfg.port, &cfg.host).await?;
            let client = SmtpClient::connect_starttls(transport, ehlo_hostname).await?;
            authenticate_and_dispatch(client, cfg, password, mail).await?;
        }
    }
    Ok(())
}

/// Drive an open `SmtpClient` through (optional) AUTH, (optional)
/// `send_message`, and `quit`. Takes the client by value because
/// `quit` consumes `self`.
async fn authenticate_and_dispatch<T: wasm_smtp::Transport>(
    mut client: SmtpClient<T>,
    cfg: &sui_id_store::models::SmtpConfigRow,
    password: Option<&str>,
    mail: Option<&OutgoingMail>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if let (Some(user), Some(pass)) = (cfg.username.as_deref(), password) {
        client.login(user, pass).await?;
    }
    if let Some(mail) = mail {
        let mut builder = MessageBuilder::new()
            .from(build_from(&cfg.from_address, cfg.from_name.as_deref()))
            .to(mail.to.as_str())
            .subject(mail.subject.as_str())
            .text_body(mail.text_body.as_str());
        if let Some(html) = mail.html_body.as_deref() {
            builder = builder.html_body(html);
        }
        client
            .send_message(&cfg.from_address, &[mail.to.as_str()], builder)
            .await?;
    }
    client.quit().await?;
    Ok(())
}

fn build_from<'a>(
    addr: &'a str,
    name: Option<&'a str>,
) -> mail_builder::headers::address::Address<'a> {
    match name {
        Some(n) => (n, addr).into(),
        None => addr.into(),
    }
}

// ---------- testing: InMemoryMailSender ----------

/// In-memory `MailSender` for tests. Captures every send so e2e
/// tests can inspect what was almost-emailed.
#[derive(Default)]
pub struct InMemoryMailSender {
    sent: Mutex<Vec<OutgoingMail>>,
}

impl InMemoryMailSender {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn drain(&self) -> Vec<OutgoingMail> {
        let mut g = self.sent.lock().await;
        std::mem::take(&mut *g)
    }

    pub async fn last(&self) -> Option<OutgoingMail> {
        self.sent.lock().await.last().cloned()
    }

    pub async fn count(&self) -> usize {
        self.sent.lock().await.len()
    }
}

impl MailSender for InMemoryMailSender {
    fn send<'a>(
        &'a self,
        mail: OutgoingMail,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = CoreResult<MailSendOutcome>> + Send + 'a>>
    {
        Box::pin(async move {
            let outcome = MailSendOutcome {
                from: "test@sui-id.test".into(),
                to: mail.to.clone(),
                subject: mail.subject.clone(),
            };
            self.sent.lock().await.push(mail);
            Ok(outcome)
        })
    }
}

// `clock` import is anchored here so we keep the trait-and-impl
// module otherwise minimal; concrete uses (forgot-password TTL,
// audit timestamps) live in the per-feature modules.
#[allow(unused)]
fn clock_anchor(_: &SharedClock) {}
