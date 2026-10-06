use super::*;

#[test]
fn parse_ipv4_cidr_round_trip() {
    let c = Cidr::parse("10.0.0.0/8").expect("parse");
    assert!(c.contains(&"10.1.2.3".parse().unwrap()));
    assert!(!c.contains(&"11.0.0.1".parse().unwrap()));
}

#[test]
fn slash_32_matches_only_exact() {
    let c = Cidr::parse("192.0.2.5/32").expect("parse");
    assert!(c.contains(&"192.0.2.5".parse().unwrap()));
    assert!(!c.contains(&"192.0.2.6".parse().unwrap()));
}

#[test]
fn slash_zero_matches_everything() {
    let c = Cidr::parse("0.0.0.0/0").expect("parse");
    assert!(c.contains(&"1.2.3.4".parse().unwrap()));
    assert!(c.contains(&"203.0.113.0".parse().unwrap()));
}

#[test]
fn ipv6_basic() {
    let c = Cidr::parse("2001:db8::/32").expect("parse");
    assert!(c.contains(&"2001:db8::1".parse().unwrap()));
    assert!(!c.contains(&"2001:db9::1".parse().unwrap()));
}

#[test]
fn v4_cidr_does_not_match_v6_addr() {
    let c = Cidr::parse("10.0.0.0/8").expect("parse");
    assert!(!c.contains(&"2001:db8::1".parse().unwrap()));
}

#[test]
fn rejects_invalid_inputs() {
    assert!(Cidr::parse("10.0.0.0").is_err()); // missing /
    assert!(Cidr::parse("not-an-ip/24").is_err());
    assert!(Cidr::parse("10.0.0.0/abc").is_err());
    assert!(Cidr::parse("10.0.0.0/33").is_err());
    assert!(Cidr::parse("2001:db8::/129").is_err());
}

#[test]
fn any_contains_short_circuits() {
    let cs: Vec<Cidr> = ["10.0.0.0/8", "192.168.0.0/16"]
        .iter()
        .map(|s| Cidr::parse(s).expect("parse"))
        .collect();
    assert!(any_contains(&cs, &"10.1.1.1".parse().unwrap()));
    assert!(any_contains(&cs, &"192.168.5.5".parse().unwrap()));
    assert!(!any_contains(&cs, &"8.8.8.8".parse().unwrap()));
}

// ---------- property-based (v0.13.0) ----------
//
// The CIDR matcher is the kind of code that goes wrong in
// off-by-one ways at /0, /32, and /33 boundaries. proptest's
// value here is exhaustive boundary coverage that targeted
// unit tests would have to remember to write.

use proptest::prelude::*;

/// Reference: brute-force "is `addr` in `prefix` /  `prefix_len`?"
/// by checking the high bits one at a time. Distinct from the
/// production code path in [`Cidr::contains`], so the property
/// below cross-checks one against the other.
fn naive_contains_v4(net: u32, prefix: u8, addr: u32) -> bool {
    if prefix == 0 {
        return true;
    }
    if prefix > 32 {
        return false;
    }
    let bits = prefix as u32;
    let net_high = net >> (32 - bits);
    let addr_high = addr >> (32 - bits);
    net_high == addr_high
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 512,
        ..ProptestConfig::default()
    })]

    #[test]
    fn ipv4_contains_matches_naive_implementation(
        net in any::<u32>(),
        prefix in 0u8..=32,
        addr in any::<u32>(),
    ) {
        let net_addr: IpAddr = std::net::Ipv4Addr::from(net.to_be_bytes()).into();
        let probe: IpAddr = std::net::Ipv4Addr::from(addr.to_be_bytes()).into();
        // We can't construct a Cidr through Cidr::parse for
        // arbitrary u32 inputs cheaply, so build one directly.
        let cidr = Cidr { network: net_addr, prefix };
        let got = cidr.contains(&probe);
        let expected = naive_contains_v4(net, prefix, addr);
        prop_assert_eq!(got, expected);
    }

    #[test]
    fn an_address_is_always_in_its_own_slash_32(addr in any::<u32>()) {
        let a: IpAddr = std::net::Ipv4Addr::from(addr.to_be_bytes()).into();
        let cidr = Cidr { network: a, prefix: 32 };
        prop_assert!(cidr.contains(&a));
    }

    #[test]
    fn slash_zero_contains_every_v4(addr in any::<u32>()) {
        let net: IpAddr = std::net::Ipv4Addr::from([0, 0, 0, 0]).into();
        let probe: IpAddr = std::net::Ipv4Addr::from(addr.to_be_bytes()).into();
        let cidr = Cidr { network: net, prefix: 0 };
        prop_assert!(cidr.contains(&probe));
    }

    #[test]
    fn v4_and_v6_never_cross_match(
        v4 in any::<u32>(),
        v6 in any::<u128>(),
        prefix in 0u8..=128,
    ) {
        let p4 = prefix.min(32);
        let v4_net: IpAddr = std::net::Ipv4Addr::from(v4.to_be_bytes()).into();
        let v6_addr: IpAddr = std::net::Ipv6Addr::from(v6.to_be_bytes()).into();
        let cidr = Cidr { network: v4_net, prefix: p4 };
        // A v6 address must never satisfy a v4 CIDR.
        prop_assert!(!cidr.contains(&v6_addr));
    }
}
