use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use sui_id_shared::ids::FederationProviderId;

#[derive(Debug, Clone, PartialEq, Eq)]
struct ToyJwks {
    kids: Vec<String>,
}

fn has_kid(doc: &ToyJwks, kid: &str) -> bool {
    doc.kids.iter().any(|k| k == kid)
}

fn validate_toy_jwks(body: &[u8]) -> Result<ToyJwks, &'static str> {
    let s = std::str::from_utf8(body).map_err(|_| "not utf8")?;
    Ok(ToyJwks {
        kids: s
            .split(',')
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect(),
    })
}

fn make_key(provider: FederationProviderId, version: u64, generation: u64) -> CacheKey {
    CacheKey::new(
        provider,
        crate::cache_freshness::ProviderVersion(version),
        crate::cache_freshness::ActivationGeneration(generation),
    )
}

async fn fetch_success(
    key: CacheKey,
    kids: &str,
) -> Result<(RetainedEntry<ToyJwks>, CacheOutcome), &'static str> {
    let clock = sui_id_core::time::system_clock();
    crate::cache_freshness::store_fresh(
        key,
        crate::cache_freshness::ResponseHeaders {
            cache_control: &["max-age=60"],
            content_type: &["application/json"],
            ..Default::default()
        },
        crate::cache_freshness::CacheKind::Jwks,
        kids.as_bytes(),
        validate_toy_jwks,
        &clock,
    )
    .map_err(|_| "store_fresh failed")
}

/// Used wherever a test asserts a fetch closure must never run: a named
/// `async fn` gives the compiler a concrete `E` to infer against, which a
/// bare `async { panic!(...) }` closure cannot (`!` constrains nothing).
async fn must_not_dispatch(
    _current: Option<RetainedEntry<ToyJwks>>,
) -> Result<(RetainedEntry<ToyJwks>, CacheOutcome), &'static str> {
    panic!("fetch must not be dispatched in this scenario")
}

async fn fetch_with_etag(
    key: CacheKey,
    kids: &str,
    etag: &str,
) -> Result<(RetainedEntry<ToyJwks>, CacheOutcome), &'static str> {
    let clock = sui_id_core::time::system_clock();
    crate::cache_freshness::store_fresh(
        key,
        crate::cache_freshness::ResponseHeaders {
            cache_control: &["max-age=60"],
            etag: &[etag],
            content_type: &["application/json"],
            ..Default::default()
        },
        crate::cache_freshness::CacheKind::Jwks,
        kids.as_bytes(),
        validate_toy_jwks,
        &clock,
    )
    .map_err(|_| "store_fresh failed")
}

/// Row 1-4: two callers arriving while one flight is in the air must
/// produce exactly one fetch and see the same outcome. The synchronization
/// is the module's natural seam (see the module doc comment): caller A's
/// own fetch closure signals its start -- which can only happen after the
/// leader transition has committed -- before blocking on release.
#[tokio::test]
async fn two_concurrent_ordinary_lookups_produce_one_request_and_see_the_same_outcome() {
    let clock: SharedMonotonicClock = Arc::new(MockMonotonicClock::new());
    let cache = Arc::new(FederationDocumentCache::<ToyJwks>::new(clock));
    let provider = FederationProviderId::new();
    let key = make_key(provider, 1, 1);

    let fetch_count = Arc::new(AtomicUsize::new(0));
    let (started_tx, started_rx) = tokio::sync::oneshot::channel::<()>();
    let (release_tx, release_rx) = tokio::sync::oneshot::channel::<()>();

    let cache_a = cache.clone();
    let fetch_count_a = fetch_count.clone();
    let task_a = tokio::spawn(async move {
        cache_a
            .lookup(
                provider,
                move || Some(key),
                move |_current| {
                    let fetch_count_a = fetch_count_a.clone();
                    async move {
                        fetch_count_a.fetch_add(1, Ordering::SeqCst);
                        started_tx.send(()).expect("test still waiting");
                        release_rx.await.expect("test still holding the release");
                        fetch_success(key, "a,b").await
                    }
                },
            )
            .await
    });

    started_rx.await.expect("leader signalled its own start");

    let cache_b = cache.clone();
    let task_b = tokio::spawn(async move {
        cache_b
            .lookup(provider, move || Some(key), must_not_dispatch)
            .await
    });

    // Give B several chances to run up to its own suspension point
    // (joining the watch channel) before A is released. On the
    // single-threaded test runtime this is cooperative scheduling, not a
    // timing race: nothing else is runnable in between.
    for _ in 0..8 {
        tokio::task::yield_now().await;
    }
    release_tx.send(()).expect("A still awaiting release");

    let (result_a, result_b) = tokio::join!(task_a, task_b);
    let (entry_a, outcome_a) = result_a
        .expect("task A did not panic")
        .expect("A succeeded");
    let (entry_b, outcome_b) = result_b
        .expect("task B did not panic")
        .expect("B succeeded");

    assert_eq!(fetch_count.load(Ordering::SeqCst), 1);
    assert_eq!(entry_a.document(), entry_b.document());
    assert_eq!(outcome_a, outcome_b);
}

