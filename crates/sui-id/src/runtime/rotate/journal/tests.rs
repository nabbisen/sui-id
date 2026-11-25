use super::*;
use crate::key_adapter::Real;
use crate::key_adapter::simulated::{Disk, RecordedPoint, Simulated, materialize};

fn keys() -> (MasterKey, MasterKey) {
    (MasterKey::generate(), MasterKey::generate())
}

const VIRTUAL_ACTIVE: &str = "/virtual/t.key";
const VIRTUAL_WORKSPACE: &str = "/virtual/t.key.rotation-work";

/// A real directory holding a locked workspace and a published intent.
struct Published {
    _dir: tempfile::TempDir,
    workspace: std::path::PathBuf,
    old_key: MasterKey,
    new_key: MasterKey,
    intent: JournalEnvelope,
}

fn published() -> Published {
    let dir = tempfile::tempdir().expect("tempdir");
    let active = dir.path().join("t.key");
    let workspace = dir.path().join("t.key.rotation-work");
    let (old_key, new_key) = keys();
    lock_workspace(&Real, &workspace).expect("lock");
    let intent = write_intent(
        &Real,
        &workspace,
        sample_body(&active, &workspace, &old_key, &new_key),
        &old_key,
        &new_key,
    )
    .expect("write_intent");
    Published {
        _dir: dir,
        workspace,
        old_key,
        new_key,
        intent,
    }
}

/// Rewrite the published `intent.json` through `edit`, as an attacker or
/// a corruption would, outside the adapter.
fn tamper(p: &Published, edit: impl FnOnce(String) -> String) {
    let path = p.workspace.join(INTENT_NAME);
    let text = std::fs::read_to_string(&path).expect("read published");
    let edited = edit(text.clone());
    assert_ne!(edited, text, "the edit must change something");
    std::fs::write(&path, edited).expect("write tampered");
}

/// The root cause is an `io::Error` of `kind`: a missing journal is
/// reported as missing, not as some other failure that also refuses.
fn is_io_kind(err: &anyhow::Error, kind: std::io::ErrorKind) -> bool {
    err.root_cause()
        .downcast_ref::<std::io::Error>()
        .is_some_and(|e| e.kind() == kind)
}

// ---------- the lock ----------

#[test]
fn lock_refuses_a_second_creation_and_changes_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let workspace = dir.path().join("t.key.rotation-work");

    lock_workspace(&Real, &workspace).expect("first lock");
    assert!(workspace.is_dir());

    let err = lock_workspace(&Real, &workspace).expect_err("a second lock must refuse");
    assert!(format!("{err:#}").contains("already exists"), "{err:#}");
    // Nothing inside the workspace was touched by the refused attempt.
    assert_eq!(
        std::fs::read_dir(&workspace).expect("read dir").count(),
        0,
        "the workspace is still empty"
    );
}

#[test]
fn lock_refuses_a_second_creation_in_the_simulator_too() {
    let sim = Simulated::new(&[]);
    let workspace = Path::new(VIRTUAL_WORKSPACE);
    lock_workspace(&sim, workspace).expect("first lock");
    let err = lock_workspace(&sim, workspace).expect_err("a second lock must refuse");
    assert!(format!("{err:#}").contains("already exists"), "{err:#}");
    sim.finish();
    let point = sim.points().pop().expect("a point");
    assert!(point.kill_state().has_dir(workspace));
    assert_eq!(
        point.kill_state().paths().count(),
        0,
        "the refused lock created nothing"
    );
}

// ---------- tags, end to end through write_intent + read ----------

#[test]
fn the_old_key_reads_as_old_the_new_key_as_new_and_a_third_key_as_neither() {
    let p = published();
    let (body, role) = read(&Real, &p.workspace, &p.old_key).expect("old key reads");
    assert_eq!(role, KeyRole::Old);
    assert_eq!(body, p.intent.body);
    let (_body, role) = read(&Real, &p.workspace, &p.new_key).expect("new key reads");
    assert_eq!(role, KeyRole::New);

    let err = read(&Real, &p.workspace, &MasterKey::generate())
        .expect_err("an unrelated key must not read");
    assert!(format!("{err:#}").contains("neither role"), "{err:#}");
}

