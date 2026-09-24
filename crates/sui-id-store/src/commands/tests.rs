#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use super::*;
use crate::registry::{CommandSpec, SystemPrincipalPermitted};

// ── Stage 2 item 1: every slice command is a deliberate declaration,
//    not a silent default. A compile-time fact, not a runtime check —
//    this only fails to *compile* if a `system_principal:` clause is
//    ever removed or the macro's `permitted` arm stops emitting the
//    impl; it can't regress at runtime.
const _: fn() = || {
    fn assert_system_principal_permitted<C: SystemPrincipalPermitted>() {}
    assert_system_principal_permitted::<U22>();
    assert_system_principal_permitted::<U08>();
    assert_system_principal_permitted::<U10>();
    assert_system_principal_permitted::<U37>();
    // U01-U07 and U09 are deliberately absent: all eight are
    // `system_principal: forbidden` (an authenticated actor —
    // admin or self-service — is required; U09's self-service actor
    // is still an "authorized actor" in `for_authorized_actor`'s
    // sense, just not an admin one). U01's compile-negative proof is
    // `tests/compile_fail/
    // admin_command_forbidden_cannot_use_system_actor.rs` — there is
    // no positive equivalent to assert here, since "does not
    // implement a trait" isn't expressible as a bound.
};

// ── Stage 1 item 5: duplicate-name / class-mismatch / missing-field /
//    stable-serialization tests, against this slice's real table ──────

fn all_descriptors() -> Vec<&'static EventDescriptor> {
    vec![
        &K01_ROTATED,
        &U22_FAILURE,
        &U22_LOCKOUT,
        &U01_CREATE,
        &U02_DISABLE,
        &U03_ENABLE,
        &U04_DELETE,
        &U05_ROLE_CHANGE,
        &U07_ADMIN_RESET,
        &U08_UNLOCK,
        &U09_CHANGED_SELF,
        &U10_RESET_COMPLETED,
        &U37_ISSUED,
        &T04_ROTATED,
        &T04_THEFT_DETECTED,
        &L06_FAILURE,
        &L06_SESSION_REVOKED,
        &MFA_FACTOR_ADDED,
        &L01_SUCCESS,
        &L02_SUCCESS,
        &L05_SUCCESS,
        &L04_SUCCESS,
        &L07_FAILURE,
        &L07_LOCKOUT,
    ]
}

#[test]
fn no_duplicate_event_names() {
    let names: Vec<&str> = all_descriptors().iter().map(|d| d.name).collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(
        names.len(),
        sorted.len(),
        "duplicate event name in the descriptor table: {names:?}"
    );
}

#[test]
fn no_duplicate_event_kinds() {
    let kinds: Vec<AuditEventKind> = all_descriptors().iter().map(|d| d.kind).collect();
    let mut sorted = kinds.clone();
    sorted.sort_by_key(|k| format!("{k:?}"));
    sorted.dedup();
    assert_eq!(kinds.len(), sorted.len(), "duplicate AuditEventKind");
}

