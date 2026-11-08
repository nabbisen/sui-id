//! RFC 094 cycle J stage 7: `audit-structure` becomes the audit-coverage
//! authority, replacing G13's string extraction (`scripts/check-audit-matrix.sh`)
//! as the thing that decides whether an event name reaches the matrix and
//! back. RFC 094's own words for why a string gate cannot be the final
//! answer: *"A string gate can only see literals in namespaces the code
//! already declares"* — it is not that the namespace list was stale (G13-b
//! already fixed that), it is structural: grep only finds what it already
//! knows to look for.
//!
//! So this module finds event names the same way the write-authority and
//! write-command checks already do -- by walking the typed paths an event
//! name actually travels through, not by matching `word.word` anywhere in
//! a file:
//!
//! 1. `EventDescriptor` and `MustAttemptDescriptor` statics' `name` field
//!    (sealed commands and RFC 096-B1's must-attempt emissions). The
//!    `MustAttemptDescriptor` source is not named in the dispatch's own
//!    enumeration (`EventDescriptor`, `declare_write_command!`,
//!    `AuditLogRow { action: … }`) -- found by measuring, not assumed: 9
//!    such statics exist (`check-audit-matrix-columns.py`'s own count
//!    confirms it), and skipping them would make this check *weaker* than
//!    G13's current string scan, which already happens to see their
//!    literals (a quoted string inside any struct literal is still a
//!    quoted string to a regex). Added so the AST version does not regress
//!    on a source the string version reaches by accident.
//! 2. Any `fn name(&self) -> &'static str { match self { Variant => "lit",
//!    .. } }` -- the shape `events.rs`'s legacy best-effort emitter uses.
//!    Not hardcoded to that file: matched by signature, crate-wide (one
//!    occurrence exists today; a second, elsewhere, is still found).
//! 3. `AuditLogRow { action: "lit".into(), .. }` construction sites,
//!    crate-wide, in production files. Only a *literal* (optionally
//!    wrapped in one `.into()`/`.to_owned()`/`.to_string()` call) counts;
//!    `action: descriptor.name.into()` or `action: name.into()` is plumbing
//!    that re-emits a name already captured at its own typed source (1) or
//!    (2), not a second literal to report.
//!
//! **Named gap, not a silent one**: a literal reached only through a named
//! `const`/`static` of type `&str` referenced at the call site (not inlined,
//! and not one of the typed descriptors above) is not resolved -- doing so
//! needs crate-wide constant-propagation, which plain `syn` parsing of one
//! file at a time cannot do. Previously measured: `AUDIT_SIGNIN_UPSTREAM_
//! FAILURE` (`repos/federation_provider.rs`) was exactly this shape, and
//! was still found anyway because the *same* name was independently
//! declared as the `name` field of `AUDIT_SIGNIN_UPSTREAM_FAILURE_
//! DESCRIPTOR`'s `MustAttemptDescriptor` literal nearby -- source (1)
//! reached it even though the `const`-indirected call site did not. RFC 094
//! M2b criterion 2 R1 (cycle J stage 10) converted that call site onto the
//! descriptor directly and removed the now-unreferenced const, so no
//! current example of the gap remains in this tree; it is named here for
//! when the next one appears, not left to be discovered then for the first
//! time. If a future `&'static str` audit-action constant exists with no
//! typed descriptor alongside it, this check would miss it.
//!
//! The matrix side (`matrix_event_names`, below) reads the `Event name`
//! column of the matrix's tables, not every backticked `word.word` in the
//! file -- a correction, stage 7b, to this module's first version, which
//! scanned every backtick and filtered by a namespace list derived from the
//! source side (the same filter G13 itself applies). That filter cuts both
//! ways: a matrix row in a namespace the source happens not to declare
//! anywhere passed the forward check silently rather than failing it, found
//! on the real tree at review. Column-keyed extraction removes the filter
//! and the blind spot together.

use std::collections::BTreeSet;
use std::path::Path;

use syn::visit::{self, Visit};
use syn::{Block, Expr, ExprStruct, FnArg, Item, ItemStatic, Lit, ReturnType, Signature, Type};

#[derive(Default)]
struct Finder {
    names: BTreeSet<String>,
}

fn path_last_ident(path: &syn::Path) -> Option<String> {
    path.segments.last().map(|s| s.ident.to_string())
}

