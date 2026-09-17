// RFC 102 B4: `not_applicable:system_principal` is written only by an entry
// that takes no session and runs as the system principal (U07's CLI). A
// session-bound gated command computes `SessionStepUpEvidence`, which has no
// such form, so it cannot record one.

use sui_id_store::commands::SessionStepUpEvidence;

fn attempt() -> SessionStepUpEvidence {
    SessionStepUpEvidence::NotApplicable
}

fn main() {}
