#![allow(clippy::expect_used, clippy::unwrap_used)]
use super::*;
use crate::crypto::MasterKey;
use sui_id_shared::ids::UserId;

fn fresh_db() -> Database {
    let key = MasterKey::generate();
    Database::open_in_memory(key).expect("db")
}

fn sample_row(action: &str) -> AuditLogRow {
    AuditLogRow {
        at: Utc::now(),
        actor: Some(UserId::new()),
        action: action.into(),
        target: Some("target-x".into()),
        result: "ok".into(),
        note: None,
    }
}

#[tokio::test]
async fn appended_rows_form_a_consistent_chain() {
    let db = fresh_db();
    for i in 0..5 {
        append(&db, &sample_row(&format!("act.{i}")))
            .await
            .expect("append");
    }
    let r = verify_chain_tail(&db, 100).await.expect("verify");
    assert_eq!(r.checked, 5);
    assert_eq!(r.broken_at_seq, None);
    assert_eq!(r.legacy_unhashed, 0);
}

#[tokio::test]
async fn first_row_chains_from_empty_prev_hash() {
    let db = fresh_db();
    append(&db, &sample_row("first")).await.expect("append");
    let (prev, hash): (String, String) = db
        .with_conn(|c| {
            let (p, h): (String, String) = c.query_row(
                "SELECT prev_hash, hash FROM audit_log ORDER BY seq ASC LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            Ok((p, h))
        })
        .await
        .unwrap();
    assert_eq!(prev, "", "first row's prev_hash must be empty");
    assert_eq!(hash.len(), 64, "hash must be 64 hex chars (SHA-256)");
}

#[tokio::test]
async fn a_tamper_that_leaves_its_own_hash_stale_is_caught_by_the_row_formula() {
    // RFC 125 D6: this is the *careless* tamper — content changed, `hash`
    // column left as it was — caught by the single-row formula check
    // that existed before this RFC (`compute_hash(prev, row) == hash`).
    // It says nothing about linkage between rows, and previously had a
    // comment that did. `a_single_row_rewrite_with_its_own_hash_recomputed_is_now_caught`,
    // below, is the harder case this RFC actually added: the same tamper
    // with `hash` also recomputed, which this check alone cannot see.
    let db = fresh_db();
    append(&db, &sample_row("a")).await.expect("append");
    append(&db, &sample_row("b")).await.expect("append");
    append(&db, &sample_row("c")).await.expect("append");

    db.with_conn(|c| {
        c.execute("UPDATE audit_log SET action = 'tampered' WHERE seq = 2", [])?;
        Ok(())
    })
    .await
    .expect("tamper");

    let r = verify_chain_tail(&db, 100).await.expect("verify");
    assert_eq!(r.broken_at_seq, Some(2), "{r:?}");
}

/// Read every column `verify_chain_tail` reads for the row at `seq`, so a
/// test can change one field and recompute a self-consistent `hash` for
/// the rest without guessing at the row's other content.
async fn read_row_raw(db: &Database, seq: i64) -> (String, Option<String>, Option<String>, String) {
    db.with_conn(move |c| {
        c.query_row(
            "SELECT at, actor, target, prev_hash FROM audit_log WHERE seq = ?1",
            [seq],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, String>(3)?,
                ))
            },
        )
        .map_err(Into::into)
    })
    .await
    .expect("read row")
}

#[tokio::test]
async fn a_single_row_rewrite_with_its_own_hash_recomputed_is_now_caught() {
    // RFC 125 D1. Before this RFC: change row 2's content, recompute
    // row 2's own `hash` from row 2's *unchanged* `prev_hash`, touch no
    // other row — `verify_chain_tail` reported the tail intact, because
    // it never compared row 2's `hash` to row 3's `prev_hash`. That
    // comparison is what this test requires.
    let db = fresh_db();
    append(&db, &sample_row("a")).await.expect("append");
    append(&db, &sample_row("b")).await.expect("append");
    append(&db, &sample_row("c")).await.expect("append");

    let (at, actor, target, prev_hash) = read_row_raw(&db, 2).await;
    let tampered = AuditLogRow {
        at: at.parse().expect("at"),
        actor: actor.map(|s| s.parse().expect("actor")),
        action: "tampered".into(),
        target,
        result: "ok".into(),
        note: None,
    };
    let new_hash = compute_hash(&prev_hash, &tampered);
    db.with_conn(move |c| {
        c.execute(
            "UPDATE audit_log SET action = ?1, hash = ?2 WHERE seq = 2",
            params![tampered.action, new_hash],
        )?;
        Ok(())
    })
    .await
    .expect("tamper with recomputed hash");

    let r = verify_chain_tail(&db, 100).await.expect("verify");
    assert_eq!(r.broken_at_seq, Some(3), "{r:?}");
}

