//! RFC 096-B2 S2a: the durable federation-provider trust policy.
//!
//! [`FederationTrustPolicy`] holds the policy digest's nine fields (RFC 096
//! `:424-428`): issuer, client ID, client-secret presence, the derived
//! token-auth method, scopes, ID-token algorithms, endpoint origins, the
//! derived HTTPS ports, and provision mode. Fields are private; the only
//! way to build one is [`FederationTrustPolicy::new`], which validates
//! every rule in RFC 096's trust-configuration table (`:355-363`) and
//! returns a typed [`PolicyError`] naming exactly which one failed.
//!
//! **One validator, not two.** Both callers that need a trust policy —
//! `Config::validate` (raw, operator-supplied strings) and a stored
//! `federation_provider` row reader (already-canonical, but possibly
//! malformed, text) — go through this same constructor. Neither
//! re-implements a rule stated here.
//!
//! Lives in `sui-id-store`, not `sui-id`, because the store persists the
//! policy and RFC 096-B2 S3's policy digest is computed from it.

use std::collections::BTreeSet;

use crate::models::ProvisionMode;

/// RFC 096 `:359`: the closed scope vocabulary, in its fixed canonical
/// order — also the order the policy digest sorts scopes in (RFC 096
/// `:430-431`, "canonical items sorted by the closed enum order").
const SCOPE_ORDER: [&str; 3] = ["openid", "email", "profile"];

/// RFC 096 `:360`: the closed ID-token-algorithm vocabulary, in its fixed
/// canonical order — matching `runtime/config.rs`'s own `PERMITTED` order,
/// not re-decided here.
const ALG_ORDER: [&str; 4] = ["RS256", "PS256", "ES256", "EdDSA"];

/// RFC 096 `:358`: derived from whether a client secret is configured —
/// never stored or asked for separately.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenAuthMethod {
    ClientSecretBasic,
    None,
}

impl TokenAuthMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ClientSecretBasic => "client_secret_basic",
            Self::None => "none",
        }
    }
}

/// Every way [`FederationTrustPolicy::new`] can refuse. `Display` renders
/// `"<field>: <detail> (RFC 096 Provider trust configuration)"` — the
/// caller (`Config::validate`) prefixes `federation_provider[{slug}]: `,
/// matching `runtime/config.rs:473-516`'s existing message shape. The
/// stored-row reader also uses this `Display` text, prefixed with the
/// provider id instead of a slug, for "no usable policy".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyError {
    Issuer(String),
    ClientId(String),
    Scopes(String),
    IdTokenAlgs(String),
    Origins(String),
}

impl std::fmt::Display for PolicyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (field, detail) = match self {
            Self::Issuer(d) => ("issuer", d),
            Self::ClientId(d) => ("client_id", d),
            Self::Scopes(d) => ("scopes", d),
            Self::IdTokenAlgs(d) => ("id_token_algs", d),
            Self::Origins(d) => ("allowed_origins", d),
        };
        write!(
            f,
            "{field}: {detail} (RFC 096 Provider trust configuration)"
        )
    }
}

impl std::error::Error for PolicyError {}

impl PolicyError {
    /// The field name alone, with none of the offending value. For logs
    /// that must not carry an issuer query or similar
    /// (`architecture.md:306-307`).
    pub fn field(&self) -> &'static str {
        match self {
            Self::Issuer(_) => "issuer",
            Self::ClientId(_) => "client_id",
            Self::Scopes(_) => "scopes",
            Self::IdTokenAlgs(_) => "id_token_algs",
            Self::Origins(_) => "allowed_origins",
        }
    }
}

/// Why a stored row has no usable trust policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoredPolicyError {
    /// `id_token_algs` is NULL: every row migration 0051 copied, until a
    /// policy replacement records it.
    NotRecorded,
    /// A stored field fails [`FederationTrustPolicy::new`].
    Invalid(PolicyError),
}

impl StoredPolicyError {
    /// The field name alone, for logs. See [`PolicyError::field`].
    pub fn field(&self) -> &'static str {
        match self {
            Self::NotRecorded => "id_token_algs",
            Self::Invalid(e) => e.field(),
        }
    }
}

