//! RFC 100 stage 2: every precondition rotation checks **before creating
//! anything** (P1-P4; P5, the filesystem probe, needs a write and so is
//! not a precondition in this sense — see [`probe_filesystem`]). One
//! function, [`check`], used by both `--check` (which stops here) and a
//! real rotation (which also runs [`probe_filesystem`], then today's
//! sequence, reusing the key [`check`] already resolved).

use crate::key_adapter::KeyFileOps;
use crate::keyring::{KeyOrigin, ResolvedKey};
use anyhow::Context;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// `<key_file>` with its file name's extension replaced by appending
/// `suffix` — the one place every rotation-adjacent sibling path (staged
/// key, workspace, probe file) is built, so they cannot drift apart from
/// each other or from `cli::handlers::run_admin_rotate_key`'s own
/// `bak_path` (still computed there, since it carries a timestamp).
pub fn sibling_with_suffix(key_file: &Path, suffix: &str) -> PathBuf {
    let mut p = key_file.to_path_buf();
    let mut name = p
        .file_name()
        .map(|s| s.to_owned())
        .unwrap_or_else(|| OsString::from("master.key"));
    name.push(suffix);
    p.set_file_name(name);
    p
}

/// `<key_file>.new` — the staged new key, during a rotation in progress.
/// The *live*, non-journal rotation sequence's own name (`rotate.rs`),
/// unrelated to [`RotationPaths::next`] below, which the journal binds
/// once stage 5 wires it in.
pub fn staged_path(key_file: &Path) -> PathBuf {
    sibling_with_suffix(key_file, ".new")
}

/// RFC 100 stage 4 §2: every rotation-adjacent path, derived once from
/// the key file -- a journal body, a precondition check, and recovery's
/// own expectations can then never name three different things for
/// "the workspace" or "next." Only one rotation is ever in progress at a
/// time (the workspace is the lock), so `backup` is one fixed name, not
/// a timestamp; a *retained* backup's longer-lived name past `Complete`
/// is stage 7's to give it.
pub struct RotationPaths {
    pub active: PathBuf,
    pub workspace: PathBuf,
    pub next: PathBuf,
    pub backup: PathBuf,
    pub intent_temp: PathBuf,
    pub update_temp: PathBuf,
    pub next_temp: PathBuf,
    pub backup_temp: PathBuf,
}

impl RotationPaths {
    pub fn for_key_file(key_file: &Path) -> Self {
        let workspace = sibling_with_suffix(key_file, ".rotation-work");
        let intent_temp = workspace.join("intent.write");
        let update_temp = workspace.join("update.write");
        let next_temp = workspace.join("next.write");
        let backup_temp = workspace.join("backup.write");
        Self {
            active: key_file.to_path_buf(),
            next: sibling_with_suffix(key_file, ".next"),
            backup: sibling_with_suffix(key_file, ".pre-rotation"),
            workspace,
            intent_temp,
            update_temp,
            next_temp,
            backup_temp,
        }
    }
}

/// `<key_file>.rotation-work` — stage 3's workspace, now one call into
/// [`RotationPaths`] so there is one derivation, not two.
pub fn workspace_path(key_file: &Path) -> PathBuf {
    RotationPaths::for_key_file(key_file).workspace
}

fn probe_path(key_file: &Path) -> PathBuf {
    sibling_with_suffix(key_file, ".rotation-probe")
}

fn probe_path_renamed(key_file: &Path) -> PathBuf {
    sibling_with_suffix(key_file, ".rotation-probe.2")
}

/// One precondition's outcome. `failure` is `None` on pass; `Some(text)`
/// names what is wrong and, where there is one, what to do about it.
pub struct PreconditionResult {
    pub id: &'static str,
    pub label: &'static str,
    pub failure: Option<String>,
}

impl PreconditionResult {
    pub fn passed(&self) -> bool {
        self.failure.is_none()
    }
}

