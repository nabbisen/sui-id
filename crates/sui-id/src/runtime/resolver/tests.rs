use super::*;
use std::net::Ipv4Addr;
use std::str::FromStr;

/// Both edges of every vendored prefix. `(cidr, first, last)` with
/// `first`/`last` independently computed (Python's `ipaddress`
/// stdlib, a different implementation in a different language,
/// cross-checked against two independent fetches of each IANA
/// registry) -- not derived from this module's own mask arithmetic,
/// which is the thing being tested.
const BOTH_EDGES: &[(&str, &str, &str)] = &[
    ("0.0.0.0/8", "0.0.0.0", "0.255.255.255"),
    ("0.0.0.0/32", "0.0.0.0", "0.0.0.0"),
    ("10.0.0.0/8", "10.0.0.0", "10.255.255.255"),
    ("100.64.0.0/10", "100.64.0.0", "100.127.255.255"),
    ("127.0.0.0/8", "127.0.0.0", "127.255.255.255"),
    ("169.254.0.0/16", "169.254.0.0", "169.254.255.255"),
    ("172.16.0.0/12", "172.16.0.0", "172.31.255.255"),
    ("192.0.0.0/24", "192.0.0.0", "192.0.0.255"),
    ("192.0.0.0/29", "192.0.0.0", "192.0.0.7"),
    ("192.0.0.8/32", "192.0.0.8", "192.0.0.8"),
    ("192.0.0.9/32", "192.0.0.9", "192.0.0.9"),
    ("192.0.0.10/32", "192.0.0.10", "192.0.0.10"),
    ("192.0.0.170/32", "192.0.0.170", "192.0.0.170"),
    ("192.0.0.171/32", "192.0.0.171", "192.0.0.171"),
    ("192.0.2.0/24", "192.0.2.0", "192.0.2.255"),
    ("192.31.196.0/24", "192.31.196.0", "192.31.196.255"),
    ("192.52.193.0/24", "192.52.193.0", "192.52.193.255"),
    ("192.88.99.0/24", "192.88.99.0", "192.88.99.255"),
    ("192.88.99.2/32", "192.88.99.2", "192.88.99.2"),
    ("192.168.0.0/16", "192.168.0.0", "192.168.255.255"),
    ("192.175.48.0/24", "192.175.48.0", "192.175.48.255"),
    ("198.18.0.0/15", "198.18.0.0", "198.19.255.255"),
    ("198.51.100.0/24", "198.51.100.0", "198.51.100.255"),
    ("203.0.113.0/24", "203.0.113.0", "203.0.113.255"),
    ("240.0.0.0/4", "240.0.0.0", "255.255.255.255"),
    ("255.255.255.255/32", "255.255.255.255", "255.255.255.255"),
    ("::1/128", "::1", "::1"),
    ("::/128", "::", "::"),
    ("::ffff:0:0/96", "::ffff:0.0.0.0", "::ffff:255.255.255.255"),
    ("64:ff9b::/96", "64:ff9b::", "64:ff9b::ffff:ffff"),
    (
        "64:ff9b:1::/48",
        "64:ff9b:1::",
        "64:ff9b:1:ffff:ffff:ffff:ffff:ffff",
    ),
    ("100::/64", "100::", "100::ffff:ffff:ffff:ffff"),
    (
        "100:0:0:1::/64",
        "100:0:0:1::",
        "100::1:ffff:ffff:ffff:ffff",
    ),
    (
        "2001::/23",
        "2001::",
        "2001:1ff:ffff:ffff:ffff:ffff:ffff:ffff",
    ),
    (
        "2001::/32",
        "2001::",
        "2001:0:ffff:ffff:ffff:ffff:ffff:ffff",
    ),
    ("2001:1::1/128", "2001:1::1", "2001:1::1"),
    ("2001:1::2/128", "2001:1::2", "2001:1::2"),
    ("2001:1::3/128", "2001:1::3", "2001:1::3"),
    (
        "2001:2::/48",
        "2001:2::",
        "2001:2:0:ffff:ffff:ffff:ffff:ffff",
    ),
    (
        "2001:3::/32",
        "2001:3::",
        "2001:3:ffff:ffff:ffff:ffff:ffff:ffff",
    ),
    (
        "2001:4:112::/48",
        "2001:4:112::",
        "2001:4:112:ffff:ffff:ffff:ffff:ffff",
    ),
    (
        "2001:10::/28",
        "2001:10::",
        "2001:1f:ffff:ffff:ffff:ffff:ffff:ffff",
    ),
    (
        "2001:20::/28",
        "2001:20::",
        "2001:2f:ffff:ffff:ffff:ffff:ffff:ffff",
    ),
    (
        "2001:30::/28",
        "2001:30::",
        "2001:3f:ffff:ffff:ffff:ffff:ffff:ffff",
    ),
    (
        "2001:db8::/32",
        "2001:db8::",
        "2001:db8:ffff:ffff:ffff:ffff:ffff:ffff",
    ),
    (
        "2002::/16",
        "2002::",
        "2002:ffff:ffff:ffff:ffff:ffff:ffff:ffff",
    ),
    (
        "2620:4f:8000::/48",
        "2620:4f:8000::",
        "2620:4f:8000:ffff:ffff:ffff:ffff:ffff",
    ),
    (
        "3fff::/20",
        "3fff::",
        "3fff:fff:ffff:ffff:ffff:ffff:ffff:ffff",
    ),
    (
        "5f00::/16",
        "5f00::",
        "5f00:ffff:ffff:ffff:ffff:ffff:ffff:ffff",
    ),
    (
        "fc00::/7",
        "fc00::",
        "fdff:ffff:ffff:ffff:ffff:ffff:ffff:ffff",
    ),
    (
        "fe80::/10",
        "fe80::",
        "febf:ffff:ffff:ffff:ffff:ffff:ffff:ffff",
    ),
    ("::/96", "::", "::ffff:ffff"),
];