/// The one reader of a stored row's trust policy. Callers (the callback
/// today; C23 and the preflight later) use this, never their own split
/// of the row's columns. Never a default: a NULL or malformed field is an
/// error.
pub fn from_stored(
    row: &crate::models::FederationProviderRow,
) -> Result<FederationTrustPolicy, StoredPolicyError> {
    let Some(algs) = row.id_token_algs.as_deref() else {
        return Err(StoredPolicyError::NotRecorded);
    };
    let algs: Vec<&str> = algs.split_whitespace().collect();
    let origins: Vec<&str> = row.allowed_origins.split_whitespace().collect();
    FederationTrustPolicy::new(
        &row.issuer,
        &row.client_id,
        row.client_secret_enc.is_some(),
        &row.scopes,
        &algs,
        &origins,
        row.provision_mode.clone(),
    )
    .map_err(StoredPolicyError::Invalid)
}

/// RFC 096 `:359`'s scope rule, applied to a raw space-separated value,
/// returning its canonical encoding. Lets startup compare a stored value
/// with a configured one by meaning, not spelling: `"email openid"` and
/// `"openid email"` are the same scopes.
pub fn canonical_scopes(raw: &str) -> Result<String, PolicyError> {
    validate_scopes(raw).map(|s| s.join(" "))
}

/// RFC 096 `:424-428`'s nine durable trust-policy fields, validated and
/// canonical. Private fields; read through the accessors below.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FederationTrustPolicy {
    issuer: String,
    client_id: String,
    has_client_secret: bool,
    scopes: Vec<String>,
    id_token_algs: Vec<String>,
    allowed_origins: Vec<String>,
    https_ports: Vec<u16>,
    provision_mode: ProvisionMode,
}

impl FederationTrustPolicy {
    /// The one validating constructor. `scopes_raw` is space-separated
    /// (the same raw form both config and the stored row use). `id_token_
    /// algs_raw` and `allowed_origins_raw` are already-split tokens — the
    /// config caller passes its `Vec<String>` borrowed as `&str`s, the
    /// row caller passes `row.allowed_origins.split_whitespace()` /
    /// `row.id_token_algs`'s own split — so neither caller duplicates
    /// the splitting rule, only this constructor's validation of what the
    /// tokens must be. `allowed_origins_raw` empty means "the issuer's
    /// origin alone" (the same meaning `discovery.rs:354-355` already
    /// gives it). `provision_mode` is pre-parsed by the caller via
    /// [`ProvisionMode::try_parse`] — both callers already have (or can
    /// cheaply get) a typed value, so this constructor does not
    /// re-implement that parse.
    pub fn new(
        issuer: &str,
        client_id: &str,
        has_client_secret: bool,
        scopes_raw: &str,
        id_token_algs_raw: &[&str],
        allowed_origins_raw: &[&str],
        provision_mode: ProvisionMode,
    ) -> Result<Self, PolicyError> {
        let issuer = validate_issuer(issuer)?;
        let client_id = validate_client_id(client_id)?;
        let scopes = validate_scopes(scopes_raw)?;
        let id_token_algs = validate_id_token_algs(id_token_algs_raw)?;
        let allowed_origins = validate_origins(issuer.as_str(), allowed_origins_raw)?;
        let https_ports = derive_ports(&allowed_origins);

        Ok(Self {
            issuer,
            client_id,
            has_client_secret,
            scopes,
            id_token_algs,
            allowed_origins,
            https_ports,
            provision_mode,
        })
    }

    pub fn issuer(&self) -> &str {
        &self.issuer
    }
    pub fn client_id(&self) -> &str {
        &self.client_id
    }
    pub fn has_client_secret(&self) -> bool {
        self.has_client_secret
    }
    /// RFC 096 `:358`, derived: never stored or asked for separately.
    pub fn token_auth_method(&self) -> TokenAuthMethod {
        if self.has_client_secret {
            TokenAuthMethod::ClientSecretBasic
        } else {
            TokenAuthMethod::None
        }
    }
    /// Canonical scope tokens, in `SCOPE_ORDER`.
    pub fn scopes(&self) -> &[String] {
        &self.scopes
    }
    /// Space-separated canonical encoding, for storage.
    pub fn scopes_canonical(&self) -> String {
        self.scopes.join(" ")
    }
    /// Canonical ID-token algorithm tokens, in `ALG_ORDER`.
    pub fn id_token_algs(&self) -> &[String] {
        &self.id_token_algs
    }
    /// Space-separated canonical encoding, for storage (migration 0051's
    /// own rule: "tokens from the closed set, in the set's fixed order,
    /// single-space separated").
    pub fn id_token_algs_canonical(&self) -> String {
        self.id_token_algs.join(" ")
    }
    /// Canonical origin strings.
    pub fn allowed_origins(&self) -> &[String] {
        &self.allowed_origins
    }
    /// Space-separated canonical encoding, for storage (the existing
    /// `federation_provider.allowed_origins` column format).
    pub fn allowed_origins_canonical(&self) -> String {
        self.allowed_origins.join(" ")
    }
    /// RFC 096 `:362`, derived: the set of the origins' effective ports.
    pub fn https_ports(&self) -> &[u16] {
        &self.https_ports
    }
    pub fn provision_mode(&self) -> ProvisionMode {
        self.provision_mode.clone()
    }
}

