//! RFC 100 stage 3: the rotation journal, as its own module. **Built,
//! not wired in** -- `rotate_key_files` does not call anything here yet
//! (stage 4 builds the recovery this would need before a live rotation
//! can leave a workspace behind; stage 5 wires it in).
//!
//! The workspace `<active>.rotation-work`'s existence is the rotation
//! lock ([`lock_workspace`]). Inside it, [`write_intent`] publishes the
//! first authenticated state (`Intent`); [`advance`] moves forward
//! through the rest (`NextTempVerified -> NextPublished ->
//! BackupTempVerified -> BackupPublished -> Prepared`), each through its
//! own reserved temporary, never backward. [`read`] is recovery's own
//! entry point: it reads only the published `intent.json`, authenticates
//! it with whichever key opens the database sentinel, and never a
//! `*.write` temporary.
//!
//! **No key material, ever** -- the body binds fingerprints only, and
//! the two tags are HMACs, not the keys themselves.

use crate::key_adapter::KeyFileOps;
use crate::keyring::sync_parent_dir_impl;
use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};
use std::path::Path;
use sui_id_store::crypto::{self, KeyRole, MasterKey};

/// Monotonic, in the order RFC 100 names: `derive(Ord)` follows
/// declaration order, so `<`/`<=` already is "earlier in the sequence."
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum JournalState {
    Intent,
    NextTempVerified,
    NextPublished,
    BackupTempVerified,
    BackupPublished,
    Prepared,
}

/// The operator's bound choice for the pre-rotation backup of the old
/// key (RFC 100, Old-key custody). A required field on [`JournalBody`]
/// -- stage 5 cannot forget it, the same way the dispatch requires.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum BackupDisposition {
    RetainUntil {
        review_at: String,
        custody_reference: String,
    },
    RemoveAfterVerified {
        verification_reference: String,
    },
}

/// Everything the journal binds. A fixed field order, no maps, no
/// floats -- `serde_json::to_vec` of a given value is deterministic, and
/// [`canonical_body_bytes`] is the one place that fact is relied on.
/// **No key material**: `old_fingerprint`/`new_fingerprint` are stage 1's
/// non-secret fingerprints, never the keys.
///
/// **`deny_unknown_fields`** here and on the envelope: the tags cover the
/// body as re-serialized from what was parsed, so a field serde silently
/// dropped would be bytes on disk that nothing authenticates. A journal
/// holding one is refused instead.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JournalBody {
    pub rotation_id: String,
    pub old_fingerprint: String,
    pub new_fingerprint: String,
    pub active_path: String,
    pub workspace_path: String,
    pub next_path: String,
    pub backup_path: String,
    pub intent_temp_path: String,
    pub update_temp_path: String,
    pub next_temp_path: String,
    pub backup_temp_path: String,
    pub expected_uid: u32,
    pub expected_mode: u32,
    pub backup_disposition: BackupDisposition,
    pub state: JournalState,
}

/// The published format: a version, the body, and two domain-separated
/// tags -- `HMAC-SHA256(old_key, ".../old" || 0x00 || body_bytes)` and
/// the same under the new key and `".../new"`. `version` is outside the
/// body, so [`read`] refuses any version but [`CURRENT_VERSION`] before it
/// trusts anything else; the tags' own `v1` domain binds it cryptographically.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JournalEnvelope {
    pub version: u32,
    pub body: JournalBody,
    pub tag_old: String,
    pub tag_new: String,
}

const CURRENT_VERSION: u32 = 1;
const INTENT_TEMP_NAME: &str = "intent.write";
const INTENT_NAME: &str = "intent.json";
const UPDATE_TEMP_NAME: &str = "update.write";

/// `serde_json::to_vec` of [`JournalBody`] alone -- never the envelope,
/// since the stored tags cover the body only. Deterministic because the
/// struct has a fixed field order and no maps or floats: round-tripping
/// an unchanged body through parse-then-reserialize always reproduces
/// these same bytes (a test holds this directly), which is what lets
/// [`verify`] recompute them from a *parsed* envelope rather than
/// needing the original on-disk bytes kept aside.
fn canonical_body_bytes(body: &JournalBody) -> Vec<u8> {
    #[allow(clippy::expect_used)]
    serde_json::to_vec(body).expect("JournalBody has no maps, floats, or NaN to fail encoding")
}

/// Seal `body` under both keys into a fresh envelope at [`CURRENT_VERSION`].
fn seal(body: JournalBody, old_key: &MasterKey, new_key: &MasterKey) -> JournalEnvelope {
    let bytes = canonical_body_bytes(&body);
    let tag_old = crypto::journal_tag_old(old_key, &bytes);
    let tag_new = crypto::journal_tag_new(new_key, &bytes);
    JournalEnvelope {
        version: CURRENT_VERSION,
        body,
        tag_old,
        tag_new,
    }
}

