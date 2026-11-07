//! RFC 094 cycle J stage 7c: the matrix's `class`, `actor` and `step_up`
//! columns move from `scripts/check-audit-matrix-columns.py` (RFC 116 stage
//! 2) into `cargo xtask audit-structure`, for the same reason stage 7 moved
//! the name-coverage check: `scripts/rust_registry.py`'s own docstring says
//! it reads the sealed-command registry *"without an AST… nothing here can
//! see a type."* class, actor and the `step_up` attribute are all typed
//! fields; a text read of them inherits the exact defect RFC 094 exists to
//! end.
//!
//! **What moves onto `syn`:** the descriptor side. `EventDescriptor` and
//! `MustAttemptDescriptor` statics' `class` (`EventDescriptor` only --
//! `MustAttemptDescriptor` has no such field; its class is synthesised as
//! `"MustAttempt"`, same as the Python reader), `actor`, and the `step_up`
//! entry inside `attributes: &[…]` (an inline `AttributeSpec { .. }` or a
//! bare identifier naming a local `const` of that type) are all read from
//! the parsed struct literal, not matched as text.
//!
//! **What stays text:** the matrix side (a markdown cell *is* text; reading
//! it as text is not the defect -- `matrix_table::read_matrix`, shared with
//! `coverage.rs`), and the Actor-cell prose heuristics
//! (`claimed_class`/`actor_shape`/`STEP_UP_REQUIRED`), ported verbatim from
//! `check-audit-matrix-columns.py` rather than redesigned: a cell contract
//! expressed as prose ("admin user id; none for the CLI") is fragile by
//! nature, and changing it is a different decision than moving where it is
//! checked.
//!
//! **Reachability, not re-derived.** `rust_registry.py`'s `read_registry`
//! includes an `EventDescriptor` only when a `declare_write_command!` event
//! maps to it (an `EventDescriptor` no command reaches, like
//! `registry.rs`'s proof-only one, "is not on the Class-A seam"); every
//! `MustAttemptDescriptor` is included unconditionally, since it is never
//! command-mapped by construction. This module does not re-implement that
//! mapping as a second text scan (which `syn` cannot give for free: a
//! `declare_write_command!` invocation is an opaque macro body to
//! `syn::parse_file`, not expanded). It reuses `write_commands::read_commands`'s
//! own, already-`syn`-adjacent-but-textual result instead -- the same
//! `DeclaredCommand.descriptors` list `write_commands.rs`'s own check
//! already computes and `main.rs` already has in hand from running that
//! check first. Re-deriving reachability a second way here would be the
//! exact "two definitions of true" RFC 094 exists to end, just moved one
//! file over.
//!
//! **A declared-but-unreachable `EventDescriptor` outside `registry.rs`
//! is a disclosed gap, not a silent one**: nothing here special-cases that
//! file by path (unlike `coverage.rs`); exclusion falls out of
//! `read_commands`'s own command-id shape check, which is what actually
//! excludes `registry.rs`'s proof-only descriptor today (its command id is
//! not inventory-shaped). If a future `EventDescriptor` is declared and
//! genuinely never reached by any command, in any file, it is excluded here
//! the same way -- correctly, not accidentally.
//!
//! **The "declarations seen/found" strict-vs-loose check does not port.**
//! `rust_registry.py`'s own strict regex can miss a path-qualified or
//! multi-line descriptor declaration (`crate::registry::MustAttemptDescriptor`
//! split across lines), which is why it carries a second, looser regex just
//! to detect its own blind spot. `syn` has no such blind spot: `is_descriptor_type`
//! matches a type's *last path segment*, so a qualified path and a
//! multi-line declaration are both found exactly as readily as the plain
//! case. There is nothing to compare two counts for, so this module reports
//! one count, not two -- see `the_drifted_*` tests below, which assert the
//! previously-invisible shape is now *found*, not that a mismatch is
//! reported (there is no mismatch to report).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use syn::{Expr, ExprStruct, Item, Lit, Type};

use crate::matrix_table;
use crate::write_authority::line_of;
use crate::write_commands::{self, DeclaredCommand};

fn path_last_ident(path: &syn::Path) -> Option<String> {
    path.segments.last().map(|s| s.ident.to_string())
}

fn is_type_named(ty: &Type, want: &str) -> bool {
    matches!(ty, Type::Path(tp) if path_last_ident(&tp.path).as_deref() == Some(want))
}

fn str_literal(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Lit(l) => match &l.lit {
            Lit::Str(s) => Some(s.value()),
            _ => None,
        },
        _ => None,
    }
}

fn bool_literal(expr: &Expr) -> Option<bool> {
    match expr {
        Expr::Lit(l) => match &l.lit {
            Lit::Bool(b) => Some(b.value),
            _ => None,
        },
        _ => None,
    }
}

