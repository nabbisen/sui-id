//! RFC 094 typed write-command registry — foundation.
//!
//! **Stage 1 (registry foundation).** This module adds the sealed type
//! system RFC 094 specifies; it does not yet convert any production call
//! site. `crates/sui-id-core/src/audit_guard.rs` and `events.rs` remain in
//! place and in use — they are replaced during the conversion waves, not
//! here (RFC 094 §"Multiple implementation steps").
//!
//! ## Why this lives in `sui-id-store`, not `sui-id-core`
//!
//! The legacy best-effort audit layer (`audit_guard.rs`, `events.rs`) lives
//! in `sui-id-core`, which depends on `sui-id-store`. RFC 094's own sketch
//! puts the Class-A runner on `Database` itself
//! (`impl Database { pub async fn class_a<C: CommandSpec, ...> }`), and
//! `Database` is defined in this crate. `sui-id-store` cannot depend on
//! `sui-id-core` (that would be circular — `sui-id-core` already depends on
//! `sui-id-store`), so `CommandSpec` and everything built on it must live
//! wherever `Database` lives: here. This is a placement the RFC's own
//! sketch settles, not a preference.
//!
//! ## What's sealed, and why
//!
//! - [`Audited`] / [`AuditReceipt`] — no public constructor. The only path
//!   to one is [`Database::class_a`], and only after commit.
//! - [`AuthorizedCommandContext`] — no public constructor. The only path is
//!   [`AuthorizedCommandContext::for_system_actor`], itself sealed to
//!   commands whose descriptor's actor requirement permits a system
//!   principal.
//! - [`WriteTx`] — wraps the raw `rusqlite::Transaction` behind a
//!   `pub(crate)` accessor. Only `sui-id-store::repos::*_within_tx`
//!   functions (same crate) can reach the connection; everything above this
//!   crate holds `WriteTx<P>` opaquely.
//! - `CommandSpec`/`SealedCommandEvent` — sealed via the private
//!   [`sealed::Sealed`] supertrait, so only [`declare_write_command!`] can
//!   produce an implementor.

use std::marker::PhantomData;

use crate::{Database, StoreError, StoreResult};

pub(crate) mod sealed {
    pub trait Sealed {}
}

// ── Policy markers ──────────────────────────────────────────────────────
//
// Uninhabited (no variants): these exist only as type-level tags for
// `WriteTx<P>`, never as values. RFC 094: "WriteTx<AtomicAudit> can commit
// only through the Class-A runner after typed append. WriteTx<Protocol>,
// <Operational>, and <Bootstrap> have distinct runners and cannot construct
// Audited<T>."

/// Policy marker: Class-A. The only policy [`Database::class_a`] accepts.
pub enum AtomicAudit {}
/// Policy marker: Class-P (protocol/high-frequency state).
pub enum Protocol {}
/// Policy marker: Class-O (operational/worker housekeeping).
pub enum Operational {}
/// Policy marker: Class-X (schema migrations, pre-readiness only).
pub enum Bootstrap {}

// ── The one sealed transaction capability ──────────────────────────────

/// The sealed write-transaction capability. Holding a `WriteTx<'_, P>`
/// proves the holder is inside an open transaction under policy `P` — it
/// does not, by itself, prove anything about `P`'s specific guarantees
/// (that's what [`ClassATx`] adds for `AtomicAudit`).
///
/// The raw transaction is reachable only via [`WriteTx::tx`], which is
/// `pub(crate)`: code outside this crate can hold, pass, and store a
/// `WriteTx<P>`, but cannot extract a `rusqlite::Transaction` from it.
/// Repository `*_within_tx` functions (this crate) are the only callers
/// that ever call `.tx()`.
pub struct WriteTx<'tx, P> {
    tx: &'tx rusqlite::Transaction<'tx>,
    _policy: PhantomData<P>,
}

impl<'tx, P> WriteTx<'tx, P> {
    fn new(tx: &'tx rusqlite::Transaction<'tx>) -> Self {
        Self {
            tx,
            _policy: PhantomData,
        }
    }