/// One byte of a *value*, so the JSON still parses into a body and the
/// refusal can only come from the tags. (Flipping a byte of a field
/// *name* fails at parsing, which proves nothing about the tags.)
#[test]
fn read_rejects_a_published_body_changed_by_one_byte() {
    let p = published();
    tamper(&p, |text| {
        let key = "\"rotation_id\":\"";
        let at = text.find(key).expect("field present") + key.len();
        let mut bytes = text.into_bytes();
        assert_eq!(bytes[at], b'1', "the sample's rotation id starts with 1");
        bytes[at] = b'2';
        String::from_utf8(bytes).expect("ascii")
    });
    for key in [&p.old_key, &p.new_key] {
        let err = read(&Real, &p.workspace, key).expect_err("a changed body must not read");
        assert!(
            format!("{err:#}").contains("neither role"),
            "refused by the tags, not by parsing: {err:#}"
        );
    }
}

/// The tags cover the body as parsed; a field serde would silently drop
/// is bytes nothing authenticates, so it is refused, in the body or in
/// the envelope.
#[test]
fn read_refuses_a_field_it_does_not_know() {
    for (needle, replacement) in [
        ("\"body\":{", "\"body\":{\"note\":\"x\","),
        ("{\"version\"", "{\"note\":\"x\",\"version\""),
        ("\"kind\":", "\"note\":\"x\",\"kind\":"),
    ] {
        let p = published();
        tamper(&p, |text| text.replacen(needle, replacement, 1));
        let err =
            read(&Real, &p.workspace, &p.old_key).expect_err("an unknown field must be refused");
        assert!(
            format!("{err:#}").contains("unknown field"),
            "{needle}: {err:#}"
        );
    }
}

#[test]
fn read_refuses_a_version_it_does_not_know() {
    let p = published();
    tamper(&p, |text| {
        text.replacen("\"version\":1,", "\"version\":2,", 1)
    });
    let err = read(&Real, &p.workspace, &p.old_key).expect_err("version 2 must be refused");
    assert!(format!("{err:#}").contains("version 2"), "{err:#}");
}

#[test]
fn write_intent_refuses_a_body_in_a_later_state() {
    let dir = tempfile::tempdir().expect("tempdir");
    let active = dir.path().join("t.key");
    let workspace = dir.path().join("t.key.rotation-work");
    let (old_key, new_key) = keys();
    lock_workspace(&Real, &workspace).expect("lock");
    let mut body = sample_body(&active, &workspace, &old_key, &new_key);
    body.state = JournalState::Prepared;
    let err = write_intent(&Real, &workspace, body, &old_key, &new_key)
        .expect_err("a later state is refused, not replaced");
    assert!(format!("{err:#}").contains("Prepared"), "{err:#}");
    assert_eq!(
        std::fs::read_dir(&workspace).expect("read dir").count(),
        0,
        "nothing was written"
    );
}

// ---------- a partial intent.write is non-authoritative ----------

/// Every point `write_intent` records up to and including the one just
/// before publication: each power-loss state, materialized into a real
/// directory, has no journal for `read` to find, and whatever temporary
/// it left can be removed, after which there is still none.
#[test]
fn a_partial_intent_write_is_non_authoritative_and_safely_deletable() {
    let sim = Simulated::new(&[]);
    let active = Path::new(VIRTUAL_ACTIVE);
    let workspace = Path::new(VIRTUAL_WORKSPACE);
    let (old_key, new_key) = keys();
    lock_workspace(&sim, workspace).expect("lock");

    let before_write_intent = sim.points().len();
    write_intent(
        &sim,
        workspace,
        sample_body(active, workspace, &old_key, &new_key),
        &old_key,
        &new_key,
    )
    .expect("write_intent");
    let points = sim.points();

    let mut examined = 0;
    let mut saw_the_temporary = false;
    for point in &points[before_write_intent..] {
        for disk in point.power_loss_states() {
            let dir = tempfile::tempdir().expect("tempdir");
            materialize(&disk, dir.path()).expect("materialize");
            let real_workspace = dir.path().join("t.key.rotation-work");

            let err = read(&Real, &real_workspace, &old_key)
                .expect_err("no journal exists before publication");
            assert!(
                is_io_kind(&err, std::io::ErrorKind::NotFound),
                "{}: {err:#}",
                point.label
            );
            let temp = real_workspace.join(INTENT_TEMP_NAME);
            if temp.exists() {
                saw_the_temporary = true;
                Real.remove("t", &temp).expect("the temporary is removable");
                let err = read(&Real, &real_workspace, &old_key)
                    .expect_err("still no journal after removing it");
                assert!(is_io_kind(&err, std::io::ErrorKind::NotFound));
            }
            examined += 1;
        }
        if point.label == "before journal.intent.publish" {
            break; // the last point before publication, examined above
        }
    }
    assert!(examined > 0, "the loop examined pre-publication states");
    assert!(
        saw_the_temporary,
        "some state left the temporary, so removing it was exercised"
    );
}