fn path_ident_value(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Path(p) => path_last_ident(&p.path),
        _ => None,
    }
}

fn struct_field<'a>(s: &'a ExprStruct, name: &str) -> Option<&'a Expr> {
    s.fields
        .iter()
        .find(|f| matches!(&f.member, syn::Member::Named(n) if n == name))
        .map(|f| &f.expr)
}

/// One `AttributeSpec { name: "...", required: ... }` found either inline
/// in an `attributes: &[…]` array or as a `const` the array names by
/// identifier.
fn attribute_from_struct(s: &ExprStruct) -> Option<(String, bool)> {
    let name = struct_field(s, "name").and_then(str_literal)?;
    let required = struct_field(s, "required").and_then(bool_literal)?;
    Some((name, required))
}

/// `const NAME: AttributeSpec = AttributeSpec { .. };`, file-scoped --
/// `rust_registry.py`'s own `consts` map is rebuilt per file too, so a
/// same-named const in a different file is never cross-referenced.
fn attribute_const(item: &syn::ItemConst) -> Option<(String, (String, bool))> {
    if !is_type_named(&item.ty, "AttributeSpec") {
        return None;
    }
    let Expr::Struct(s) = &*item.expr else {
        return None;
    };
    attribute_from_struct(s).map(|spec| (item.ident.to_string(), spec))
}

/// Resolve `attributes: &[…]` into a name -> required map. Each element is
/// either an inline `AttributeSpec { .. }` literal or a bare identifier
/// naming a file-scoped const; anything else contributes nothing (the same
/// silent skip `rust_registry.py`'s own loop falls through to when neither
/// shape matches at a position).
fn resolve_attributes(
    descriptor: &ExprStruct,
    attribute_consts: &BTreeMap<String, (String, bool)>,
    failures: &mut Vec<String>,
    where_: &str,
    event_name: &str,
) -> BTreeMap<String, bool> {
    let mut out = BTreeMap::new();
    let Some(attrs_expr) = struct_field(descriptor, "attributes") else {
        return out;
    };
    let array = match attrs_expr {
        Expr::Reference(r) => &*r.expr,
        other => other,
    };
    let Expr::Array(arr) = array else {
        return out;
    };
    for elem in &arr.elems {
        match elem {
            Expr::Struct(s) => {
                if let Some((name, required)) = attribute_from_struct(s) {
                    out.insert(name, required);
                }
            }
            Expr::Path(p) => {
                let Some(ident) = path_last_ident(&p.path) else {
                    continue;
                };
                match attribute_consts.get(&ident) {
                    Some((name, required)) => {
                        out.insert(name.clone(), *required);
                    }
                    None => failures.push(format!(
                        "{where_}: descriptor {event_name} names attribute `{ident}`, which is \
                         not an `AttributeSpec` const in this file"
                    )),
                }
            }
            _ => {}
        }
    }
    out
}

/// The registry entry for one event, read from its `EventDescriptor` or
/// `MustAttemptDescriptor` static -- `klass`/`actor` are the source's own
/// variant identifiers, not re-typed into a Rust enum, so a fixture that
/// writes an invalid variant (`AuditClass::MustAttempt`, which does not
/// exist in the real enum) is read exactly as written rather than rejected
/// at this layer; whether that value is *meaningful* is the class check's
/// job, not the reader's.
#[derive(Debug, Clone)]
pub(crate) struct Descriptor {
    pub(crate) name: String,
    pub(crate) static_name: String,
    pub(crate) klass: String,
    pub(crate) actor: String,
    pub(crate) attributes: BTreeMap<String, bool>,
    pub(crate) file: String,
    pub(crate) line: usize,
}

