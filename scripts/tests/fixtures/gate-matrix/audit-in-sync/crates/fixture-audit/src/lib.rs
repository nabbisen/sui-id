//! A3.2 fixture crate: the source side of the audit-coverage check.
//!
//! Never compiled. G13 is a bash-and-grep lane, so what matters is that
//! these literals are written the way the real emitters write them, which
//! is what `scripts/check-audit-matrix.sh` greps for.

pub fn emit() {
    let _ = "auth.login";
    let _ = "user.create";
}