/// Every precondition (P1-P4), gathered — not stopping at the first
/// failure, so `--check` and a real refusal can both report everything
/// wrong at once. Read-only throughout: P2's own check
/// (`keyring::resolve_checked_impl`) opens the database only through a
/// read-only preview connection, and nothing else here touches the
/// database or writes a file.
///
/// Returns the resolved old key alongside the results **only when P1 and
/// P2 both passed** — a real rotation reuses it instead of resolving a
/// second time; `--check` discards it.
pub fn check(
    adapter: &dyn KeyFileOps,
    key_file: &Path,
    db_path: &Path,
) -> (Vec<PreconditionResult>, Option<ResolvedKey>) {
    let mut results = Vec::with_capacity(4);

    // P2: resolving also tells us the key's origin, which P1 needs,
    // without resolving twice. `resolve_checked_impl` is read-only (a
    // preview connection) *except* when neither the variable nor the key
    // file holds a key yet, in which case it mints one -- correct for
    // first start, wrong for a precondition check, which must create
    // nothing. Guard that one case here instead of calling it.
    let env_set = std::env::var("SUI_ID_MASTER_KEY").is_ok();
    let key_file_present = adapter.metadata("rotate.check.key_file", key_file).is_ok();

    let (p1_fail, p2_fail, resolved_key) = if !env_set && !key_file_present {
        (None, Some(no_existing_key_text(key_file)), None)
    } else {
        match crate::keyring::resolve_checked_impl(adapter, key_file, db_path) {
            Ok(resolved) => {
                let p1_fail =
                    (resolved.origin == KeyOrigin::Env).then(|| env_origin_text(key_file));
                (p1_fail, None, Some(resolved))
            }
            // An env-set variable that turns out to hold the *wrong* key
            // (P2 itself failing) still fails P1 independently: the
            // variable being set at all is the fact P1 checks, not
            // whether the key behind it happens to work.
            // `resolve_checked_impl` prefers the variable when set, so a
            // P2 failure with the variable set is both P1 and P2 failing
            // together, and both are named.
            Err(e) => {
                let p1_fail = env_set.then(|| env_origin_text(key_file));
                (p1_fail, Some(format!("{e:#}")), None)
            }
        }
    };

    results.push(PreconditionResult {
        id: "P1",
        label: "the key comes from the file, not the environment",
        failure: p1_fail,
    });
    results.push(PreconditionResult {
        id: "P2",
        label: "the active key opens the sentinel",
        failure: p2_fail,
    });

    // P3: no leftover staged key, and no leftover workspace.
    let staged = staged_path(key_file);
    let workspace = workspace_path(key_file);
    let mut p3_failures = Vec::new();
    if adapter.metadata("rotate.check.staged", &staged).is_ok() {
        p3_failures.push(leftover_staged_text(&staged));
    }
    if adapter
        .metadata("rotate.check.workspace", &workspace)
        .is_ok()
    {
        p3_failures.push(leftover_workspace_text(&workspace));
    }
    results.push(PreconditionResult {
        id: "P3",
        label: "no leftover staged key or rotation workspace",
        failure: (!p3_failures.is_empty()).then(|| p3_failures.join("\n")),
    });

    // P4: the key file is a regular file, not a symlink standing in for
    // one -- a symlinked key file would have `rename` replace the link
    // and leave the target untouched, the opposite of activation (RFC
    // 100 M6). Absence here (a missing key file) is not this
    // precondition's concern; P2 already named that.
    let p4_fail = match adapter.metadata("rotate.check.symlink", key_file) {
        Ok(meta) if meta.is_symlink => Some(symlink_text(key_file, meta.symlink_target.as_deref())),
        _ => None,
    };
    results.push(PreconditionResult {
        id: "P4",
        label: "the key file is a regular file, not a symlink",
        failure: p4_fail,
    });

    let resolved_key = resolved_key.filter(|_| results.iter().all(PreconditionResult::passed));
    (results, resolved_key)
}

fn env_origin_text(key_file: &Path) -> String {
    format!(
        "refusing to rotate: the current master key came from the \
        SUI_ID_MASTER_KEY environment variable, not the key file at \
        {}. Rotation only writes the key *file* -- with the \
        variable still set, the server would ignore that file on \
        its next start and keep using the old key, which would no \
        longer open a database this command had already re-sealed. \
        Unset SUI_ID_MASTER_KEY and point key_file at a file \
        holding the current key before rotating here, or rotate \
        the key in whatever deployment system sets the variable.",
        key_file.display()
    )
}

fn no_existing_key_text(key_file: &Path) -> String {
    format!(
        "refusing to rotate: no master key found to rotate -- {} does not \
        exist, and SUI_ID_MASTER_KEY is not set. There is nothing here \
        yet for rotation to replace; run the server once first, or point \
        key_file at the file holding the current key.",
        key_file.display()
    )
}

fn leftover_staged_text(staged_path: &Path) -> String {
    format!(
        "refusing to rotate: {} already exists. A previous rotation \
        may not have completed -- that file may be the only copy of \
        the master key the database is already sealed under. Check \
        it against the database (does it open with it as the \
        key_file?) before removing it and retrying.",
        staged_path.display()
    )
}

fn leftover_workspace_text(workspace_path: &Path) -> String {
    format!(
        "refusing to rotate: {} already exists. A previous rotation \
        may not have completed. Starting sui-id, or running \
        `sui-id admin master-key rotate` again, lets recovery check it and either \
        clean it or refuse and say why -- do not remove it by hand.",
        workspace_path.display()
    )
}

