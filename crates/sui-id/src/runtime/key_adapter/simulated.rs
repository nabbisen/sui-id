//! The power-loss model, against the real code.
//!
//! Ported from the harness's prototype
//! (`rfcs/handoffs/100-master-key-rotation/crash-injection-harness-prototype-2026-11-14.md`),
//! generalized from named strings to real paths and real bytes, and with
//! explicit `point` labels (the prototype auto-generated one label per
//! call; here the caller already has to name the point for
//! [`super::failpoint::Failpoint`], so [`Simulated`] reuses it). Driven
//! through the real rotation code via [`super::KeyFileOps`] — only the
//! crash itself is simulated; everything it records actually happened in
//! this process.
//!
//! **Never reachable outside `#[cfg(test)]`.** It exists to run today's
//! rotation once, record every operation, enumerate every disk a power
//! loss could leave at each point, and materialize each one into a real
//! temporary directory for the real-code oracle in
//! [`super::tests`]/[`super::tests::stage0`].

use super::{KeyFileMetadata, KeyFileOps};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

type Ino = usize;

#[derive(Clone, Debug, PartialEq)]
enum Op {
    Link { name: PathBuf, ino: Ino },
    Rename { from: PathBuf, to: PathBuf },
    Write { ino: Ino, data: Vec<u8> },
    Marker(String),
}

/// One directory's worth of durable state: entries (including hard
/// links — two names can share an `Ino`), their data, and the markers
/// that have become durable (a checkpointed database commit).
#[derive(Clone, Default, Debug)]
pub struct Disk {
    entries: BTreeMap<PathBuf, Ino>,
    data: BTreeMap<Ino, Vec<u8>>,
    markers: Vec<String>,
}

impl Disk {
    fn apply(&mut self, op: &Op) {
        match op {
            Op::Link { name, ino } => {
                self.entries.insert(name.clone(), *ino);
                self.data.entry(*ino).or_default();
            }
            Op::Rename { from, to } => {
                if let Some(i) = self.entries.remove(from) {
                    self.entries.insert(to.clone(), i);
                }
            }
            Op::Write { ino, data } => {
                self.data.insert(*ino, data.clone());
            }
            Op::Marker(m) => self.markers.push(m.clone()),
        }
    }

    pub fn read(&self, name: &Path) -> Option<&[u8]> {
        self.entries
            .get(name)
            .map(|i| self.data.get(i).map(|d| d.as_slice()).unwrap_or(&[]))
    }

    pub fn paths(&self) -> impl Iterator<Item = &PathBuf> {
        self.entries.keys()
    }

    pub fn has_marker(&self, m: &str) -> bool {
        self.markers.iter().any(|x| x == m)
    }
}

/// One crash point: the (durable, pending) snapshot taken just before an
/// operation, labeled by that operation's `point`.
struct CrashPoint {
    label: String,
    durable: Disk,
    pending_entries: Vec<Op>,
    pending_data: BTreeMap<Ino, Vec<u8>>,
}

struct Inner {
    durable: Disk,
    view: Disk,
    pending_entries: Vec<Op>,
    pending_data: BTreeMap<Ino, Vec<u8>>,
    next_ino: Ino,
    points: Vec<CrashPoint>,
}

/// Records every key-file operation issued through it during one real
/// run, and the (durable, pending) state just before each one. The
/// database's commit and its durability are recorded the same way, as
/// markers, via [`KeyFileOps::db_committed`]/[`KeyFileOps::db_durable`] —
/// pending until a power loss could lose them, exactly like an unsynced
/// directory entry.
pub struct Simulated {
    inner: RefCell<Inner>,
}

impl Simulated {
    /// `initial` seeds the directory's durable state before anything in
    /// the recorded run touches it — for rotation, `[(old_path, OLD_KEY)]`.
    pub fn new(initial: &[(PathBuf, Vec<u8>)]) -> Self {
        let mut d = Disk::default();
        let mut next = 0;
        for (name, data) in initial {
            d.apply(&Op::Link {
                name: name.clone(),
                ino: next,
            });
            d.apply(&Op::Write {
                ino: next,
                data: data.clone(),
            });
            next += 1;
        }
        Simulated {
            inner: RefCell::new(Inner {
                durable: d.clone(),
                view: d,
                pending_entries: vec![],
                pending_data: BTreeMap::new(),
                next_ino: next,
                points: vec![],
            }),
        }
    }

    fn point(&self, label: &str) {
        let mut inner = self.inner.borrow_mut();
        let durable = inner.durable.clone();
        let pending_entries = inner.pending_entries.clone();
        let pending_data = inner.pending_data.clone();
        inner.points.push(CrashPoint {
            label: label.to_owned(),
            durable,
            pending_entries,
            pending_data,
        });
    }

    fn ino_of(&self, path: &Path) -> Option<Ino> {
        self.inner.borrow().view.entries.get(path).copied()
    }

