//! RFC 134 D2 — a DNS resolver that validates, and returns exactly one
//! address.
//!
//! The connector dials what [`ValidatingResolver::resolve`] returns, so
//! there is no window between the check and the connection and no second
//! answer to fall back to — a pre-flight lookup followed by an ordinary
//! connect would be exactly the defect this closes (DNS rebinding: the
//! name resolves to something safe at check time and something else at
//! connect time). Wired into the one federation egress client via
//! `.dns_resolver(...)` in `egress.rs`; G19 already asserts nothing else
//! may construct a federation-bound `reqwest::Client`.
//!
//! ## The vendored prefix table
//!
//! Every row of IANA's *IPv4 Special-Purpose Address Registry* and *IPv6
//! Special-Purpose Address Registry*, **regardless of their own "Globally
//! Reachable" column** (RFC 096's matrix: "any IANA special row regardless
//! of global flag" is a denial — several rows IANA marks globally
//! reachable, such as the PCP/TURN/AS112/AMT anycast rows, are denied here
//! anyway, deliberately), plus one row neither registry carries:
//! IPv4-compatible addresses (`::/96`, RFC 4291 §2.5.5.1) — distinct from
//! IPv4-mapped (`::ffff:0:0/96`, which *is* a registry row) and named
//! separately in the matrix's own explicit deny list.
//!
//! **Vendored against both registries as retrieved 2026-10-03; each
//! registry's own "Last Updated" field read 2025-10-09** (`iana-ipv4-
//! special-registry.xhtml`, `iana-ipv6-special-registry.xhtml` — fetched
//! directly, cross-checked against each registry's raw XML source at
//! `ftp.iana.org`, not transcribed from memory). Every row below is
//! traceable to a citation in its own comment; a row with no citation here
//! would be the thing this comment warns against.
//!
//! `is_global()` is **not used** — it is still unstable on this project's
//! pinned `+stable` (1.99.0; `error[E0658]`, checked directly). The table
//! is the policy regardless; `is_global` would only ever have been a
//! convenience over it, never a replacement for the rows the matrix
//! actually names (several of which IANA itself calls globally reachable).

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use reqwest::dns::{Addrs, Name, Resolve, Resolving};

/// One denied prefix, traceable to its source.
pub struct DeniedPrefix {
    pub network: IpAddr,
    pub prefix_len: u8,
    pub name: &'static str,
}

const fn v4(a: u8, b: u8, c: u8, d: u8, prefix_len: u8, name: &'static str) -> DeniedPrefix {
    DeniedPrefix {
        network: IpAddr::V4(Ipv4Addr::new(a, b, c, d)),
        prefix_len,
        name,
    }
}

#[allow(clippy::too_many_arguments)]
const fn v6(
    a: u16,
    b: u16,
    c: u16,
    d: u16,
    e: u16,
    f: u16,
    g: u16,
    h: u16,
    prefix_len: u8,
    name: &'static str,
) -> DeniedPrefix {
    DeniedPrefix {
        network: IpAddr::V6(Ipv6Addr::new(a, b, c, d, e, f, g, h)),
        prefix_len,
        name,
    }
}