/// Unwrap `"lit".into()` / `.to_owned()` / `.to_string()` down to the
/// underlying literal, if that's what it is. Anything else (a bare
/// identifier, a field access, `format!`, …) is not a literal source.
fn literal_of(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Lit(l) => match &l.lit {
            Lit::Str(s) => Some(s.value()),
            _ => None,
        },
        Expr::MethodCall(m)
            if matches!(
                m.method.to_string().as_str(),
                "into" | "to_owned" | "to_string"
            ) =>
        {
            literal_of(&m.receiver)
        }
        _ => None,
    }
}

fn is_descriptor_type(ty: &Type) -> bool {
    if let Type::Path(tp) = ty {
        matches!(
            path_last_ident(&tp.path).as_deref(),
            Some("EventDescriptor") | Some("MustAttemptDescriptor")
        )
    } else {
        false
    }
}

fn struct_name_field(s: &ExprStruct) -> Option<String> {
    s.fields
        .iter()
        .find(|f| matches!(&f.member, syn::Member::Named(n) if n == "name"))
        .and_then(|f| literal_of(&f.expr))
}

/// `fn name(&self) -> &'static str` (or `&str`): the `events.rs`-shaped
/// legacy emitter signature, matched structurally rather than by file path.
fn is_name_signature(sig: &Signature) -> bool {
    if sig.ident != "name" {
        return false;
    }
    let takes_self_ref =
        matches!(sig.inputs.first(), Some(FnArg::Receiver(r)) if r.reference.is_some());
    let returns_str_ref = matches!(
        &sig.output,
        ReturnType::Type(_, ty) if matches!(&**ty, Type::Reference(r) if matches!(&*r.elem, Type::Path(p) if path_last_ident(&p.path).as_deref() == Some("str")))
    );
    takes_self_ref && returns_str_ref
}

fn collect_if_name_method(sig: &Signature, body: &Block, out: &mut BTreeSet<String>) {
    if !is_name_signature(sig) {
        return;
    }
    // The match is ordinarily the function's sole, tail expression; walk
    // every statement's expression form rather than assuming position, so
    // an intervening statement still gets the arms found.
    for stmt in &body.stmts {
        if let syn::Stmt::Expr(e, _) = stmt {
            collect_match_arm_literals(e, out);
        }
    }
}

/// Walk a `match self { Variant { .. } => "literal", A | B => "literal", .. }`
/// expression's arms, collecting every arm whose body is directly a string
/// literal (optionally through the same `.into()`-style wrapping).
fn collect_match_arm_literals(expr: &Expr, out: &mut BTreeSet<String>) {
    if let Expr::Match(m) = expr {
        for arm in &m.arms {
            if let Some(lit) = literal_of(&arm.body) {
                out.insert(lit);
            }
        }
    }
}

impl<'ast> Visit<'ast> for Finder {
    fn visit_item_static(&mut self, node: &'ast ItemStatic) {
        if is_descriptor_type(&node.ty)
            && let Expr::Struct(s) = &*node.expr
            && let Some(name) = struct_name_field(s)
        {
            self.names.insert(name);
        }
        visit::visit_item_static(self, node);
    }

    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        collect_if_name_method(&node.sig, &node.block, &mut self.names);
        visit::visit_item_fn(self, node);
    }

    /// `events.rs`'s `name()` is a *method* (inside `impl SecurityEvent`),
    /// not a free function -- `syn` represents the two differently
    /// (`ImplItemFn` vs. `ItemFn`), with separate `Visit` callbacks. The
    /// free-function case above is kept too: the signature shape is
    /// checked structurally, not tied to which form it takes.
    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        collect_if_name_method(&node.sig, &node.block, &mut self.names);
        visit::visit_impl_item_fn(self, node);
    }

    fn visit_expr_struct(&mut self, node: &'ast ExprStruct) {
        if path_last_ident(&node.path).as_deref() == Some("AuditLogRow")
            && let Some(field) = node
                .fields
                .iter()
                .find(|f| matches!(&f.member, syn::Member::Named(n) if n == "action"))
            && let Some(lit) = literal_of(&field.expr)
        {
            self.names.insert(lit);
        }
        visit::visit_expr_struct(self, node);
    }
}

