//! RFC 149 Q4 §1: the near-miss / standard-scope check for the app
//! form's *Additional scopes* field, and the distance function it is
//! built on — implemented once, tested on its own (the dispatch's own
//! instruction), not duplicated at each caller.

use crate::discovery::STANDARD_SCOPES;

/// Restricted edit distance — Levenshtein's three operations (insert,
/// delete, substitute) plus a fourth: swapping two adjacent
/// characters counts as one edit too (the "optimal string alignment"
/// variant of Damerau-Levenshtein). Without the fourth, "emial" is
/// two edits from "email" (substitute the `a`, substitute the `i`),
/// not the one a reader would call a typo.
///
/// Operates byte-wise: every scope token this is ever compared
/// against has already passed RFC 6749 §3.3's token syntax (ASCII
/// only), so a byte is a character here.
pub fn restricted_edit_distance(a: &str, b: &str) -> usize {
    let a = a.as_bytes();
    let b = b.as_bytes();
    let (n, m) = (a.len(), b.len());
    let mut dp = vec![vec![0usize; m + 1]; n + 1];
    for (i, row) in dp.iter_mut().enumerate().take(n + 1) {
        row[0] = i;
    }
    for (j, cell) in dp[0].iter_mut().enumerate().take(m + 1) {
        *cell = j;
    }
    for i in 1..=n {
        for j in 1..=m {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            let mut best = (dp[i - 1][j] + 1)
                .min(dp[i][j - 1] + 1)
                .min(dp[i - 1][j - 1] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                best = best.min(dp[i - 2][j - 2] + 1);
            }
            dp[i][j] = best;
        }
    }
    dp[n][m]
}

/// What an *additional scope* token resolves to against
/// [`STANDARD_SCOPES`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StandardScopeMatch {
    /// Not close to any standard scope — a genuine additional scope.
    None,
    /// The exact standard scope, byte for byte, typed where it does
    /// not belong (it has its own checkbox).
    ExactStandardScope(&'static str),
    /// A case-only difference, or within one restricted edit, of a
    /// standard scope — but not that scope's own exact spelling.
    NearMiss(&'static str),
}

/// Classify `token` (already known to pass the RFC 6749 §3.3 token
/// syntax check) against every standard scope. The four standard
/// scopes are distinct enough in both length and content that no
/// token is ever within one edit of more than one of them — the first
/// match found is unambiguous, not merely convenient.
pub fn classify_against_standard_scopes(token: &str) -> StandardScopeMatch {
    if let Some(&exact) = STANDARD_SCOPES.iter().find(|&&s| s == token) {
        return StandardScopeMatch::ExactStandardScope(exact);
    }
    let lower_token = token.to_ascii_lowercase();
    for &standard in STANDARD_SCOPES {
        if lower_token == standard || restricted_edit_distance(&lower_token, standard) <= 1 {
            return StandardScopeMatch::NearMiss(standard);
        }
    }
    StandardScopeMatch::None
}

#[cfg(test)]
#[path = "scope_check/tests.rs"]
mod tests;