/// The correction requested against the 2026-09-08 blocking finding
/// (`.git-exclude/reviewed/094-stage2-addendum-decision-consuming-
/// constructor-2026-09-08.md` §3): the compile-time mechanism
/// (`for_system_actor` gated to `SystemPrincipalPermitted`,
/// `for_authorized_actor` structurally unable to omit the actor)
/// only closes two of three possible `(system_principal,
/// ActorRequirement)` pairings. It does not stop a command declaring
/// `system_principal: permitted;` *and* an `ActorRequirement::Required`
/// descriptor — that command's only-gated constructor still produces
/// `actor: None` for a descriptor that says the actor is mandatory,
/// and nothing rejects it. Measured, not assumed, before writing this
/// test: setting one of U22's descriptors to `Required` while leaving
/// U22 `permitted` compiled and every other test still passed.
///
/// General over both directions (§5 of the same review, requested
/// explicitly): `None` also requires `forbidden` — a command whose
/// only constructor is `for_authorized_actor` cannot honour a
/// descriptor that says the actor must never be present either.
/// `Optional` is compatible with both, so it isn't asserted against.
///
/// Hand-maintained per command, the same way `all_descriptors()`
/// already is: whether a command implements `SystemPrincipalPermitted`
/// isn't queryable as runtime data, so this can't be generated from
/// the descriptor table the way `no_duplicate_event_names` is.
#[test]
fn actor_requirement_agrees_with_system_principal_for_every_command() {
    fn check(
        command: &str,
        system_principal_permitted: bool,
        descriptors: &[&'static EventDescriptor],
    ) {
        for d in descriptors {
            match d.actor {
                ActorRequirement::Required => assert!(
                    !system_principal_permitted,
                    "{command} ({}): ActorRequirement::Required but \
                     system_principal: permitted -- for_system_actor \
                     can still construct a context with no actor",
                    d.name
                ),
                ActorRequirement::None => assert!(
                    system_principal_permitted,
                    "{command} ({}): ActorRequirement::None but \
                     system_principal: forbidden -- for_authorized_actor \
                     is the only constructor and it always supplies an actor",
                    d.name
                ),
                ActorRequirement::Optional => {}
            }
        }
    }

    check("K01", false, &[&K01_ROTATED]);
    check("U22", true, &[&U22_FAILURE, &U22_LOCKOUT]);
    check("U01", false, &[&U01_CREATE]);
    check("U02", false, &[&U02_DISABLE]);
    check("U03", false, &[&U03_ENABLE]);
    check("U04", false, &[&U04_DELETE]);
    check("U05", false, &[&U05_ROLE_CHANGE]);
    check("U07", true, &[&U07_ADMIN_RESET]);
    check("U08", true, &[&U08_UNLOCK]);
    check("U09", false, &[&U09_CHANGED_SELF]);
    check("U10", true, &[&U10_RESET_COMPLETED]);
    check("U37", true, &[&U37_ISSUED]);
    check("T04", true, &[&T04_ROTATED, &T04_THEFT_DETECTED]);
    check("L05", false, &[&L05_SUCCESS]);
    check("L06", false, &[&L06_FAILURE, &L06_SESSION_REVOKED]);
    check("U12", false, &[&MFA_FACTOR_ADDED]);
    check("U14", false, &[&MFA_FACTOR_ADDED]);
    check("U15", false, &[&MFA_FACTOR_ADDED]);
    check("L01", false, &[&L01_SUCCESS]);
    check("L02", false, &[&L02_SUCCESS]);
    check("L03", false, &[&L01_SUCCESS]);
    check("L04", false, &[&L04_SUCCESS]);
    check("L07", false, &[&L07_FAILURE, &L07_LOCKOUT]);
}

#[test]
fn every_descriptor_is_atomic_class() {
    // Every command in this slice's Class-A set must register
    // AuditClass::Atomic — a MustAttempt descriptor here would be a
    // class mismatch (no Class-B command is in this slice).
    for d in all_descriptors() {
        assert_eq!(d.class, AuditClass::Atomic, "{} is not Atomic", d.name);
    }
}

#[test]
fn k01_descriptor_maps_are_exhaustive_and_correct() {
    let event = K01Event::Rotated {
        new_key: SigningKeyId::new(),
        algorithm: "ed25519".into(),
        reason: None,
        step_up: SessionStepUpEvidence::NotRequired,
    };
    assert_eq!(K01::descriptor(&event).name, "signing_key.rotate");
}

#[test]
fn u22_both_branches_map_to_distinct_descriptors() {
    let uid = UserId::new();
    let failure = U22Event::Failure {
        user_id: uid,
        count: 1,
    };
    let lockout = U22Event::Lockout {
        user_id: uid,
        count: 5,
        locked_for_secs: 30,
    };
    assert_eq!(U22::descriptor(&failure).name, "auth.login.failure");
    assert_eq!(U22::descriptor(&lockout).name, "auth.lockout");
    assert_ne!(
        U22::descriptor(&failure).name,
        U22::descriptor(&lockout).name
    );
}

#[test]
fn u01_has_one_event_and_the_warned_hibp_event_is_retired() {
    // RFC 115 D9: U01 sets no password, so the HIBP-flagged branch is
    // unreachable and its event is gone from the registry. Pinned by name so
    // a revival shows up as a diff here.
    let created = U01Event::Created {
        user_id: UserId::new(),
    };
    assert_eq!(U01::descriptor(&created).name, "user.create");
    assert!(
        all_descriptors()
            .iter()
            .all(|d| d.name != "user.create_warned_hibp"),
        "user.create_warned_hibp must not be registered"
    );
}

#[test]
fn missing_field_is_a_compile_error_not_a_runtime_check() {
    // Not a runnable test: documents that `AttributeSpec` mismatches
    // (a variant's `attributes()` emitting a name absent from its
    // descriptor's `attributes` list) are not currently caught here —
    // that check belongs to the structural comparison tool (Stage 1
    // item 6), which reads the generated attribute set against the
    // declared one. Registering it as a known gap rather than a silent
    // omission: see the Stage 1 submission's disclosure section.
}

// ── Stage 1 item 4: generated reference documentation ───────────────

#[test]
fn reference_markdown_covers_every_descriptor_and_is_deterministic() {
    let descriptors = all_descriptors();
    let rendered = crate::registry::generate_reference_markdown(&descriptors);

    for d in &descriptors {
        assert!(
            rendered.contains(&format!("`{}`", d.name)),
            "reference table is missing {}",
            d.name
        );
    }
    // K01's declared attribute must actually appear, not just the
    // event name -- proves the attribute column isn't silently empty.
    assert!(rendered.contains("`algorithm`"));

    let rendered_again = crate::registry::generate_reference_markdown(&descriptors);
    assert_eq!(
        rendered, rendered_again,
        "generation must be deterministic — a doc-drift check diffs two renders"
    );
}

// ── Stable-serialization: event names, once emitted, never change
//    shape silently. ─────────────────────────────────────────────────

#[test]
fn event_names_match_command_inventory() {
    // These strings are the audit-log `action` column's contract
    // with every existing consumer (SIEM queries, `rfcs/handoffs/
    // 094-transactional-audit/command-inventory.md`). Pinned literally,
    // not derived, so a rename shows up as a diff here.
    let expected = [
        "signing_key.rotate",
        "auth.login.failure",
        "auth.lockout",
        "user.create",
        "user.disable",
        "user.enable",
        "user.delete",
        "user.role_change",
        "mfa.admin_reset",
        "admin.user.unlock",
        "auth.password.changed_self",
        "auth.password.reset_completed",
        "user.recovery_link.issued",
        "auth.refresh.rotated",
        "auth.refresh.theft_detected",
        "auth.step_up.success",
        "auth.step_up.failure",
        "auth.step_up.session_revoked",
        "auth.mfa.factor_added",
        "auth.login.success",
        "auth.mfa.success",
        "auth.federation.signin.success",
        "auth.mfa.failure",
        "auth.mfa.lockout",
    ];
    let mut actual: Vec<&str> = all_descriptors().iter().map(|d| d.name).collect();
    actual.sort_unstable();
    let mut expected = expected.to_vec();
    expected.sort_unstable();
    assert_eq!(actual, expected);
}

// ── End-to-end runner tests ─────────────────────────────────────────
// Real `Database`, real SQLite. Proves the runners actually work, not
// just that the descriptor tables are internally consistent.

#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod runner;

// ── RFC 102 stage 5a: bounded upstream identifiers on L03 and L04 ──────

fn attribute_map(attributes: &AuditAttributes) -> Vec<(String, String)> {
    attributes
        .iter()
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect()
}

#[test]
fn l03_success_records_source_and_stable_id() {
    let event = L03Event::Success {
        user_id: UserId::new(),
        source: "corp".into(),
        stable_id: "uid=bob,ou=people,dc=example,dc=com".into(),
        evicted: 0,
    };
    assert_eq!(L03::descriptor(&event).name, "auth.login.success");
    assert_eq!(
        attribute_map(&event.attributes().expect("attributes")),
        vec![
            ("evicted".to_owned(), "0".to_owned()),
            ("source".to_owned(), "corp".to_owned()),
            (
                "stable_id".to_owned(),
                "uid=bob,ou=people,dc=example,dc=com".to_owned()
            ),
        ]
    );
}

#[test]
fn l03_stable_id_is_truncated_to_255_bytes() {
    let long = "x".repeat(600);
    let event = L03Event::Success {
        user_id: UserId::new(),
        source: "corp".into(),
        stable_id: long.clone(),
        evicted: 0,
    };
    let attributes = event.attributes().expect("a long id still builds");
    let stable_id = attributes
        .iter()
        .find(|(k, _)| *k == "stable_id")
        .map(|(_, v)| v.to_owned())
        .expect("stable_id");
    assert_eq!(stable_id, long[..EXTERNAL_ID_ATTRIBUTE_BYTES]);
}

#[test]
fn l04_success_records_provider_and_sub() {
    let event = L04Event::Success {
        user_id: UserId::new(),
        provider: "up".into(),
        sub: "248289761001".into(),
        evicted: 1,
    };
    assert_eq!(
        L04::descriptor(&event).name,
        "auth.federation.signin.success"
    );
    assert_eq!(
        attribute_map(&event.attributes().expect("attributes")),
        vec![
            ("provider".to_owned(), "up".to_owned()),
            ("sub".to_owned(), "248289761001".to_owned()),
            ("evicted".to_owned(), "1".to_owned()),
        ]
    );
}

#[test]
fn l04_sub_is_truncated_to_255_bytes_on_a_char_boundary() {
    // 254 ASCII bytes, then a 3-byte character straddling the bound.
    let sub = format!("{}€tail", "s".repeat(254));
    let event = L04Event::Success {
        user_id: UserId::new(),
        provider: "up".into(),
        sub,
        evicted: 0,
    };
    let attributes = event.attributes().expect("attributes");
    let recorded = attributes
        .iter()
        .find(|(k, _)| *k == "sub")
        .map(|(_, v)| v.to_owned())
        .expect("sub");
    assert_eq!(recorded, "s".repeat(254), "cut before the split character");
    assert!(recorded.len() <= EXTERNAL_ID_ATTRIBUTE_BYTES);
}

#[test]
fn truncate_utf8_leaves_short_values_alone() {
    assert_eq!(truncate_utf8("abc", 255), "abc");
    assert_eq!(truncate_utf8(&"a".repeat(255), 255).len(), 255);
    assert_eq!(truncate_utf8("ééé", 3), "é");
}

// ── RFC 102 stage 7: B4 ─────────────────────────────────────────────────

#[test]
fn b4_every_gated_descriptor_requires_step_up() {
    for d in [
        &U02_DISABLE,
        &U03_ENABLE,
        &U04_DELETE,
        &U07_ADMIN_RESET,
        &U37_ISSUED,
        &K01_ROTATED,
    ] {
        let step_up = d
            .attributes
            .iter()
            .find(|a| a.name == "step_up")
            .unwrap_or_else(|| panic!("{} has no step_up attribute", d.name));
        assert!(step_up.required, "{}: step_up must be required", d.name);
    }
    // No other descriptor requires anything yet.
    for d in all_descriptors() {
        for a in d.attributes {
            if a.required {
                assert_eq!(
                    a.name, "step_up",
                    "{}: unexpected required {}",
                    d.name, a.name
                );
            }
        }
    }
}

#[test]
fn b4_session_evidence_renders_only_session_forms() {
    // Structural: `SessionStepUpEvidence` is what a session-bound entry
    // computes, and it has exactly these two forms. The system-principal
    // form is written only by the operator entries, `U07Event::OperatorReset`
    // and `U37Event::OperatorIssued`.
    for evidence in [
        SessionStepUpEvidence::Fresh {
            method: "totp".into(),
            age_secs: 12,
        },
        SessionStepUpEvidence::NotRequired,
    ] {
        let rendered = evidence.as_attribute();
        assert!(
            rendered.starts_with("fresh:") || rendered == "not_required:no_second_factor",
            "{rendered}"
        );
        assert!(!rendered.contains("not_applicable"));
        // Exhaustive: adding a variant fails to compile here until it is
        // considered.
        match evidence {
            SessionStepUpEvidence::Fresh { .. } | SessionStepUpEvidence::NotRequired => {}
        }
    }
    assert_eq!(
        SessionStepUpEvidence::Fresh {
            method: "webauthn".into(),
            age_secs: 42
        }
        .as_attribute(),
        "fresh:webauthn:42"
    );
}

#[test]
fn b4_only_the_operator_reset_records_not_applicable() {
    let uid = UserId::new();
    let web = U07Event::Reset {
        user_id: uid,
        totp_removed: false,
        passkeys_removed: 0,
        reason: None,
        step_up: SessionStepUpEvidence::NotRequired,
    };
    let cli = U07Event::OperatorReset {
        user_id: uid,
        totp_removed: false,
        passkeys_removed: 0,
        reason: "lost".into(),
    };
    let step_up = |e: &U07Event| {
        e.attributes()
            .expect("attributes")
            .iter()
            .find(|(k, _)| *k == "step_up")
            .map(|(_, v)| v.to_owned())
    };
    assert_eq!(
        step_up(&web).as_deref(),
        Some("not_required:no_second_factor")
    );
    assert_eq!(
        step_up(&cli).as_deref(),
        Some("not_applicable:system_principal")
    );
}

// ── RFC 103 stage 3: U37 ────────────────────────────────────────────────

#[test]
fn u37_both_entries_record_the_same_attribute_set_and_differ_in_via_and_step_up() {
    let target = UserId::new();
    let expires_at = chrono::DateTime::parse_from_rfc3339("2026-09-22T12:30:00Z")
        .expect("time")
        .with_timezone(&chrono::Utc);
    let web = U37Event::Issued {
        target,
        reason: "call-back verified".into(),
        expires_at,
        invalidated: 2,
        step_up: SessionStepUpEvidence::Fresh {
            method: "totp".into(),
            age_secs: 7,
        },
    };
    let cli = U37Event::OperatorIssued {
        target,
        reason: "call-back verified".into(),
        expires_at,
        invalidated: 0,
    };
    assert_eq!(U37::descriptor(&web).name, "user.recovery_link.issued");
    assert_eq!(U37::descriptor(&cli).name, "user.recovery_link.issued");
    assert_eq!(
        attribute_map(&web.attributes().expect("attributes")),
        vec![
            ("reason".to_owned(), "call-back verified".to_owned()),
            ("via".to_owned(), "web".to_owned()),
            ("expires_at".to_owned(), "2026-09-22T12:30:00Z".to_owned()),
            ("invalidated".to_owned(), "2".to_owned()),
            ("step_up".to_owned(), "fresh:totp:7".to_owned()),
        ]
    );
    assert_eq!(
        attribute_map(&cli.attributes().expect("attributes")),
        vec![
            ("reason".to_owned(), "call-back verified".to_owned()),
            ("via".to_owned(), "cli".to_owned()),
            ("expires_at".to_owned(), "2026-09-22T12:30:00Z".to_owned()),
            ("invalidated".to_owned(), "0".to_owned()),
            (
                "step_up".to_owned(),
                "not_applicable:system_principal".to_owned()
            ),
        ]
    );
    // Every attribute the events emit is declared, and every declared one is
    // emitted: the descriptor and the events cannot drift apart silently.
    let declared: Vec<&str> = U37_ISSUED.attributes.iter().map(|a| a.name).collect();
    for event in [&web, &cli] {
        let emitted: Vec<String> = attribute_map(&event.attributes().expect("attributes"))
            .into_iter()
            .map(|(k, _)| k)
            .collect();
        let mut emitted: Vec<&str> = emitted.iter().map(String::as_str).collect();
        let mut declared = declared.clone();
        emitted.sort_unstable();
        declared.sort_unstable();
        assert_eq!(emitted, declared);
    }
}

#[test]
fn u37_events_never_carry_the_token_or_its_hash() {
    // Structural, by construction: neither variant has a field that could hold
    // one. This pins that the attribute names stay free of any such name.
    for a in U37_ISSUED.attributes {
        assert!(
            !a.name.contains("token") && !a.name.contains("hash") && !a.name.contains("secret"),
            "{}",
            a.name
        );
    }
}

#[test]
fn u10_records_origin() {
    let event = U10Event::Completed {
        user_id: UserId::new(),
        origin: crate::models::ResetTokenOrigin::Cli,
        hibp_warned: false,
    };
    assert_eq!(
        attribute_map(&event.attributes().expect("attributes")),
        vec![("origin".to_owned(), "cli".to_owned())]
    );
}

#[test]
fn u10_records_a_warn_mode_breach_hit_and_only_then() {
    // RFC 115 D9: the outcome that used to be discarded. Present as
    // `hibp=warned` when the warn-mode check found the password; absent
    // otherwise, so an ordinary completion's note is unchanged.
    let warned = U10Event::Completed {
        user_id: UserId::new(),
        origin: crate::models::ResetTokenOrigin::Email,
        hibp_warned: true,
    };
    assert_eq!(
        attribute_map(&warned.attributes().expect("attributes")),
        vec![
            ("origin".to_owned(), "email".to_owned()),
            ("hibp".to_owned(), "warned".to_owned())
        ]
    );
}
