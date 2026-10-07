use super::*;
use chrono::TimeZone;
use std::sync::Arc;
use sui_id_core::time::MockClock;
use sui_id_shared::ids::FederationProviderId;

fn at(y: i32, mo: u32, d: u32, h: u32, mi: u32, s: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(y, mo, d, h, mi, s).unwrap()
}

fn imf_fixdate(t: DateTime<Utc>) -> String {
    t.format("%a, %d %b %Y %H:%M:%S GMT").to_string()
}

// ---- freshness arithmetic: the boundary the dispatch said to write first ----

#[test]
fn equality_of_lifetime_and_current_age_is_stale_not_fresh() {
    let lifetime = Duration::from_secs(60);
    assert!(!is_fresh(Some(lifetime), Duration::from_secs(60)));
    assert!(is_fresh(Some(lifetime), Duration::from_secs(59)));
    assert!(!is_fresh(Some(lifetime), Duration::from_secs(61)));
}

#[test]
fn no_store_is_never_fresh_regardless_of_current_age() {
    assert!(!is_fresh(None, Duration::ZERO));
}

#[test]
fn current_age_only_grows_with_resident_time_never_with_the_wall_clock() {
    // current_age takes no wall-clock reading at all -- a regression
    // occurring between two freshness checks has no parameter to enter
    // through. It can only grow as `resident` grows.
    let initial = Duration::from_secs(10);
    let soon = current_age(initial, Duration::from_secs(5));
    let later = current_age(initial, Duration::from_secs(50));
    assert_eq!(soon, Duration::from_secs(15));
    assert_eq!(later, Duration::from_secs(60));
    assert!(later > soon);
}

#[test]
fn initial_current_age_is_the_larger_of_age_and_the_date_gap() {
    let now = at(2026, 1, 1, 0, 10, 0);
    // Age dominates.
    let date = now - chrono::Duration::seconds(30);
    assert_eq!(
        initial_current_age(Duration::from_secs(100), Some(date), now),
        Duration::from_secs(100)
    );
    // Date gap dominates.
    assert_eq!(
        initial_current_age(Duration::from_secs(5), Some(date), now),
        Duration::from_secs(30)
    );
    // Both absent/zero.
    assert_eq!(
        initial_current_age(Duration::ZERO, None, now),
        Duration::ZERO
    );
}

#[test]
fn initial_current_age_date_term_is_floored_at_zero_when_trusted_now_precedes_date() {
    let date = at(2026, 1, 1, 0, 0, 10);
    let rolled_back = at(2026, 1, 1, 0, 0, 0);
    // trusted_now is 10s *before* Date -- a naive signed subtraction would
    // be negative. The floor means this cannot read as "fresher than Age
    // alone already says", which is what "clock regression expires the
    // entry [on schedule, same as always]" rests on: this term can only
    // ever be pushed toward zero, never below it.
    assert_eq!(
        initial_current_age(Duration::from_secs(3), Some(date), rolled_back),
        Duration::from_secs(3)
    );
}

// ---- Age ----

#[test]
fn age_absent_is_zero() {
    assert_eq!(validate_age(&[]).unwrap(), Duration::ZERO);
}

#[test]
fn age_at_u32_max_is_accepted_one_past_rejects() {
    let at_max = u32::MAX.to_string();
    assert_eq!(
        validate_age(&[&at_max]).unwrap(),
        Duration::from_secs(u32::MAX as u64)
    );
    let one_past = (u32::MAX as u64 + 1).to_string();
    assert_eq!(
        validate_age(&[&one_past]),
        Err(CacheValidationError::AgeOverflow)
    );
}

#[test]
fn two_age_lines_reject() {
    assert_eq!(
        validate_age(&["1", "2"]),
        Err(CacheValidationError::DuplicateField("Age"))
    );
}

#[test]
fn malformed_age_rejects() {
    assert_eq!(
        validate_age(&["not-a-number"]),
        Err(CacheValidationError::MalformedAge)
    );
    assert_eq!(
        validate_age(&["+5"]),
        Err(CacheValidationError::MalformedAge)
    );
}

// ---- Date ----

