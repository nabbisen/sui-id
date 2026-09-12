//! A3.2 fixture crate: the source side of the audit-coverage check.
//!
//! Never compiled. G13 is a bash-and-grep lane, so what matters is that
//! these literals are written the way the real emitters write them — as the
//! `action:` of an `AuditLogRow` — because that shape is what
//! `scripts/check-audit-matrix.sh` derives its namespace allowlist from.

pub struct AuditLogRow {
    pub action: &'static str,
}

pub fn emit() -> [AuditLogRow; 2] {
    [
        AuditLogRow { action: "auth.login" },
        AuditLogRow { action: "user.create" },
    ]
}
