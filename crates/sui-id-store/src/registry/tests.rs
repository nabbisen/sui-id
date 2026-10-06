use super::*;

// ── Registry unit tests (Stage 1 item 5) ────────────────────────────
// Duplicate-name, class-mismatch, missing-field, and stable-serialization
// are checked against the slice's real descriptor table in
// `commands.rs`'s own test module (it owns the table); this module
// tests the registry machinery itself, independent of any one slice.

#[test]
fn audit_attributes_builder_rejects_duplicate_names() {
    let err = AuditAttributes::builder()
        .attribute("a", "1")
        .attribute("a", "2")
        .build()
        .unwrap_err();
    assert_eq!(err, AuditBuildError::DuplicateAttribute("a"));
}

#[test]
fn audit_attributes_builder_rejects_too_many() {
    let mut builder = AuditAttributes::builder();
    for i in 0..MAX_ATTRIBUTES {
        builder = builder.attribute(Box::leak(i.to_string().into_boxed_str()), "v");
    }
    builder = builder.attribute("one_too_many", "v");
    assert_eq!(
        builder.build().unwrap_err(),
        AuditBuildError::TooManyAttributes
    );
}

#[test]
fn audit_attributes_builder_rejects_oversize_value() {
    let long = "x".repeat(MAX_ATTRIBUTE_VALUE_BYTES + 1);
    let err = AuditAttributes::builder()
        .attribute("a", long)
        .build()
        .unwrap_err();
    assert_eq!(err, AuditBuildError::AttributeTooLong("a"));
}

#[test]
fn audit_attributes_builder_accepts_within_bounds() {
    let attrs = AuditAttributes::builder()
        .attribute("a", "1")
        .attribute("b", "2")
        .build()
        .unwrap();
    let collected: Vec<_> = attrs.iter().collect();
    assert_eq!(collected, vec![("a", "1"), ("b", "2")]);
}

#[test]
fn audit_result_as_str_is_stable() {
    // SIEM queries and audit-log alerts pivot on these strings.
    assert_eq!(AuditResult::Ok.as_str(), "ok");
    assert_eq!(AuditResult::Failure.as_str(), "failure");
}

// ── Stage 2 item 1: AuthorizedCommandContext gating ─────────────────
// The negative property (a `forbidden` command cannot use
// `for_system_actor`) is a compile-time fact, proved by
// `tests/compile_fail/system_principal_forbidden_cannot_use_system_
// actor.rs`, not by anything runnable here. This test only checks that
// the proof-only command's own macro-generated wiring is correct —
// that `declare_write_command!`'s `system_principal: forbidden;` arm
// didn't silently produce a mismatched or unreachable descriptor.

#[test]
fn proof_only_forbidden_command_descriptor_matches() {
    let descriptor = ProofOnlyForbiddenSystemPrincipalCommand::descriptor(
        &ProofOnlyForbiddenSystemPrincipalEvent::Occurred,
    );
    assert_eq!(
        descriptor.kind,
        AuditEventKind::ProofOnlySystemPrincipalForbidden
    );
    assert_eq!(descriptor.name, "proof_only.system_principal_forbidden");
}