#[test]
fn date_at_trusted_now_plus_60s_is_accepted_one_past_rejects() {
    let trusted_now = at(2026, 1, 1, 0, 0, 0);
    let at_60 = imf_fixdate(trusted_now + chrono::Duration::seconds(60));
    assert!(validate_date(&[&at_60], trusted_now).is_ok());

    let at_61 = imf_fixdate(trusted_now + chrono::Duration::seconds(61));
    assert_eq!(
        validate_date(&[&at_61], trusted_now),
        Err(CacheValidationError::DateInFuture)
    );
}

#[test]
fn malformed_date_rejects() {
    let trusted_now = at(2026, 1, 1, 0, 0, 0);
    assert_eq!(
        validate_date(&["not a date"], trusted_now),
        Err(CacheValidationError::MalformedDate)
    );
    // Obsolete RFC 850 form, not IMF-fixdate.
    assert_eq!(
        validate_date(&["Sunday, 01-Jan-26 00:00:00 GMT"], trusted_now),
        Err(CacheValidationError::MalformedDate)
    );
}

#[test]
fn two_date_lines_reject() {
    let trusted_now = at(2026, 1, 1, 0, 0, 0);
    let d = imf_fixdate(trusted_now);
    assert_eq!(
        validate_date(&[&d, &d], trusted_now),
        Err(CacheValidationError::DuplicateField("Date"))
    );
}

// ---- ETag ----

#[test]
fn etag_at_256_bytes_is_accepted_257_rejects() {
    let at_256 = format!("\"{}\"", "a".repeat(256));
    assert!(validate_etag(&[&at_256]).unwrap().is_some());

    let at_257 = format!("\"{}\"", "a".repeat(257));
    assert_eq!(
        validate_etag(&[&at_257]),
        Err(CacheValidationError::EtagTooLarge)
    );
}

#[test]
fn weak_etag_is_accepted() {
    let weak = "W/\"abc\"";
    assert_eq!(validate_etag(&[weak]).unwrap(), Some(weak.to_string()));
}

#[test]
fn malformed_etag_rejects() {
    assert_eq!(
        validate_etag(&["abc"]),
        Err(CacheValidationError::MalformedEtag)
    );
    assert_eq!(
        validate_etag(&["\"abc"]),
        Err(CacheValidationError::MalformedEtag)
    );
    // A control character inside the quotes.
    assert_eq!(
        validate_etag(&["\"ab\tc\""]),
        Err(CacheValidationError::MalformedEtag)
    );
}

#[test]
fn two_etag_lines_reject() {
    assert_eq!(
        validate_etag(&["\"a\"", "\"b\""]),
        Err(CacheValidationError::DuplicateField("ETag"))
    );
}

// ---- Content-Type multiplicity ----

#[test]
fn two_content_type_lines_reject() {
    assert_eq!(
        validate_content_type_multiplicity(&["application/json", "application/json"]),
        Err(CacheValidationError::DuplicateField("Content-Type"))
    );
    assert!(validate_content_type_multiplicity(&["application/json"]).is_ok());
    assert!(validate_content_type_multiplicity(&[]).is_ok());
}

// ---- Cache-Control: directive set, combination, conflicts ----

#[test]
fn two_cache_control_lines_combine() {
    let d = parse_cache_control(&["no-cache", "must-revalidate"]).unwrap();
    assert!(d.no_cache);
    assert!(d.must_revalidate);
}

#[test]
fn directive_names_are_case_insensitive() {
    let d = parse_cache_control(&["NO-CACHE, Must-Revalidate"]).unwrap();
    assert!(d.no_cache);
    assert!(d.must_revalidate);
}

#[test]
fn unknown_directives_are_ignored() {
    let d = parse_cache_control(&["foo=bar, no-cache, private"]).unwrap();
    assert!(d.no_cache);
}

#[test]
fn a_recognised_directive_occurring_twice_rejects() {
    assert_eq!(
        parse_cache_control(&["no-store, no-store"]),
        Err(CacheValidationError::DuplicateDirective("no-store"))
    );
    assert_eq!(
        parse_cache_control(&["max-age=1, max-age=2"]),
        Err(CacheValidationError::DuplicateDirective("max-age"))
    );
}

