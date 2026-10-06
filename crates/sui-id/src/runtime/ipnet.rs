//! Minimal CIDR matcher for `trusted_proxies`.
//!
//! We pull this in-house rather than depending on the `ipnet` crate so the
//! dependency graph stays small. The semantics implemented are exactly the
//! semantics we need: parse `"192.0.2.0/24"` or `"2001:db8::/32"`, then ask
//! `contains(addr)`.

use std::net::IpAddr;

#[derive(Debug, Clone, Copy)]
pub struct Cidr {
    network: IpAddr,
    prefix: u8,
}

impl Cidr {
    /// Parse a CIDR block. Accepts both IPv4 and IPv6.
    pub fn parse(s: &str) -> Result<Self, String> {
        let (addr, prefix) = match s.split_once('/') {
            Some((a, p)) => (a, p),
            None => return Err(format!("missing '/prefix' in {s:?}")),
        };
        let network: IpAddr = addr
            .parse()
            .map_err(|_| format!("invalid IP literal {addr:?}"))?;
        let prefix: u8 = prefix
            .parse()
            .map_err(|_| format!("invalid prefix length {prefix:?}"))?;
        let max = match &network {
            IpAddr::V4(_) => 32,
            IpAddr::V6(_) => 128,
        };
        if prefix > max {
            return Err(format!("prefix /{prefix} exceeds /{max}"));
        }
        Ok(Self { network, prefix })
    }

    /// Test whether `addr` is inside this CIDR block.
    pub fn contains(&self, addr: &IpAddr) -> bool {
        match (self.network, addr) {
            (IpAddr::V4(net), IpAddr::V4(a)) => {
                let net_bits = u32::from_be_bytes(net.octets());
                let a_bits = u32::from_be_bytes(a.octets());
                let mask = mask32(self.prefix);
                (net_bits & mask) == (a_bits & mask)
            }
            (IpAddr::V6(net), IpAddr::V6(a)) => {
                let net_bits = u128::from_be_bytes(net.octets());
                let a_bits = u128::from_be_bytes(a.octets());
                let mask = mask128(self.prefix);
                (net_bits & mask) == (a_bits & mask)
            }
            _ => false,
        }
    }
}

fn mask32(prefix: u8) -> u32 {
    if prefix == 0 {
        0
    } else if prefix >= 32 {
        u32::MAX
    } else {
        u32::MAX << (32 - prefix)
    }
}

fn mask128(prefix: u8) -> u128 {
    if prefix == 0 {
        0
    } else if prefix >= 128 {
        u128::MAX
    } else {
        u128::MAX << (128 - prefix)
    }
}

/// True if `ip` is in any of `cidrs`.
pub fn any_contains(cidrs: &[Cidr], ip: &IpAddr) -> bool {
    cidrs.iter().any(|c| c.contains(ip))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
#[path = "ipnet/tests.rs"]
mod tests;
