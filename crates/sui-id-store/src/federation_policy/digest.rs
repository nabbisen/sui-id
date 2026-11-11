//! RFC 096-B2 S3: the policy digest (RFC 096 `:424-437`). Private to
//! `federation_policy` -- re-exported pieces only (`PolicyDigest`, and
//! [`compute`] as `federation_policy::compute_policy_digest`); nothing
//! outside this file knows the encoding.
//!
//! **Field-tag order (`096-b2-stage3-the-preflight-capability-2026-11-11.
//! md` §A row 2: the architect's own choice -- RFC 096 does not give one,
//! "record it in the code"):** 1 issuer, 2 client ID, 3 sealed-secret-
//! envelope SHA-256 (explicit absent/present), 4 token-auth-method tag,
//! 5 scopes, 6 ID-token algorithms, 7 endpoint origins, 8 HTTPS ports,
//! 9 provision mode -- the same order RFC 096 `:424-428` lists the nine
//! fields in.
//!
//! **Encoding, verbatim from RFC 096 `:424-437`:** "Each scalar is
//! encoded as a one-byte field tag, a big-endian `u32` byte length, and
//! exact canonical bytes; lists use their field tag, a big-endian `u16`
//! item count, and independently length-prefixed canonical items sorted
//! by the closed enum order or canonical bytes; ports are sorted
//! big-endian `u16` values." Ports are encoded as a list (field 8) whose
//! items are each a port's own 2-byte big-endian form, through the same
//! generic list encoder as every other list -- a plain reading of "ports
//! are [themselves] sorted big-endian u16 values" rather than a special
//! case, and `derive_ports`'s own `BTreeSet` already yields them sorted.
//!
//! A `python3.14`/`hashlib`-only re-implementation of this exact encoding
//! lives at `scripts/federation-policy-digest-known-answer.py` and is
//! pinned against this module's own output in `tests.rs` -- "two
//! implementations of one encoding agreeing is the evidence that the
//! encoding is what RFC 096 `:424-437` says."

use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

use super::{FederationTrustPolicy, TokenAuthMethod};

const DOMAIN: &[u8] = b"sui-id/federation-provider-policy/v1\0";

const TAG_ISSUER: u8 = 1;
const TAG_CLIENT_ID: u8 = 2;
const TAG_SECRET: u8 = 3;
const TAG_TOKEN_AUTH_METHOD: u8 = 4;
const TAG_SCOPES: u8 = 5;
const TAG_ID_TOKEN_ALGS: u8 = 6;
const TAG_ENDPOINT_ORIGINS: u8 = 7;
const TAG_HTTPS_PORTS: u8 = 8;
const TAG_PROVISION_MODE: u8 = 9;

/// RFC 096 `:424-437`'s policy digest: exactly 32 bytes. Row 9: compared
/// only in constant time ([`ct_eq`](Self::ct_eq)) -- deliberately no
/// `PartialEq` derive, which would let `==` compare it non-constant-
/// time. Row 10: never accepted from a caller, serialized, logged,
/// audited, or exposed through metrics -- no `Debug`, no `Serialize`, no
/// public constructor (only [`compute`] builds one), and
/// [`as_bytes`](Self::as_bytes) is `pub(crate)`: bytes leave this crate
/// only through S4's evidence-column write, never reaching `sui-id` or
/// `sui-id-core`.
#[derive(Clone, Copy)]
pub struct PolicyDigest([u8; 32]);

impl PolicyDigest {
    pub fn ct_eq(&self, other: &Self) -> bool {
        self.0.ct_eq(&other.0).into()
    }

    /// For S4's evidence column -- the store persisting its own value.
    /// Unused until S4 exists (this stage builds no caller of the
    /// capability's contents, by its own dispatch); kept rather than
    /// deleted, since deleting it would just mean re-adding the same
    /// function next stage.
    #[allow(dead_code)]
    pub(crate) fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

fn push_scalar(buf: &mut Vec<u8>, tag: u8, bytes: &[u8]) {
    buf.push(tag);
    #[allow(clippy::cast_possible_truncation)]
    buf.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    buf.extend_from_slice(bytes);
}

fn push_list<'a>(buf: &mut Vec<u8>, tag: u8, items: impl IntoIterator<Item = &'a [u8]>) {
    let items: Vec<&[u8]> = items.into_iter().collect();
    buf.push(tag);
    #[allow(clippy::cast_possible_truncation)]
    buf.extend_from_slice(&(items.len() as u16).to_be_bytes());
    for item in items {
        #[allow(clippy::cast_possible_truncation)]
        buf.extend_from_slice(&(item.len() as u32).to_be_bytes());
        buf.extend_from_slice(item);
    }
}