#[test]
fn no_store_with_max_age_parses_and_no_store_governs() {
    // RFC 096's review (4b -> 4c): no pair in this closed set is
    // unsatisfiable, so there is no "conflicting directives" rejection --
    // only duplicates reject. `no-store` alongside `max-age` parses fine;
    // `no-store` is simply the strictest directive present and governs,
    // which `lifetime()` already guarantees by checking it first.
    let d = parse_cache_control(&["no-store, max-age=60"]).unwrap();
    assert!(d.no_store);
    assert_eq!(d.max_age, Some(Duration::from_secs(60)));
    assert_eq!(lifetime(&d, CacheKind::Discovery), None);

    // Also true of s-maxage and must-revalidate alongside no-store.
    assert!(
        parse_cache_control(&["no-store, s-maxage=60"])
            .unwrap()
            .no_store
    );
    assert!(
        parse_cache_control(&["no-store, must-revalidate"])
            .unwrap()
            .no_store
    );
    // no-store alone, or with no-cache, parses too.
    assert!(parse_cache_control(&["no-store"]).unwrap().no_store);
    assert!(
        parse_cache_control(&["no-store, no-cache"])
            .unwrap()
            .no_store
    );
}

#[test]
fn quoted_delta_seconds_rejects() {
    assert_eq!(
        parse_cache_control(&["max-age=\"60\""]),
        Err(CacheValidationError::QuotedDeltaSeconds("max-age"))
    );
}

#[test]
fn non_canonical_delta_seconds_rejects() {
    assert_eq!(
        parse_cache_control(&["max-age=007"]),
        Err(CacheValidationError::NonCanonicalDeltaSeconds("max-age"))
    );
    assert_eq!(
        parse_cache_control(&["max-age=+60"]),
        Err(CacheValidationError::NonCanonicalDeltaSeconds("max-age"))
    );
    assert_eq!(
        parse_cache_control(&["max-age=1.5"]),
        Err(CacheValidationError::NonCanonicalDeltaSeconds("max-age"))
    );
    // "0" itself is canonical.
    assert_eq!(
        parse_cache_control(&["max-age=0"]).unwrap().max_age,
        Some(Duration::ZERO)
    );
}

#[test]
fn delta_seconds_overflow_rejects() {
    assert_eq!(
        parse_cache_control(&["max-age=99999999999999999999999999999999"]),
        Err(CacheValidationError::DeltaSecondsOverflow("max-age"))
    );
}

#[test]
fn invalid_list_syntax_rejects() {
    // Unterminated quoted-string.
    assert_eq!(
        parse_cache_control(&["max-age=\"60"]),
        Err(CacheValidationError::InvalidListSyntax)
    );
}

#[test]
fn obsolete_line_folding_rejects() {
    assert_eq!(
        parse_cache_control(&["no-cache\r\n max-age=60"]),
        Err(CacheValidationError::ObsoleteLineFolding)
    );
}

#[test]
fn oversized_cache_control_rejects() {
    let huge = "a".repeat(MAX_CACHE_CONTROL_LEN + 1);
    assert_eq!(
        parse_cache_control(&[&huge]),
        Err(CacheValidationError::CacheControlOversized)
    );
}

#[test]
fn s_maxage_is_accepted_and_has_no_effect() {
    let d = parse_cache_control(&["s-maxage=120, max-age=50"]).unwrap();
    assert_eq!(d.s_maxage, Some(Duration::from_secs(120)));
    assert_eq!(lifetime(&d, CacheKind::Jwks), Some(Duration::from_secs(50)));
}

#[test]
fn malformed_s_maxage_still_rejects() {
    assert_eq!(
        parse_cache_control(&["s-maxage=abc"]),
        Err(CacheValidationError::NonCanonicalDeltaSeconds("s-maxage"))
    );
}

#[test]
fn must_revalidate_is_recorded_and_changes_nothing() {
    let d = parse_cache_control(&["must-revalidate, max-age=50"]).unwrap();
    assert!(d.must_revalidate);
    assert_eq!(lifetime(&d, CacheKind::Jwks), Some(Duration::from_secs(50)));
}

// ---- lifetime() precedence table ----

#[test]
fn no_store_has_no_lifetime_at_all() {
    let d = parse_cache_control(&["no-store"]).unwrap();
    assert_eq!(lifetime(&d, CacheKind::Discovery), None);
}

#[test]
fn no_cache_floors_freshness_at_zero_even_with_max_age_present() {
    let d = parse_cache_control(&["no-cache, max-age=500"]).unwrap();
    assert_eq!(lifetime(&d, CacheKind::Discovery), Some(Duration::ZERO));
}

#[test]
fn max_age_zero_is_zero_freshness() {
    let d = parse_cache_control(&["max-age=0"]).unwrap();
    assert_eq!(lifetime(&d, CacheKind::Discovery), Some(Duration::ZERO));
}