#[tokio::test]
async fn a_hashed_row_demoted_to_look_legacy_is_caught() {
    // RFC 125 stage 2. Before this rule: blank row 2's `hash` (so it is
    // read as a pre-v0.17.0 legacy row and skipped, its own `prev_hash`
    // never compared) and relink row 3 to treat row 2 as that legacy
    // boundary (`prev_hash = ""`, `hash` recomputed to match). Two
    // writes, and the exact shape the residual's own probe measured:
    // `ChainVerifyReport { checked: 2, broken_at_seq: None,
    // legacy_unhashed: 1 }`. A hashed row can never legitimately be
    // followed by a legacy one, which is what catches it now.
    let db = fresh_db();
    append(&db, &sample_row("a")).await.expect("append");
    append(&db, &sample_row("b")).await.expect("append");
    append(&db, &sample_row("c")).await.expect("append");

    let (at, actor, target, _) = read_row_raw(&db, 3).await;
    let row3 = AuditLogRow {
        at: at.parse().expect("at"),
        actor: actor.map(|s| s.parse().expect("actor")),
        action: "c".into(),
        target,
        result: "ok".into(),
        note: None,
    };
    let relinked_hash = compute_hash("", &row3);
    db.with_conn(move |c| {
        c.execute("UPDATE audit_log SET hash = '' WHERE seq = 2", [])?;
        c.execute(
            "UPDATE audit_log SET prev_hash = '', hash = ?1 WHERE seq = 3",
            params![relinked_hash],
        )?;
        Ok(())
    })
    .await
    .expect("demote row 2 and relink row 3");

    let r = verify_chain_tail(&db, 100).await.expect("verify");
    assert_eq!(r.broken_at_seq, Some(2), "{r:?}");
}

#[tokio::test]
async fn the_window_edge_catches_a_rewrite_just_outside_it() {
    // RFC 125 D3. Same tamper as above, but the window (`limit = 3`)
    // does not include row 2 at all — only rows 3, 4, 5. If the window's
    // oldest row (3) were treated as verified without checking its true
    // predecessor, this tamper on row 2 would be invisible to a caller
    // using a limit smaller than the whole table, which is every caller.
    let db = fresh_db();
    for a in ["a", "b", "c", "d", "e"] {
        append(&db, &sample_row(a)).await.expect("append");
    }
    let (at, actor, target, prev_hash) = read_row_raw(&db, 2).await;
    let tampered = AuditLogRow {
        at: at.parse().expect("at"),
        actor: actor.map(|s| s.parse().expect("actor")),
        action: "tampered".into(),
        target,
        result: "ok".into(),
        note: None,
    };
    let new_hash = compute_hash(&prev_hash, &tampered);
    db.with_conn(move |c| {
        c.execute(
            "UPDATE audit_log SET action = ?1, hash = ?2 WHERE seq = 2",
            params![tampered.action, new_hash],
        )?;
        Ok(())
    })
    .await
    .expect("tamper");

    let r = verify_chain_tail(&db, 3).await.expect("verify");
    assert_eq!(r.broken_at_seq, Some(3), "{r:?}");
}

#[tokio::test]
async fn the_window_edge_does_not_false_positive_on_a_clean_predecessor() {
    // The positive twin of the test above: nothing tampered, and a
    // narrow window still verifies clean once the boundary read confirms
    // its oldest row's `prev_hash` matches the row just outside it.
    let db = fresh_db();
    for a in ["a", "b", "c", "d", "e"] {
        append(&db, &sample_row(a)).await.expect("append");
    }
    let r = verify_chain_tail(&db, 3).await.expect("verify");
    assert_eq!(r.checked, 3);
    assert_eq!(r.broken_at_seq, None, "{r:?}");
}