/// Verify a just-parsed envelope's tags against *both* keys at once --
/// used right after writing, before publication, since at that point
/// both keys are in hand and the write must be provably good under
/// either role before anything is published. `true` only if the old key
/// authenticates as [`KeyRole::Old`] **and** the new key as
/// [`KeyRole::New`]; anything else is a defect in the write itself.
fn verify_both(envelope: &JournalEnvelope, old_key: &MasterKey, new_key: &MasterKey) -> bool {
    let bytes = canonical_body_bytes(&envelope.body);
    let old_role =
        crypto::verify_journal_tags(old_key, &bytes, &envelope.tag_old, &envelope.tag_new);
    let new_role =
        crypto::verify_journal_tags(new_key, &bytes, &envelope.tag_old, &envelope.tag_new);
    old_role == Some(KeyRole::Old) && new_role == Some(KeyRole::New)
}

/// `pub(crate)` production code, exercised only by [`sample_body`] until
/// stage 5 builds the real call site that constructs a [`JournalBody`]
/// from actual paths.
#[cfg_attr(not(test), allow(dead_code))]
fn path_to_string(path: &Path) -> anyhow::Result<String> {
    path.to_str()
        .map(str::to_owned)
        .with_context(|| format!("{} is not valid UTF-8", path.display()))
}

/// §3's lock: `create_dir` the workspace, 0700, no symlink, failing if
/// anything is already there -- *that* failure is the lock already
/// held, reported as such rather than a bare `AlreadyExists`. Then sync
/// its parent, so the workspace's own existence is durable before
/// anything is written inside it.
pub fn lock_workspace(adapter: &dyn KeyFileOps, workspace: &Path) -> anyhow::Result<()> {
    match adapter.create_dir("journal.lock.create_dir", workspace) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            bail!(
                "refusing to prepare a rotation: {} already exists. A previous \
                 rotation or recovery may be in progress, or one stopped without \
                 cleaning up -- do not remove it by hand; let recovery own it.",
                workspace.display()
            );
        }
        Err(e) => return Err(e).context(format!("creating the workspace {}", workspace.display())),
    }
    sync_parent_dir_impl(adapter, "journal.lock.sync_parent", workspace)
        .context("syncing the workspace's parent directory")?;
    Ok(())
}

/// §3's intent write: `intent.write` (checked write, synced), read back
/// and verify both tags, publish without replacement to `intent.json`,
/// sync the workspace. The intent is always the first published state,
/// so a `body.state` other than [`JournalState::Intent`] is refused, not
/// silently replaced: a caller who passed one has misunderstood where it is.
pub fn write_intent(
    adapter: &dyn KeyFileOps,
    workspace: &Path,
    body: JournalBody,
    old_key: &MasterKey,
    new_key: &MasterKey,
) -> anyhow::Result<JournalEnvelope> {
    if body.state != JournalState::Intent {
        bail!(
            "refusing to write the rotation intent in state {:?}: the intent is \
             always the first state, Intent. Later states go through advance.",
            body.state
        );
    }
    let envelope = seal(body, old_key, new_key);
    let bytes = serde_json::to_vec(&envelope).context("encoding the intent journal")?;

    let temp = workspace.join(INTENT_TEMP_NAME);
    write_key_file_bytes(adapter, "journal.intent", &temp, &bytes)?;

    let read_back = adapter
        .read("journal.intent.readback", &temp)
        .context("reading the intent temporary back")?;
    let parsed: JournalEnvelope =
        serde_json::from_slice(&read_back).context("the intent temporary is not valid JSON")?;
    if read_back != bytes || !verify_both(&parsed, old_key, new_key) {
        bail!(
            "refusing to publish the rotation intent: the write at {} did not \
             read back as written (its tags do not authenticate under the keys \
             that just sealed it).",
            temp.display()
        );
    }

    let published = workspace.join(INTENT_NAME);
    adapter
        .publish_no_replace("journal.intent.publish", &temp, &published)
        .with_context(|| format!("publishing {}", published.display()))?;
    sync_parent_dir_impl(adapter, "journal.intent.sync_dir", &published)
        .context("syncing the workspace after publishing the intent")?;

    Ok(envelope)
}