#[tokio::test]
async fn a_failed_flight_starts_cooldown_and_a_second_caller_fails_without_a_request() {
    let clock: SharedMonotonicClock = Arc::new(MockMonotonicClock::new());
    let cache = FederationDocumentCache::<ToyJwks>::new(clock);
    let provider = FederationProviderId::new();
    let key = make_key(provider, 1, 1);

    let failed: Result<(RetainedEntry<ToyJwks>, CacheOutcome), LookupError<&'static str>> = cache
        .lookup(
            provider,
            || Some(key),
            |_current| async { Err("network down") },
        )
        .await;
    assert_eq!(failed, Err(LookupError::Fetch("network down")));

    let result = cache
        .lookup(provider, || Some(key), must_not_dispatch)
        .await;
    assert_eq!(result, Err(LookupError::Cooldown));
}

#[tokio::test]
async fn unknown_kid_during_cooldown_fails_without_spending_the_forced_budget() {
    let clock = Arc::new(MockMonotonicClock::new());
    let cache = FederationDocumentCache::<ToyJwks>::new(clock.clone());
    let provider = FederationProviderId::new();
    let key = make_key(provider, 1, 1);

    let failed: Result<(RetainedEntry<ToyJwks>, CacheOutcome), LookupError<&'static str>> = cache
        .lookup(
            provider,
            || Some(key),
            |_current| async { Err("network down") },
        )
        .await;
    assert_eq!(failed, Err(LookupError::Fetch("network down")));

    let during_cooldown = cache
        .lookup_for_kid(
            provider,
            "missing",
            || Some(key),
            has_kid,
            must_not_dispatch,
        )
        .await;
    assert_eq!(during_cooldown, Err(LookupError::Cooldown));

    // If that cooldown-blocked call had wrongly spent the forced budget,
    // this would now fail with ForcedBudgetSpent instead of dispatching.
    clock.advance(Duration::from_secs(31));
    let fetch_count = Arc::new(AtomicUsize::new(0));
    let fc = fetch_count.clone();
    let after_cooldown = cache
        .lookup_for_kid(
            provider,
            "a",
            || Some(key),
            has_kid,
            move |_current| {
                let fc = fc.clone();
                async move {
                    fc.fetch_add(1, Ordering::SeqCst);
                    fetch_success(key, "a,b").await
                }
            },
        )
        .await;
    assert!(after_cooldown.is_ok());
    assert_eq!(fetch_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn forced_budget_window_is_fifty_nine_blocked_sixty_one_allowed() {
    let clock = Arc::new(MockMonotonicClock::new());
    let cache = FederationDocumentCache::<ToyJwks>::new(clock.clone());
    let provider = FederationProviderId::new();
    let key = make_key(provider, 1, 1);

    let fetch_count = Arc::new(AtomicUsize::new(0));
    let fc1 = fetch_count.clone();
    let first = cache
        .lookup_for_kid(
            provider,
            "missing",
            || Some(key),
            has_kid,
            move |_current| {
                let fc1 = fc1.clone();
                async move {
                    fc1.fetch_add(1, Ordering::SeqCst);
                    fetch_success(key, "a").await
                }
            },
        )
        .await;
    assert_eq!(first, Err(LookupError::KidNotFound));
    assert_eq!(fetch_count.load(Ordering::SeqCst), 1);

    clock.advance(Duration::from_secs(59));
    let second = cache
        .lookup_for_kid(
            provider,
            "missing",
            || Some(key),
            has_kid,
            must_not_dispatch,
        )
        .await;
    assert_eq!(second, Err(LookupError::ForcedBudgetSpent));

    clock.advance(Duration::from_secs(2)); // 61s since the first dispatch
    let fc3 = fetch_count.clone();
    let third = cache
        .lookup_for_kid(
            provider,
            "missing",
            || Some(key),
            has_kid,
            move |_current| {
                let fc3 = fc3.clone();
                async move {
                    fc3.fetch_add(1, Ordering::SeqCst);
                    fetch_success(key, "a").await
                }
            },
        )
        .await;
    assert_eq!(third, Err(LookupError::KidNotFound));
    assert_eq!(fetch_count.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn a_forced_flight_returning_304_fails_the_unknown_kid_with_no_second_request() {
    let clock = Arc::new(MockMonotonicClock::new());
    let cache = FederationDocumentCache::<ToyJwks>::new(clock.clone());
    let provider = FederationProviderId::new();
    let key = make_key(provider, 1, 1);

    let (seeded, outcome) = cache
        .lookup(
            provider,
            || Some(key),
            |_current| fetch_with_etag(key, "a", "\"v1\""),
        )
        .await
        .expect("seed succeeds");
    assert_eq!(outcome, CacheOutcome::Retain);
    assert!(has_kid(seeded.document(), "a"));

    let fetch_count = Arc::new(AtomicUsize::new(0));
    let fc = fetch_count.clone();
    let result = cache
        .lookup_for_kid(
            provider,
            "missing",
            || Some(key),
            has_kid,
            move |current| {
                let fc = fc.clone();
                async move {
                    fc.fetch_add(1, Ordering::SeqCst);
                    let wall = sui_id_core::time::system_clock();
                    let entry = current.expect("row 8 passes the current entry through");
                    crate::cache_freshness::apply_304(
                        &entry,
                        key,
                        Some("\"v1\""),
                        crate::cache_freshness::RevalidationHeaders::default(),
                        crate::cache_freshness::CacheKind::Jwks,
                        &wall,
                    )
                    .map_err(|_| "304 handling failed")
                }
            },
        )
        .await;
    assert_eq!(result, Err(LookupError::KidNotFound));
    assert_eq!(fetch_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn a_flight_completing_after_the_provider_is_disabled_does_not_publish() {
    let clock = Arc::new(MockMonotonicClock::new());
    let cache = FederationDocumentCache::<ToyJwks>::new(clock);
    let provider = FederationProviderId::new();
    let key = make_key(provider, 1, 1);

    // Enabled at dispatch time (the decision block's own authority check
    // must pass), disabled by the time the flight resolves and tries to
    // publish -- `current_key` is `Fn`, called at both points, and
    // reports `Some(key)` only on its first call.
    let calls = Arc::new(AtomicUsize::new(0));
    let calls_for_closure = calls.clone();
    let current_key = move || {
        if calls_for_closure.fetch_add(1, Ordering::SeqCst) == 0 {
            Some(key)
        } else {
            None
        }
    };
    let first = cache
        .lookup(provider, current_key, |_current| fetch_success(key, "a"))
        .await;
    assert!(
        first.is_ok(),
        "the caller still gets the data for its own operation"
    );

    // The discard must be real: a second ordinary lookup, now reporting the
    // provider enabled again at the same key, must still dispatch rather
    // than finding a retained (and wrongly published) entry.
    let fetch_count = Arc::new(AtomicUsize::new(0));
    let fc = fetch_count.clone();
    let second = cache
        .lookup(
            provider,
            || Some(key),
            move |_current| {
                let fc = fc.clone();
                async move {
                    fc.fetch_add(1, Ordering::SeqCst);
                    fetch_success(key, "a").await
                }
            },
        )
        .await;
    assert!(second.is_ok());
    assert_eq!(fetch_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn a_flight_completing_after_a_version_change_does_not_publish() {
    let clock = Arc::new(MockMonotonicClock::new());
    let cache = FederationDocumentCache::<ToyJwks>::new(clock);
    let provider = FederationProviderId::new();
    let key = make_key(provider, 1, 1);
    let newer_key = make_key(provider, 2, 1);

    // The flight is dispatched for `key`'s activation, but by the time it
    // resolves `current_key` reports the newer one -- a version bump that
    // happened while the request was in flight.
    let first = cache
        .lookup(
            provider,
            move || Some(newer_key),
            |_current| fetch_success(key, "a"),
        )
        .await;
    assert!(first.is_ok());

    let fetch_count = Arc::new(AtomicUsize::new(0));
    let fc = fetch_count.clone();
    let second = cache
        .lookup(
            provider,
            move || Some(newer_key),
            move |_current| {
                let fc = fc.clone();
                async move {
                    fc.fetch_add(1, Ordering::SeqCst);
                    fetch_success(newer_key, "a").await
                }
            },
        )
        .await;
    assert!(second.is_ok());
    assert_eq!(fetch_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn a_refreshed_jwks_omitting_a_previously_present_kid_makes_it_fail() {
    let clock = Arc::new(MockMonotonicClock::new());
    let cache = FederationDocumentCache::<ToyJwks>::new(clock.clone());
    let provider = FederationProviderId::new();
    let key = make_key(provider, 1, 1);

    let (seeded, _) = cache
        .lookup(provider, || Some(key), |_current| fetch_success(key, "a"))
        .await
        .expect("seed succeeds");
    assert!(has_kid(seeded.document(), "a"));

    // Force-refresh for an unrelated unknown kid "b"; the rotated document
    // this returns no longer carries "a" at all.
    let refreshed = cache
        .lookup_for_kid(
            provider,
            "b",
            || Some(key),
            has_kid,
            |_current| fetch_success(key, "b"),
        )
        .await;
    assert!(
        refreshed.is_ok(),
        "the forced refresh found \"b\" in the new document"
    );

    // The 60s forced window from that dispatch is still open; confirm it
    // fails for that reason first (not yet proof the document lacks "a").
    let still_in_window = cache
        .lookup_for_kid(provider, "a", || Some(key), has_kid, must_not_dispatch)
        .await;
    assert_eq!(still_in_window, Err(LookupError::ForcedBudgetSpent));

    // Past the window: "a" is genuinely gone, not retained as a fallback.
    clock.advance(Duration::from_secs(61));
    let fetch_count = Arc::new(AtomicUsize::new(0));
    let fc = fetch_count.clone();
    let still_missing = cache
        .lookup_for_kid(
            provider,
            "a",
            || Some(key),
            has_kid,
            move |_current| {
                let fc = fc.clone();
                async move {
                    fc.fetch_add(1, Ordering::SeqCst);
                    fetch_success(key, "b").await
                }
            },
        )
        .await;
    assert_eq!(still_missing, Err(LookupError::KidNotFound));
    assert_eq!(fetch_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn a_fresh_ordinary_lookup_makes_no_network_request() {
    let clock = Arc::new(MockMonotonicClock::new());
    let cache = FederationDocumentCache::<ToyJwks>::new(clock);
    let provider = FederationProviderId::new();
    let key = make_key(provider, 1, 1);

    cache
        .lookup(provider, || Some(key), |_current| fetch_success(key, "a"))
        .await
        .expect("first lookup dispatches and succeeds");

    let result = cache
        .lookup(provider, || Some(key), must_not_dispatch)
        .await;
    assert!(result.is_ok());
}

/// Rule 1, proven rather than only asserted by the other tests: the
/// forced-refresh window starts at dispatch, not completion. A slow
/// request (simulated here by advancing the clock *while the fetch is in
/// flight*, not after it returns) must not extend the budget.
#[tokio::test]
async fn the_forced_window_is_measured_from_dispatch_not_completion() {
    let clock = Arc::new(MockMonotonicClock::new());
    let cache = FederationDocumentCache::<ToyJwks>::new(clock.clone());
    let provider = FederationProviderId::new();
    let key = make_key(provider, 1, 1);

    let clock_for_fetch = clock.clone();
    let first = cache
        .lookup_for_kid(
            provider,
            "missing",
            || Some(key),
            has_kid,
            move |_current| {
                let clock_for_fetch = clock_for_fetch.clone();
                async move {
                    // Time passes while the fetch is in flight -- not after it
                    // completes.
                    clock_for_fetch.advance(Duration::from_secs(70));
                    fetch_success(key, "a").await
                }
            },
        )
        .await;
    assert_eq!(first, Err(LookupError::KidNotFound));

    // If the window were anchored at completion, this immediate second
    // call would be well inside a fresh 60s budget and must fail.
    // Anchored at dispatch -- 70 seconds ago, now that the slow fetch has
    // elapsed -- it must be allowed to dispatch again.
    let fetch_count = Arc::new(AtomicUsize::new(0));
    let fc = fetch_count.clone();
    let second = cache
        .lookup_for_kid(
            provider,
            "missing",
            || Some(key),
            has_kid,
            move |_current| {
                let fc = fc.clone();
                async move {
                    fc.fetch_add(1, Ordering::SeqCst);
                    fetch_success(key, "a").await
                }
            },
        )
        .await;
    assert_eq!(second, Err(LookupError::KidNotFound));
    assert_eq!(fetch_count.load(Ordering::SeqCst), 1);
}

// ---- stage 4d fix: authority on the read paths ----

#[tokio::test]
async fn a_fresh_generation_1_entry_is_not_served_at_generation_2() {
    let clock = Arc::new(MockMonotonicClock::new());
    let cache = FederationDocumentCache::<ToyJwks>::new(clock);
    let provider = FederationProviderId::new();
    let gen1 = make_key(provider, 1, 1);
    let gen2 = make_key(provider, 1, 2);

    cache
        .lookup(provider, || Some(gen1), |_current| fetch_success(gen1, "a"))
        .await
        .expect("seed at generation 1");

    // At generation 1 it is fresh and needs no network -- confirms the
    // seed actually landed before testing the generation-2 case.
    let still_gen1 = cache
        .lookup(provider, || Some(gen1), must_not_dispatch)
        .await;
    assert!(still_gen1.is_ok());

    let fetch_count = Arc::new(AtomicUsize::new(0));
    let fc = fetch_count.clone();
    let at_gen2 = cache
        .lookup(
            provider,
            || Some(gen2),
            move |_current| {
                let fc = fc.clone();
                async move {
                    fc.fetch_add(1, Ordering::SeqCst);
                    fetch_success(gen2, "a").await
                }
            },
        )
        .await;
    assert!(at_gen2.is_ok(), "generation 2 dispatches its own fetch");
    assert_eq!(
        fetch_count.load(Ordering::SeqCst),
        1,
        "the generation-1 entry was not served; a real fetch happened"
    );
}

#[tokio::test]
async fn a_fresh_entry_is_not_served_after_a_version_change_either() {
    let clock = Arc::new(MockMonotonicClock::new());
    let cache = FederationDocumentCache::<ToyJwks>::new(clock);
    let provider = FederationProviderId::new();
    let v1 = make_key(provider, 1, 1);
    let v2 = make_key(provider, 2, 1);

    cache
        .lookup(provider, || Some(v1), |_current| fetch_success(v1, "a"))
        .await
        .expect("seed at version 1");

    let fetch_count = Arc::new(AtomicUsize::new(0));
    let fc = fetch_count.clone();
    let at_v2 = cache
        .lookup(
            provider,
            || Some(v2),
            move |_current| {
                let fc = fc.clone();
                async move {
                    fc.fetch_add(1, Ordering::SeqCst);
                    fetch_success(v2, "a").await
                }
            },
        )
        .await;
    assert!(at_v2.is_ok());
    assert_eq!(fetch_count.load(Ordering::SeqCst), 1);
}

/// A caller whose current key differs from the active flight's does not
/// join it and gets its own flight instead -- and the superseded flight's
/// own leader does not clobber the new one when it finishes. Uses the same
/// scheduling seam as `two_concurrent_ordinary_lookups_...`: caller A's
/// fetch closure signals its own start (proof the generation-1 leadership
/// has committed) before blocking.
#[tokio::test]
async fn a_caller_at_generation_2_does_not_join_a_generation_1_flight_and_leads_its_own() {
    let clock: SharedMonotonicClock = Arc::new(MockMonotonicClock::new());
    let cache = Arc::new(FederationDocumentCache::<ToyJwks>::new(clock));
    let provider = FederationProviderId::new();
    let gen1 = make_key(provider, 1, 1);
    let gen2 = make_key(provider, 1, 2);

    let gen1_fetch_count = Arc::new(AtomicUsize::new(0));
    let (started_tx, started_rx) = tokio::sync::oneshot::channel::<()>();
    let (release_tx, release_rx) = tokio::sync::oneshot::channel::<()>();

    let cache_a = cache.clone();
    let gen1_fetch_count_a = gen1_fetch_count.clone();
    let task_a = tokio::spawn(async move {
        cache_a
            .lookup(
                provider,
                move || Some(gen1),
                move |_current| {
                    let gen1_fetch_count_a = gen1_fetch_count_a.clone();
                    async move {
                        gen1_fetch_count_a.fetch_add(1, Ordering::SeqCst);
                        started_tx.send(()).expect("test still waiting");
                        release_rx.await.expect("test still holding the release");
                        fetch_success(gen1, "a").await
                    }
                },
            )
            .await
    });

    started_rx
        .await
        .expect("generation-1 leader signalled its own start");

    // Caller B arrives at generation 2 while A's generation-1 flight is
    // still in the air. It must not join A's flight -- its own closure
    // must run.
    let gen2_fetch_count = Arc::new(AtomicUsize::new(0));
    let cache_b = cache.clone();
    let gen2_fetch_count_b = gen2_fetch_count.clone();
    let task_b = tokio::spawn(async move {
        cache_b
            .lookup(
                provider,
                move || Some(gen2),
                move |_current| {
                    let gen2_fetch_count_b = gen2_fetch_count_b.clone();
                    async move {
                        gen2_fetch_count_b.fetch_add(1, Ordering::SeqCst);
                        fetch_success(gen2, "b").await
                    }
                },
            )
            .await
    });

    let result_b = task_b.await.expect("task B did not panic");
    let (entry_b, _) = result_b.expect("B led and completed its own flight");
    assert_eq!(
        entry_b.document(),
        &ToyJwks {
            kids: vec!["b".to_string()]
        }
    );
    assert_eq!(gen2_fetch_count.load(Ordering::SeqCst), 1);

    release_tx.send(()).expect("A still awaiting release");
    let result_a = task_a.await.expect("task A did not panic");
    let (entry_a, _) = result_a.expect("A still gets its own fetched data back");
    assert_eq!(
        entry_a.document(),
        &ToyJwks {
            kids: vec!["a".to_string()]
        }
    );
    assert_eq!(gen1_fetch_count.load(Ordering::SeqCst), 1);

    // A's superseded flight must not have clobbered B's retained entry:
    // a generation-2 lookup now finds it fresh, no new fetch.
    let third = cache
        .lookup(provider, move || Some(gen2), must_not_dispatch)
        .await;
    let (entry_3, outcome_3) = third.expect("B's entry is retained");
    assert_eq!(outcome_3, CacheOutcome::Retain);
    assert_eq!(
        entry_3.document(),
        &ToyJwks {
            kids: vec!["b".to_string()]
        }
    );
}