// ---------- the short-write mode ----------

/// The third failure mode: a short write at the temporary (constructed
/// directly, not through `SUI_ID_FAILPOINT` -- there is no live command
/// yet to spawn a child process through, per this stage's own scope).
/// `Failpoint`'s `ShortWrite` returns an error alongside the partial
/// write, so this is caught at the write call itself; nothing reaches
/// publication.
#[test]
fn a_short_write_at_the_intent_temporary_refuses_before_publication() {
    use crate::key_adapter::failpoint::Failpoint;

    let dir = tempfile::tempdir().expect("tempdir");
    let active = dir.path().join("t.key");
    let workspace = dir.path().join("t.key.rotation-work");
    let (old_key, new_key) = keys();

    lock_workspace(&Real, &workspace).expect("lock");
    let fp = Failpoint::from_spec("journal.intent.write:short-write");
    let err = write_intent(
        &fp,
        &workspace,
        sample_body(&active, &workspace, &old_key, &new_key),
        &old_key,
        &new_key,
    )
    .expect_err("a short write must be refused");
    assert!(
        format!("{err:#}").contains("intent"),
        "the refusal names what failed: {err:#}"
    );
    assert!(
        !workspace.join(INTENT_NAME).exists(),
        "a short write must never reach publication"
    );
}

// ---------- every state advance is atomic, and durable once it returns ----------

/// Materialize `disk` and read it with the real code under both keys:
/// it must authenticate as old and as new, and hold one of `acceptable`.
fn read_materialized(
    disk: &Disk,
    label: &str,
    old_key: &MasterKey,
    new_key: &MasterKey,
    acceptable: &[&JournalEnvelope],
) -> JournalState {
    let dir = tempfile::tempdir().expect("tempdir");
    materialize(disk, dir.path()).expect("materialize");
    let real_workspace = dir.path().join("t.key.rotation-work");
    let (body, role) = read(&Real, &real_workspace, old_key)
        .unwrap_or_else(|e| panic!("{label}: the old key must read the journal: {e:#}"));
    assert_eq!(role, KeyRole::Old, "{label}");
    let (body_new, role) = read(&Real, &real_workspace, new_key)
        .unwrap_or_else(|e| panic!("{label}: the new key must read the journal: {e:#}"));
    assert_eq!(role, KeyRole::New, "{label}");
    assert_eq!(body, body_new, "{label}");
    assert!(
        acceptable.iter().any(|e| e.body == body),
        "{label}: published state {:?} is not one of the acceptable states",
        body.state
    );
    body.state
}

fn assert_advance_is_atomic(
    points: &[RecordedPoint],
    old_key: &MasterKey,
    new_key: &MasterKey,
    prior: &JournalEnvelope,
    next: &JournalEnvelope,
) {
    assert!(
        !points.is_empty(),
        "the advance must record at least one point"
    );
    for point in points {
        for disk in point.power_loss_states() {
            read_materialized(&disk, &point.label, old_key, new_key, &[prior, next]);
        }
        read_materialized(
            &point.kill_state(),
            &point.label,
            old_key,
            new_key,
            &[prior, next],
        );
    }
}

