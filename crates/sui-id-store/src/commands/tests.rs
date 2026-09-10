#![allow(clippy::expect_used, clippy::unwrap_used)]

use super::*;
use crate::registry::{CommandSpec, SystemPrincipalPermitted};

// ── Stage 2 item 1: every slice command is a deliberate declaration,
//    not a silent default. A compile-time fact, not a runtime check —
//    this only fails to *compile* if a `system_principal:` clause is
//    ever removed or the macro's `permitted` arm stops emitting the
//    impl; it can't regress at runtime.
const _: fn() = || {
    fn assert_system_principal_permitted<C: SystemPrincipalPermitted>() {}
    assert_system_principal_permitted::<K01>();
    assert_system_principal_permitted::<U22>();
    assert_system_principal_permitted::<U08>();
    assert_system_principal_permitted::<U10>();
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
        &U01_CREATE_WARNED_HIBP,
        &U02_DISABLE,
        &U03_ENABLE,
        &U04_DELETE,
        &U05_ROLE_CHANGE,
        &U06_RESET_PASSWORD,
        &U07_ADMIN_RESET,
        &U08_UNLOCK,
        &U09_CHANGED_SELF,
        &U10_RESET_COMPLETED,
        &T04_ROTATED,
        &T04_THEFT_DETECTED,
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

    check("K01", true, &[&K01_ROTATED]);
    check("U22", true, &[&U22_FAILURE, &U22_LOCKOUT]);
    check("U01", false, &[&U01_CREATE, &U01_CREATE_WARNED_HIBP]);
    check("U02", false, &[&U02_DISABLE]);
    check("U03", false, &[&U03_ENABLE]);
    check("U04", false, &[&U04_DELETE]);
    check("U05", false, &[&U05_ROLE_CHANGE]);
    check("U06", false, &[&U06_RESET_PASSWORD]);
    check("U07", false, &[&U07_ADMIN_RESET]);
    check("U08", true, &[&U08_UNLOCK]);
    check("U09", false, &[&U09_CHANGED_SELF]);
    check("U10", true, &[&U10_RESET_COMPLETED]);
    check("T04", true, &[&T04_ROTATED, &T04_THEFT_DETECTED]);
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
fn u01_both_branches_map_to_distinct_descriptors() {
    let uid = UserId::new();
    let created = U01Event::Created { user_id: uid };
    let warned = U01Event::CreatedWarnedHibp { user_id: uid };
    assert_eq!(U01::descriptor(&created).name, "user.create");
    assert_eq!(U01::descriptor(&warned).name, "user.create_warned_hibp");
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
        "user.create_warned_hibp",
        "user.disable",
        "user.enable",
        "user.delete",
        "user.role_change",
        "user.reset_password",
        "mfa.admin_reset",
        "admin.user.unlock",
        "auth.password.changed_self",
        "auth.password.reset_completed",
        "auth.refresh.rotated",
        "auth.refresh.theft_detected",
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