#[test]
fn max_age_clamps_at_the_per_cache_maximum_rather_than_rejecting() {
    let max = CacheKind::Discovery.max_freshness();
    let at_max = parse_cache_control(&[&format!("max-age={}", max.as_secs())]).unwrap();
    assert_eq!(lifetime(&at_max, CacheKind::Discovery), Some(max));

    let one_past = parse_cache_control(&[&format!("max-age={}", max.as_secs() + 1)]).unwrap();
    assert_eq!(lifetime(&one_past, CacheKind::Discovery), Some(max));
}

#[test]
fn max_age_below_the_default_is_not_raised() {
    let default = CacheKind::Discovery.default_freshness();
    let small = Duration::from_secs(10);
    assert!(small < default);
    let d = parse_cache_control(&["max-age=10"]).unwrap();
    assert_eq!(lifetime(&d, CacheKind::Discovery), Some(small));
}

#[test]
fn absent_max_age_is_the_table_default_unless_no_cache() {
    let empty = parse_cache_control(&[]).unwrap();
    assert_eq!(
        lifetime(&empty, CacheKind::Discovery),
        Some(CacheKind::Discovery.default_freshness())
    );
    assert_eq!(
        lifetime(&empty, CacheKind::Jwks),
        Some(CacheKind::Jwks.default_freshness())
    );

    let no_cache = parse_cache_control(&["no-cache"]).unwrap();
    assert_eq!(
        lifetime(&no_cache, CacheKind::Discovery),
        Some(Duration::ZERO)
    );
}

// ---- the top-level orchestrator and its one clock read ----

#[test]
fn validate_response_injects_the_clock_exactly_where_date_needs_it() {
    let trusted_now = at(2026, 6, 1, 12, 0, 0);
    let clock: SharedClock = Arc::new(MockClock::at(trusted_now));
    let date = imf_fixdate(trusted_now - chrono::Duration::seconds(5));

    let result = validate_response(
        &["max-age=60"],
        &["2"],
        &[&date],
        &["\"v1\""],
        &["application/json"],
        CacheKind::Jwks,
        &clock,
    )
    .unwrap();

    assert_eq!(result.lifetime, Some(Duration::from_secs(60)));
    // Age (2) vs. the Date gap (5): the larger wins.
    assert_eq!(result.initial_current_age, Duration::from_secs(5));
    assert_eq!(result.etag, Some("\"v1\"".to_string()));
}

#[test]
fn validate_response_rejects_a_date_the_mock_clock_says_is_too_far_ahead() {
    let trusted_now = at(2026, 6, 1, 12, 0, 0);
    let clock: SharedClock = Arc::new(MockClock::at(trusted_now));
    let too_far = imf_fixdate(trusted_now + chrono::Duration::seconds(61));

    let result = validate_response(
        &["max-age=60"],
        &[],
        &[&too_far],
        &[],
        &[],
        CacheKind::Jwks,
        &clock,
    );

    assert_eq!(result, Err(CacheValidationError::DateInFuture));
}

// ---- stage 4c: store_fresh / RetainedEntry / apply_304 ----

#[derive(Debug, Clone, PartialEq, Eq)]
struct ToyDoc(String);

fn validate_toy(body: &[u8]) -> Result<ToyDoc, &'static str> {
    let s = std::str::from_utf8(body).map_err(|_| "not utf8")?;
    if s == "invalid" {
        return Err("rejected for test purposes");
    }
    Ok(ToyDoc(s.to_string()))
}

fn key(provider: FederationProviderId, version: u64, generation: u64) -> CacheKey {
    CacheKey::new(
        provider,
        ProviderVersion(version),
        ActivationGeneration(generation),
    )
}

fn store_toy(
    k: CacheKey,
    headers: ResponseHeaders<'_>,
    body: &[u8],
    clock: &SharedClock,
) -> Result<(RetainedEntry<ToyDoc>, CacheOutcome), Store200Error<&'static str>> {
    store_fresh(k, headers, CacheKind::Jwks, body, validate_toy, clock)
}

fn base_entry(clock: &SharedClock, provider: FederationProviderId) -> RetainedEntry<ToyDoc> {
    let (entry, _) = store_toy(
        key(provider, 1, 1),
        ResponseHeaders {
            cache_control: &["max-age=60"],
            etag: &["\"v1\""],
            content_type: &["application/json"],
            ..Default::default()
        },
        b"good",
        clock,
    )
    .unwrap();
    entry
}