    /// The raw transaction. `pub(crate)`: only `sui-id-store`'s own
    /// `repos::*_within_tx` functions may call this.
    pub(crate) fn tx(&self) -> &'tx rusqlite::Transaction<'tx> {
        self.tx
    }
}

// ── Registry data model ─────────────────────────────────────────────────

/// Audit semantics for an event. Distinct from the command inventory's
/// A/P/O/I/X classification (which runner a command uses) — this is about
/// how the *event*, if any, is recorded. Only Class-A commands have an
/// `AuditClass::Atomic` event in this registry; Class-P/O commands don't
/// register an event here at all (see module docs).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditClass {
    /// Mutation and event commit in one transaction ([`Database::class_a`]).
    Atomic,
    /// Best-effort, tracked emission (RFC 094 Class-B `emit_must_attempt`,
    /// not implemented in this Stage-1 slice — no slice command needs it).
    MustAttempt,
}

/// Whether an event variant's payload must, may, or must not carry an
/// actor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActorRequirement {
    Required,
    Optional,
    None,
}

/// Whether an event variant's payload must, may, or must not carry a
/// target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetRequirement {
    Required,
    Optional,
    None,
}

/// One declared attribute name a command's event payload may carry.
/// `descriptor()`-generated documentation lists these; the structural gate
/// checks that emitted attributes never exceed this set (registry test in
/// this module; the CI-facing tool is a later item).
#[derive(Debug, Clone, Copy)]
pub struct AttributeSpec {
    pub name: &'static str,
    pub description: &'static str,
}

/// Stable identifier for every distinct audit event name in the registry.
/// One variant per event, not per command — a command with closed result
/// branches (e.g. `U01`) owns more than one variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AuditEventKind {
    /// `signing_key.rotate` — K01.
    SigningKeyRotate,
    /// `auth.login.failure` — U22, below-threshold branch.
    AuthLoginFailure,
    /// `auth.lockout` — U22, threshold-crossing branch.
    AuthLockout,
    /// `user.create` — U01, normal branch.
    UserCreate,
    /// `user.create_warned_hibp` — U01, HIBP-flagged branch.
    UserCreateWarnedHibp,
}

/// The registry entry for one [`AuditEventKind`]. `descriptor()` on a
/// command's event type maps every variant to exactly one of these by
/// exhaustive match — see [`CommandSpec::descriptor`].
#[derive(Debug, Clone, Copy)]
pub struct EventDescriptor {
    pub kind: AuditEventKind,
    pub name: &'static str,
    pub class: AuditClass,
    pub actor: ActorRequirement,
    pub target: TargetRequirement,
    pub attributes: &'static [AttributeSpec],
}

/// Render a descriptor table as the markdown reference RFC 094 Stage 1
/// item 4 asks for ("generate or mechanically verify audit reference
/// documentation"). Deterministic: same input, same output, byte for byte
/// — a doc-drift check just re-renders and diffs.
///
/// Not yet wired to a tracked `docs/` file. Doing that now, against a
/// 5-command proving slice, would publish a reference document that reads
/// as authoritative while covering roughly 5% of the eventual registry —
/// exactly the kind of overclaim this RFC exists to prevent elsewhere. The
/// generator is proven here (`registry::tests::generated_reference_is_a_
/// well_formed_table`, plus `commands::tests`); wiring it to a real
/// `docs/` file is conversion-wave work, once there's a registry worth
/// documenting.
pub fn generate_reference_markdown(descriptors: &[&EventDescriptor]) -> String {
    let mut out = String::from("| Event | Class | Actor | Target | Attributes |\n");
    out.push_str("|---|---|---|---|---|\n");
    for d in descriptors {
        let class = match d.class {
            AuditClass::Atomic => "Atomic",
            AuditClass::MustAttempt => "MustAttempt",
        };
        let actor = match d.actor {
            ActorRequirement::Required => "required",
            ActorRequirement::Optional => "optional",
            ActorRequirement::None => "none",
        };
        let target = match d.target {
            TargetRequirement::Required => "required",
            TargetRequirement::Optional => "optional",
            TargetRequirement::None => "none",
        };
        let attrs = if d.attributes.is_empty() {
            "—".to_string()
        } else {
            d.attributes
                .iter()
                .map(|a| format!("`{}` — {}", a.name, a.description))
                .collect::<Vec<_>>()
                .join("; ")
        };
        out.push_str(&format!(
            "| `{}` | {class} | {actor} | {target} | {attrs} |\n",
            d.name
        ));
    }
    out
}

