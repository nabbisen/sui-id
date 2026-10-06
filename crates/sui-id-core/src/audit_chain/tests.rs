use super::*;
use crate::time::system_clock;
use sui_id_shared::ids::UserId;
use sui_id_store::crypto::MasterKey;
use sui_id_store::models::AuditLogRow;

fn fresh_db() -> Database {
    Database::open_in_memory(MasterKey::generate()).expect("db")
}

fn sample_row(action: &str) -> AuditLogRow {
    AuditLogRow {
        at: chrono::Utc::now(),
        actor: Some(UserId::new()),
        action: action.into(),
        target: Some("target-x".into()),
        result: "ok".into(),
        note: None,
    }
}

#[tokio::test]
async fn an_empty_chain_is_intact() {
    let db = fresh_db();
    let clock = system_clock();
    let r = check(&db, &clock, 100).await;
    assert!(matches!(
        r,
        ChainCheck::Intact {
            checked: 0,
            legacy_unhashed: 0,
            limit: 100
        }
    ));
    assert_eq!(r.limit(), 100);
}

#[tokio::test]
async fn a_consistent_chain_is_intact() {
    let db = fresh_db();
    let clock = system_clock();
    audit::append(&db, &sample_row("a")).await.expect("append");
    audit::append(&db, &sample_row("b")).await.expect("append");
    let r = check(&db, &clock, 100).await;
    match r {
        ChainCheck::Intact { checked, .. } => assert_eq!(checked, 2),
        other => panic!("expected Intact, got {other:?}"),
    }
}

#[tokio::test]
async fn a_broken_chain_names_the_row_and_carries_the_limit() {
    let db = fresh_db();
    let clock = system_clock();
    audit::append(&db, &sample_row("a")).await.expect("append");
    audit::append(&db, &sample_row("b")).await.expect("append");
    db.with_conn(|c| {
        c.execute("UPDATE audit_log SET action = 'tampered' WHERE seq = 1", [])?;
        Ok(())
    })
    .await
    .expect("tamper");
    let r = check(&db, &clock, 100).await;
    match r {
        ChainCheck::Broken { at_seq, limit, .. } => {
            assert_eq!(at_seq, 1);
            assert_eq!(limit, 100);
        }
        other => panic!("expected Broken, got {other:?}"),
    }
}

#[tokio::test]
async fn a_read_error_is_could_not_verify_not_intact_and_not_broken() {
    let db = fresh_db();
    let clock = system_clock();
    audit::append(&db, &sample_row("a")).await.expect("append");
    db.with_conn(|c| {
        c.execute(
            "UPDATE audit_log SET actor = 'not-a-uuid' WHERE seq = 1",
            [],
        )?;
        Ok(())
    })
    .await
    .expect("corrupt actor");
    let r = check(&db, &clock, 100).await;
    assert!(
        matches!(r, ChainCheck::CouldNotVerify { .. }),
        "expected CouldNotVerify, got {r:?}"
    );
    assert_eq!(r.limit(), 100);
}

#[tokio::test]
async fn a_could_not_verify_result_writes_a_best_effort_audit_row() {
    let db = fresh_db();
    let clock = system_clock();
    audit::append(&db, &sample_row("a")).await.expect("append");
    db.with_conn(|c| {
        c.execute(
            "UPDATE audit_log SET actor = 'not-a-uuid' WHERE seq = 1",
            [],
        )?;
        Ok(())
    })
    .await
    .expect("corrupt actor");
    let _ = check(&db, &clock, 100).await;
    // Read the row directly: `verify_chain_tail` itself cannot be used to
    // observe it, since the corrupted actor value makes every call
    // through it fail the same way.
    let rows: Vec<String> = db
        .with_conn(|c| {
            let mut stmt = c.prepare(
                "SELECT action FROM audit_log WHERE action = 'audit.chain.verification_failed'",
            )?;
            let rows = stmt
                .query_map([], |r| r.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .await
        .expect("read");
    assert_eq!(rows.len(), 1, "expected exactly one recorded failure row");
}

#[tokio::test]
async fn a_could_not_verify_notes_detail_encoded_so_a_forged_pair_does_not_parse() {
    // RFC 121 review: `detail` is a store error's message, and a future
    // `StoreError` variant could put arbitrary content — including a
    // forged `key=value` pair — inside it. Calling `record` directly
    // with a `detail` shaped exactly like that attack lets this test
    // control the content precisely, independent of what any real
    // `StoreError` happens to produce today.
    let db = fresh_db();
    let clock = system_clock();
    let forged = ChainCheck::CouldNotVerify {
        detail: "forged via=web trailing".into(),
        limit: 42,
    };
    record(&db, &clock, &forged).await;

    let note: String = db
        .with_conn(|c| {
            c.query_row(
                "SELECT note FROM audit_log WHERE action = 'audit.chain.verification_failed'",
                [],
                |r| r.get(0),
            )
            .map_err(Into::into)
        })
        .await
        .expect("read note");

    // The forged pair is not readable as a real attribute: `via` was
    // never a key this event declares, and encoding stops it from
    // being read as one anyway.
    assert_eq!(registry::note_field(&note, "via"), None, "{note}");
    // The real attribute round-trips exactly, space and `=` intact.
    assert_eq!(
        registry::note_field(&note, "detail").as_deref(),
        Some("forged via=web trailing"),
        "{note}"
    );
    assert_eq!(
        registry::note_field(&note, "limit").as_deref(),
        Some("42"),
        "{note}"
    );
}

#[tokio::test]
async fn an_intact_result_writes_no_extra_audit_row() {
    let db = fresh_db();
    let clock = system_clock();
    audit::append(&db, &sample_row("a")).await.expect("append");
    let before = db
        .with_conn(|c| {
            c.query_row("SELECT COUNT(*) FROM audit_log", [], |r| r.get::<_, i64>(0))
                .map_err(Into::into)
        })
        .await
        .expect("count");
    let _ = check(&db, &clock, 100).await;
    let after = db
        .with_conn(|c| {
            c.query_row("SELECT COUNT(*) FROM audit_log", [], |r| r.get::<_, i64>(0))
                .map_err(Into::into)
        })
        .await
        .expect("count");
    assert_eq!(before, after);
}
