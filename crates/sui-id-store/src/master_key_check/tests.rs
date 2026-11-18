use super::*;
use crate::Database;

// ── the chooser, each of the eight cells ────────────────────────────────

fn sealed_no_sentinel() -> Inspection {
    Inspection::SealedNoSentinel {
        sample: vec![0u8; 8],
        aad: b"probe",
    }
}

fn sentinel() -> Inspection {
    Inspection::Sentinel(SentinelRow {
        sealed: vec![0u8; 8],
        key_fingerprint: "deadbeefdeadbeef".into(),
        written_at: Utc::now(),
    })
}

#[test]
fn no_file_with_a_key_uses_it() {
    assert_eq!(
        decide(true, &Inspection::NoFile),
        KeyCheckDecision::UseThenWriteSentinel
    );
}

#[test]
fn no_file_with_no_key_generates_one() {
    assert_eq!(
        decide(false, &Inspection::NoFile),
        KeyCheckDecision::GenerateThenWriteSentinel
    );
}

#[test]
fn sentinel_with_a_key_verifies_against_it() {
    assert_eq!(
        decide(true, &sentinel()),
        KeyCheckDecision::VerifyAgainstSentinel
    );
}

#[test]
fn sentinel_with_no_key_refuses_and_never_generates() {
    assert_eq!(
        decide(false, &sentinel()),
        KeyCheckDecision::RefuseMissingKeyOverSentinel
    );
}

#[test]
fn sealed_no_sentinel_with_a_key_verifies_one_sealed_row() {
    assert_eq!(
        decide(true, &sealed_no_sentinel()),
        KeyCheckDecision::VerifyOneSealedRowThenWriteSentinel
    );
}

#[test]
fn sealed_no_sentinel_with_no_key_refuses_and_never_generates() {
    assert_eq!(
        decide(false, &sealed_no_sentinel()),
        KeyCheckDecision::RefuseMissingKeyOverSealedRows
    );
}

#[test]
fn empty_no_sentinel_with_a_key_uses_it() {
    assert_eq!(
        decide(true, &Inspection::EmptyNoSentinel),
        KeyCheckDecision::UseThenWriteSentinel
    );
}

#[test]
fn empty_no_sentinel_with_no_key_generates_one() {
    assert_eq!(
        decide(false, &Inspection::EmptyNoSentinel),
        KeyCheckDecision::GenerateThenWriteSentinel
    );
}

// ── the sentinel's own seal/open ─────────────────────────────────────────

#[test]
fn the_right_key_opens_its_own_sentinel() {
    let key = MasterKey::generate();
    let sealed = seal_sentinel(&key).expect("seal");
    assert!(sentinel_opens(&key, &sealed));
}

#[test]
fn the_wrong_key_does_not_open_the_sentinel() {
    let key = MasterKey::generate();
    let other = MasterKey::generate();
    let sealed = seal_sentinel(&key).expect("seal");
    assert!(!sentinel_opens(&other, &sealed));
}

#[test]
fn garbage_bytes_do_not_open_as_a_sentinel() {
    let key = MasterKey::generate();
    assert!(!sentinel_opens(&key, b"not a real sealed value"));
}

#[test]
fn read_sentinel_and_first_sealed_row_tolerate_a_database_too_old_to_have_the_tables() {
    // Migration 1 predates every one of the six sealed tables and the
    // sentinel table (0052) by a wide margin — none of them exist yet.
    // The read-only path must say "none", not surface rusqlite's "no
    // such table" as a real error.
    let mut conn = Connection::open_in_memory().expect("open");
    crate::migrations::run_up_to(&mut conn, 1).expect("migrate to version 1");
    assert!(read_sentinel(&conn).expect("read_sentinel").is_none());
    assert!(first_sealed_row(&conn).expect("first_sealed_row").is_none());
    assert!(matches!(
        inspect(&conn).expect("inspect"),
        Inspection::EmptyNoSentinel
    ));
}

// ── read_sentinel / ensure_sentinel_written / inspect / first_sealed_row ──

#[tokio::test]
async fn read_sentinel_is_none_on_a_fresh_database() {
    let key = MasterKey::generate();
    let db = Database::open_in_memory(key).expect("open");
    let found = db
        .with_conn(|conn| Ok(read_sentinel(conn)?))
        .await
        .expect("with_conn");
    assert!(found.is_none());
}