/// §3's state advance: the reserved update temporary (one name, reused
/// by every advance), checked write, sync, readback and verification,
/// atomic *replace* of `intent.json`, workspace sync. Refuses, before
/// touching anything, if `new_state` is not strictly after the current
/// state.
pub fn advance(
    adapter: &dyn KeyFileOps,
    workspace: &Path,
    current: &JournalEnvelope,
    new_state: JournalState,
    old_key: &MasterKey,
    new_key: &MasterKey,
) -> anyhow::Result<JournalEnvelope> {
    if new_state <= current.body.state {
        bail!(
            "refusing to advance the rotation journal: {:?} is not after the \
             current state {:?}. States only move forward.",
            new_state,
            current.body.state
        );
    }

    let mut body = current.body.clone();
    body.state = new_state;
    let envelope = seal(body, old_key, new_key);
    let bytes = serde_json::to_vec(&envelope).context("encoding the advanced journal")?;

    let temp = workspace.join(UPDATE_TEMP_NAME);
    write_key_file_bytes(adapter, "journal.advance", &temp, &bytes)?;

    let read_back = adapter
        .read("journal.advance.readback", &temp)
        .context("reading the update temporary back")?;
    let parsed: JournalEnvelope =
        serde_json::from_slice(&read_back).context("the update temporary is not valid JSON")?;
    if read_back != bytes || !verify_both(&parsed, old_key, new_key) {
        bail!(
            "refusing to advance the rotation journal: the write at {} did not \
             read back as written.",
            temp.display()
        );
    }

    let published = workspace.join(INTENT_NAME);
    adapter
        .rename("journal.advance.replace", &temp, &published)
        .with_context(|| format!("replacing {}", published.display()))?;
    sync_parent_dir_impl(adapter, "journal.advance.sync_dir", &published)
        .context("syncing the workspace after advancing the journal")?;

    Ok(envelope)
}

/// §3's own read, for recovery: `intent.json` only, never a `*.write`
/// temporary. Authenticates with `candidate_key` -- whichever key opens
/// the database sentinel -- and reports which role it holds alongside
/// the body, or an error naming that it authenticates as neither.
pub fn read(
    adapter: &dyn KeyFileOps,
    workspace: &Path,
    candidate_key: &MasterKey,
) -> anyhow::Result<(JournalBody, KeyRole)> {
    let published = workspace.join(INTENT_NAME);
    let bytes = adapter
        .read("journal.read", &published)
        .with_context(|| format!("reading {}", published.display()))?;
    let envelope: JournalEnvelope = serde_json::from_slice(&bytes).with_context(|| {
        format!(
            "{} is not a rotation journal this version reads",
            published.display()
        )
    })?;
    if envelope.version != CURRENT_VERSION {
        bail!(
            "refusing to trust {}: it is journal version {}, and this sui-id \
             reads only version {CURRENT_VERSION}.",
            published.display(),
            envelope.version
        );
    }
    let body_bytes = canonical_body_bytes(&envelope.body);
    match crypto::verify_journal_tags(
        candidate_key,
        &body_bytes,
        &envelope.tag_old,
        &envelope.tag_new,
    ) {
        Some(role) => Ok((envelope.body, role)),
        None => bail!(
            "refusing to trust {}: its tags authenticate under neither role \
             for the given key.",
            published.display()
        ),
    }
}

/// `create_new` + `write_all` + `sync_file`, without the trailing
/// directory sync `write_key_file_impl` always does -- the journal's
/// own protocol syncs the workspace itself only *after* publication,
/// never right after the temporary write.
fn write_key_file_bytes(
    adapter: &dyn KeyFileOps,
    point_prefix: &str,
    path: &Path,
    data: &[u8],
) -> anyhow::Result<()> {
    adapter
        .create_new(&format!("{point_prefix}.create_new"), path)
        .with_context(|| format!("creating {}", path.display()))?;
    adapter
        .write_all(&format!("{point_prefix}.write"), path, data)
        .with_context(|| format!("writing {}", path.display()))?;
    adapter
        .sync_file(&format!("{point_prefix}.sync_file"), path)
        .with_context(|| format!("syncing {}", path.display()))?;
    Ok(())
}

/// A fresh [`JournalBody`] for tests and callers that have everything
/// `write_intent` needs but don't want to restate every field by hand,
/// in state [`JournalState::Intent`], with the two keys' real fingerprints.
#[cfg(test)]
#[allow(clippy::expect_used)]
pub(crate) fn sample_body(
    active: &Path,
    workspace: &Path,
    old_key: &MasterKey,
    new_key: &MasterKey,
) -> JournalBody {
    JournalBody {
        rotation_id: "11111111-1111-1111-1111-111111111111".to_owned(),
        old_fingerprint: crypto::fingerprint(old_key),
        new_fingerprint: crypto::fingerprint(new_key),
        active_path: path_to_string(active).expect("utf8"),
        workspace_path: path_to_string(workspace).expect("utf8"),
        next_path: path_to_string(&active.with_extension("ready")).expect("utf8"),
        backup_path: path_to_string(&active.with_extension("bak")).expect("utf8"),
        intent_temp_path: path_to_string(&workspace.join(INTENT_TEMP_NAME)).expect("utf8"),
        update_temp_path: path_to_string(&workspace.join(UPDATE_TEMP_NAME)).expect("utf8"),
        next_temp_path: path_to_string(&workspace.join("next.write")).expect("utf8"),
        backup_temp_path: path_to_string(&workspace.join("backup.write")).expect("utf8"),
        expected_uid: 0,
        expected_mode: 0o600,
        backup_disposition: BackupDisposition::RemoveAfterVerified {
            verification_reference: "test-only".to_owned(),
        },
        state: JournalState::Intent,
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
#[path = "journal/tests.rs"]
mod tests;