/// RFC 096 `:355`, read with `:367-372`: the issuer's *"exact serialized
/// value is authoritative"*, but `url` serializes an empty path as `/`,
/// and a real issuer may have none (Google's is `https://accounts.google.
/// com`). The returned value is `s` itself (never `url`'s own
/// re-serialization) — `:600` compares the stored issuer byte-for-byte
/// against discovery's raw `issuer` claim, which for a real provider is
/// the un-slashed form, so storing anything else would make that
/// comparison fail for every provider whose issuer has no path.
///
/// TODO(096-B2 issuer host rule): RFC 096 `:355` also requires "no IP
/// literal, no `localhost`, `.localhost`, `.local`, `.internal` or
/// `.home.arpa`" for the issuer host. Not enforced here yet -- S2a
/// measured the cost (41 e2e tests across 8 files configure a federation
/// provider's issuer as `https://127.0.0.1:PORT`, the architect's own
/// stage-plan §6) and left it for its own stage. This is the one place
/// the check will go.
fn validate_issuer(s: &str) -> Result<String, PolicyError> {
    if s.is_empty() || s.len() > 2048 {
        return Err(PolicyError::Issuer(format!(
            "must be 1-2048 bytes, got {}",
            s.len()
        )));
    }
    let Ok(u) = url::Url::parse(s) else {
        return Err(PolicyError::Issuer(format!("{s:?} is not a valid URL")));
    };
    let canonical = u.as_str() == s || (u.path() == "/" && u.as_str().strip_suffix('/') == Some(s));
    if u.scheme() != "https"
        || !u.username().is_empty()
        || u.password().is_some()
        || u.query().is_some()
        || u.fragment().is_some()
        || !canonical
    {
        return Err(PolicyError::Issuer(format!(
            "{s:?} is not a canonical absolute HTTPS URL with no userinfo, query, or \
             fragment (RFC 096 :355, :367-372)"
        )));
    }
    Ok(s.to_string())
}

/// RFC 096 `:356`: 1-512 UTF-8 bytes, no control characters.
fn validate_client_id(s: &str) -> Result<String, PolicyError> {
    if s.is_empty() || s.len() > 512 {
        return Err(PolicyError::ClientId(format!(
            "must be 1-512 UTF-8 bytes, got {}",
            s.len()
        )));
    }
    if s.chars().any(|c| c.is_control()) {
        return Err(PolicyError::ClientId(format!(
            "{s:?} contains a control character"
        )));
    }
    Ok(s.to_string())
}

/// RFC 096 `:359`: a unique subset of `openid email profile`, `openid`
/// mandatory. "At most three tokens" is automatically met once every
/// token is a distinct member of this three-element closed set — not
/// separately checked, since nothing could ever violate it without
/// first violating "unique" or "closed".
fn validate_scopes(raw: &str) -> Result<Vec<String>, PolicyError> {
    let mut seen = BTreeSet::new();
    for tok in raw.split_whitespace() {
        if !SCOPE_ORDER.contains(&tok) {
            return Err(PolicyError::Scopes(format!(
                "{tok:?} is not one of openid, email, profile"
            )));
        }
        if !seen.insert(tok) {
            return Err(PolicyError::Scopes(format!("{tok:?} is listed twice")));
        }
    }
    if !seen.contains("openid") {
        return Err(PolicyError::Scopes(
            "must include \"openid\" (mandatory)".to_string(),
        ));
    }
    Ok(SCOPE_ORDER
        .iter()
        .filter(|s| seen.contains(**s))
        .map(|s| s.to_string())
        .collect())
}