/// Every advance, `Intent` through `Prepared`: each power-loss and kill
/// state recorded during it holds the prior journal or the next one,
/// authenticated under both keys, read by the real code from a real
/// directory -- and once it has returned, **every** power-loss state
/// holds the next one. That last check is what finds a missing workspace
/// sync on the final advance, where no later point would.
#[test]
fn every_state_advance_is_atomic_across_power_loss_and_kill_states() {
    let sim = Simulated::new(&[]);
    let active = Path::new(VIRTUAL_ACTIVE);
    let workspace = Path::new(VIRTUAL_WORKSPACE);
    let (old_key, new_key) = keys();

    lock_workspace(&sim, workspace).expect("lock");
    let mut current = write_intent(
        &sim,
        workspace,
        sample_body(active, workspace, &old_key, &new_key),
        &old_key,
        &new_key,
    )
    .expect("write_intent");

    let mut advanced = 0;
    for next_state in [
        JournalState::NextTempVerified,
        JournalState::NextPublished,
        JournalState::BackupTempVerified,
        JournalState::BackupPublished,
        JournalState::Prepared,
    ] {
        let before = sim.points().len();
        let next = advance(&sim, workspace, &current, next_state, &old_key, &new_key)
            .unwrap_or_else(|e| panic!("advance to {next_state:?}: {e:#}"));
        let points = sim.points();
        assert_advance_is_atomic(&points[before..], &old_key, &new_key, &current, &next);

        sim.finish();
        let after = sim.points().pop().expect("the point after the advance");
        for disk in after.power_loss_states() {
            let state = read_materialized(
                &disk,
                &format!("after advancing to {next_state:?}"),
                &old_key,
                &new_key,
                &[&next],
            );
            assert_eq!(state, next_state);
        }
        current = next;
        advanced += 1;
    }
    assert_eq!(advanced, 5, "every state after Intent was reached");
}

// ---------- no key material ----------

#[test]
fn the_published_journal_holds_no_key_material_raw_or_base64() {
    let p = published();
    let bytes = std::fs::read(p.workspace.join(INTENT_NAME)).expect("read published");
    for key in [&p.old_key, &p.new_key] {
        assert!(
            !contains_bytes(&bytes, key.as_bytes()),
            "the published journal contains raw key bytes"
        );
        assert!(
            !String::from_utf8_lossy(&bytes).contains(&key.to_base64()),
            "the published journal contains a base64-encoded key"
        );
    }
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() || haystack.len() < needle.len() {
        return false;
    }
    haystack.windows(needle.len()).any(|w| w == needle)
}

// ---------- monotonic ----------

#[test]
fn advancing_to_an_earlier_or_equal_state_refuses() {
    let p = published();
    let advanced = advance(
        &Real,
        &p.workspace,
        &p.intent,
        JournalState::NextTempVerified,
        &p.old_key,
        &p.new_key,
    )
    .expect("advance forward");

    for backward in [JournalState::Intent, JournalState::NextTempVerified] {
        let err = advance(
            &Real,
            &p.workspace,
            &advanced,
            backward,
            &p.old_key,
            &p.new_key,
        )
        .expect_err("an equal or earlier state must refuse");
        assert!(format!("{err:#}").contains("not after"), "{err:#}");
    }
    // Nothing was written by either refused attempt.
    let (body, _) = read(&Real, &p.workspace, &p.old_key).expect("read");
    assert_eq!(body.state, JournalState::NextTempVerified);
    assert!(!p.workspace.join(UPDATE_TEMP_NAME).exists());
}

// ---------- the body round-trips byte-identical ----------

#[test]
fn the_body_round_trips_byte_identical_through_json() {
    let (old_key, new_key) = keys();
    let body = sample_body(
        Path::new(VIRTUAL_ACTIVE),
        Path::new(VIRTUAL_WORKSPACE),
        &old_key,
        &new_key,
    );

    let first = canonical_body_bytes(&body);
    let parsed: JournalBody = serde_json::from_slice(&first).expect("parse");
    let second = canonical_body_bytes(&parsed);

    assert_eq!(first, second, "a round trip must reproduce identical bytes");
    assert_eq!(parsed, body);
}

// ---------- the readback is the bytes written, not only bytes that verify ----------