    /// Every crash point recorded so far, with the two state-producing
    /// models the design names: `power_loss_states` and `kill_state`.
    pub fn points(&self) -> Vec<RecordedPoint> {
        self.inner
            .borrow()
            .points
            .iter()
            .map(|p| RecordedPoint {
                label: p.label.clone(),
                durable: p.durable.clone(),
                pending_entries: p.pending_entries.clone(),
                pending_data: p.pending_data.clone(),
            })
            .collect()
    }

    /// The final durable+pending state is a kill state in its own right
    /// (nothing crashed), useful for materializing "rotation finished."
    pub fn finish(&self) {
        self.point("after the last op");
    }
}

/// A snapshot handed out of [`Simulated`] for enumeration, decoupled from
/// the `RefCell` borrow.
pub struct RecordedPoint {
    pub label: String,
    durable: Disk,
    pending_entries: Vec<Op>,
    pending_data: BTreeMap<Ino, Vec<u8>>,
}

impl RecordedPoint {
    /// Every disk a power loss at this point could leave: any subset of
    /// the pending directory operations (applied in issue order — a
    /// later rename cannot persist without an earlier one that produced
    /// the name it moves), and, for each inode with a pending write, one
    /// of [`Self::torn_candidates`]'s contents for it — its last-durable
    /// data, the full new data, or a prefix of the new data at a
    /// bounded set of lengths. The design names a torn write as exactly
    /// what the harness adds over the prototype (§4): "a write that
    /// persisted may have persisted partly, as a prefix."
    pub fn power_loss_states(&self) -> Vec<Disk> {
        let inos: Vec<Ino> = self.pending_data.keys().copied().collect();
        let candidates: Vec<Vec<Vec<u8>>> =
            inos.iter().map(|ino| self.torn_candidates(*ino)).collect();

        let mut out = vec![];
        let e = self.pending_entries.len();
        for emask in 0u32..(1u32 << e) {
            let mut base = self.durable.clone();
            for (i, op) in self.pending_entries.iter().enumerate() {
                if emask & (1 << i) != 0 {
                    base.apply(op);
                }
            }
            Self::cartesian_writes(base, &inos, &candidates, 0, &mut out);
        }
        out
    }

    /// Every content an inode's pending write could have persisted as:
    /// its last-durable content (not written at all), the full new
    /// content (fully written), and a prefix of the new content at 0,
    /// 1, half, and `len - 1` bytes — a bounded sample, not every
    /// length, with duplicates (an empty prefix equals the durable
    /// content of a just-created file) removed.
    fn torn_candidates(&self, ino: Ino) -> Vec<Vec<u8>> {
        let new = &self.pending_data[&ino];
        let durable = self.durable.data.get(&ino).cloned().unwrap_or_default();
        let len = new.len();
        let half = len / 2;
        let last = len.saturating_sub(1);
        let mut out = vec![durable, new.clone()];
        for prefix_len in [0, len.min(1), half, last] {
            out.push(new[..prefix_len].to_vec());
        }
        out.sort();
        out.dedup();
        out
    }

    /// Applies one candidate content per inode in `inos`, for every
    /// combination in `candidates` (`candidates[i]` is `inos[i]`'s own
    /// list) — the Cartesian product, since more than one inode can
    /// hold a pending write at the same crash point.
    fn cartesian_writes(
        base: Disk,
        inos: &[Ino],
        candidates: &[Vec<Vec<u8>>],
        idx: usize,
        out: &mut Vec<Disk>,
    ) {
        if idx == inos.len() {
            out.push(base);
            return;
        }
        for data in &candidates[idx] {
            let mut d = base.clone();
            d.apply(&Op::Write {
                ino: inos[idx],
                data: data.clone(),
            });
            Self::cartesian_writes(d, inos, candidates, idx + 1, out);
        }
    }

    /// A killed process: the kernel keeps every write; this is the one
    /// state a kill can leave at this point.
    pub fn kill_state(&self) -> Disk {
        let mut disk = self.durable.clone();
        for op in &self.pending_entries {
            disk.apply(op);
        }
        for (ino, data) in &self.pending_data {
            disk.apply(&Op::Write {
                ino: *ino,
                data: data.clone(),
            });
        }
        disk
    }
}

impl KeyFileOps for Simulated {
    fn create_new(&self, point: &str, path: &Path) -> io::Result<()> {
        self.point(&format!("before {point}"));
        let mut inner = self.inner.borrow_mut();
        if inner.view.entries.contains_key(path) {
            return Err(io::Error::new(io::ErrorKind::AlreadyExists, "create_new"));
        }
        let ino = inner.next_ino;
        inner.next_ino += 1;
        let op = Op::Link {
            name: path.to_path_buf(),
            ino,
        };
        inner.view.apply(&op);
        inner.pending_entries.push(op);
        Ok(())
    }

