//! Strongly-typed identifiers.
//!
//! All resources expose UUID v4 ids. Wrapping them in newtypes prevents the
//! classic "I passed a user id where a client id was expected" bug at compile
//! time, while keeping the wire format identical to a plain UUID string.

use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

macro_rules! define_id {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(pub Uuid);

        impl $name {
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }
            pub fn from_uuid(u: Uuid) -> Self {
                Self(u)
            }
            pub fn as_uuid(&self) -> Uuid {
                self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl std::str::FromStr for $name {
            type Err = uuid::Error;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Ok(Self(Uuid::parse_str(s)?))
            }
        }
    };
}

define_id!(UserId, "Identifier of a normal user account.");
define_id!(ClientId, "Identifier of an OAuth/OIDC client (relying party).");
define_id!(SessionId, "Identifier of a server-side session.");
define_id!(SigningKeyId, "Identifier of a JWT signing key.");

#[cfg(test)]
mod tests;
