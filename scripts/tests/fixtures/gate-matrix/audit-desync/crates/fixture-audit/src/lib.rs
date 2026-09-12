//! A3.2 fixture crate: the source side of the audit-coverage check.
//!
//! Never compiled. Since G13-b the script derives its namespace allowlist
//! from `action:` literals, so each event here is written in the
//! `AuditLogRow` shape the real emitters use: a bare string would declare no
//! namespace and fall outside the gate.
//!
//! - client.in_source_only: the backward-direction desync, with no matrix row.
//! - webauthn.fixture_only: the namespace-blindness desync, with no matrix row,
//!   in a namespace the pre-G13-b hand-written allowlist did not contain, so
//!   only a derived allowlist can report it.
//! - user.fixture_declared: in sync. It exists so the `user` namespace is
//!   declared and the matrix's forward-direction desync stays visible.

pub struct AuditLogRow {
    pub action: &'static str,
}

pub fn emit() -> [AuditLogRow; 4] {
    [
        AuditLogRow { action: "auth.login" },
        AuditLogRow { action: "client.in_source_only" },
        AuditLogRow { action: "webauthn.fixture_only" },
        AuditLogRow { action: "user.fixture_declared" },
    ]
}