    fn write_all(&self, point: &str, path: &Path, data: &[u8]) -> io::Result<()> {
        self.point(&format!("before {point}"));
        let ino = self
            .ino_of(path)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "write_all"))?;
        let mut inner = self.inner.borrow_mut();
        inner.view.apply(&Op::Write {
            ino,
            data: data.to_vec(),
        });
        inner.pending_data.insert(ino, data.to_vec());
        Ok(())
    }

    fn sync_file(&self, point: &str, path: &Path) -> io::Result<()> {
        self.point(&format!("before {point}"));
        let ino = self
            .ino_of(path)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "sync_file"))?;
        let mut inner = self.inner.borrow_mut();
        if let Some(d) = inner.pending_data.remove(&ino) {
            inner.durable.apply(&Op::Write { ino, data: d });
        }
        inner.durable.data.entry(ino).or_default();
        Ok(())
    }

    fn rename(&self, point: &str, from: &Path, to: &Path) -> io::Result<()> {
        self.point(&format!("before {point}"));
        let op = Op::Rename {
            from: from.to_path_buf(),
            to: to.to_path_buf(),
        };
        let mut inner = self.inner.borrow_mut();
        inner.view.apply(&op);
        inner.pending_entries.push(op);
        Ok(())
    }

    fn publish_no_replace(&self, point: &str, from: &Path, to: &Path) -> io::Result<()> {
        self.point(&format!("before {point}"));
        let mut inner = self.inner.borrow_mut();
        if inner.view.entries.contains_key(to) {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "publish_no_replace",
            ));
        }
        // Modeled as a rename: the directory-entry durability question
        // (did the new name survive a power loss) is the same either
        // way, and this adapter does not model two names sharing one
        // inode as a distinct fact a power loss could lose.
        let op = Op::Rename {
            from: from.to_path_buf(),
            to: to.to_path_buf(),
        };
        inner.view.apply(&op);
        inner.pending_entries.push(op);
        Ok(())
    }

    fn sync_dir(&self, point: &str, _dir: &Path) -> io::Result<()> {
        self.point(&format!("before {point}"));
        let mut inner = self.inner.borrow_mut();
        let (dir, rest): (Vec<Op>, Vec<Op>) = inner
            .pending_entries
            .drain(..)
            .partition(|o| !matches!(o, Op::Marker(_)));
        inner.pending_entries = rest;
        for op in dir {
            inner.durable.apply(&op);
        }
        Ok(())
    }

    fn read(&self, _point: &str, path: &Path) -> io::Result<Vec<u8>> {
        let inner = self.inner.borrow();
        inner
            .view
            .read(path)
            .map(|d| d.to_vec())
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "read"))
    }

    fn remove(&self, point: &str, path: &Path) -> io::Result<()> {
        self.point(&format!("before {point}"));
        let mut inner = self.inner.borrow_mut();
        inner.view.entries.remove(path);
        Ok(())
    }

    fn metadata(&self, _point: &str, path: &Path) -> io::Result<KeyFileMetadata> {
        let inner = self.inner.borrow();
        if inner.view.entries.contains_key(path) {
            Ok(KeyFileMetadata {
                uid: 0,
                mode: 0o600,
                is_symlink: false,
            })
        } else {
            Err(io::Error::new(io::ErrorKind::NotFound, "metadata"))
        }
    }

    /// SQLite's commit in WAL mode with `synchronous=NORMAL`: atomic,
    /// and survives a killed process, but may roll back on a power loss
    /// until a checkpoint — so it is pending, like a directory entry.
    fn db_committed(&self, label: &str) {
        self.point(&format!("before db_committed {label}"));
        let op = Op::Marker(label.to_owned());
        let mut inner = self.inner.borrow_mut();
        inner.view.apply(&op);
        inner.pending_entries.push(op);
    }

    /// `PRAGMA wal_checkpoint`, completed: every commit recorded so far
    /// becomes durable.
    fn db_durable(&self, label: &str) {
        self.point(&format!("before db_durable {label}"));
        let mut inner = self.inner.borrow_mut();
        let (markers, rest): (Vec<Op>, Vec<Op>) = inner
            .pending_entries
            .drain(..)
            .partition(|o| matches!(o, Op::Marker(_)));
        inner.pending_entries = rest;
        for m in markers {
            inner.durable.apply(&m);
        }
    }
}

/// Write `disk`'s entries into `dir`, a real temporary directory, with
/// owner-only permissions — the materialization step, before the
/// real-code oracle runs against it.
pub fn materialize(disk: &Disk, dir: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    for path in disk.paths() {
        let name = path
            .file_name()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no file name"))?;
        let dest = dir.join(name);
        let data = disk.read(path).unwrap_or(&[]);
        std::fs::write(&dest, data)?;
        std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}
