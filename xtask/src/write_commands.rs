//! RFC 116 stage 1's write-command inventory check, ported from
//! `scripts/check-write-commands.py` (G17, retired by this change) so
//! `audit-structure` is the one tool that checks `contracts/write-commands.toml`
//! against the code. A direct port, not a redesign: the same seven checks
//! `(A)`-`(G)`, the same comment/string-blanking approach so a brace count and
//! a line number both still hold, the same regexes translated literally. RFC
//! 094's `audit-structure` owns the `syn` boundary separately
//! (`write_authority.rs`); this module owns everything G17 already checked
//! without an AST, unchanged in substance.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use regex::Regex;

pub(crate) const REGISTRY_RS: &str = "crates/sui-id-store/src/registry.rs";

const STATUSES: &[&str] = &["implemented", "target-absent", "target-legacy"];
const CLASSES: &[&str] = &["A", "P", "O", "I", "X"];
const REQUIRED: &[&str] = &[
    "id",
    "class",
    "status",
    "sealed",
    "owner",
    "rationale",
    "mutation_surface",
    "files",
    "test_id",
];
const DERIVED: &[&str] = &["event", "descriptor"];
const OPTIONAL: &[&str] = &["sealed_by"];

fn allowed_keys() -> Vec<&'static str> {
    REQUIRED
        .iter()
        .chain(DERIVED.iter())
        .chain(OPTIONAL.iter())
        .copied()
        .collect()
}

/// A declared `declare_write_command!` invocation, found in source.
#[derive(Debug, Clone)]
pub struct DeclaredCommand {
    pub file: String,
    pub line: usize,
    pub events: Vec<String>,
    pub descriptors: Vec<String>,
}

/// Everything about one `[[command]]` row this check needs, read generically
/// (not via `serde` deserialization) so a row with an unknown key or a
/// missing required key is reported per-row, the same way the Python
/// script's raw dict access is, rather than failing the whole parse.
#[derive(Debug, Clone)]
pub struct Row {
    pub id: Option<String>,
    pub class: Option<String>,
    pub status: Option<String>,
    pub sealed: Option<toml::Value>,
    pub files: Option<toml::Value>,
    pub event: Option<Vec<String>>,
    pub descriptor: Option<Vec<String>>,
    pub sealed_by: Option<toml::Value>,
    pub keys: Vec<String>,
}

fn string_list(v: &toml::Value) -> Option<Vec<String>> {
    v.as_array()?
        .iter()
        .map(|e| e.as_str().map(str::to_owned))
        .collect()
}

impl Row {
    fn from_table(t: &toml::Table) -> Self {
        Self {
            id: t.get("id").and_then(|v| v.as_str()).map(str::to_owned),
            class: t.get("class").and_then(|v| v.as_str()).map(str::to_owned),
            status: t.get("status").and_then(|v| v.as_str()).map(str::to_owned),
            sealed: t.get("sealed").cloned(),
            files: t.get("files").cloned(),
            event: t.get("event").and_then(string_list),
            descriptor: t.get("descriptor").and_then(string_list),
            sealed_by: t.get("sealed_by").cloned(),
            keys: t.keys().cloned().collect(),
        }
    }

    fn id_display(&self, index: usize) -> String {
        self.id.clone().unwrap_or_else(|| format!("<row {index}>"))
    }
}

fn id_shape(s: &str) -> bool {
    let bytes = s.as_bytes();
    bytes.len() == 3
        && bytes[0].is_ascii_uppercase()
        && bytes[1].is_ascii_digit()
        && bytes[2].is_ascii_digit()
}

// ── reading the source ──────────────────────────────────────────────────