#[tokio::test]
async fn a_deleted_row_relinked_to_hide_it_is_caught_by_sequence_continuity() {
    // RFC 125 D2. The harder case D2 exists for: row 3 is deleted *and*
    // row 4 is rewritten to point at row 2 with a correctly recomputed
    // `hash`, so linkage is fully consistent across the survivors — the
    // only remaining evidence is that `seq` jumps from 2 to 4.
    let db = fresh_db();
    for a in ["a", "b", "c", "d", "e"] {
        append(&db, &sample_row(a)).await.expect("append");
    }
    let row2_hash: String = db
        .with_conn(|c| {
            c.query_row("SELECT hash FROM audit_log WHERE seq = 2", [], |r| r.get(0))
                .map_err(Into::into)
        })
        .await
        .expect("read row 2");
    let (at, actor, target, _) = read_row_raw(&db, 4).await;
    let row4 = AuditLogRow {
        at: at.parse().expect("at"),
        actor: actor.map(|s| s.parse().expect("actor")),
        action: "d".into(),
        target,
        result: "ok".into(),
        note: None,
    };
    let relinked_hash = compute_hash(&row2_hash, &row4);
    db.with_conn(move |c| {
        c.execute("DELETE FROM audit_log WHERE seq = 3", [])?;
        c.execute(
            "UPDATE audit_log SET prev_hash = ?1, hash = ?2 WHERE seq = 4",
            params![row2_hash, relinked_hash],
        )?;
        Ok(())
    })
    .await
    .expect("delete and relink");

    let r = verify_chain_tail(&db, 100).await.expect("verify");
    assert_eq!(r.broken_at_seq, Some(4), "{r:?}");
}

#[tokio::test]
async fn a_truncated_head_with_no_surviving_predecessor_is_caught() {
    // RFC 125 D3 + D4. Rows 1 and 2 are deleted outright (no relinking
    // attempted); row 3 survives untouched and is internally
    // self-consistent and correctly linked to nothing that still
    // exists. The only tell is that nothing precedes it at all, and its
    // own `seq` is not `1` — the shape D4 names as illegitimate.
    let db = fresh_db();
    for a in ["a", "b", "c"] {
        append(&db, &sample_row(a)).await.expect("append");
    }
    db.with_conn(|c| {
        c.execute("DELETE FROM audit_log WHERE seq IN (1, 2)", [])?;
        Ok(())
    })
    .await
    .expect("truncate head");

    let r = verify_chain_tail(&db, 100).await.expect("verify");
    assert_eq!(r.broken_at_seq, Some(3), "{r:?}");
}

#[tokio::test]
async fn a_truncated_legacy_prefix_is_caught_even_though_prev_hash_still_matches() {
    // RFC 125 D4's narrowest case. Rows 1-2 are legacy (pre-v0.17.0,
    // `hash` and `prev_hash` both `""`). Row 3 is the first row this
    // build ever hashed, so its `prev_hash` is legitimately `""` too —
    // it read row 2's `hash`, which was `""`. If rows 1-2 are later
    // deleted, row 3 has no predecessor at all, and its `prev_hash` of
    // `""` now matches the *default* used when there is no boundary —
    // linkage alone cannot tell this apart from a genuine, lone first
    // row. Only checking that a no-predecessor row's own `seq` is `1`
    // catches it; row 3's `seq` is not `1`.
    let db = fresh_db();
    let now = Utc::now();
    for _ in 0..2 {
        db.with_conn(move |c| {
            c.execute(
                "INSERT INTO audit_log(at, actor, action, target, result, note, prev_hash, hash) \
                     VALUES(?1, NULL, 'legacy', NULL, 'ok', NULL, '', '')",
                [now],
            )?;
            Ok(())
        })
        .await
        .expect("insert legacy row");
    }
    append(&db, &sample_row("genesis-after-legacy"))
        .await
        .expect("append"); // seq 3, prev_hash legitimately ""
    append(&db, &sample_row("d")).await.expect("append"); // seq 4
    append(&db, &sample_row("e")).await.expect("append"); // seq 5

    db.with_conn(|c| {
        c.execute("DELETE FROM audit_log WHERE seq IN (1, 2)", [])?;
        Ok(())
    })
    .await
    .expect("delete the legacy prefix");

    let r = verify_chain_tail(&db, 100).await.expect("verify");
    assert_eq!(r.broken_at_seq, Some(3), "{r:?}");
}