/// Read one file's `EventDescriptor`/`MustAttemptDescriptor` statics, keyed
/// by static name (not yet filtered by command-reachability -- the caller
/// joins that separately). A descriptor missing `kind`, `name`, `class`
/// (`EventDescriptor` only) or `actor` is reported and dropped, mirroring
/// `rust_registry.py`'s own "not in the shape … this reader understands".
fn read_file(rel: &str, raw: &str, failures: &mut Vec<String>) -> BTreeMap<String, Descriptor> {
    let mut out = BTreeMap::new();
    let Ok(file) = syn::parse_file(raw) else {
        return out;
    };

    let mut attribute_consts: BTreeMap<String, (String, bool)> = BTreeMap::new();
    for item in &file.items {
        if let Item::Const(c) = item
            && let Some((ident, spec)) = attribute_const(c)
        {
            attribute_consts.insert(ident, spec);
        }
    }

    for item in &file.items {
        let Item::Static(s) = item else { continue };
        let is_must_attempt = is_type_named(&s.ty, "MustAttemptDescriptor");
        let is_event = is_type_named(&s.ty, "EventDescriptor");
        if !is_must_attempt && !is_event {
            continue;
        }
        let Expr::Struct(es) = &*s.expr else { continue };
        let static_name = s.ident.to_string();
        let line = line_of(s.ident.span());
        let where_ = format!("{rel}:{line}");

        let has_kind = struct_field(es, "kind").is_some();
        let name = struct_field(es, "name").and_then(str_literal);
        let actor = struct_field(es, "actor").and_then(path_ident_value);
        let klass = if is_must_attempt {
            Some("MustAttempt".to_string())
        } else {
            struct_field(es, "class").and_then(path_ident_value)
        };

        let shape = if is_must_attempt {
            "kind, name, actor, …"
        } else {
            "kind, name, class, actor, …"
        };
        let (Some(name), Some(actor), Some(klass)) = (name, actor, klass) else {
            failures.push(format!(
                "{where_}: descriptor {static_name} is not in the shape `{shape}` this reader \
                 understands"
            ));
            continue;
        };
        if !has_kind {
            failures.push(format!(
                "{where_}: descriptor {static_name} is not in the shape `{shape}` this reader \
                 understands"
            ));
            continue;
        }

        let attributes = resolve_attributes(es, &attribute_consts, failures, &where_, &name);
        out.insert(
            static_name.clone(),
            Descriptor {
                name,
                static_name,
                klass,
                actor,
                attributes,
                file: rel.to_string(),
                line,
            },
        );
    }
    out
}

/// Every event reachable from the typed registry, keyed by event name:
/// every `MustAttemptDescriptor` unconditionally, and every `EventDescriptor`
/// whose static name a real command (`code`) maps to. Direct counterpart of
/// `rust_registry.py`'s `read_registry`, with reachability sourced from
/// `write_commands::read_commands`'s own result rather than re-parsed here.
pub(crate) fn read_registry(
    root: &Path,
    code: &BTreeMap<String, DeclaredCommand>,
) -> (BTreeMap<String, Descriptor>, Vec<String>) {
    let mut failures = Vec::new();
    let mut registry: BTreeMap<String, Descriptor> = BTreeMap::new();

    // `(file, static name)` pairs, not bare names: `write_commands::read_commands`
    // resolves a command's descriptor reference against statics declared in
    // *the same file* (its own `statics` map is reset per file), so two
    // different files may each declare a static of the same name without
    // one's reachability leaking onto the other's. Flattening to bare names
    // here would re-introduce exactly that leak -- measured concretely by
    // `a_descriptor_no_command_maps_to_is_not_on_the_seam`'s fixture, which
    // deliberately re-declares `U02_DISABLE`/`U07_RESET`/`U08_UNLOCK` in
    // `registry.rs` under the same static names `commands.rs` uses, reachable
    // only there.
    let reachable_statics: BTreeSet<(String, String)> = code
        .values()
        .flat_map(|c| c.descriptors.iter().map(|d| (c.file.clone(), d.clone())))
        .collect();

    for path in write_commands::source_files(root) {
        let rel = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        let Ok(raw) = std::fs::read_to_string(&path) else {
            continue;
        };
        let file_descriptors = read_file(&rel_str, &raw, &mut failures);
        for (static_name, d) in file_descriptors {
            let reachable = d.klass == "MustAttempt"
                || reachable_statics.contains(&(rel_str.clone(), static_name.clone()));
            if !reachable {
                continue;
            }
            if let Some(existing) = registry.get(&d.name)
                && existing.static_name != d.static_name
            {
                failures.push(format!(
                    "{rel_str}: event {} is declared by two descriptors",
                    d.name
                ));
            }
            registry.insert(d.name.clone(), d);
        }
    }
    (registry, failures)
}

#[allow(clippy::expect_used)]
fn claimed_class(cell: &str) -> Option<&'static str> {
    let re_a = regex::Regex::new(r"^A\b").expect("fixed pattern");
    let re_b = regex::Regex::new(r"^B\b").expect("fixed pattern");
    let s = cell.replace('*', "");
    let s = s.trim();
    if re_a.is_match(s) {
        Some("A")
    } else if re_b.is_match(s) {
        Some("B")
    } else {
        None
    }
}

#[allow(clippy::expect_used)]
fn actor_shape(cell: &str) -> Option<&'static str> {
    let s = cell.trim();
    if s.is_empty() {
        return None;
    }
    if s.starts_with('—') || s.starts_with('-') {
        return Some("None");
    }
    let re_none = regex::Regex::new(r"(?i)\bnone\b").expect("fixed pattern");
    if re_none.is_match(s) {
        Some("Optional")
    } else {
        Some("Required")
    }
}

#[allow(clippy::expect_used)]
fn step_up_required_re() -> regex::Regex {
    regex::Regex::new(r"`step_up`\s*\(required\)").expect("fixed pattern")
}