/// IANA IPv4 Special-Purpose Address Registry, every row, "Last Updated"
/// 2025-10-09 — <https://www.iana.org/assignments/iana-ipv4-special-registry/iana-ipv4-special-registry.xhtml>.
pub static IPV4_DENIED: &[DeniedPrefix] = &[
    v4(0, 0, 0, 0, 8, "\"This network\" -- RFC 791 S3.2"),
    v4(
        0,
        0,
        0,
        0,
        32,
        "\"This host on this network\" -- RFC 1122 S3.2.1.3",
    ),
    v4(10, 0, 0, 0, 8, "Private-Use -- RFC 1918"),
    v4(100, 64, 0, 0, 10, "Shared Address Space -- RFC 6598"),
    v4(127, 0, 0, 0, 8, "Loopback -- RFC 1122 S3.2.1.3"),
    v4(169, 254, 0, 0, 16, "Link Local -- RFC 3927"),
    v4(172, 16, 0, 0, 12, "Private-Use -- RFC 1918"),
    v4(
        192,
        0,
        0,
        0,
        24,
        "IETF Protocol Assignments -- RFC 6890 S2.1",
    ),
    v4(
        192,
        0,
        0,
        0,
        29,
        "IPv4 Service Continuity Prefix -- RFC 7335",
    ),
    v4(192, 0, 0, 8, 32, "IPv4 dummy address -- RFC 7600"),
    v4(
        192,
        0,
        0,
        9,
        32,
        "Port Control Protocol Anycast -- RFC 7723 (Global=True in registry; denied anyway)",
    ),
    v4(
        192,
        0,
        0,
        10,
        32,
        "Traversal Using Relays around NAT Anycast -- RFC 8155 (Global=True; denied anyway)",
    ),
    v4(
        192,
        0,
        0,
        170,
        32,
        "NAT64/DNS64 Discovery -- RFC 8880, RFC 7050",
    ),
    v4(
        192,
        0,
        0,
        171,
        32,
        "NAT64/DNS64 Discovery -- RFC 8880, RFC 7050",
    ),
    v4(192, 0, 2, 0, 24, "Documentation (TEST-NET-1) -- RFC 5737"),
    v4(
        192,
        31,
        196,
        0,
        24,
        "AS112-v4 -- RFC 7535 (Global=True; denied anyway)",
    ),
    v4(
        192,
        52,
        193,
        0,
        24,
        "AMT -- RFC 7450 (Global=True; denied anyway)",
    ),
    v4(
        192,
        88,
        99,
        0,
        24,
        "Deprecated (6to4 Relay Anycast) -- RFC 7526",
    ),
    v4(192, 88, 99, 2, 32, "6a44-relay anycast address -- RFC 6751"),
    v4(192, 168, 0, 0, 16, "Private-Use -- RFC 1918"),
    v4(
        192,
        175,
        48,
        0,
        24,
        "Direct Delegation AS112 Service -- RFC 7534 (Global=True; denied anyway)",
    ),
    v4(198, 18, 0, 0, 15, "Benchmarking -- RFC 2544"),
    v4(
        198,
        51,
        100,
        0,
        24,
        "Documentation (TEST-NET-2) -- RFC 5737",
    ),
    v4(203, 0, 113, 0, 24, "Documentation (TEST-NET-3) -- RFC 5737"),
    v4(240, 0, 0, 0, 4, "Reserved -- RFC 1112 S4"),
    v4(
        255,
        255,
        255,
        255,
        32,
        "Limited Broadcast -- RFC 8190, RFC 919",
    ),
];

