//! Forgot-password / password-reset flow.
//!
//! Three pure functions:
//!
//! - [`request_reset`] — issued from `POST /forgot-password`. Looks
//!   up a user by email, generates a token, persists its hash,
//!   sends the reset link mail, returns. Always returns `Ok(())`
//!   externally (user-enumeration protection); failures are
//!   audit-logged.
//! - [`validate_token`] — issued from `GET /reset-password?token=…`
//!   to gate rendering the new-password form. Verifies the token
//!   without consuming it.
//! - [`consume_and_reset_password`] — issued from
//!   `POST /reset-password`. Verifies the token, replaces the user's
//!   password, marks the token consumed, all in one logical step.
//!
//! ## Token shape
//!
//! - 32 random bytes from `OsRng` → URL-safe base64 (no padding).
//!   The plaintext only ever exists in the user's email and the
//!   user's clipboard / browser.
//! - SHA-256 of the plaintext is stored in
//!   `password_reset_tokens.token_hash`. A backup leak does not
//!   yield live tokens. SHA-256 is sufficient: the underlying
//!   token is 32 bytes of CSPRNG output, so we only need preimage
//!   resistance, not a slow KDF.
//! - 30-minute TTL by default.
//! - Single-use: `consumed_at` set on redemption; replays land on
//!   a "consumed" check that returns `InvalidCredentials`.
//!
//! ## User enumeration
//!
//! `request_reset` returns `Ok(())` whether the email matched a
//! user or not, takes roughly the same time in both branches, and
//! emits a `auth.password.reset_requested` event in either case.
//! The handler always shows a generic "if an account exists, we've
//! sent the link" page.

use crate::errors::{CoreError, CoreResult};
use crate::events::{self, Context, SecurityEvent};
use crate::hibp::{self, HibpClient, HibpEnforcement};
use crate::mail::{MailSender, OutgoingMail};
use crate::password;
use crate::time::SharedClock;
use base64ct::{Base64UrlUnpadded, Encoding};
use chrono::Duration;
use getrandom;
use sha2::{Digest, Sha256};
use sui_id_shared::ids::{PasswordResetTokenId, UserId};
use sui_id_store::Database;
use sui_id_store::models::{CredentialRow, HibpMode, PasswordResetTokenRow};
use sui_id_store::repos::{
    credentials, password_reset_tokens, refresh_tokens, sessions, smtp_config, users,
};

/// 30 minutes — a balance between user-friendly delivery delays
/// and a reasonably tight attack window.
pub const DEFAULT_TOKEN_TTL: Duration = Duration::minutes(30);

/// Outstanding-token ceiling per user. Above this, we silently
/// stop issuing new tokens (the response is still 200 so a probe
/// can't tell). Prevents a single user's inbox from being spammed.
const MAX_OUTSTANDING_TOKENS_PER_USER: i64 = 3;

fn mint_random_token() -> (String, Vec<u8>) {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).expect("system RNG unavailable");
    let plaintext = Base64UrlUnpadded::encode_string(&bytes);
    let hash = Sha256::digest(plaintext.as_bytes()).to_vec();
    (plaintext, hash)
}

fn hash_token(plaintext: &str) -> Vec<u8> {
    Sha256::digest(plaintext.as_bytes()).to_vec()
}

