use super::{HibpMode, HibpModeChange, classify_hibp_mode_change};

/// RFC 149 Q3b §2: every pair, both directions, including the
/// diagonal — the chooser itself, not just one call site's use of
/// it (`settings.rs`'s own mutation table tests the call sites;
/// this pins the function they both call).
#[test]
fn every_pair_classifies_as_its_own_direction() {
    let modes = [HibpMode::Off, HibpMode::Warn, HibpMode::Block];
    for &old in &modes {
        for &new in &modes {
            let got = classify_hibp_mode_change(old, new);
            let expected = match (old, new) {
                (HibpMode::Off, HibpMode::Off)
                | (HibpMode::Warn, HibpMode::Warn)
                | (HibpMode::Block, HibpMode::Block) => HibpModeChange::Unchanged,
                (HibpMode::Off, HibpMode::Warn)
                | (HibpMode::Off, HibpMode::Block)
                | (HibpMode::Warn, HibpMode::Block) => HibpModeChange::Strengthened,
                (HibpMode::Warn, HibpMode::Off)
                | (HibpMode::Block, HibpMode::Off)
                | (HibpMode::Block, HibpMode::Warn) => HibpModeChange::Weakened,
            };
            assert_eq!(got, expected, "{old:?} -> {new:?}");
        }
    }
}