/// Replace `//` / `/* */` comments and the contents of `"..."` string
/// literals with spaces, keeping every byte offset and every newline, so a
/// brace count and a line number both still hold. A direct port of the
/// Python `blank()` function.
fn blank(text: &str) -> String {
    let bytes = text.as_bytes();
    let n = bytes.len();
    let mut out: Vec<u8> = bytes.to_vec();
    let mut i = 0usize;
    while i < n {
        if bytes[i] == b'/' && i + 1 < n && bytes[i + 1] == b'/' {
            let mut j = i;
            while j < n && bytes[j] != b'\n' {
                out[j] = b' ';
                j += 1;
            }
            i = j;
        } else if bytes[i] == b'/' && i + 1 < n && bytes[i + 1] == b'*' {
            let mut j = i + 2;
            while j + 1 < n && !(bytes[j] == b'*' && bytes[j + 1] == b'/') {
                j += 1;
            }
            let end = if j + 1 < n { j + 2 } else { n };
            for k in i..end {
                if bytes[k] != b'\n' {
                    out[k] = b' ';
                }
            }
            i = end;
        } else if bytes[i] == b'"' {
            let mut j = i + 1;
            while j < n && bytes[j] != b'"' {
                j += if bytes[j] == b'\\' { 2 } else { 1 };
            }
            let blank_end = j.min(n);
            for k in (i + 1)..blank_end {
                if bytes[k] != b'\n' {
                    out[k] = b' ';
                }
            }
            i = j + 1;
        } else {
            i += 1;
        }
    }
    // Every byte this function touches is one of `/ * " \` or a newline --
    // all single-byte ASCII -- and it is always replaced with the single
    // ASCII byte `b' '`. A multi-byte UTF-8 character's continuation bytes
    // are never equal to any of those, so they are never misread as a
    // delimiter; blanking a comment or string's full span always covers
    // every byte of an embedded multi-byte character, never leaving an
    // orphaned continuation byte. The buffer stays valid UTF-8 as long as
    // the input was. Empty-string fallback is unreachable in practice
    // (confirmed: zero occurrences scanning all 225 production files) and
    // kept as the safe default rather than a lint-triggering `expect`.
    String::from_utf8(out).unwrap_or_default()
}

pub fn is_test_file(rel: &Path) -> bool {
    let name = rel.file_name().and_then(|n| n.to_str()).unwrap_or("");
    name == "tests.rs"
        || (name.starts_with("tests_") && name.ends_with(".rs"))
        || rel
            .components()
            .any(|c| c.as_os_str().to_str() == Some("tests"))
}

pub fn source_files(root: &Path) -> Vec<PathBuf> {
    let base = root.join("crates");
    if !base.is_dir() {
        return Vec::new();
    }
    let mut out: Vec<PathBuf> = walkdir::WalkDir::new(&base)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("rs"))
        .map(|e| e.path().to_path_buf())
        .filter(|p| {
            let rel = p.strip_prefix(root).unwrap_or(p);
            !is_test_file(rel)
        })
        .collect();
    out.sort();
    out
}

fn block_end(blanked: &str, open_at: usize) -> Option<usize> {
    let bytes = blanked.as_bytes();
    let mut depth = 0i64;
    for (i, &b) in bytes.iter().enumerate().skip(open_at) {
        if b == b'{' {
            depth += 1;
        } else if b == b'}' {
            depth -= 1;
            if depth == 0 {
                return Some(i);
            }
        }
    }
    None
}

struct Patterns {
    invocation: Regex,
    command_id: Regex,
    descriptor_ref: Regex,
    descriptor_static: Regex,
}