#[tokio::test]
async fn a_lone_genesis_row_verifies_clean() {
    // RFC 125 D4, the legitimate half: the table's true first row has an
    // empty `prev_hash` and no predecessor at all, and that is fine.
    let db = fresh_db();
    append(&db, &sample_row("a")).await.expect("append");
    let r = verify_chain_tail(&db, 100).await.expect("verify");
    assert_eq!(r.checked, 1);
    assert_eq!(r.broken_at_seq, None, "{r:?}");
}

#[tokio::test]
async fn a_later_row_forged_to_look_like_genesis_is_rejected() {
    // RFC 125 D4, the illegitimate half: row 2's `prev_hash` is forged to
    // `""` and its own `hash` is recomputed to match that forgery, so the
    // single-row formula check alone would pass. A real predecessor
    // (row 1) still exists and disagrees, which is what catches it.
    let db = fresh_db();
    append(&db, &sample_row("a")).await.expect("append");
    append(&db, &sample_row("b")).await.expect("append");

    let (at, actor, target, _) = read_row_raw(&db, 2).await;
    let row2 = AuditLogRow {
        at: at.parse().expect("at"),
        actor: actor.map(|s| s.parse().expect("actor")),
        action: "b".into(),
        target,
        result: "ok".into(),
        note: None,
    };
    let forged_hash = compute_hash("", &row2);
    db.with_conn(move |c| {
        c.execute(
            "UPDATE audit_log SET prev_hash = '', hash = ?1 WHERE seq = 2",
            params![forged_hash],
        )?;
        Ok(())
    })
    .await
    .expect("forge genesis");

    let r = verify_chain_tail(&db, 100).await.expect("verify");
    assert_eq!(r.broken_at_seq, Some(2), "{r:?}");
}

#[tokio::test]
async fn legacy_unhashed_rows_are_reported_separately() {
    let db = fresh_db();
    let now = Utc::now();
    db.with_conn(move |c| {
        c.execute(
            "INSERT INTO audit_log(at, actor, action, target, result, note, prev_hash, hash) \
                 VALUES(?1, NULL, 'legacy', NULL, 'ok', NULL, '', '')",
            [now],
        )?;
        Ok(())
    })
    .await
    .unwrap();
    append(&db, &sample_row("post-upgrade"))
        .await
        .expect("append");

    let r = verify_chain_tail(&db, 100).await.expect("verify");
    assert_eq!(r.checked, 1);
    assert_eq!(r.legacy_unhashed, 1);
    assert_eq!(r.broken_at_seq, None);
}

#[tokio::test]
async fn canonical_bytes_distinguishes_field_boundaries() {
    // Length-prefix protects against a row {"a", "bc"} hashing
    // the same as a row {"ab", "c"}.
    let at = Utc::now();
    let r1 = AuditLogRow {
        at,
        actor: None,
        action: "a".into(),
        target: Some("bc".into()),
        result: "ok".into(),
        note: None,
    };
    let r2 = AuditLogRow {
        at,
        actor: None,
        action: "ab".into(),
        target: Some("c".into()),
        result: "ok".into(),
        note: None,
    };
    assert_ne!(canonical_bytes(&r1), canonical_bytes(&r2));
}

// ── RFC 096-B1 stage 6b: `emit_must_attempt` ────────────────────────────────

use crate::registry::{ActorRequirement, AttributeSpec, AuditEventKind, TargetRequirement};

static TEST_MUST_ATTEMPT: crate::registry::MustAttemptDescriptor =
    crate::registry::MustAttemptDescriptor {
        kind: AuditEventKind::AuthFederationTakeoverBlocked,
        name: "test.must_attempt",
        actor: ActorRequirement::None,
        target: TargetRequirement::None,
        attributes: &[AttributeSpec {
            name: "reason",
            description: "test attribute",
            required: true,
        }],
    };

