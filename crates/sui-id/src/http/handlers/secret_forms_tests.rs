#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
//! RFC 115 D5 — a form's `Debug` cannot print a secret.
//!
//! Every form or query struct that carries a password, client secret, token,
//! TOTP or recovery code, an authorization code, a PKCE verifier or a setup
//! token holds it as `secrecy::SecretString`, whose `Debug` is redacted by the
//! type. Nothing in the tree formats these today (the design review looked and
//! found nothing), so this is a latent hazard closed at the type, not a bug
//! fixed. One test per struct deserialises it from a body carrying a distinctive
//! value for each secret field and asserts the formatted output contains none
//! of them, and does contain the redaction marker.

use axum::body::Body;
use axum::extract::{FromRequest, Query};
use axum::http::{Request, header};
use serde::de::DeserializeOwned;

/// Deserialise `T` from a urlencoded POST body, as the handler's `Form` does.
async fn from_form<T: DeserializeOwned>(body: &str) -> T {
    let req = Request::builder()
        .method("POST")
        .uri("/")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body.to_owned()))
        .expect("request");
    axum::Form::<T>::from_request(req, &())
        .await
        .unwrap_or_else(|_| panic!("the fixture body must deserialise: {body}"))
        .0
}

/// Deserialise `T` from a query string, as the handler's `Query` does.
fn from_query<T: DeserializeOwned>(query: &str) -> T {
    let uri: axum::http::Uri = format!("/?{query}").parse().expect("uri");
    Query::<T>::try_from_uri(&uri)
        .unwrap_or_else(|_| panic!("the fixture query must deserialise: {query}"))
        .0
}

fn assert_redacted(debug: &str, secrets: &[&str]) {
    for secret in secrets {
        assert!(
            !debug.contains(secret),
            "Debug printed the secret {secret:?}: {debug}"
        );
    }
    assert!(
        debug.contains("REDACTED"),
        "Debug shows no redaction marker, so the field is not a SecretString: {debug}"
    );
}

/// One test per struct: the marker values are unique, so a field that fell back
/// to a plain `String` prints its marker and fails here.
macro_rules! redaction_test {
    ($name:ident, form, $ty:ty, $body:expr, [$($secret:expr),+ $(,)?]) => {
        #[tokio::test]
        async fn $name() {
            let value: $ty = from_form($body).await;
            assert_redacted(&format!("{value:?}"), &[$($secret),+]);
            assert_redacted(&format!("{value:#?}"), &[$($secret),+]);
        }
    };
    ($name:ident, query, $ty:ty, $body:expr, [$($secret:expr),+ $(,)?]) => {
        #[test]
        fn $name() {
            let value: $ty = from_query($body);
            assert_redacted(&format!("{value:?}"), &[$($secret),+]);
            assert_redacted(&format!("{value:#?}"), &[$($secret),+]);
        }
    };
}

use super::admin::auth::{LoginForm, MfaChallengeForm};
use super::admin::clients::ClientEditQuery;
use super::admin::webauthn::WebauthnAuthCompleteForm;
use super::forgot_password::{ResetPasswordForm, ResetTokenQuery};
use super::me_security::forms::{
    MfaConfirmForm, MfaEnrollStartForm, PasskeyRegisterCompleteForm, PasskeyRegisterStartForm,
    PasswordChangeForm,
};
use super::oauth_token::{IntrospectForm, RevokeForm};
use super::oidc::TokenForm;
use super::settings::EmailSettingsForm;
use super::setup::{SetupAdminForm, SetupAdminQuery, WelcomeQuery};
use super::step_up::{StepUpForm, WebauthnFinishForm};