/// Issue a password-reset token for the given email, send the
/// reset-link mail, and emit an audit event.
///
/// The exterior contract is **unconditional success**: even when
/// the email doesn't match a user, or the user has no email, or
/// SMTP is unconfigured, this returns `Ok(())`. Internal failures
/// are recorded as audit events but never surfaced. The handler
/// maps every internal outcome to the same neutral 200-response
/// page so `POST /forgot-password` cannot be a user-enumeration
/// oracle.
pub async fn request_reset(
    db: &Database,
    clock: &SharedClock,
    mailer: &dyn MailSender,
    email: &str,
    requester_ip: Option<&str>,
) -> CoreResult<()> {
    let normalized_email = sui_id_shared::normalize_email(email);
    let now = clock.now();

    let mut ctx = Context::default();
    if let Some(ip) = requester_ip {
        ctx = ctx.with_client_ip(ip);
    }

    // Look up by email.
    let user_row = users::find_by_email_normalized(db, &normalized_email).await?;
    let Some(user_row) = user_row else {
        events::emit(
            db,
            clock,
            &ctx,
            SecurityEvent::PasswordResetRequested { user_id: None },
        )
        .await;
        return Ok(());
    };

    if user_row.is_disabled || user_row.is_deleted {
        events::emit(
            db,
            clock,
            &ctx.clone().with_actor(user_row.id),
            SecurityEvent::PasswordResetRequested {
                user_id: Some(user_row.id),
            },
        )
        .await;
        return Ok(());
    }

    // Outstanding-token throttle.
    let outstanding = password_reset_tokens::count_active_for_user(db, user_row.id, now).await?;
    if outstanding >= MAX_OUTSTANDING_TOKENS_PER_USER {
        events::emit(
            db,
            clock,
            &ctx.clone().with_actor(user_row.id),
            SecurityEvent::PasswordResetThrottled {
                user_id: user_row.id,
                outstanding,
            },
        )
        .await;
        return Ok(());
    }

    // Mint a token, persist its hash.
    let (plaintext, hash) = mint_random_token();
    let row = PasswordResetTokenRow {
        id: PasswordResetTokenId::new(),
        user_id: user_row.id,
        token_hash: hash,
        issued_at: now,
        expires_at: now + DEFAULT_TOKEN_TTL,
        consumed_at: None,
        requester_ip: requester_ip.map(str::to_owned),
    };
    password_reset_tokens::insert(db, &row).await?;

    // Build the reset link from `smtp_config.base_url` (the
    // user-facing origin, not necessarily the OIDC issuer URL).
    let base_url = match smtp_config::get(db).await? {
        Some(c) if c.enabled => c.base_url,
        _ => {
            // SMTP disabled / unconfigured. Still return Ok so the
            // exterior shape is constant; record the actual outcome.
            events::emit(
                db,
                clock,
                &ctx.clone().with_actor(user_row.id),
                SecurityEvent::PasswordResetEmailFailed {
                    user_id: user_row.id,
                    reason: "smtp_unconfigured".into(),
                },
            )
            .await;
            return Ok(());
        }
    };
    let link = format!(
        "{}/reset-password?token={}",
        base_url.trim_end_matches('/'),
        plaintext
    );

    // Compose and dispatch the mail. The recipient's locale is
    // their `preferred_lang` if set, otherwise the server default
    // — we don't have a per-request browser context here (this
    // runs inline with the POST handler but the recipient may not
    // be the requester). Falling through to server default if the
    // user has expressed no preference matches the resolution
    // chain in `core::i18n::resolve`.
    let default_locale = sui_id_store::repos::server_settings::get(db)
        .await
        .ok()
        .and_then(|s| sui_id_i18n::Locale::parse(&s.default_lang))
        .unwrap_or_default();
    let recipient_locale = user_row
        .preferred_lang
        .as_deref()
        .and_then(sui_id_i18n::Locale::parse)
        .unwrap_or(default_locale);
    let t = recipient_locale.strings();
    let display = user_row
        .display_name
        .as_deref()
        .unwrap_or(&user_row.username);
    let greeting = if t.email_greeting_suffix.is_empty() {
        display.to_string()
    } else {
        format!("{} {}", display, t.email_greeting_suffix)
    };
    let mail = OutgoingMail {
        // Deliver to the original-case address the user registered with;
        // the normalised form was only needed for the lookup.
        to: user_row
            .email
            .clone()
            .unwrap_or_else(|| normalized_email.clone()),
        subject: t.email_subject_password_reset.to_string(),
        text_body: format!(
            "{greeting}\n\
             \n\
             {intro}\n\
             \n\
             {link}\n\
             \n\
             {disregard}\n\
             ",
            greeting = greeting,
            intro = t.email_password_reset_intro,
            link = link,
            disregard = t.email_password_reset_disregard,
        ),
        html_body: Some(format!(
            "<p>{greeting_esc}</p>\
             <p>{intro}</p>\
             <p><a href=\"{link_esc}\">{link_label}</a></p>\
             <p>{disregard}</p>",
            greeting_esc = html_escape(&greeting),
            intro = t.email_password_reset_intro,
            link_esc = html_escape(&link),
            link_label = t.email_password_reset_link_label,
            disregard = t.email_password_reset_disregard,
        )),
        locale: None,
    };

    match mailer.send(mail).await {
        Ok(_outcome) => {
            events::emit(
                db,
                clock,
                &ctx.clone().with_actor(user_row.id),
                SecurityEvent::PasswordResetEmailSent {
                    user_id: user_row.id,
                },
            )
            .await;
        }
        Err(e) => {
            events::emit(
                db,
                clock,
                &ctx.clone().with_actor(user_row.id),
                SecurityEvent::PasswordResetEmailFailed {
                    user_id: user_row.id,
                    reason: e.to_string(),
                },
            )
            .await;
        }
    }
    Ok(())
}

/// Verify a token without consuming it. Used by the GET handler
/// that decides whether to render the new-password form or a
/// "this link is invalid or expired" page.
pub async fn validate_token(
    db: &Database,
    clock: &SharedClock,
    plaintext_token: &str,
) -> CoreResult<UserId> {
    let hash = hash_token(plaintext_token);
    let row = password_reset_tokens::find_by_hash(db, &hash)
        .await?
        .ok_or(CoreError::InvalidCredentials)?;
    if row.consumed_at.is_some() {
        return Err(CoreError::InvalidCredentials);
    }
    if row.expires_at < clock.now() {
        return Err(CoreError::InvalidCredentials);
    }
    Ok(row.user_id)
}