/// Real DB-level failure, the same mechanism `tests/e2e/r102_stage4.rs`'s
/// `break_audit_log` already uses: a trigger that rejects every insert,
/// rather than a simulated flag. `emit_must_attempt`'s own `audit::append`
/// call runs inside this, so there is nothing `emit_must_attempt` could
/// special-case to tell a genuine failure from an injected one.
async fn break_audit_log(db: &Database) {
    db.with_conn(|conn| {
        conn.execute_batch(
            "CREATE TRIGGER stage6b_reject_audit BEFORE INSERT ON audit_log \
             BEGIN SELECT RAISE(ABORT, 'stage6b test: audit_log insert rejected'); END;",
        )?;
        Ok(())
    })
    .await
    .expect("install trigger");
}

#[tokio::test]
async fn emit_must_attempt_records_a_successful_emission() {
    let db = fresh_db();
    let attributes = AuditAttributes::builder()
        .attribute("reason", "test_reason")
        .build();

    let outcome = emit_must_attempt(
        &db,
        &TEST_MUST_ATTEMPT,
        Utc::now(),
        None,
        None,
        "denied",
        attributes,
    )
    .await;

    assert_eq!(outcome, MustAttemptOutcome::Recorded);
    let rows = recent(&db, 1).await.expect("recent");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].action, "test.must_attempt");
    assert_eq!(rows[0].result, "denied");
    assert_eq!(rows[0].note.as_deref(), Some("reason=test_reason"));
}

/// The failure-injection test item 3/4 of the dispatch ask for: a genuine
/// append failure reports `Failed`, and (checked at the e2e level,
/// `tests/e2e/r096_b1_stage6b.rs`) still emits structured telemetry and
/// still lets the caller's own primary response proceed.
#[tokio::test]
async fn emit_must_attempt_reports_failed_when_the_append_itself_fails() {
    let db = fresh_db();
    break_audit_log(&db).await;
    let attributes = AuditAttributes::builder()
        .attribute("reason", "test_reason")
        .build();

    let outcome = emit_must_attempt(
        &db,
        &TEST_MUST_ATTEMPT,
        Utc::now(),
        None,
        None,
        "denied",
        attributes,
    )
    .await;

    assert_eq!(outcome, MustAttemptOutcome::Failed);
}

/// `AuditAttributesBuilder::build()` itself catches duplicates, too many
/// entries, and an over-long value — not "a required one was never
/// added", since the builder alone has no descriptor to check against.
/// `break_audit_log` is not installed: if this reported `Recorded`
/// anyway, that would mean the builder's own error was silently ignored
/// rather than propagated, not that the database happened to cooperate.
#[tokio::test]
async fn emit_must_attempt_reports_failed_when_the_builder_itself_errors() {
    let db = fresh_db();
    let attributes = AuditAttributes::builder()
        .attribute("reason", "first")
        .attribute("reason", "second")
        .build();
    assert!(matches!(
        attributes,
        Err(AuditBuildError::DuplicateAttribute("reason"))
    ));

    let outcome = emit_must_attempt(
        &db,
        &TEST_MUST_ATTEMPT,
        Utc::now(),
        None,
        None,
        "denied",
        attributes,
    )
    .await;

    assert_eq!(outcome, MustAttemptOutcome::Failed);
}

/// The cross-check `emit_must_attempt` itself runs: the builder succeeds
/// (nothing wrong with the *entries present*), but `TEST_MUST_ATTEMPT`
/// declares `reason` required and none was added. Same check
/// `Database::class_a` runs against a Class-A event's own descriptor
/// (`registry.rs`'s `MissingRequiredAttribute` site) — this proves
/// `emit_must_attempt` runs the equivalent for a `MustAttemptDescriptor`,
/// which the builder alone cannot do.
#[tokio::test]
async fn emit_must_attempt_reports_failed_when_a_required_attribute_is_missing() {
    let db = fresh_db();
    let attributes = AuditAttributes::builder().build();
    assert!(attributes.is_ok(), "nothing wrong with zero entries alone");

    let outcome = emit_must_attempt(
        &db,
        &TEST_MUST_ATTEMPT,
        Utc::now(),
        None,
        None,
        "denied",
        attributes,
    )
    .await;

    assert_eq!(outcome, MustAttemptOutcome::Failed);
    assert_eq!(
        recent(&db, 1).await.expect("recent").len(),
        0,
        "a missing required attribute must not reach the audit log at all"
    );
}
