//! RFC 094 cycle J stage 7c: the markdown table reader shared by the
//! audit-coverage check (`coverage.rs`) and the column check (`columns.rs`),
//! ported from `scripts/check-audit-matrix-columns.py`'s `split_row` and
//! `read_matrix` so the two Rust checks parse one matrix file one way,
//! rather than each carrying its own copy that could drift from the other.
//!
//! A row is recognised only under a header whose first cell is (case-
//! insensitively) `Event name`; a name-shaped first cell under any other
//! header is reported as a schema violation rather than silently ignored or
//! silently accepted -- a table shape change cannot make its rows invisible
//! to one check while still visible to the other.

use std::collections::BTreeMap;

/// Cells of one markdown table line, honouring a `|` inside inline code or
/// escaped as `\|`.
pub(crate) fn split_row(line: &str) -> Vec<String> {
    let mut s = line.trim();
    if let Some(rest) = s.strip_prefix('|') {
        s = rest;
    }
    if s.ends_with('|') && !s.ends_with("\\|") {
        s = &s[..s.len() - 1];
    }
    let chars: Vec<char> = s.chars().collect();
    let mut cells = Vec::new();
    let mut cur = String::new();
    let mut in_code = false;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\\' && chars.get(i + 1) == Some(&'|') {
            cur.push('|');
            i += 2;
            continue;
        }
        if c == '`' {
            in_code = !in_code;
        }
        if c == '|' && !in_code {
            cells.push(cur.trim().to_string());
            cur.clear();
        } else {
            cur.push(c);
        }
        i += 1;
    }
    cells.push(cur.trim().to_string());
    cells
}

/// A markdown table rule line (`|---|---|`, optionally with `:` alignment
/// markers): every cell is three-or-more dashes, optionally colon-bounded.
#[allow(clippy::expect_used)]
pub(crate) fn is_separator(cells: &[String]) -> bool {
    let re = regex::Regex::new(r"^:?-{3,}:?$").expect("fixed pattern");
    !cells.is_empty() && cells.iter().all(|c| re.is_match(c))
}

/// One row of an `Event name`-headed table: its line number, its event
/// name (the first cell, unbackticked), and the full lower-cased-header ->
/// cell-text map -- the shape both the coverage check (which uses only
/// `event`) and the column check (which uses the whole map) need.
pub(crate) struct MatrixRow {
    pub(crate) line: usize,
    pub(crate) event: String,
    pub(crate) cells: BTreeMap<String, String>,
}

#[allow(clippy::expect_used)]
fn event_cell_re() -> regex::Regex {
    regex::Regex::new(r"^`([a-z0-9_]+(?:\.[a-z0-9_]+)+)`$").expect("fixed pattern")
}

#[allow(clippy::expect_used)]
fn nameish_re() -> regex::Regex {
    regex::Regex::new(r"^`?[a-z0-9_]+(?:\.[a-z0-9_]+)+`?$").expect("fixed pattern")
}

/// Every `Event name`-headed row, and (separately) a schema violation for:
/// a malformed Event-name cell, a cell-count mismatch against the header, or
/// a name-shaped first cell under a header that does not start with `Event
/// name`. Direct port of `check-audit-matrix-columns.py`'s `read_matrix`.
pub(crate) fn read_matrix(text: &str) -> (Vec<MatrixRow>, Vec<String>) {
    let event_cell = event_cell_re();
    let nameish = nameish_re();
    let lines: Vec<&str> = text.lines().collect();
    let mut rows = Vec::new();
    let mut failures = Vec::new();
    let mut header: Option<Vec<String>> = None;
    let mut in_fence = false;
    for (idx, line) in lines.iter().enumerate() {
        let n = idx + 1;
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            header = None;
            continue;
        }
        if in_fence || !line.trim_start().starts_with('|') {
            header = None;
            continue;
        }
        let cells = split_row(line);
        if header.is_none() {
            let next = lines.get(idx + 1).map(|l| split_row(l)).unwrap_or_default();
            if is_separator(&next) {
                header = Some(cells.iter().map(|c| c.to_lowercase()).collect());
            }
            continue;
        }
        if is_separator(&cells) {
            continue;
        }
        let Some(header_cells) = header.as_ref() else {
            continue;
        };
        let first = cells.first().cloned().unwrap_or_default();
        let header_first = header_cells.first().cloned().unwrap_or_default();
        if header_first == "event name" {
            let Some(m) = event_cell.captures(&first) else {
                failures.push(format!(
                    "line {n}: the Event name cell is not a single backticked event name: {first:?}"
                ));
                continue;
            };
            if cells.len() != header_cells.len() {
                failures.push(format!(
                    "line {n}: {} cells under a {}-column header",
                    cells.len(),
                    header_cells.len()
                ));
                continue;
            }
            let cell_map: BTreeMap<String, String> = header_cells
                .iter()
                .cloned()
                .zip(cells.iter().cloned())
                .collect();
            rows.push(MatrixRow {
                line: n,
                event: m[1].to_string(),
                cells: cell_map,
            });
        } else if nameish.is_match(&first) {
            failures.push(format!(
                "line {n}: a table row whose first cell names an event ({first}) under a \
                 header that does not start with `Event name` ({header_first:?}); its columns \
                 cannot be checked, so it must not be there"
            ));
        }
    }
    (rows, failures)
}