/// IANA IPv6 Special-Purpose Address Registry, every row, "Last Updated"
/// 2025-10-09 — <https://www.iana.org/assignments/iana-ipv6-special-registry/iana-ipv6-special-registry.xhtml>
/// — plus one explicit addition the matrix names that neither registry
/// carries: IPv4-compatible (`::/96`, last entry below).
pub static IPV6_DENIED: &[DeniedPrefix] = &[
    v6(0, 0, 0, 0, 0, 0, 0, 1, 128, "Loopback Address -- RFC 4291"),
    v6(
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        128,
        "Unspecified Address -- RFC 4291",
    ),
    v6(
        0,
        0,
        0,
        0,
        0,
        0xffff,
        0,
        0,
        96,
        "IPv4-mapped Address -- RFC 4291",
    ),
    v6(
        0x64,
        0xff9b,
        0,
        0,
        0,
        0,
        0,
        0,
        96,
        "IPv4-IPv6 Translat. (NAT64 #1) -- RFC 6052 (Global=True; denied anyway)",
    ),
    v6(
        0x64,
        0xff9b,
        1,
        0,
        0,
        0,
        0,
        0,
        48,
        "IPv4-IPv6 Translat. (NAT64 #2) -- RFC 8215",
    ),
    v6(
        0x100,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        64,
        "Discard-Only Address Block -- RFC 6666",
    ),
    v6(
        0x100,
        0,
        0,
        1,
        0,
        0,
        0,
        0,
        64,
        "Dummy IPv6 Prefix -- RFC 9780",
    ),
    v6(
        0x2001,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        23,
        "IETF Protocol Assignments -- RFC 2928",
    ),
    v6(
        0x2001,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        32,
        "TEREDO -- RFC 4380, RFC 8190",
    ),
    v6(
        0x2001,
        1,
        0,
        0,
        0,
        0,
        0,
        1,
        128,
        "Port Control Protocol Anycast -- RFC 7723 (Global=True; denied anyway)",
    ),
    v6(
        0x2001,
        1,
        0,
        0,
        0,
        0,
        0,
        2,
        128,
        "Traversal Using Relays around NAT Anycast -- RFC 8155 (Global=True; denied anyway)",
    ),
    v6(
        0x2001,
        1,
        0,
        0,
        0,
        0,
        0,
        3,
        128,
        "DNS-SD Service Registration Protocol Anycast -- RFC 9665 (Global=True; denied anyway)",
    ),
    v6(0x2001, 2, 0, 0, 0, 0, 0, 0, 48, "Benchmarking -- RFC 5180"),
    v6(
        0x2001,
        3,
        0,
        0,
        0,
        0,
        0,
        0,
        32,
        "AMT -- RFC 7450 (Global=True; denied anyway)",
    ),
    v6(
        0x2001,
        4,
        0x112,
        0,
        0,
        0,
        0,
        0,
        48,
        "AS112-v6 -- RFC 7535 (Global=True; denied anyway)",
    ),
    v6(
        0x2001,
        0x10,
        0,
        0,
        0,
        0,
        0,
        0,
        28,
        "Deprecated (previously ORCHID) -- RFC 4843",
    ),
    v6(
        0x2001,
        0x20,
        0,
        0,
        0,
        0,
        0,
        0,
        28,
        "ORCHIDv2 -- RFC 7343 (Global=True; denied anyway)",
    ),
    v6(
        0x2001,
        0x30,
        0,
        0,
        0,
        0,
        0,
        0,
        28,
        "Drone Remote ID Protocol Entity Tags (DETs) Prefix -- RFC 9374 (Global=True; denied anyway)",
    ),
    v6(
        0x2001,
        0xdb8,
        0,
        0,
        0,
        0,
        0,
        0,
        32,
        "Documentation -- RFC 3849",
    ),
    v6(0x2002, 0, 0, 0, 0, 0, 0, 0, 16, "6to4 -- RFC 3056"),
    v6(
        0x2620,
        0x4f,
        0x8000,
        0,
        0,
        0,
        0,
        0,
        48,
        "Direct Delegation AS112 Service -- RFC 7534 (Global=True; denied anyway)",
    ),
    v6(0x3fff, 0, 0, 0, 0, 0, 0, 0, 20, "Documentation -- RFC 9637"),
    v6(
        0x5f00,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        16,
        "Segment Routing (SRv6) SIDs -- RFC 9602",
    ),
    v6(
        0xfc00,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        7,
        "Unique-Local -- RFC 4193, RFC 8190",
    ),
    v6(
        0xfe80,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        10,
        "Link-Local Unicast -- RFC 4291",
    ),
    v6(
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        96,
        "IPv4-compatible Address -- RFC 4291 S2.5.5.1 (not an IANA registry row -- the matrix's own explicit deny-list entry, distinct from IPv4-mapped above)",
    ),
];

fn ipv4_in(addr: Ipv4Addr, network: Ipv4Addr, prefix_len: u8) -> bool {
    let mask: u32 = if prefix_len == 0 {
        0
    } else {
        u32::MAX << (32 - prefix_len)
    };
    (u32::from(addr) & mask) == (u32::from(network) & mask)
}

fn ipv6_in(addr: Ipv6Addr, network: Ipv6Addr, prefix_len: u8) -> bool {
    let mask: u128 = if prefix_len == 0 {
        0
    } else {
        u128::MAX << (128 - prefix_len)
    };
    (u128::from(addr) & mask) == (u128::from(network) & mask)
}

/// `Some(name)` of the first matching row if `addr` is denied, `None` if
/// it is an ordinary public unicast address.
///
/// IPv4-mapped/compatible normalization ("tested as both", RFC 134 D2):
/// an IPv6 address already matches its own `::ffff:0:0/96` or `::/96` row
/// directly; this additionally extracts the embedded IPv4 address (`Ipv6Addr::
/// to_ipv4`, stable, covers both forms per its own documented behavior --
/// checked directly against the stdlib source, not assumed) and checks it
/// against the IPv4 table too. Belt and braces: if either `/96` row were
/// ever narrowed, the embedded check still catches a dangerous embedded
/// address on its own account.
pub fn denied_by(addr: IpAddr) -> Option<&'static str> {
    match addr {
        IpAddr::V4(v4) => IPV4_DENIED
            .iter()
            .find(|p| matches!(p.network, IpAddr::V4(n) if ipv4_in(v4, n, p.prefix_len)))
            .map(|p| p.name),
        IpAddr::V6(v6) => {
            if let Some(hit) = IPV6_DENIED
                .iter()
                .find(|p| matches!(p.network, IpAddr::V6(n) if ipv6_in(v6, n, p.prefix_len)))
            {
                return Some(hit.name);
            }
            if let Some(embedded) = v6.to_ipv4() {
                return IPV4_DENIED
                    .iter()
                    .find(|p| matches!(p.network, IpAddr::V4(n) if ipv4_in(embedded, n, p.prefix_len)))
                    .map(|p| p.name);
            }
            None
        }
    }
}

