use super::*;

#[tokio::test]
async fn concurrent_class_a_commands_maintain_one_unbroken_chain() {
    // RFC 094 Stage 2: "prove the audit chain is read and written
    // on the caller transaction." `repos::audit`'s own tests
    // already prove `append_within_tx` behaves correctly called
    // directly and in isolation; this proves the same property
    // through the real Class-A/registry path, under genuinely
    // concurrent execution rather than by reading the
    // mutex-serialization argument in its doc comment. If the
    // chain-head read and the row insert were ever split across
    // two separate transactions (a regression this test would
    // catch), concurrent commands could compute the same
    // `prev_hash` and fork the chain, or `verify_chain_tail` would
    // report a break.
    let db = fresh_db();
    const N: usize = 20;
    let mut user_ids = Vec::with_capacity(N);
    for _ in 0..N {
        let user = a_user();
        repos::users::create(&db, &user).await.expect("create user");
        user_ids.push(user.id);
    }

    let handles: Vec<_> = user_ids
        .into_iter()
        .map(|user_id| {
            let db = db.clone();
            tokio::spawn(async move {
                record_login_failure(&db, user_id, |_count| None)
                    .await
                    .expect("record failure")
            })
        })
        .collect();
    for handle in handles {
        handle.await.expect("task join");
    }

    let report = repos::audit::verify_chain_tail(&db, (N * 2) as i64)
        .await
        .expect("verify chain");
    assert_eq!(
        report.checked, N,
        "exactly one audit row per concurrent command, no lost or duplicated rows"
    );
    assert!(
        report.broken_at_seq.is_none(),
        "no fork: every row's prev_hash must chain from exactly one predecessor, \
         even though the audit-row writes raced"
    );
}