#[test]
fn an_unchanged_etag_does_not_exempt_an_invalid_body_from_validation() {
    let clock: SharedClock = Arc::new(MockClock::at(at(2026, 1, 1, 0, 0, 0)));
    let provider = FederationProviderId::new();

    let (old, _) = store_toy(
        key(provider, 1, 1),
        ResponseHeaders {
            cache_control: &["max-age=60"],
            etag: &["\"v1\""],
            content_type: &["application/json"],
            ..Default::default()
        },
        b"good",
        &clock,
    )
    .unwrap();
    assert_eq!(old.document(), &ToyDoc("good".to_string()));

    // Same ETag "v1", but a now-invalid body: the matching ETag is not a
    // licence to skip validation, so this must still be rejected.
    let result = store_toy(
        key(provider, 1, 1),
        ResponseHeaders {
            cache_control: &["max-age=60"],
            etag: &["\"v1\""],
            content_type: &["application/json"],
            ..Default::default()
        },
        b"invalid",
        &clock,
    );
    assert!(matches!(result, Err(Store200Error::Semantic(_))));
    // `old` was never passed to the failing call, so there is no code
    // path through which it could have been touched.
    assert_eq!(old.document(), &ToyDoc("good".to_string()));
}

#[test]
fn store_fresh_header_failure_is_reported_distinctly_from_a_semantic_one() {
    let clock: SharedClock = Arc::new(MockClock::at(at(2026, 1, 1, 0, 0, 0)));
    let provider = FederationProviderId::new();
    let result = store_toy(
        key(provider, 1, 1),
        ResponseHeaders {
            cache_control: &["max-age=abc"],
            ..Default::default()
        },
        b"good",
        &clock,
    );
    assert!(matches!(result, Err(Store200Error::Header(_))));
}

#[test]
fn store_fresh_no_store_outcome_is_use_once_then_evict_and_retains_no_etag() {
    let clock: SharedClock = Arc::new(MockClock::at(at(2026, 1, 1, 0, 0, 0)));
    let provider = FederationProviderId::new();
    let (entry, outcome) = store_toy(
        key(provider, 1, 1),
        ResponseHeaders {
            cache_control: &["no-store"],
            etag: &["\"v1\""],
            content_type: &["application/json"],
            ..Default::default()
        },
        b"good",
        &clock,
    )
    .unwrap();
    assert_eq!(outcome, CacheOutcome::UseOnceThenEvict);
    assert_eq!(entry.lifetime(), None);
    assert_eq!(entry.etag(), None);
}

#[test]
fn a_304_with_a_returned_etag_differing_by_one_byte_rejects() {
    let clock: SharedClock = Arc::new(MockClock::at(at(2026, 1, 1, 0, 0, 0)));
    let entry = base_entry(&clock, FederationProviderId::new());
    let result = apply_304(
        &entry,
        entry.key(),
        Some("\"v2\""),
        RevalidationHeaders::default(),
        CacheKind::Jwks,
        &clock,
    );
    assert_eq!(result, Err(RevalidationError::EtagMismatch));
}

#[test]
fn a_304_for_a_different_provider_version_rejects() {
    let clock: SharedClock = Arc::new(MockClock::at(at(2026, 1, 1, 0, 0, 0)));
    let provider = FederationProviderId::new();
    let entry = base_entry(&clock, provider);
    let result = apply_304(
        &entry,
        key(provider, 2, 1),
        Some("\"v1\""),
        RevalidationHeaders::default(),
        CacheKind::Jwks,
        &clock,
    );
    assert_eq!(result, Err(RevalidationError::KeyMismatch));
}

#[test]
fn a_304_for_a_different_activation_generation_rejects() {
    let clock: SharedClock = Arc::new(MockClock::at(at(2026, 1, 1, 0, 0, 0)));
    let provider = FederationProviderId::new();
    let entry = base_entry(&clock, provider);
    let result = apply_304(
        &entry,
        key(provider, 1, 2),
        Some("\"v1\""),
        RevalidationHeaders::default(),
        CacheKind::Jwks,
        &clock,
    );
    assert_eq!(result, Err(RevalidationError::KeyMismatch));
}