/// `Real`, except that reading a `*.write` temporary returns the same
/// JSON re-spelled with one extra space: it parses to the same envelope,
/// and its tags verify, but it is not what was written. A disk or a
/// filesystem that hands back other bytes is a disk to stop on.
struct RespellingReads;

impl KeyFileOps for RespellingReads {
    fn create_new(&self, p: &str, path: &Path) -> std::io::Result<()> {
        Real.create_new(p, path)
    }
    fn write_all(&self, p: &str, path: &Path, data: &[u8]) -> std::io::Result<()> {
        Real.write_all(p, path, data)
    }
    fn sync_file(&self, p: &str, path: &Path) -> std::io::Result<()> {
        Real.sync_file(p, path)
    }
    fn rename(&self, p: &str, from: &Path, to: &Path) -> std::io::Result<()> {
        Real.rename(p, from, to)
    }
    fn publish_no_replace(&self, p: &str, from: &Path, to: &Path) -> std::io::Result<()> {
        Real.publish_no_replace(p, from, to)
    }
    fn sync_dir(&self, p: &str, dir: &Path) -> std::io::Result<()> {
        Real.sync_dir(p, dir)
    }
    fn read(&self, p: &str, path: &Path) -> std::io::Result<Vec<u8>> {
        let mut bytes = Real.read(p, path)?;
        if path.extension().is_some_and(|e| e == "write") {
            bytes.insert(1, b' ');
        }
        Ok(bytes)
    }
    fn remove(&self, p: &str, path: &Path) -> std::io::Result<()> {
        Real.remove(p, path)
    }
    fn metadata(
        &self,
        p: &str,
        path: &Path,
    ) -> std::io::Result<crate::key_adapter::KeyFileMetadata> {
        Real.metadata(p, path)
    }
    fn create_dir(&self, p: &str, path: &Path) -> std::io::Result<()> {
        Real.create_dir(p, path)
    }
    fn remove_dir(&self, p: &str, path: &Path) -> std::io::Result<()> {
        Real.remove_dir(p, path)
    }
    fn list_dir(&self, p: &str, dir: &Path) -> std::io::Result<Vec<std::path::PathBuf>> {
        Real.list_dir(p, dir)
    }
    fn db_committed(&self, _label: &str) {}
    fn db_durable(&self, _label: &str) {}
    fn lock_exclusive(
        &self,
        p: &str,
        path: &Path,
        blocking: bool,
    ) -> std::io::Result<Box<dyn crate::key_adapter::LockGuard>> {
        Real.lock_exclusive(p, path, blocking)
    }
    fn lock_shared(
        &self,
        p: &str,
        path: &Path,
        blocking: bool,
    ) -> std::io::Result<Box<dyn crate::key_adapter::LockGuard>> {
        Real.lock_shared(p, path, blocking)
    }
}

#[test]
fn a_readback_that_differs_from_the_write_refuses_even_if_it_verifies() {
    // Intent: the re-spelled readback is refused, nothing is published.
    let dir = tempfile::tempdir().expect("tempdir");
    let active = dir.path().join("t.key");
    let workspace = dir.path().join("t.key.rotation-work");
    let (old_key, new_key) = keys();
    lock_workspace(&Real, &workspace).expect("lock");
    let err = write_intent(
        &RespellingReads,
        &workspace,
        sample_body(&active, &workspace, &old_key, &new_key),
        &old_key,
        &new_key,
    )
    .expect_err("a readback other than the write must refuse");
    assert!(format!("{err:#}").contains("read back"), "{err:#}");
    assert!(!workspace.join(INTENT_NAME).exists());

    // Advance: the published journal is left as it was.
    let p = published();
    let err = advance(
        &RespellingReads,
        &p.workspace,
        &p.intent,
        JournalState::NextTempVerified,
        &p.old_key,
        &p.new_key,
    )
    .expect_err("a readback other than the write must refuse");
    assert!(format!("{err:#}").contains("read back"), "{err:#}");
    let (body, _) = read(&Real, &p.workspace, &p.old_key).expect("read");
    assert_eq!(body.state, JournalState::Intent);
}