#[tokio::test]
async fn ensure_sentinel_written_writes_once_and_is_idempotent() {
    let key = MasterKey::generate();
    let db = Database::open_in_memory(key).expect("open");

    ensure_sentinel_written(&db, db.key())
        .await
        .expect("first write");
    let row = db
        .with_conn(|conn| Ok(read_sentinel(conn)?))
        .await
        .expect("with_conn")
        .expect("sentinel exists");
    assert!(sentinel_opens(db.key(), &row.sealed));
    assert_eq!(row.key_fingerprint, crypto::fingerprint(db.key()));
    let written_at_first = row.written_at;

    // A second call must not touch the row (nothing is written twice,
    // and there is no column to update even if it tried).
    ensure_sentinel_written(&db, db.key())
        .await
        .expect("second write is a no-op");
    let row_again = db
        .with_conn(|conn| Ok(read_sentinel(conn)?))
        .await
        .expect("with_conn")
        .expect("sentinel still exists");
    assert_eq!(row_again.written_at, written_at_first);
}

#[tokio::test]
async fn inspect_sees_the_sentinel_once_written() {
    let key = MasterKey::generate();
    let db = Database::open_in_memory(key).expect("open");
    let before = db.with_conn(|conn| Ok(inspect(conn)?)).await.expect("ok");
    assert!(matches!(before, Inspection::EmptyNoSentinel));

    ensure_sentinel_written(&db, db.key()).await.expect("write");
    let after = db.with_conn(|conn| Ok(inspect(conn)?)).await.expect("ok");
    assert!(matches!(after, Inspection::Sentinel(_)));
}

#[tokio::test]
async fn inspect_sees_a_sealed_row_with_no_sentinel_as_the_upgrade_case() {
    let key = MasterKey::generate();
    let db = Database::open_in_memory(key).expect("open");
    // A real sealed row via a real repo write, not a hand-crafted one:
    // signing-key insertion seals `private_key_enc` under the master
    // key, the same column `first_for_key_check` reads.
    db.with_conn({
        let key_bytes = *db.key().as_bytes();
        move |conn| {
            let sealed = crate::crypto::seal(
                &MasterKey::from_bytes(key_bytes),
                b"private-key-material",
                b"sui-id/signing_key/v1",
            )?;
            conn.execute(
                "INSERT INTO signing_keys (id, algorithm, private_key_enc, public_key, is_active, created_at) \
                 VALUES ('k1', 'Ed25519', ?1, X'00', 1, '2026-01-01T00:00:00Z')",
                rusqlite::params![sealed],
            )?;
            Ok(())
        }
    })
    .await
    .expect("seed a sealed signing key row directly");

    let found = db.with_conn(|conn| Ok(inspect(conn)?)).await.expect("ok");
    assert!(matches!(found, Inspection::SealedNoSentinel { .. }));
}

#[tokio::test]
async fn first_sealed_row_stops_at_the_first_table_in_its_fixed_order() {
    let key = MasterKey::generate();
    let db = Database::open_in_memory(key).expect("open");
    // Seed only `email_outbox` (the last of the six probed) and confirm
    // it is still found — proves the probe does not stop silently on an
    // earlier, empty table.
    db.with_conn({
        let key_bytes = *db.key().as_bytes();
        move |conn| {
            let k = MasterKey::from_bytes(key_bytes);
            let rec = crate::crypto::seal(&k, b"user@example.com", b"email_outbox.recipient")?;
            let pay = crate::crypto::seal(&k, b"payload", b"email_outbox.payload")?;
            conn.execute(
                "INSERT INTO email_outbox \
                 (id, state, template, recipient_enc, payload_enc, attempt_count, \
                  next_attempt_at, created_at, updated_at) \
                 VALUES ('e1', 'queued', 'test', ?1, ?2, 0, \
                  '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                rusqlite::params![rec, pay],
            )?;
            Ok(())
        }
    })
    .await
    .expect("seed email_outbox");

    let found = db
        .with_conn(|conn| Ok(first_sealed_row(conn)?))
        .await
        .expect("ok");
    let (bytes, aad) = found.expect("found a row");
    assert_eq!(aad, b"email_outbox.recipient");
    assert!(crate::crypto::open(db.key(), &bytes, aad).is_ok());
}