#[test]
fn a_304_after_no_store_rejects() {
    let clock: SharedClock = Arc::new(MockClock::at(at(2026, 1, 1, 0, 0, 0)));
    let provider = FederationProviderId::new();
    let (entry, outcome) = store_toy(
        key(provider, 1, 1),
        ResponseHeaders {
            cache_control: &["no-store"],
            etag: &["\"v1\""],
            content_type: &["application/json"],
            ..Default::default()
        },
        b"good",
        &clock,
    )
    .unwrap();
    assert_eq!(outcome, CacheOutcome::UseOnceThenEvict);

    let result = apply_304(
        &entry,
        entry.key(),
        Some("\"v1\""),
        RevalidationHeaders::default(),
        CacheKind::Jwks,
        &clock,
    );
    assert_eq!(result, Err(RevalidationError::EntryAlreadyNoStore));
}

#[test]
fn a_304_against_an_entry_with_no_etag_at_all_rejects() {
    let clock: SharedClock = Arc::new(MockClock::at(at(2026, 1, 1, 0, 0, 0)));
    let provider = FederationProviderId::new();
    let (entry, _) = store_toy(
        key(provider, 1, 1),
        ResponseHeaders {
            cache_control: &["max-age=60"],
            content_type: &["application/json"],
            ..Default::default()
        },
        b"good",
        &clock,
    )
    .unwrap();
    assert_eq!(entry.etag(), None);

    let result = apply_304(
        &entry,
        entry.key(),
        None,
        RevalidationHeaders::default(),
        CacheKind::Jwks,
        &clock,
    );
    assert_eq!(result, Err(RevalidationError::NoValidatorToRevalidate));
}

#[test]
fn a_304_omitting_age_becomes_zero_rather_than_inheriting() {
    let clock: SharedClock = Arc::new(MockClock::at(at(2026, 1, 1, 0, 0, 0)));
    let provider = FederationProviderId::new();
    let (entry, _) = store_toy(
        key(provider, 1, 1),
        ResponseHeaders {
            cache_control: &["max-age=60"],
            age: &["50"],
            etag: &["\"v1\""],
            content_type: &["application/json"],
            ..Default::default()
        },
        b"good",
        &clock,
    )
    .unwrap();
    assert_eq!(entry.initial_current_age(), Duration::from_secs(50));

    let (updated, outcome) = apply_304(
        &entry,
        entry.key(),
        Some("\"v1\""),
        RevalidationHeaders::default(),
        CacheKind::Jwks,
        &clock,
    )
    .unwrap();
    assert_eq!(outcome, CacheOutcome::Retain);
    // Age absent on the 304 -> zero, not the retained 50s.
    assert_eq!(updated.initial_current_age(), Duration::ZERO);
}

#[test]
fn a_304_omitting_date_becomes_the_receipt_time() {
    let clock: SharedClock = Arc::new(MockClock::at(at(2026, 1, 1, 0, 0, 0)));
    let provider = FederationProviderId::new();
    let old_date = imf_fixdate(at(2025, 1, 1, 0, 0, 0));
    let (entry, _) = store_toy(
        key(provider, 1, 1),
        ResponseHeaders {
            cache_control: &["max-age=60"],
            date: &[&old_date],
            etag: &["\"v1\""],
            content_type: &["application/json"],
            ..Default::default()
        },
        b"good",
        &clock,
    )
    .unwrap();
    assert!(entry.initial_current_age() > Duration::ZERO);

    let (updated, _) = apply_304(
        &entry,
        entry.key(),
        Some("\"v1\""),
        RevalidationHeaders::default(),
        CacheKind::Jwks,
        &clock,
    )
    .unwrap();
    // Date absent on the 304 -> this 304's own receipt time, so its gap
    // against trusted_now is zero -- not the old Date's now-large gap.
    assert_eq!(updated.initial_current_age(), Duration::ZERO);
}

#[test]
fn no_store_on_a_304_permits_one_operation_then_signals_eviction() {
    let clock: SharedClock = Arc::new(MockClock::at(at(2026, 1, 1, 0, 0, 0)));
    let entry = base_entry(&clock, FederationProviderId::new());
    let (updated, outcome) = apply_304(
        &entry,
        entry.key(),
        Some("\"v1\""),
        RevalidationHeaders {
            cache_control: &["no-store"],
            ..Default::default()
        },
        CacheKind::Jwks,
        &clock,
    )
    .unwrap();
    assert_eq!(outcome, CacheOutcome::UseOnceThenEvict);
    assert_eq!(updated.document(), entry.document());
    assert_eq!(updated.etag(), None);
}