fn symlink_text(key_file: &Path, target: Option<&Path>) -> String {
    match target {
        Some(target) => format!(
            "refusing to rotate: {} is a symlink to {}, not a regular file. \
            Rotation renames the key file in place; with a symlink, that \
            would replace the link and leave {} untouched -- the opposite \
            of activation. Replace the symlink with a regular file holding \
            the current key before rotating.",
            key_file.display(),
            target.display(),
            target.display()
        ),
        None => format!(
            "refusing to rotate: {} is a symlink, not a regular file. \
            Rotation renames the key file in place; with a symlink, that \
            would replace the link and leave its target untouched -- the \
            opposite of activation. Replace the symlink with a regular \
            file holding the current key before rotating.",
            key_file.display()
        ),
    }
}

/// RFC 100 stage 2 §2: P5. Proving the filesystem actually supports what
/// rotation needs (`create_new`, `write_all`, `sync_file`, `rename`, a
/// directory sync) can only be done by trying them, so this is not a
/// precondition `--check` can report — a real rotation runs this first,
/// after P1-P4 pass and before anything else, with a temporary probe
/// file it removes either way. In `Simulated`, a dropped probe file
/// records exactly the same operations a real one would, at its own
/// named points, distinct from the rotation's own.
pub async fn probe_filesystem(adapter: &dyn KeyFileOps, key_file: &Path) -> anyhow::Result<()> {
    let probe = probe_path(key_file);
    let renamed = probe_path_renamed(key_file);

    // A probe file left by an interrupted rotation is this process's own
    // bound temporary only if it holds exactly the marker; then it is
    // removed. Anything else at either name is not ours: refuse, naming
    // it, and leave it untouched (review fold-in, 0.93.0 O3; the stage
    // rules: "delete only bound partial temporaries").
    for path in [&probe, &renamed] {
        match adapter.read("rotate.probe.stale.read", path) {
            Ok(body) if body == probe_file_bytes() => {
                adapter
                    .remove("rotate.probe.stale.remove", path)
                    .with_context(|| {
                        format!(
                            "a probe file left by an earlier, interrupted rotation could not be \
                         removed: {}",
                            path.display()
                        )
                    })?;
            }
            Ok(_) => anyhow::bail!(
                "refusing to rotate: {} exists and is not a rotation probe file. \
                 Rotation uses that name for a temporary file; move it aside and retry.",
                path.display()
            ),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(
                    anyhow::Error::new(e).context(format!("could not inspect {}", path.display()))
                );
            }
        }
    }

    // Track what this probe itself created, so cleanup removes only that.
    let mut created: Option<&Path> = None;
    let result: anyhow::Result<()> = (|| {
        crate::keyring::write_key_file_impl(adapter, "rotate.probe", &probe, PROBE_MARKER)
            .context("the filesystem does not support creating and writing a file here")?;
        created = Some(&probe);
        adapter
            .rename("rotate.probe.rename", &probe, &renamed)
            .context("the filesystem does not support renaming a file here")?;
        created = Some(&renamed);
        crate::keyring::sync_parent_dir_impl(
            adapter,
            "rotate.probe.sync_dir.after_rename",
            &renamed,
        )
        .context("the filesystem does not support syncing this directory")?;
        Ok(())
    })();

    // `write_key_file_impl` may fail after its `create_new` succeeded,
    // leaving the probe file; it then holds the marker or less, so it is
    // still ours. Remove it only if it is there and is the marker or a
    // prefix of it.
    let leftover: Option<&Path> = match created {
        Some(path) => Some(path),
        None => match adapter.read("rotate.probe.cleanup.read", &probe) {
            Ok(body) if probe_file_bytes().starts_with(&body) => Some(&probe),
            _ => None,
        },
    };
    if let Some(path) = leftover
        && let Err(e) = adapter.remove("rotate.probe.remove", path)
    {
        let removal = anyhow::Error::new(e).context(format!(
            "probing the filesystem left a file behind that could not be removed: {}",
            path.display()
        ));
        return Err(match result {
            Ok(()) => removal,
            Err(probe_error) => probe_error.context(removal.to_string()),
        });
    }

    result.context(
        "this filesystem does not support an operation rotation needs; rotation refuses \
         rather than fail partway through",
    )
}

/// The probe's marker. `write_key_file_impl` writes it followed by a
/// newline, so the file's whole content is [`probe_file_bytes`]. A file at
/// a probe name holding exactly that (or, after a short write, a prefix of
/// it) is a rotation's own temporary; anything else is not, and is never
/// removed.
const PROBE_MARKER: &str = "sui-id rotation probe v1";

fn probe_file_bytes() -> Vec<u8> {
    let mut bytes = PROBE_MARKER.as_bytes().to_vec();
    bytes.push(b'\n');
    bytes
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
#[path = "preconditions/tests.rs"]
mod tests;