/// An answer rejected before any connection attempt. `Display` is safe to
/// put in a server log (RFC 134 D2 S3c: "log the name and the reason");
/// callers must never put it in front of the browser -- but nothing here
/// does, since this surfaces through `reqwest`'s ordinary connect-error
/// path, which `fetch_discovery`'s existing `FetchDiscoveryError::Network`
/// handling already treats as a generic upstream failure.
#[derive(Debug)]
enum ResolveError {
    EmptyOrOverLimit {
        name: String,
        count: usize,
    },
    Denied {
        name: String,
        addr: IpAddr,
        reason: &'static str,
    },
}

impl std::fmt::Display for ResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyOrOverLimit { name, count } => {
                write!(f, "resolving {name:?}: {count} addresses (must be 1-8)")
            }
            Self::Denied { name, addr, reason } => {
                write!(f, "resolving {name:?}: {addr} is denied ({reason})")
            }
        }
    }
}

impl std::error::Error for ResolveError {}

/// Maximum number of addresses a resolved name may carry before the
/// answer is rejected outright (RFC 134 D2).
pub const MAX_ANSWERS: usize = 8;

/// RFC 134 D2: resolves through `tokio::net::lookup_host` (chosen over
/// `reqwest`'s own `GaiResolver` because that type is private to
/// `reqwest` -- not re-exported from `reqwest::dns` -- so nothing outside
/// the crate can construct one; `tokio::net::lookup_host` is already in
/// the dependency graph regardless of this choice, per the handoff's own
/// fact 4), rejects an empty or over-limit answer, rejects the whole
/// answer if any one address fails the vendored table (never filters —
/// a mixed answer is a rebinding signal), and returns exactly one
/// surviving address with port 0 (reqwest fills in the real port; see the
/// module doc comment and the handoff's fact 2).
pub struct ValidatingResolver;

/// The whole of 3b's conditions 2-4, as one pure (no I/O) function: reject
/// an empty or over-limit answer; reject the whole answer if any one
/// address fails the vendored table — never filter, since a mixed
/// public/private answer is a rebinding signal, not a list to pick from;
/// otherwise return the first surviving address. Split out from
/// [`ValidatingResolver::resolve`] so these rules are unit-testable
/// without mocking DNS — `resolve`'s own job, once this returns, is only
/// to call `tokio::net::lookup_host` and wrap the result.
fn validate_answer(name: &str, addrs: Vec<IpAddr>) -> Result<IpAddr, ResolveError> {
    if addrs.is_empty() || addrs.len() > MAX_ANSWERS {
        tracing::warn!(name = %name, count = addrs.len(), "federation DNS answer rejected");
        return Err(ResolveError::EmptyOrOverLimit {
            name: name.to_owned(),
            count: addrs.len(),
        });
    }

    for addr in &addrs {
        if let Some(reason) = denied_by(*addr) {
            tracing::warn!(name = %name, addr = %addr, reason, "federation DNS answer rejected");
            return Err(ResolveError::Denied {
                name: name.to_owned(),
                addr: *addr,
                reason,
            });
        }
    }

    #[allow(clippy::expect_used)]
    Ok(*addrs.first().expect("checked non-empty above"))
}

impl Resolve for ValidatingResolver {
    fn resolve(&self, name: Name) -> Resolving {
        Box::pin(async move {
            let host = name.as_str().to_owned();
            // Port 0: only the IP matters for lookup_host's own addressing
            // here, and reqwest/hyper-util fill in the real port on the
            // `SocketAddr`s we hand back (fact 2) -- never ours to invent.
            let addrs: Vec<IpAddr> = tokio::net::lookup_host((host.as_str(), 0))
                .await?
                .map(|s| s.ip())
                .collect();
            let chosen = validate_answer(&host, addrs)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
            let result: Addrs = Box::new(std::iter::once(SocketAddr::new(chosen, 0)));
            Ok(result)
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
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
}