#[test]
fn no_cache_or_max_age_zero_on_a_304_allows_the_operation_but_expires_immediately() {
    let clock: SharedClock = Arc::new(MockClock::at(at(2026, 1, 1, 0, 0, 0)));
    let entry = base_entry(&clock, FederationProviderId::new());
    let (updated, outcome) = apply_304(
        &entry,
        entry.key(),
        Some("\"v1\""),
        RevalidationHeaders {
            cache_control: &["no-cache"],
            ..Default::default()
        },
        CacheKind::Jwks,
        &clock,
    )
    .unwrap();
    assert_eq!(outcome, CacheOutcome::UseAndExpire);
    assert_eq!(updated.lifetime(), Some(Duration::ZERO));
}

#[test]
fn cache_control_present_on_304_replaces_absent_inherits() {
    let clock: SharedClock = Arc::new(MockClock::at(at(2026, 1, 1, 0, 0, 0)));
    let entry = base_entry(&clock, FederationProviderId::new()); // max-age=60
    assert_eq!(entry.directives().max_age, Some(Duration::from_secs(60)));

    let (inherited, _) = apply_304(
        &entry,
        entry.key(),
        Some("\"v1\""),
        RevalidationHeaders::default(),
        CacheKind::Jwks,
        &clock,
    )
    .unwrap();
    assert_eq!(
        inherited.directives().max_age,
        Some(Duration::from_secs(60))
    );

    let (replaced, _) = apply_304(
        &entry,
        entry.key(),
        Some("\"v1\""),
        RevalidationHeaders {
            cache_control: &["max-age=10"],
            ..Default::default()
        },
        CacheKind::Jwks,
        &clock,
    )
    .unwrap();
    assert_eq!(replaced.directives().max_age, Some(Duration::from_secs(10)));
}

#[test]
fn clock_regression_since_the_entrys_last_receipt_forces_it_stale() {
    let later: SharedClock = Arc::new(MockClock::at(at(2026, 1, 2, 0, 0, 0)));
    let entry = base_entry(&later, FederationProviderId::new());

    // The 304's own clock reading is *before* the entry's last receipt.
    let rolled_back: SharedClock = Arc::new(MockClock::at(at(2026, 1, 1, 0, 0, 0)));
    let (updated, outcome) = apply_304(
        &entry,
        entry.key(),
        Some("\"v1\""),
        RevalidationHeaders {
            cache_control: &["max-age=600"],
            ..Default::default()
        },
        CacheKind::Jwks,
        &rolled_back,
    )
    .unwrap();

    assert_eq!(outcome, CacheOutcome::ClockRegressedExpired);
    // Forced to zero regardless of the otherwise-generous max-age=600 the
    // 304 itself presented.
    assert_eq!(updated.lifetime(), Some(Duration::ZERO));
}

#[test]
fn clock_regression_with_a_no_store_304_still_reports_the_stronger_eviction() {
    let later: SharedClock = Arc::new(MockClock::at(at(2026, 1, 2, 0, 0, 0)));
    let entry = base_entry(&later, FederationProviderId::new());

    let rolled_back: SharedClock = Arc::new(MockClock::at(at(2026, 1, 1, 0, 0, 0)));
    let (_, outcome) = apply_304(
        &entry,
        entry.key(),
        Some("\"v1\""),
        RevalidationHeaders {
            cache_control: &["no-store"],
            ..Default::default()
        },
        CacheKind::Jwks,
        &rolled_back,
    )
    .unwrap();
    assert_eq!(outcome, CacheOutcome::UseOnceThenEvict);
}

#[test]
fn a_304_never_takes_a_body_parameter_so_it_cannot_validate_a_different_one() {
    // Structural, not behavioural: `apply_304`'s signature has no body
    // parameter at all, so there is no argument through which a 304
    // could supply a different document than the one already retained.
    let clock: SharedClock = Arc::new(MockClock::at(at(2026, 1, 1, 0, 0, 0)));
    let entry = base_entry(&clock, FederationProviderId::new());
    let (updated, _) = apply_304(
        &entry,
        entry.key(),
        Some("\"v1\""),
        RevalidationHeaders::default(),
        CacheKind::Jwks,
        &clock,
    )
    .unwrap();
    assert_eq!(updated.document(), entry.document());
}