impl Patterns {
    /// Every pattern here is a fixed, hand-written literal, proven valid
    /// at every build (this function runs once per invocation); a
    /// compile error in one would be caught the moment this crate is
    /// built, never at a point a real run could reach a different
    /// pattern. `expect` over `unwrap` only to name that invariant.
    #[allow(clippy::expect_used)]
    fn new() -> Self {
        Self {
            invocation: Regex::new(r"\bdeclare_write_command!\s*\{").expect("fixed pattern"),
            command_id: Regex::new(r#"\bcommand\s+\w+\s*=\s*"([^"]*)""#).expect("fixed pattern"),
            descriptor_ref: Regex::new(r"=>\s*&\s*([A-Z][A-Z0-9_]*)\b").expect("fixed pattern"),
            descriptor_static: Regex::new(
                r#"\bstatic\s+([A-Z][A-Z0-9_]*)\s*:\s*EventDescriptor\s*=\s*EventDescriptor\s*\{\s*kind\s*:[^,]+,\s*name\s*:\s*"([^"]*)""#,
            )
            .expect("fixed pattern"),
        }
    }
}

/// Every declared command: id -> `DeclaredCommand`. Direct port of
/// `read_commands`. The command-id literal is still read back from the
/// *raw* text at the matched span (not the blanked copy), the same
/// reason the Python version does: blanking only ever replaces characters
/// with spaces at the same byte offsets, so a captured group's span is
/// valid in both.
pub fn read_commands(root: &Path, failures: &mut Vec<String>) -> BTreeMap<String, DeclaredCommand> {
    let pat = Patterns::new();
    let mut found: BTreeMap<String, DeclaredCommand> = BTreeMap::new();
    for path in source_files(root) {
        let rel = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        let raw = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let text = blank(&raw);

        let mut statics: BTreeMap<String, String> = BTreeMap::new();
        for m in pat.descriptor_static.captures_iter(&text) {
            // Groups 1 and 2 are both outside any `?`/`*` in the pattern,
            // so a successful match guarantees both are present.
            let (Some(name_m), Some(span)) = (m.get(1), m.get(2)) else {
                continue;
            };
            let name = name_m.as_str().to_owned();
            let name_literal = raw[span.start()..span.end()].to_owned();
            statics.insert(name, name_literal);
        }

        for m in pat.invocation.find_iter(&text) {
            let Some(open_at) = text[m.start()..].find('{').map(|o| m.start() + o) else {
                continue;
            };
            let line = raw[..m.start()].matches('\n').count() + 1;
            let Some(end) = block_end(&text, open_at) else {
                failures.push(format!(
                    "(A) {rel_str}:{line}: unbalanced declare_write_command! block"
                ));
                continue;
            };
            let body = &text[open_at..=end];

            // The pattern matches against `body` (blanked -- string
            // contents are spaces), but the literal's real text is read
            // back from `raw` at the same absolute offset, exactly as
            // the Python version reads `raw[open_at + m2.start(1) :]`,
            // not from the blanked copy.
            let ids: Vec<String> = pat
                .command_id
                .captures_iter(body)
                .filter_map(|c| {
                    // Group 1 is outside any `?`/`*`; present on any match.
                    let span = c.get(1)?;
                    Some(raw[open_at + span.start()..open_at + span.end()].to_owned())
                })
                .collect();
            if ids.len() != 1 {
                failures.push(format!(
                    "(A) {rel_str}:{line}: expected one `command … = \"ID\"` in the block, found {}",
                    ids.len()
                ));
                continue;
            }
            let cid = ids[0].clone();
            if !id_shape(&cid) {
                if rel_str != REGISTRY_RS {
                    failures.push(format!(
                        "(A) {rel_str}:{line}: command id {cid:?} is not of the inventory's shape \
                         ([A-Z][0-9][0-9]); only {REGISTRY_RS}'s proof-only command may be"
                    ));
                }
                continue;
            }
            if let Some(existing) = found.get(&cid) {
                failures.push(format!(
                    "(A) {rel_str}:{line}: command {cid} is declared twice \
                     (first at {}:{})",
                    existing.file, existing.line
                ));
                continue;
            }

            let mut events = Vec::new();
            let mut descriptors = Vec::new();
            for refc in pat.descriptor_ref.captures_iter(body) {
                // Group 1 is outside any `?`/`*`; present on any match.
                let Some(name_m) = refc.get(1) else { continue };
                let name = name_m.as_str().to_owned();
                match statics.get(&name) {
                    None => {
                        failures.push(format!(
                            "(C) {rel_str}:{line}: command {cid} maps to `{name}`, \
                             which is not an `EventDescriptor` static in the same file"
                        ));
                    }
                    Some(event_name) => {
                        if !descriptors.contains(&name) {
                            descriptors.push(name);
                            events.push(event_name.clone());
                        }
                    }
                }
            }
            if descriptors.is_empty() {
                failures.push(format!(
                    "(C) {rel_str}:{line}: command {cid} maps to no descriptor"
                ));
            }
            found.insert(
                cid,
                DeclaredCommand {
                    file: rel_str.clone(),
                    line,
                    events,
                    descriptors,
                },
            );
        }
    }
    found
}

// ── reading the inventory ───────────────────────────────────────────────

pub fn check_shape(rows: &[Row], failures: &mut Vec<String>) -> BTreeMap<String, Row> {
    let mut by_id: BTreeMap<String, Row> = BTreeMap::new();
    let allowed = allowed_keys();
    for (index, row) in rows.iter().enumerate() {
        let index = index + 1;
        let rid_display = row.id_display(index);
        let where_ = format!("(E) row {rid_display}");
        for key in &row.keys {
            if !allowed.contains(&key.as_str()) {
                failures.push(format!("{where_}: unknown key `{key}`"));
            }
        }
        for key in REQUIRED {
            if !row.keys.iter().any(|k| k == key) {
                failures.push(format!("{where_}: missing `{key}`"));
            }
        }
        let Some(rid) = &row.id else { continue };
        if !id_shape(rid) {
            failures.push(format!("{where_}: id is not of the shape [A-Z][0-9][0-9]"));
        }
        if by_id.contains_key(rid) {
            failures.push(format!("{where_}: id appears twice"));
            continue;
        }
        if let Some(status) = &row.status {
            if !STATUSES.contains(&status.as_str()) {
                failures.push(format!(
                    "{where_}: status {status:?} is not one of {STATUSES:?}"
                ));
            }
        } else {
            failures.push(format!("{where_}: status None is not one of {STATUSES:?}"));
        }
        if let Some(class) = &row.class {
            if !CLASSES.contains(&class.as_str()) {
                failures.push(format!(
                    "{where_}: class {class:?} is not one of {CLASSES:?}"
                ));
            }
        } else {
            failures.push(format!("{where_}: class None is not one of {CLASSES:?}"));
        }
        if !matches!(row.sealed, Some(toml::Value::Boolean(_))) {
            failures.push(format!("{where_}: `sealed` must be true or false"));
        }
        let files_ok = row
            .files
            .as_ref()
            .and_then(|v| v.as_array())
            .is_some_and(|a| a.iter().all(|e| e.as_str().is_some()));
        if !files_ok {
            failures.push(format!("{where_}: `files` must be a list of paths"));
        }
        let sealed_true = matches!(row.sealed, Some(toml::Value::Boolean(true)));
        if sealed_true {
            if row.status.as_deref() != Some("implemented") {
                failures.push(format!("{where_}: a sealed row must be `implemented`"));
            }
            if row.class.as_deref() != Some("A") {
                failures.push(format!(
                    "{where_}: a sealed command is Class A by construction"
                ));
            }
        }
        by_id.insert(rid.clone(), row.clone());
    }
    by_id
}

pub fn check_direction(
    by_id: &BTreeMap<String, Row>,
    code: &BTreeMap<String, DeclaredCommand>,
    failures: &mut Vec<String>,
) {
    for (cid, info) in code {
        match by_id.get(cid) {
            None => failures.push(format!(
                "(A) command {cid} is declared at {}:{} but has no row in the inventory",
                info.file, info.line
            )),
            Some(row) => {
                if !matches!(row.sealed, Some(toml::Value::Boolean(true))) {
                    failures.push(format!(
                        "(A) command {cid} is declared at {}:{} but its row says `sealed = false`",
                        info.file, info.line
                    ));
                }
            }
        }
    }
    for (rid, row) in by_id {
        if matches!(row.sealed, Some(toml::Value::Boolean(true))) && !code.contains_key(rid) {
            failures.push(format!(
                "(B) row {rid} says `sealed = true` but no command {rid} is declared"
            ));
        }
    }
}

pub fn check_derived(
    by_id: &BTreeMap<String, Row>,
    code: &BTreeMap<String, DeclaredCommand>,
    failures: &mut Vec<String>,
) {
    for (rid, row) in by_id {
        if let Some(info) = code.get(rid) {
            if matches!(row.sealed, Some(toml::Value::Boolean(true))) {
                for (key, derived) in [("event", &info.events), ("descriptor", &info.descriptors)] {
                    let have = match key {
                        "event" => row.event.as_ref(),
                        _ => row.descriptor.as_ref(),
                    };
                    match have {
                        None => failures.push(format!(
                            "(C) row {rid}: `{key}` is missing; the code says {derived:?}"
                        )),
                        Some(v) if v != derived => failures.push(format!(
                            "(C) row {rid}: `{key}` is {v:?}; the code says {derived:?}"
                        )),
                        _ => {}
                    }
                }
            }
        } else if matches!(row.sealed, Some(toml::Value::Boolean(false))) {
            for key in DERIVED {
                let present = match *key {
                    "event" => row.event.is_some(),
                    _ => row.descriptor.is_some(),
                };
                if present {
                    failures.push(format!("(C) row {rid}: a planned row carries no `{key}`"));
                }
            }
        }
    }
}

pub fn check_sealed_by(by_id: &BTreeMap<String, Row>, failures: &mut Vec<String>) {
    for (rid, row) in by_id {
        let Some(sealer_value) = &row.sealed_by else {
            continue;
        };
        if matches!(row.sealed, Some(toml::Value::Boolean(true))) {
            failures.push(format!(
                "(G) row {rid}: `sealed = true` and `sealed_by` together are a contradiction"
            ));
            continue;
        }
        let Some(sealer) = sealer_value.as_str() else {
            failures.push(format!(
                "(G) row {rid}: `sealed_by` must be a string naming a command id"
            ));
            continue;
        };
        match by_id.get(sealer) {
            None => failures.push(format!(
                "(G) row {rid}: `sealed_by` names {sealer:?}, which is not a declared row"
            )),
            Some(sealer_row) => {
                if !matches!(sealer_row.sealed, Some(toml::Value::Boolean(true))) {
                    failures.push(format!(
                        "(G) row {rid}: `sealed_by` names {sealer:?}, which is not `sealed = true`"
                    ));
                }
            }
        }
    }
}

pub fn check_files(root: &Path, by_id: &BTreeMap<String, Row>, failures: &mut Vec<String>) {
    for (rid, row) in by_id {
        let Some(files) = row.files.as_ref().and_then(|v| v.as_array()) else {
            continue;
        };
        let files: Vec<String> = files
            .iter()
            .filter_map(|f| f.as_str())
            .map(str::to_owned)
            .collect();
        if row.status.as_deref() == Some("target-absent") {
            if !files.is_empty() {
                failures.push(format!(
                    "(D) row {rid}: `target-absent` names no file, but lists {files:?}"
                ));
            }
            continue;
        }
        if files.is_empty() {
            failures.push(format!(
                "(D) row {rid}: status {:?} names no file",
                row.status
            ));
        }
        for f in &files {
            if !root.join(f).is_file() {
                failures.push(format!(
                    "(D) row {rid}: `files` names {f}, which does not exist"
                ));
            }
        }
    }
}

/// Fixed, hand-written pattern, proven valid at every build.
#[allow(clippy::expect_used)]
fn hand_count_re() -> Regex {
    Regex::new(r"(?i)\(\d+[:)]|\b\d+\s+(?:rows|entries|commands|implemented)\b")
        .expect("fixed pattern")
}

pub fn check_header(text: &str, failures: &mut Vec<String>) {
    let re = hand_count_re();
    let mut header: Vec<&str> = Vec::new();
    for line in text.lines() {
        if line.starts_with("[[") || (!line.is_empty() && !line.starts_with('#')) {
            break;
        }
        header.push(line);
    }
    for (n, line) in header.iter().enumerate() {
        let n = n + 1;
        if re.is_match(line) {
            failures.push(format!(
                "(F) header line {n}: a hand-written count ({line:?}); \
                 the gate derives counts on every run"
            ));
        }
    }
}

// ── --fix: rewrite the derived fields from the code ─────────────────────

fn toml_list(items: &[String]) -> String {
    let parts: Vec<String> = items
        .iter()
        .map(|s| format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")))
        .collect();
    format!("[{}]", parts.join(", "))
}

/// Rewrite `event`/`descriptor` on every sealed row and drop them from
/// every planned one. Line-based, so every other byte, comment included,
/// is kept -- a direct port of `fix_text`.
/// Fixed, hand-written patterns throughout, proven valid at every build.
#[allow(clippy::expect_used)]
pub fn fix_text(text: &str, code: &BTreeMap<String, DeclaredCommand>) -> String {
    let id_re = Regex::new(r#"^id = "([^"]+)""#).expect("fixed pattern");
    let sealed_re = Regex::new(r"^sealed = true\b").expect("fixed pattern");
    let derived_re = Regex::new(r"^(event|descriptor) = ").expect("fixed pattern");

    let mut out: Vec<String> = Vec::new();
    let mut block: Vec<String> = Vec::new();

    let flush = |block: &mut Vec<String>, out: &mut Vec<String>| {
        if block.is_empty() {
            return;
        }
        let rid = block
            .iter()
            .find_map(|l| id_re.captures(l).map(|c| c[1].to_owned()));
        let sealed = block.iter().any(|l| sealed_re.is_match(l));
        let mut kept: Vec<String> = block
            .iter()
            .filter(|l| !derived_re.is_match(l))
            .cloned()
            .collect();
        while kept.last().map(String::as_str) == Some("") {
            kept.pop();
        }
        if sealed && let Some(info) = rid.as_ref().and_then(|rid| code.get(rid)) {
            kept.push(format!("event = {}", toml_list(&info.events)));
            kept.push(format!("descriptor = {}", toml_list(&info.descriptors)));
        }
        out.extend(kept);
        out.push(String::new());
        block.clear();
    };

    let mut seen_first = false;
    for line in text.lines() {
        if line.starts_with("[[command]]") {
            flush(&mut block, &mut out);
            seen_first = true;
            block.push(line.to_owned());
        } else if seen_first {
            block.push(line.to_owned());
        } else {
            out.push(line.to_owned());
        }
    }
    flush(&mut block, &mut out);
    while out.last().map(String::as_str) == Some("") {
        out.pop();
    }
    out.join("\n") + "\n"
}

// ── entry point ─────────────────────────────────────────────────────────

pub struct Outcome {
    pub failures: Vec<String>,
    pub summary: Option<String>,
    /// Every declared command found, for a caller that wants to report
    /// on it (e.g. a future `--list` mode); unused by `main.rs` today.
    #[allow(dead_code)]
    pub code: BTreeMap<String, DeclaredCommand>,
}

/// Run every check, in the same order the Python script does. `fix`
/// rewrites `inventory` in place (if it changed) before checking, exactly
/// as `--fix` does there.
pub fn run(root: &Path, inventory_rel: &Path, fix: bool) -> anyhow::Result<Outcome> {
    let inventory = root.join(inventory_rel);
    let mut text = std::fs::read_to_string(&inventory)
        .map_err(|e| anyhow::anyhow!("cannot read {}: {e}", inventory_rel.display()))?;
    let mut data: toml::Table = toml::from_str(&text)
        .map_err(|e| anyhow::anyhow!("cannot read {}: {e}", inventory_rel.display()))?;
    let mut rows: Vec<Row> = match data.get("command").and_then(|v| v.as_array()) {
        Some(arr) if !arr.is_empty() => arr
            .iter()
            .filter_map(|v| v.as_table())
            .map(Row::from_table)
            .collect(),
        _ => {
            return Err(anyhow::anyhow!(
                "{} has no [[command]] rows",
                inventory_rel.display()
            ));
        }
    };

    let mut failures: Vec<String> = Vec::new();
    let code = read_commands(root, &mut failures);
    if code.is_empty() {
        return Err(anyhow::anyhow!(
            "no sealed command found under crates/ (fails closed)"
        ));
    }

    if fix {
        let fixed = fix_text(&text, &code);
        if fixed != text {
            std::fs::write(&inventory, &fixed)
                .map_err(|e| anyhow::anyhow!("cannot write {}: {e}", inventory_rel.display()))?;
            data = toml::from_str(&fixed)
                .map_err(|e| anyhow::anyhow!("cannot read {}: {e}", inventory_rel.display()))?;
            rows = data
                .get("command")
                .and_then(|v| v.as_array())
                .into_iter()
                .flatten()
                .filter_map(|v| v.as_table())
                .map(Row::from_table)
                .collect();
            text = fixed;
        }
    }

    let by_id = check_shape(&rows, &mut failures);
    check_direction(&by_id, &code, &mut failures);
    check_derived(&by_id, &code, &mut failures);
    check_sealed_by(&by_id, &mut failures);
    check_files(root, &by_id, &mut failures);
    check_header(&text, &mut failures);

    let summary = if failures.is_empty() {
        let sealed = rows
            .iter()
            .filter(|r| matches!(r.sealed, Some(toml::Value::Boolean(true))))
            .count();
        let implemented = rows
            .iter()
            .filter(|r| r.status.as_deref() == Some("implemented"))
            .count();
        Some(format!(
            "{} rows: {sealed} sealed, {} planned; {implemented} implemented",
            rows.len(),
            rows.len() - sealed
        ))
    } else {
        None
    };

    Ok(Outcome {
        failures,
        summary,
        code,
    })
}