redaction_test!(
    login_form,
    form,
    LoginForm,
    "username=alice&password=PW-login-7f3a",
    ["PW-login-7f3a"]
);
redaction_test!(
    mfa_challenge_form,
    form,
    MfaChallengeForm,
    "_csrf=c&code=CODE-mfa-challenge-91",
    ["CODE-mfa-challenge-91"]
);
redaction_test!(
    client_edit_query,
    query,
    ClientEditQuery,
    "rotated_secret=SECRET-rotated-client-33",
    ["SECRET-rotated-client-33"]
);
redaction_test!(
    webauthn_auth_complete_form,
    form,
    WebauthnAuthCompleteForm,
    "_csrf=c&credential=CRED-webauthn-auth-55",
    ["CRED-webauthn-auth-55"]
);
redaction_test!(
    reset_token_query,
    query,
    ResetTokenQuery,
    "token=TOKEN-reset-query-12",
    ["TOKEN-reset-query-12"]
);
redaction_test!(
    reset_password_form,
    form,
    ResetPasswordForm,
    "_csrf=c&token=TOKEN-reset-form-21&password=PW-reset-new-44&confirm_password=PW-reset-confirm-45",
    [
        "TOKEN-reset-form-21",
        "PW-reset-new-44",
        "PW-reset-confirm-45"
    ]
);
redaction_test!(
    password_change_form,
    form,
    PasswordChangeForm,
    "_csrf=c&current_password=PW-current-61&new_password=PW-new-62&confirm_password=PW-confirm-63",
    ["PW-current-61", "PW-new-62", "PW-confirm-63"]
);
redaction_test!(
    mfa_confirm_form,
    form,
    MfaConfirmForm,
    "_csrf=c&code=CODE-mfa-confirm-71",
    ["CODE-mfa-confirm-71"]
);
redaction_test!(
    passkey_register_start_form,
    form,
    PasskeyRegisterStartForm,
    "_csrf=c&nickname=laptop&current_password=PW-passkey-start-81",
    ["PW-passkey-start-81"]
);
redaction_test!(
    mfa_enroll_start_form,
    form,
    MfaEnrollStartForm,
    "_csrf=c&current_password=PW-mfa-enroll-82",
    ["PW-mfa-enroll-82"]
);
redaction_test!(
    passkey_register_complete_form,
    form,
    PasskeyRegisterCompleteForm,
    "_csrf=c&credential=CRED-passkey-register-83",
    ["CRED-passkey-register-83"]
);
redaction_test!(
    introspect_form,
    form,
    IntrospectForm,
    "token=TOKEN-introspect-91&client_id=cid&client_secret=SECRET-introspect-92",
    ["TOKEN-introspect-91", "SECRET-introspect-92"]
);
redaction_test!(
    revoke_form,
    form,
    RevokeForm,
    "token=TOKEN-revoke-93&client_id=cid&client_secret=SECRET-revoke-94",
    ["TOKEN-revoke-93", "SECRET-revoke-94"]
);
redaction_test!(
    token_form,
    form,
    TokenForm,
    "grant_type=authorization_code&code=CODE-authz-01&redirect_uri=https%3A%2F%2Frp%2Fcb\
     &client_id=cid&client_secret=SECRET-token-02&code_verifier=VERIFIER-pkce-03\
     &refresh_token=TOKEN-refresh-04",
    [
        "CODE-authz-01",
        "SECRET-token-02",
        "VERIFIER-pkce-03",
        "TOKEN-refresh-04"
    ]
);
redaction_test!(
    email_settings_form,
    form,
    EmailSettingsForm,
    "_csrf=c&host=smtp.test&port=587&tls_mode=starttls&username=u&password=PW-smtp-05\
     &from_address=a%40b.test&base_url=https%3A%2F%2Fidp.test",
    ["PW-smtp-05"]
);
redaction_test!(
    welcome_query,
    query,
    WelcomeQuery,
    "lang=en&token=TOKEN-setup-welcome-06",
    ["TOKEN-setup-welcome-06"]
);
redaction_test!(
    setup_admin_query,
    query,
    SetupAdminQuery,
    "token=TOKEN-setup-admin-query-07",
    ["TOKEN-setup-admin-query-07"]
);
redaction_test!(
    setup_admin_form,
    form,
    SetupAdminForm,
    "setup_token=TOKEN-setup-form-08&username=root&password=PW-setup-09&confirm_password=PW-setup-10",
    ["TOKEN-setup-form-08", "PW-setup-09", "PW-setup-10"]
);
redaction_test!(
    step_up_form,
    form,
    StepUpForm,
    "_csrf=c&code=CODE-step-up-11",
    ["CODE-step-up-11"]
);
redaction_test!(
    webauthn_finish_form,
    form,
    WebauthnFinishForm,
    "_csrf=c&credential=CRED-step-up-webauthn-12",
    ["CRED-step-up-webauthn-12"]
);