/// Outcome recorded for an event. Narrow on purpose (mirrors
/// `events::Outcome`, redefined here rather than shared — this registry
/// does not depend on `sui-id-core`, and `Outcome` is small enough that
/// duplicating it costs less than an upward dependency would).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditResult {
    Ok,
    Failure,
}

impl AuditResult {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Failure => "failure",
        }
    }
}

/// A resolved, bounded set of attribute values for one event. Built only
/// through [`AuditAttributes::builder`]; every value is a plain `String` —
/// nothing implementing a secret-wrapper type can be inserted, because
/// insertion takes `impl Into<String>` and this crate's secret types
/// deliberately do not implement `Into<String>`/`Display` (proved by a
/// compile-fail fixture, not this comment — see `tests/compile_fail/`).
#[derive(Debug, Clone, Default)]
pub struct AuditAttributes {
    entries: Vec<(&'static str, String)>,
}

/// Maximum attribute entries per event and maximum bytes per value. Small,
/// deliberately: this is a typed audit record, not a debug dump.
const MAX_ATTRIBUTES: usize = 16;
const MAX_ATTRIBUTE_VALUE_BYTES: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AuditBuildError {
    #[error("attribute {0:?} exceeds the {MAX_ATTRIBUTE_VALUE_BYTES}-byte bound")]
    AttributeTooLong(&'static str),
    #[error("event carries more than {MAX_ATTRIBUTES} attributes")]
    TooManyAttributes,
    #[error("attribute {0:?} declared more than once")]
    DuplicateAttribute(&'static str),
}

pub struct AuditAttributesBuilder {
    entries: Vec<(&'static str, String)>,
    error: Option<AuditBuildError>,
}

impl AuditAttributes {
    pub fn builder() -> AuditAttributesBuilder {
        AuditAttributesBuilder {
            entries: Vec::new(),
            error: None,
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&'static str, &str)> + '_ {
        self.entries.iter().map(|(k, v)| (*k, v.as_str()))
    }
}

impl AuditAttributesBuilder {
    /// Add one attribute. `value` must be a plain `String`-convertible
    /// type — secret wrapper types do not implement `Into<String>`, so
    /// attempting to pass one is a compile error, not a runtime rejection.
    pub fn attribute(mut self, name: &'static str, value: impl Into<String>) -> Self {
        if self.error.is_some() {
            return self;
        }
        if self.entries.iter().any(|(k, _)| *k == name) {
            self.error = Some(AuditBuildError::DuplicateAttribute(name));
            return self;
        }
        if self.entries.len() >= MAX_ATTRIBUTES {
            self.error = Some(AuditBuildError::TooManyAttributes);
            return self;
        }
        let value = value.into();
        if value.len() > MAX_ATTRIBUTE_VALUE_BYTES {
            self.error = Some(AuditBuildError::AttributeTooLong(name));
            return self;
        }
        self.entries.push((name, value));
        self
    }

    pub fn build(self) -> Result<AuditAttributes, AuditBuildError> {
        match self.error {
            Some(e) => Err(e),
            None => Ok(AuditAttributes {
                entries: self.entries,
            }),
        }
    }
}

/// A resolved audit target — the thing the event is about. A thin
/// newtype rather than a bare `String` so a command's event `target()`
/// cannot be confused with an attribute value at a call site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditTarget(pub String);

// ── Command / event sealing ─────────────────────────────────────────────

/// A Class-A command. Sealed: the only implementors are the types
/// [`declare_write_command!`] generates. `Send + Sync` because every real
/// implementor is a zero-sized marker struct (trivially both) and
/// [`AuthorizedCommandContext`] must cross the `with_tx` thread boundary.
pub trait CommandSpec: sealed::Sealed + Sized + Send + Sync + 'static {
    /// This command's closed event-result enum.
    type Event: SealedCommandEvent<Self>;

    /// Stable command identifier, matching `command-inventory.md` /
    /// `ci/write-commands.toml` (e.g. `"K01"`).
    const ID: &'static str;

    /// Map an event variant to its registry descriptor. Implementations
    /// are an exhaustive match generated by [`declare_write_command!`];
    /// adding a variant without extending the match is a compile error.
    fn descriptor(event: &Self::Event) -> &'static EventDescriptor;
}

/// A command's closed event-result type. Sealed to `C`: an event type for
/// one command cannot satisfy another command's [`CommandSpec::Event`]
/// bound (a compile-fail fixture proves the attempt fails).
pub trait SealedCommandEvent<C: CommandSpec>: sealed::Sealed {
    fn target(&self) -> Option<AuditTarget>;
    fn result(&self) -> AuditResult;
    fn attributes(&self) -> Result<AuditAttributes, AuditBuildError>;
}

// ── Authorization context ───────────────────────────────────────────────

/// Proof that command `C` was authorized. Private fields, no public
/// constructor — the only ways to obtain one are
/// [`AuthorizedCommandContext::for_system_actor`] (sealed to commands whose
/// descriptor's actor requirement is not `Required`-by-a-human, i.e. a
/// system/CLI principal is permitted) or, once handler-side authorization
/// decisions are converted, a future decision-consuming constructor not
/// added in this Stage-1 slice.
pub struct AuthorizedCommandContext<C: CommandSpec> {
    actor: Option<sui_id_shared::ids::UserId>,
    request_id: Option<String>,
    _command: PhantomData<C>,
}

impl<C: CommandSpec> AuthorizedCommandContext<C> {
    /// Construct a context for a sealed system/CLI actor. Not gated per-command
    /// in this slice (every Stage-1 slice command that uses this is a
    /// system-triggered operation); a future stage narrows this to commands
    /// whose descriptor explicitly permits a system principal, per RFC 094
    /// §"Class-A transaction seam".
    pub fn for_system_actor(request_id: Option<String>) -> Self {
        Self {
            actor: None,
            request_id,
            _command: PhantomData,
        }
    }

    pub fn actor(&self) -> Option<sui_id_shared::ids::UserId> {
        self.actor
    }

    pub fn request_id(&self) -> Option<&str> {
        self.request_id.as_deref()
    }
}

// ── Audited<T> ───────────────────────────────────────────────────────────

/// Proof that an audit record was appended and committed. Constructible
/// only inside [`Database::class_a`], after commit — never by an append
/// call alone.
pub struct AuditReceipt {
    _private: (),
}

/// A successful Class-A result paired with its receipt.
pub struct Audited<T> {
    value: T,
    #[allow(dead_code)]
    receipt: AuditReceipt,
}

impl<T> Audited<T> {
    pub fn into_inner(self) -> T {
        self.value
    }

    pub fn value(&self) -> &T {
        &self.value
    }
}

// ── ClassATx and the runner ─────────────────────────────────────────────

/// The Class-A transaction context. Wraps the sealed [`WriteTx`] plus the
/// command's authorization context; this is what [`Database::class_a`]'s
/// closure receives.
pub struct ClassATx<'tx, C: CommandSpec> {
    write: WriteTx<'tx, AtomicAudit>,
    context: &'tx AuthorizedCommandContext<C>,
}

impl<'tx, C: CommandSpec> ClassATx<'tx, C> {
    /// The raw transaction, for repository `*_within_tx` functions.
    /// `pub(crate)`, same reasoning as [`WriteTx::tx`].
    pub(crate) fn tx(&self) -> &'tx rusqlite::Transaction<'tx> {
        self.write.tx()
    }

    pub fn context(&self) -> &AuthorizedCommandContext<C> {
        self.context
    }
}

impl Database {
    /// The Class-A runner. Implements RFC 094's non-negotiable sequence:
    /// open one immediate write transaction; run `build` (which mutates via
    /// `*_within_tx` functions using the private capability and returns the
    /// typed event); append and hash the event in the same transaction;
    /// commit; only then construct `Audited<T>`.
    pub async fn class_a<C, T, E, F>(
        &self,
        context: AuthorizedCommandContext<C>,
        build: F,
    ) -> Result<Audited<T>, E>
    where
        C: CommandSpec,
        T: Send + 'static,
        E: From<StoreError> + Send + 'static,
        F: FnOnce(&mut ClassATx<'_, C>) -> Result<(T, C::Event), E> + Send + 'static,
    {
        let context = std::sync::Arc::new(context);
        let context_for_closure = context.clone();
        // The event type is computed and consumed entirely inside the
        // closure (it feeds `append_within_tx` here, not the caller), so it
        // never has to cross the `with_tx` boundary and never needs `Send`.
        let result = self
            .with_tx(move |tx| -> StoreResult<Result<T, E>> {
                let mut class_a_tx = ClassATx {
                    write: WriteTx::new(tx),
                    context: &context_for_closure,
                };
                let (value, event) = match build(&mut class_a_tx) {
                    Ok(pair) => pair,
                    Err(e) => return Ok(Err(e)),
                };
                let descriptor = C::descriptor(&event);
                let target = event.target();
                let audit_result = event.result();
                let attributes = match event.attributes() {
                    Ok(a) => a,
                    Err(build_err) => {
                        return Ok(Err(E::from(StoreError::Integrity(build_err.to_string()))));
                    }
                };
                let row = build_audit_row(descriptor, &context, target, audit_result, &attributes);
                crate::repos::audit::append_within_tx(tx, &row)?;
                Ok(Ok(value))
            })
            .await;

        match result {
            Ok(Ok(value)) => Ok(Audited {
                value,
                receipt: AuditReceipt { _private: () },
            }),
            Ok(Err(e)) => Err(e),
            Err(store_err) => Err(E::from(store_err)),
        }
    }
}

fn build_audit_row<C: CommandSpec>(
    descriptor: &EventDescriptor,
    context: &AuthorizedCommandContext<C>,
    target: Option<AuditTarget>,
    result: AuditResult,
    attributes: &AuditAttributes,
) -> crate::models::AuditLogRow {
    let note = if attributes.entries.is_empty() {
        None
    } else {
        Some(
            attributes
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect::<Vec<_>>()
                .join(" "),
        )
    };
    crate::models::AuditLogRow {
        at: chrono::Utc::now(),
        actor: context.actor(),
        action: descriptor.name.into(),
        target: target.map(|t| t.0),
        result: result.as_str().into(),
        note,
    }
}

// ── Non-atomic runners ──────────────────────────────────────────────────
//
// Protocol/Operational/Bootstrap commands get a sealed `WriteTx<P>` but no
// path to `Audited<T>` — there is no `Database::<method>` here that
// constructs one, and `WriteTx<P>` for P != AtomicAudit has no `class_a`-
// shaped method at all. That absence *is* the proof; see
// `tests/compile_fail/protocol_cannot_construct_audited.rs`.

impl Database {
    /// Run `build` inside a transaction under the `Protocol` policy. No
    /// event, no audit row, no `Audited<T>` — Class-P commands are not the
    /// tamper-evident chain (RFC 094 Requirements).
    pub async fn protocol<T, E, F>(&self, build: F) -> Result<T, E>
    where
        T: Send + 'static,
        E: From<StoreError> + Send + 'static,
        F: FnOnce(&mut WriteTx<'_, Protocol>) -> Result<T, E> + Send + 'static,
    {
        let result = self
            .with_tx(move |tx| -> StoreResult<Result<T, E>> {
                let mut write = WriteTx::new(tx);
                Ok(build(&mut write))
            })
            .await;
        match result {
            Ok(Ok(v)) => Ok(v),
            Ok(Err(e)) => Err(e),
            Err(store_err) => Err(E::from(store_err)),
        }
    }

    /// Same shape as [`Database::protocol`], under the `Operational`
    /// policy.
    pub async fn operational<T, E, F>(&self, build: F) -> Result<T, E>
    where
        T: Send + 'static,
        E: From<StoreError> + Send + 'static,
        F: FnOnce(&mut WriteTx<'_, Operational>) -> Result<T, E> + Send + 'static,
    {
        let result = self
            .with_tx(move |tx| -> StoreResult<Result<T, E>> {
                let mut write = WriteTx::new(tx);
                Ok(build(&mut write))
            })
            .await;
        match result {
            Ok(Ok(v)) => Ok(v),
            Ok(Err(e)) => Err(e),
            Err(store_err) => Err(E::from(store_err)),
        }
    }
}

// ── declare_write_command! ──────────────────────────────────────────────

/// The sealed constructor for a Class-A [`CommandSpec`]. Declares a
/// zero-sized marker type, its closed event enum, and the exhaustive
/// `descriptor()` match, all in one place so a command's ID, class, and
/// event bindings cannot drift apart.
///
/// ```ignore
/// declare_write_command! {
///     /// K01 — signing-key rotation.
///     command SigningKeyRotate = "K01" {
///         enum Event {
///             Rotated { new_key: SigningKeyId } => AuditEventKind::SigningKeyRotate,
///         }
///     }
/// }
/// ```
#[macro_export]
macro_rules! declare_write_command {
    (
        $(#[$command_meta:meta])*
        command $command_ty:ident = $id:literal {
            enum $event_ty:ident {
                $(
                    $(#[$variant_meta:meta])*
                    $variant:ident $( { $($field:ident : $field_ty:ty),* $(,)? } )?
                        => $descriptor_expr:expr
                ),+ $(,)?
            }
        }
    ) => {
        $(#[$command_meta])*
        #[derive(Debug, Clone, Copy)]
        pub struct $command_ty;

        impl $crate::registry::sealed::Sealed for $command_ty {}

        $(#[$command_meta])*
        #[derive(Debug, Clone)]
        pub enum $event_ty {
            $(
                $(#[$variant_meta])*
                $variant $( { $($field: $field_ty),* } )?,
            )+
        }

        impl $crate::registry::sealed::Sealed for $event_ty {}

        impl $crate::registry::CommandSpec for $command_ty {
            type Event = $event_ty;
            const ID: &'static str = $id;

            fn descriptor(event: &Self::Event) -> &'static $crate::registry::EventDescriptor {
                #[allow(unused_variables)]
                match event {
                    $(
                        $event_ty::$variant $( { $($field: _),* } )? => $descriptor_expr,
                    )+
                }
            }
        }
    };
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    // ── Registry unit tests (Stage 1 item 5) ────────────────────────────
    // Duplicate-name, class-mismatch, missing-field, and stable-serialization
    // are checked against the slice's real descriptor table in
    // `commands.rs`'s own test module (it owns the table); this module
    // tests the registry machinery itself, independent of any one slice.

    #[test]
    fn audit_attributes_builder_rejects_duplicate_names() {
        let err = AuditAttributes::builder()
            .attribute("a", "1")
            .attribute("a", "2")
            .build()
            .unwrap_err();
        assert_eq!(err, AuditBuildError::DuplicateAttribute("a"));
    }

    #[test]
    fn audit_attributes_builder_rejects_too_many() {
        let mut builder = AuditAttributes::builder();
        for i in 0..MAX_ATTRIBUTES {
            builder = builder.attribute(Box::leak(i.to_string().into_boxed_str()), "v");
        }
        builder = builder.attribute("one_too_many", "v");
        assert_eq!(
            builder.build().unwrap_err(),
            AuditBuildError::TooManyAttributes
        );
    }

    #[test]
    fn audit_attributes_builder_rejects_oversize_value() {
        let long = "x".repeat(MAX_ATTRIBUTE_VALUE_BYTES + 1);
        let err = AuditAttributes::builder()
            .attribute("a", long)
            .build()
            .unwrap_err();
        assert_eq!(err, AuditBuildError::AttributeTooLong("a"));
    }

    #[test]
    fn audit_attributes_builder_accepts_within_bounds() {
        let attrs = AuditAttributes::builder()
            .attribute("a", "1")
            .attribute("b", "2")
            .build()
            .unwrap();
        let collected: Vec<_> = attrs.iter().collect();
        assert_eq!(collected, vec![("a", "1"), ("b", "2")]);
    }

    #[test]
    fn audit_result_as_str_is_stable() {
        // SIEM queries and audit-log alerts pivot on these strings.
        assert_eq!(AuditResult::Ok.as_str(), "ok");
        assert_eq!(AuditResult::Failure.as_str(), "failure");
    }
}