/// RFC 096 `:424-437`, computed only here (row 8: "computed only by the
/// private policy module from typed durable/proposed policy"; this
/// module's own privacy is what makes that true, not a runtime check).
///
/// `secret_envelope` is the raw sealed `client_secret_enc` bytes (or
/// `None` for a public client) -- never the plaintext, and never
/// decrypted: the digest binds the envelope itself (RFC 096 `:426`), so
/// resealing the same plaintext changes the digest, by design (a fresh
/// seal always has a fresh nonce) -- the reason S2b never reseals an
/// unchanged secret.
pub fn compute(policy: &FederationTrustPolicy, secret_envelope: Option<&[u8]>) -> PolicyDigest {
    let mut buf = Vec::new();
    buf.extend_from_slice(DOMAIN);

    push_scalar(&mut buf, TAG_ISSUER, policy.issuer().as_bytes());
    push_scalar(&mut buf, TAG_CLIENT_ID, policy.client_id().as_bytes());

    // Field 3: explicit absent/present tag (row 6, "empty and absent are
    // distinct"). A present envelope can never be empty -- sealing always
    // appends a nonce and an authentication tag -- so this is the one
    // field where the distinction is actually reachable; every other
    // field (issuer, client_id, scopes, ...) already refuses an empty
    // value at `FederationTrustPolicy::new` time, so "empty" never
    // reaches this function for them.
    let secret_field: Vec<u8> = match secret_envelope {
        None => vec![0],
        Some(enc) => {
            let mut v = Vec::with_capacity(33);
            v.push(1);
            v.extend_from_slice(Sha256::digest(enc).as_slice());
            v
        }
    };
    push_scalar(&mut buf, TAG_SECRET, &secret_field);

    let token_auth_tag: u8 = match policy.token_auth_method() {
        TokenAuthMethod::None => 0,
        TokenAuthMethod::ClientSecretBasic => 1,
    };
    push_scalar(&mut buf, TAG_TOKEN_AUTH_METHOD, &[token_auth_tag]);

    // Scopes and algorithms are already in their closed-enum canonical
    // order (row 4's first alternative) via the policy's own accessors.
    push_list(
        &mut buf,
        TAG_SCOPES,
        policy.scopes().iter().map(String::as_bytes),
    );
    push_list(
        &mut buf,
        TAG_ID_TOKEN_ALGS,
        policy.id_token_algs().iter().map(String::as_bytes),
    );

    // Origins: row 4's second alternative, "canonical bytes" -- sorted
    // here, not relying on the policy's own stored order (S2b's own
    // fold-in: reordering origins in the config is not a policy change,
    // and must not disable a provider).
    let mut origins: Vec<&str> = policy
        .allowed_origins()
        .iter()
        .map(String::as_str)
        .collect();
    origins.sort_unstable();
    push_list(
        &mut buf,
        TAG_ENDPOINT_ORIGINS,
        origins.iter().map(|s| s.as_bytes()),
    );

    // Ports: row 5. Already sorted (`derive_ports`'s own `BTreeSet`);
    // each item is that one port's 2-byte big-endian form.
    //
    // **The reading of RFC 096 `:432`, recorded because `:437` freezes the
    // format once evidence is stored:** "ports are sorted big-endian `u16`
    // values" gives their sort (numeric) and their value form (two bytes).
    // Each is still a list item, length-prefixed like every other list's
    // items, so all lists share one encoding. The known-answer script
    // (`scripts/federation-policy-digest-known-answer.py`) applies the same
    // reading, and vector 2 exercises two ports.
    let port_bytes: Vec<[u8; 2]> = policy
        .https_ports()
        .iter()
        .map(|p| p.to_be_bytes())
        .collect();
    push_list(
        &mut buf,
        TAG_HTTPS_PORTS,
        port_bytes.iter().map(|b| b.as_slice()),
    );

    push_scalar(
        &mut buf,
        TAG_PROVISION_MODE,
        policy.provision_mode().as_str().as_bytes(),
    );

    PolicyDigest(Sha256::digest(&buf).into())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;
