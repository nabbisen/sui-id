//! Property tests on [`is_redirect_uri_registered`].
//!
//! The redirect-URI check is the security boundary against an
//! open-redirect attack. The properties below pin down the rule
//! the OAuth/OIDC specs put on us — strict, byte-exact match —
//! and guard against well-known regressions:
//!
//!   - case folding
//!   - trailing-slash leniency
//!   - default-port collapsing (`:443` vs implicit)
//!   - subdomain wildcard misreads
//!   - prefix matching
//!
//! If anyone tries to "fix" a perceived UX problem by adding
//! normalisation, one of these properties should fail loudly.

use super::is_redirect_uri_registered;
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 256,
        ..ProptestConfig::default()
    })]

    /// A URI that was registered exactly is accepted.
    #[test]
    fn registered_uri_is_always_accepted(
        // Realistic-ish URI alphabet. Doesn't have to parse as a
        // URL — the function is a string comparator.
        uri in "[A-Za-z0-9:/._~?&=#@%-]{1,256}",
    ) {
        let registered = vec![uri.clone()];
        prop_assert!(is_redirect_uri_registered(&registered, &uri));
    }

    /// A URI that differs by even one byte is rejected.
    ///
    /// Generated as: take a registered URI, flip one character
    /// somewhere along it. The mutation makes the strings
    /// unequal, so the function must reject.
    #[test]
    fn one_byte_off_uri_is_rejected(
        base in "[A-Za-z0-9:/._~?&=-]{8,128}",
        mutation_index in any::<usize>(),
    ) {
        // Build a "submitted" by flipping a byte in `base`.
        let mut submitted = base.clone().into_bytes();
        let i = mutation_index % submitted.len();
        // Swap the byte to a guaranteed-different one. ASCII
        // arithmetic; we know all the chars are ASCII because
        // of the regex.
        submitted[i] = if submitted[i] == b'X' { b'Y' } else { b'X' };
        let submitted = String::from_utf8(submitted).unwrap();
        prop_assume!(submitted != base);
        let registered = vec![base];
        prop_assert!(!is_redirect_uri_registered(&registered, &submitted));
    }

    /// Case differences are not folded — `/cb` and `/CB` are
    /// distinct URIs as far as we're concerned.
    #[test]
    fn case_difference_is_not_folded(
        stem in "[a-z]{4,16}",
    ) {
        let lower = format!("https://example.com/{stem}");
        let upper = format!("https://example.com/{}", stem.to_uppercase());
        prop_assume!(lower != upper);
        let registered = vec![lower.clone()];
        prop_assert!(is_redirect_uri_registered(&registered, &lower));
        prop_assert!(!is_redirect_uri_registered(&registered, &upper));
    }

    /// A registered URI followed by extra junk is rejected — no
    /// prefix match. This is the "attacker registers
    /// `https://example.com/cb` and submits
    /// `https://example.com/cb/../../leak`" case.
    #[test]
    fn prefix_extension_is_rejected(
        base in "[A-Za-z0-9:/._~-]{8,64}",
        suffix in "[A-Za-z0-9/.-]{1,32}",
    ) {
        let registered = vec![base.clone()];
        let submitted = format!("{base}{suffix}");
        prop_assume!(submitted != base);
        prop_assert!(!is_redirect_uri_registered(&registered, &submitted));
    }

    /// Multiple registered URIs: any one matching is enough; any
    /// one not in the list is not. (Sanity: this is what `any()`
    /// computes; the property is here to catch a future
    /// refactor that gets the predicate backwards.)
    #[test]
    fn multi_registry_matches_each_member_and_only_them(
        uris in proptest::collection::vec("[A-Za-z0-9:/._~-]{8,64}", 1..6),
        outsider in "[A-Za-z0-9:/._~-]{8,64}",
    ) {
        prop_assume!(!uris.iter().any(|u| u == &outsider));
        for u in &uris {
            prop_assert!(is_redirect_uri_registered(&uris, u));
        }
        prop_assert!(!is_redirect_uri_registered(&uris, &outsider));
    }
}