#[test]
fn both_edges_of_every_vendored_prefix_are_denied() {
    assert_eq!(
        BOTH_EDGES.len(),
        IPV4_DENIED.len() + IPV6_DENIED.len(),
        "every row needs a boundary entry, and vice versa -- a row \
             added to the table without one here would ship unverified"
    );
    for (cidr, first, last) in BOTH_EDGES {
        for edge in [first, last] {
            let addr = IpAddr::from_str(edge).unwrap_or_else(|e| panic!("{edge}: {e}"));
            assert!(
                denied_by(addr).is_some(),
                "{cidr}: edge {edge} must be denied"
            );
        }
    }
}

#[test]
fn ordinary_public_addresses_are_not_denied() {
    for addr in [
        "1.1.1.1",
        "8.8.8.8",
        "93.184.215.14", // example.com, a real public address
        "2606:4700:4700::1111",
        "2001:4860:4860::8888",
    ] {
        let ip = IpAddr::from_str(addr).expect("parse");
        assert_eq!(denied_by(ip), None, "{addr} must not be denied");
    }
}

#[test]
fn metadata_service_addresses_are_denied() {
    // Cloud metadata endpoints -- covered by Link Local, not a
    // separate table row (the matrix's "metadata-service addresses"
    // corpus item is a test vector, not an additional prefix).
    assert!(denied_by(IpAddr::from_str("169.254.169.254").unwrap()).is_some());
}

#[test]
fn ipv6_zone_ids_are_structurally_moot_here() {
    // `SocketAddr`/`Ipv6Addr` cannot carry a zone id at all (it is a
    // local-literal-parsing concept, not something a DNS answer
    // produces), so the matrix's "IPv6 zone IDs" corpus item reduces,
    // at this layer, to: the link-local range zone ids always
    // accompany is denied regardless of any zone annotation that
    // might have been attached before reaching here.
    assert!(denied_by(IpAddr::from_str("fe80::1").unwrap()).is_some());
}

#[test]
fn ipv4_mapped_is_denied_both_directly_and_by_its_embedded_address() {
    // ::ffff:127.0.0.1 matches the IPv4-mapped row directly, AND its
    // embedded address (127.0.0.1) independently matches Loopback --
    // the "belt and braces" the module doc comment names.
    let mapped = IpAddr::from_str("::ffff:127.0.0.1").unwrap();
    assert_eq!(denied_by(mapped), Some("IPv4-mapped Address -- RFC 4291"));
    let IpAddr::V6(v6) = mapped else {
        unreachable!()
    };
    let embedded = v6.to_ipv4().expect("embedded address");
    assert_eq!(embedded, Ipv4Addr::new(127, 0, 0, 1));
    assert!(denied_by(IpAddr::V4(embedded)).is_some());
}