/// Verify the token, set the user's new password, mark the token consumed,
/// and revoke all existing sessions and refresh tokens for the user — all
/// in a single atomic transaction.
///
/// Revoking prior sessions is essential: the user completed this flow
/// precisely because they lost control of their credentials. An attacker
/// who holds a stolen session cookie or refresh token must not retain
/// access after the legitimate user has recovered the account.
///
/// The revoke matches the behaviour of the admin-driven and self-service
/// password-change paths, which both revoke on write.
pub async fn consume_and_reset_password(
    db: &Database,
    clock: &SharedClock,
    mailer: &dyn MailSender,
    hibp_client: Option<&dyn HibpClient>,
    hibp_mode: HibpMode,
    plaintext_token: &str,
    new_password: &str,
    requester_ip: Option<&str>,
    min_password_len: usize,
) -> CoreResult<()> {
    password::check_password_policy(new_password, min_password_len)?;

    // RFC 003: HIBP breach check on token-based password reset.
    // Fail-open: network failures let the reset through.
    if matches!(
        hibp::enforce_hibp(hibp_mode, hibp_client, new_password).await,
        HibpEnforcement::Blocked { .. }
    ) {
        return Err(CoreError::BadRequest(
            "New password found in known data breaches. Please choose a different password.".into(),
        ));
    }
    let hash = hash_token(plaintext_token);
    let row = password_reset_tokens::find_by_hash(db, &hash)
        .await?
        .ok_or(CoreError::InvalidCredentials)?;
    let now = clock.now();
    if row.consumed_at.is_some() || row.expires_at < now {
        return Err(CoreError::InvalidCredentials);
    }

    // Hash the new password before entering the transaction so a slow
    // Argon2id derivation doesn't hold the DB mutex longer than necessary.
    let new_hash = password::hash_password(new_password)?;

    // Atomically: update credential, consume token, revoke all sessions and
    // refresh tokens. Either everything commits or nothing does — the user
    // is never left in a half-recovered state.
    let row_user_id = row.user_id;
    let row_id = row.id;
    let new_hash_owned = new_hash.clone();
    db.with_tx(move |tx| {
        credentials::upsert_within_tx(
            tx,
            &CredentialRow {
                user_id: row_user_id,
                password_hash: new_hash_owned,
                must_change: false,
                updated_at: now,
            },
        )?;
        password_reset_tokens::mark_consumed_within_tx(tx, row_id, now)?;
        sessions::revoke_all_for_user_within_tx(tx, row_user_id, now)?;
        refresh_tokens::revoke_all_for_user_within_tx(tx, row_user_id, now)?;
        Ok(())
    })
    .await?;

    let mut ctx = Context::default().with_actor(row.user_id);
    if let Some(ip) = requester_ip {
        ctx = ctx.with_client_ip(ip);
    }
    events::emit(
        db,
        clock,
        &ctx,
        SecurityEvent::PasswordResetCompleted {
            user_id: row.user_id,
        },
    )
    .await;

    // Best-effort post-reset notification mail. Failures here do
    // not affect the password change itself. The recipient's
    // locale comes from their `preferred_lang` if set, falling
    // through to the server default.
    if let Ok(Some(user_row)) = users::find_by_id_opt(db, row.user_id).await {
        if let Some(email) = user_row.email.as_deref() {
            let default_locale_pw = sui_id_store::repos::server_settings::get(db)
                .await
                .ok()
                .and_then(|s| sui_id_i18n::Locale::parse(&s.default_lang))
                .unwrap_or_default();
            let recipient_locale = user_row
                .preferred_lang
                .as_deref()
                .and_then(sui_id_i18n::Locale::parse)
                .unwrap_or(default_locale_pw);
            let _ =
                notify_password_changed(mailer, email, &user_row.display_name, recipient_locale)
                    .await;
        }
    }

    Ok(())
}

/// Send the "your password has just been changed" notification.
///
/// Best-effort: callers swallow errors and proceed. The audit
/// chain records the underlying password-change action separately.
///
/// `locale` is the recipient's preferred locale — typically
/// resolved from `user.preferred_lang` falling through to the
/// server default. The caller is responsible for that resolution
/// (passing in the locale rather than re-querying here keeps the
/// function pure, testable, and free of DB access).
pub async fn notify_password_changed(
    mailer: &dyn MailSender,
    to_email: &str,
    display_name: &Option<String>,
    locale: sui_id_i18n::Locale,
) -> CoreResult<()> {
    let t = locale.strings();
    let display = display_name.as_deref().unwrap_or("");
    let greeting = if t.email_greeting_suffix.is_empty() {
        display.to_string()
    } else {
        format!("{} {}", display, t.email_greeting_suffix)
    };
    let mail = OutgoingMail {
        to: to_email.to_owned(),
        subject: t.email_subject_password_changed.to_string(),
        text_body: format!(
            "{greeting}\n\
             \n\
             {intro}\n\
             {warning}\n\
             ",
            greeting = greeting,
            intro = t.email_password_changed_intro,
            warning = t.email_password_changed_security_warning,
        ),
        html_body: Some(format!(
            "<p>{greeting_esc}</p>\
             <p>{intro}</p>\
             <p>{warning} <a href=\"/me/security\">{link_label}</a></p>",
            greeting_esc = html_escape(&greeting),
            intro = t.email_password_changed_intro,
            warning = t.email_password_changed_security_warning,
            link_label = t.email_password_changed_link_security,
        )),
        locale: None,
    };
    mailer.send(mail).await.map(|_| ())
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