pub(crate) struct ColumnsOutcome {
    pub(crate) failures: Vec<String>,
    pub(crate) summary: String,
}

/// Run the ported column check: `class` (both directions), `actor`,
/// `step_up`, and the matrix's own schema rule. Direct counterpart of
/// `check-audit-matrix-columns.py`'s `check`.
pub(crate) fn check(
    root: &Path,
    matrix_rel: &Path,
    code: &BTreeMap<String, DeclaredCommand>,
) -> anyhow::Result<ColumnsOutcome> {
    let matrix_path = root.join(matrix_rel);
    let matrix_text = std::fs::read_to_string(&matrix_path)
        .map_err(|e| anyhow::anyhow!("cannot read {}: {e}", matrix_rel.display()))?;

    let (rows, schema_failures) = matrix_table::read_matrix(&matrix_text);
    let mut failures: Vec<String> = schema_failures
        .into_iter()
        .map(|f| format!("(schema) {f}"))
        .collect();

    let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
    for row in &rows {
        if let Some(&first_line) = seen.get(row.event.as_str()) {
            failures.push(format!(
                "(schema) line {}: {} has a second row (first at line {first_line})",
                row.line, row.event
            ));
        } else {
            seen.insert(&row.event, row.line);
        }
    }

    let (registry, src_failures) = read_registry(root, code);
    failures.extend(src_failures.into_iter().map(|f| format!("(source) {f}")));

    let sealed_atomic: BTreeSet<&str> = registry
        .iter()
        .filter(|(_, d)| d.klass == "Atomic")
        .map(|(n, _)| n.as_str())
        .collect();

    let by_event: BTreeSet<&str> = rows.iter().map(|r| r.event.as_str()).collect();
    let step_up_required = step_up_required_re();

    let mut checked_for_actor = 0usize;
    for row in &rows {
        let class_cell = row.cells.get("class").map(String::as_str).unwrap_or("");
        match claimed_class(class_cell) {
            None => failures.push(format!(
                "(class) line {}: {}: the Class cell {class_cell:?} does not start with A or B",
                row.line, row.event
            )),
            Some("A") if !sealed_atomic.contains(row.event.as_str()) => failures.push(format!(
                "(class) line {}: {} claims Class A but no sealed `EventDescriptor` of class \
                 `Atomic` carries that name",
                row.line, row.event
            )),
            Some("B") if sealed_atomic.contains(row.event.as_str()) => failures.push(format!(
                "(class) line {}: {} claims Class B but a sealed Class-A descriptor carries \
                 that name",
                row.line, row.event
            )),
            _ => {}
        }

        let Some(d) = registry.get(&row.event) else {
            continue;
        };
        checked_for_actor += 1;
        let actor_cell = row.cells.get("actor").map(String::as_str).unwrap_or("");
        match actor_shape(actor_cell) {
            None => failures.push(format!(
                "(actor) line {}: {}: the Actor cell is empty or absent",
                row.line, row.event
            )),
            Some(got) if got != d.actor => failures.push(format!(
                "(actor) line {}: {}: the descriptor says `ActorRequirement::{}`, the Actor \
                 cell {actor_cell:?} reads as {got}",
                row.line, row.event, d.actor
            )),
            _ => {}
        }

        if let Some(note) = row.cells.get("note fields") {
            let in_doc = step_up_required.is_match(note);
            let in_code = d.attributes.get("step_up").copied() == Some(true);
            if in_doc && !in_code {
                failures.push(format!(
                    "(step_up) line {}: {}: the note says `step_up` (required); the descriptor \
                     has no required `step_up` attribute",
                    row.line, row.event
                ));
            }
            if in_code && !in_doc {
                failures.push(format!(
                    "(step_up) line {}: {}: the descriptor requires `step_up`; the note does \
                     not say so",
                    row.line, row.event
                ));
            }
        }
    }

    for (name, d) in &registry {
        if d.klass == "Atomic" && !by_event.contains(name.as_str()) {
            failures.push(format!(
                "(class) {name} is a sealed Class-A event ({}:{}) with no table row claiming \
                 it; a mention in prose does not count",
                d.file, d.line
            ));
        }
    }

    let event_descriptor_count = registry.values().filter(|d| d.klass == "Atomic").count();
    let must_attempt_count = registry
        .values()
        .filter(|d| d.klass == "MustAttempt")
        .count();
    let summary = format!(
        "{} table rows, {} sealed Class-A events; {checked_for_actor} rows checked for actor \
         and step_up; descriptors found: EventDescriptor {event_descriptor_count}, \
         MustAttemptDescriptor {must_attempt_count}",
        rows.len(),
        sealed_atomic.len(),
    );

    Ok(ColumnsOutcome { failures, summary })
}