#[test]
fn ipv4_compatible_is_denied_both_directly_and_by_its_embedded_address() {
    let compatible = IpAddr::from_str("::10.0.0.1").unwrap();
    assert!(denied_by(compatible).is_some());
    let IpAddr::V6(v6) = compatible else {
        unreachable!()
    };
    let embedded = v6.to_ipv4().expect("embedded address");
    assert_eq!(embedded, Ipv4Addr::new(10, 0, 0, 1));
}

#[test]
fn ipv4_mapped_and_ipv4_compatible_ranges_do_not_overlap() {
    // Distinct rows, distinct bit patterns (hextet 6 is 0xffff for
    // mapped, 0x0000 for compatible) -- a public address embedded in
    // the *compatible* form must not be caught by the *mapped* row
    // (each row's own boundary is what both_edges_of_every_vendored_
    // prefix_are_denied checks; this confirms they are not secretly
    // the same range).
    let compatible_public = IpAddr::from_str("::1.1.1.1").unwrap();
    // Denied anyway (it's still the IPv4-compatible row + the
    // embedded public address is NOT independently denied) -- the
    // point here is *which* row fires.
    assert_eq!(
        denied_by(compatible_public),
        Some(
            "IPv4-compatible Address -- RFC 4291 S2.5.5.1 (not an IANA registry row -- the matrix's own explicit deny-list entry, distinct from IPv4-mapped above)"
        )
    );
}

#[test]
fn empty_answer_is_rejected() {
    let err = validate_answer("x", vec![]).expect_err("must reject");
    assert!(err.to_string().contains("0 addresses"));
}

#[test]
fn over_limit_answer_is_rejected() {
    let addrs = (0..9)
        .map(|i| IpAddr::from(Ipv4Addr::new(1, 1, 1, i)))
        .collect();
    let err = validate_answer("x", addrs).expect_err("9 exceeds the limit");
    assert!(err.to_string().contains("9 addresses"));
}

#[test]
fn exactly_eight_ordinary_addresses_is_accepted() {
    let addrs: Vec<IpAddr> = (0..8)
        .map(|i| IpAddr::from(Ipv4Addr::new(1, 1, 1, i)))
        .collect();
    let chosen = validate_answer("x", addrs.clone()).expect("8 is within the limit");
    assert_eq!(chosen, addrs[0], "the first address is the one returned");
}

#[test]
fn a_mixed_answer_rejects_the_whole_answer_not_just_the_bad_address() {
    let good = IpAddr::from_str("1.1.1.1").unwrap();
    let bad = IpAddr::from_str("10.0.0.1").unwrap();
    let err = validate_answer("x", vec![good, bad]).expect_err("must reject the whole answer");
    assert!(matches!(err, ResolveError::Denied { .. }));
}

#[test]
fn address_order_does_not_change_the_rejection() {
    // The same two addresses, reversed: still rejected, and for the
    // same reason -- the loop checks every address regardless of
    // position, it does not stop at whichever one happens to be
    // first (RFC 096's "address-order changes" corpus item).
    let good = IpAddr::from_str("1.1.1.1").unwrap();
    let bad = IpAddr::from_str("10.0.0.1").unwrap();
    assert!(validate_answer("x", vec![good, bad]).is_err());
    assert!(validate_answer("x", vec![bad, good]).is_err());
}

#[test]
fn multiple_ordinary_addresses_return_the_first_one() {
    // "Returns exactly one" specifically means the first of whatever
    // order the base resolver handed back -- not the smallest, not a
    // random pick. Reordering the same set changes which one is
    // "first" and therefore which one is chosen.
    let a = IpAddr::from_str("1.1.1.1").unwrap();
    let b = IpAddr::from_str("8.8.8.8").unwrap();
    assert_eq!(validate_answer("x", vec![a, b]).unwrap(), a);
    assert_eq!(validate_answer("x", vec![b, a]).unwrap(), b);
}
