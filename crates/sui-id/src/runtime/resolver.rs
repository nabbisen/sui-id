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
#[path = "resolver/tests.rs"]
mod tests;