/// Every event name reachable from the four typed sources, across all
/// production files under `crates/` (test files excluded by the same rule
/// `write_commands::is_test_file` uses -- RFC 094's gate and this one agree
/// on what "production" means, not two separate definitions).
///
/// `registry.rs` is skipped entirely, the same carve-out
/// `write_commands::REGISTRY_RS` already names: its one `EventDescriptor`
/// (`PROOF_ONLY_FORBIDDEN_DESCRIPTOR`) exists to give a compile-fail test
/// something to name and is "not an inventory row" by its own comment --
/// not a real event, so not a real coverage obligation either.
pub fn source_event_names(root: &Path) -> BTreeSet<String> {
    let mut finder = Finder::default();
    for path in crate::write_commands::source_files(root) {
        let rel = path.strip_prefix(root).unwrap_or(&path);
        if rel.to_string_lossy().replace('\\', "/") == crate::write_commands::REGISTRY_RS {
            continue;
        }
        let Ok(src) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(file) = syn::parse_file(&src) else {
            continue;
        };
        finder.visit_file(&file);
        // registry.rs's own proof-only test artefact aside, `Item::Macro`
        // invocations of `declare_write_command!` are not expanded by
        // `syn::parse_file` (macros are opaque token streams to it) --
        // their descriptors are `EventDescriptor` *statics* declared
        // alongside the macro, already visited above, not inside it.
        let _ = file.items.iter().any(|i| matches!(i, Item::Macro(_)));
    }
    finder.names
}

/// Every event name in the matrix's **`Event name` column** -- the same
/// keying `scripts/check-audit-matrix-columns.py`'s `read_matrix` uses, not
/// every backticked `word.word` anywhere in the file. Reuses
/// `matrix_table::read_matrix`, stage 7c's shared reader (built for the
/// column check, which needs the same rows' full cell map); this function
/// now only needs each row's event name and discards the rest, and
/// discards `read_matrix`'s schema failures too -- those are the column
/// check's concern (`columns.rs`), not coverage's.
///
/// G13's own extraction (and this check's first version, stage 7) took any
/// backtick-quoted name, which also catches stray matches like `` `db.rs` ``
/// in prose; G13 narrows that with a namespace filter derived from the
/// source, but the filter cuts both ways -- a matrix row in a namespace the
/// source happens not to declare *anywhere* passes silently in the forward
/// direction, rather than failing. Found on the real tree at review (cycle J
/// stage 7, `@nabbisen`): `` `ghost.probe.forward` `` added as a matrix row
/// was not reported, because `ghost` is not a namespace any source literal
/// uses. Reading the column instead removes the reason the filter existed
/// (a table cell cannot be mistaken for a prose mention) and the blind spot
/// with it -- there is no longer a filter to have a blind spot.
pub fn matrix_event_names(matrix_text: &str) -> BTreeSet<String> {
    crate::matrix_table::read_matrix(matrix_text)
        .0
        .into_iter()
        .map(|row| row.event)
        .collect()
}

#[derive(Default)]
pub struct CoverageOutcome {
    pub failures: Vec<String>,
    pub matrix_count: usize,
    pub source_count: usize,
}

pub fn check(root: &Path, matrix_rel: &Path) -> anyhow::Result<CoverageOutcome> {
    let matrix_path = root.join(matrix_rel);
    let matrix_text = std::fs::read_to_string(&matrix_path)
        .map_err(|e| anyhow::anyhow!("cannot read {}: {e}", matrix_rel.display()))?;

    let source_names = source_event_names(root);
    if source_names.is_empty() {
        return Err(anyhow::anyhow!(
            "no audit event names derived from crates/ (fails closed)"
        ));
    }
    let matrix_names = matrix_event_names(&matrix_text);
    if matrix_names.is_empty() {
        return Err(anyhow::anyhow!(
            "no event names extracted from {}",
            matrix_rel.display()
        ));
    }

    let mut failures = Vec::new();
    for name in &matrix_names {
        if !source_names.contains(name) {
            failures.push(format!(
                "(coverage forward) {name}  (in matrix but not found in crates/**/*.rs)"
            ));
        }
    }
    for name in &source_names {
        if !matrix_names.contains(name) {
            failures.push(format!(
                "(coverage backward) {name}  (in source but not in {})",
                matrix_rel.display()
            ));
        }
    }

    Ok(CoverageOutcome {
        failures,
        matrix_count: matrix_names.len(),
        source_count: source_names.len(),
    })
}
