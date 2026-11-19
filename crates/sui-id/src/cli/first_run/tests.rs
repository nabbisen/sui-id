use super::*;

/// Every one of the eight `(explicit_config, config_exists, is_terminal)`
/// combinations, not just the three the dispatch's prose names --
/// `config_exists = true` must win regardless of the other two, and
/// that is itself two combinations worth asserting, not one.
#[test]
fn decide_covers_all_eight_cases() {
    let cases: &[(bool, bool, bool, Decision)] = &[
        (false, true, false, Decision::Proceed),
        (false, true, true, Decision::Proceed),
        (true, true, false, Decision::Proceed),
        (true, true, true, Decision::Proceed),
        (false, false, true, Decision::Prompt),
        (false, false, false, Decision::GuidanceAndExit),
        (true, false, false, Decision::GuidanceAndExit),
        (true, false, true, Decision::GuidanceAndExit),
    ];
    for (explicit, exists, terminal, expected) in cases {
        assert_eq!(
            decide(*explicit, *exists, *terminal),
            *expected,
            "decide(explicit_config={explicit}, config_exists={exists}, is_terminal={terminal})"
        );
    }
}