/// RFC 096 `:360`, moved verbatim from `runtime/config.rs`'s former
/// `validate_id_token_algs` (`jose-strategy-2026-10-06.md`'s ruling):
/// nonempty, at most four, each a member of RS256/PS256/ES256/EdDSA,
/// `"none"` and the symmetric HS* family refused with their own
/// messages, no duplicates.
fn validate_id_token_algs(raw: &[&str]) -> Result<Vec<String>, PolicyError> {
    if raw.is_empty() {
        return Err(PolicyError::IdTokenAlgs(
            "is empty. An empty list is a configuration error, not \"trust nothing by \
             default\"; omit the key to get the default RS256"
                .to_string(),
        ));
    }
    if raw.len() > 4 {
        return Err(PolicyError::IdTokenAlgs(format!(
            "has {} entries, more than the maximum of 4",
            raw.len()
        )));
    }
    let mut seen = BTreeSet::new();
    for entry in raw {
        if *entry == "none" {
            return Err(PolicyError::IdTokenAlgs(
                "entry \"none\" is never acceptable: an unsigned token cannot be \
                 verified against a key"
                    .to_string(),
            ));
        }
        if matches!(*entry, "HS256" | "HS384" | "HS512") {
            return Err(PolicyError::IdTokenAlgs(format!(
                "entry {entry:?} is a symmetric HS* algorithm. It has no meaning \
                 against a public JWKS, and accepting it is the classic \
                 algorithm-confusion attack. Permitted: RS256, PS256, ES256, EdDSA"
            )));
        }
        if !ALG_ORDER.contains(entry) {
            return Err(PolicyError::IdTokenAlgs(format!(
                "entry {entry:?} is not permitted. Permitted: RS256, PS256, ES256, EdDSA"
            )));
        }
        if !seen.insert(*entry) {
            return Err(PolicyError::IdTokenAlgs(format!(
                "lists {entry:?} twice. It is a set, a subset of the four permitted \
                 algorithms"
            )));
        }
    }
    Ok(ALG_ORDER
        .iter()
        .filter(|a| seen.contains(**a))
        .map(|a| a.to_string())
        .collect())
}

/// RFC 096 `:361`: 1-8 canonical HTTPS origins, the issuer's origin
/// included. Empty `raw` means "the issuer's origin alone" (`discovery.
/// rs:354-355`'s own meaning, reused rather than redecided). The
/// canonical-URL rule: an origin input must equal `Url::parse(s)?.
/// origin().ascii_serialization()` byte for byte — refuses
/// `https://Example.com`, `https://example.com:443`, and
/// `https://example.com/` (verified empirically against this project's
/// `url` crate version before relying on it: each of those three differs
/// from its own origin's ascii serialization).
fn validate_origins(issuer: &str, raw: &[&str]) -> Result<Vec<String>, PolicyError> {
    let issuer_url = url::Url::parse(issuer)
        .map_err(|_| PolicyError::Origins(format!("issuer {issuer:?} does not parse as a URL")))?;
    let issuer_origin = issuer_url.origin().ascii_serialization();

    let origins: Vec<String> = if raw.is_empty() {
        vec![issuer_origin.clone()]
    } else {
        raw.iter().map(|s| s.to_string()).collect()
    };

    if origins.len() > 8 {
        return Err(PolicyError::Origins(format!(
            "has {} entries, more than the maximum of 8",
            origins.len()
        )));
    }
    let mut seen = BTreeSet::new();
    for origin in &origins {
        let Ok(u) = url::Url::parse(origin) else {
            return Err(PolicyError::Origins(format!(
                "{origin:?} is not a valid URL"
            )));
        };
        if u.scheme() != "https" {
            return Err(PolicyError::Origins(format!("{origin:?} must be https")));
        }
        let canonical = u.origin().ascii_serialization();
        if canonical != *origin {
            return Err(PolicyError::Origins(format!(
                "{origin:?} is not canonical (expected {canonical:?})"
            )));
        }
        if !seen.insert(origin.clone()) {
            return Err(PolicyError::Origins(format!("{origin:?} is listed twice")));
        }
    }
    if !seen.contains(&issuer_origin) {
        return Err(PolicyError::Origins(format!(
            "must include the issuer's own origin {issuer_origin:?}"
        )));
    }
    Ok(origins)
}

/// RFC 096 `:362`, derived: the set of the origins' effective ports, not
/// a separate setting (the stage plan's own reasoning, `096-b2-stage-
/// plan-2026-11-11.md` §6: a separate `https_ports` setting would enforce
/// nothing the origin set does not already enforce, and an operator could
/// misread it).
fn derive_ports(origins: &[String]) -> Vec<u16> {
    let mut ports: BTreeSet<u16> = BTreeSet::new();
    for origin in origins {
        if let Ok(u) = url::Url::parse(origin)
            && let Some(port) = u.port_or_known_default()
        {
            ports.insert(port);
        }
    }
    ports.into_iter().collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;
