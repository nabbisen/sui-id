# RFC 100 — the power-loss model, the harness's prototype

**Written 2026-11-14 by the architect**, to test the
[harness design](crash-injection-harness-design-2026-11-14.md) before
proposing it. The design was **approved 2026-11-14**. The build's stage 0
([`implementation.md`](implementation.md)) starts from this model and
reproduces its results against the real code.
**What it is.** A model of one directory that **loses what was not synced**,
and the list of every disk a power loss could leave at each point of a
rotation. It is std-only Rust. **It is not the harness**, and it touches no
real file.
**Why it is kept here.** The design's §2 numbers come from it. A review of
stage 0 compares against them.

## How to run it

Copy the source below into `src/main.rs` of a scratch crate **outside this
workspace**, with a `[workspace]` table of its own in `Cargo.toml`, so cargo
does not adopt it. Then `cargo run`. It needs no dependency.

## The output, 2026-11-14 (rerun 2026-11-15, identical)

```text
0.90.0 order, kill model        : 6 crash points, 6 states; M2 violated in 0; active path wrong in 2; wrong after success in 0
0.90.0 order, power-loss model  : 6 crash points, 63 states; M2 violated in 23; active path wrong in 40; wrong after success in 30
    first M2 violation: before rename key -> key.bak: [("key", "OLD-KEY")]
stage 1 syncs, kill model       : 10 crash points, 10 states; M2 violated in 0; active path wrong in 3; wrong after success in 0
stage 1 syncs, power-loss model : 10 crash points, 24 states; M2 violated in 0; active path wrong in 10; wrong after success in 1
drop sync_file(key.new)          : 10 crash points, 27 states; M2 violated in 8; active path wrong in 14; wrong after success in 1
    first M2 violation: before checkpoint: [("key", "OLD-KEY"), ("key.new", "")]
drop sync_dir before the reseal  : 10 crash points, 23 states; M2 violated in 4; active path wrong in 10; wrong after success in 0
    first M2 violation: before checkpoint: [("key", "OLD-KEY")]
drop sync_dir after rename 1     : 10 crash points, 20 states; M2 violated in 0; active path wrong in 6; wrong after success in 0
drop sync_dir after rename 2     : 10 crash points, 18 states; M2 violated in 0; active path wrong in 6; wrong after success in 1
stage 1 + checkpoint, power-loss: 11 crash points, 19 states; M2 violated in 0; active path wrong in 6; wrong after success in 0
stage 1 + checkpoint, kill model: 11 crash points, 11 states; M2 violated in 0; active path wrong in 4; wrong after success in 0
```

How to read it: *"M2 violated"* means no file held the key the database is
sealed under. *"Wrong after success"* means that, once the command had
reported success, the active path did not hold that key. The four *drop*
lines each remove one of stage 1's syncs, with A2's checkpoint kept.

## The source

```rust
//! Prototype for RFC 100's crash-injection harness: a filesystem model that
//! loses what was not synced, and the enumeration of every state a power loss
//! could leave. Std only. One directory, inode-based, enough for rotation.
use std::collections::BTreeMap;

type Ino = usize;

#[derive(Clone, Debug, PartialEq)]
enum Op {
    // Directory-entry operations: durable after sync_dir.
    Link { name: String, ino: Ino },   // create
    Rename { from: String, to: String },
    // Data operation: durable after sync_file of that inode.
    Write { ino: Ino, data: Vec<u8> },
    // An external durable event (SQLite's own fsynced commit).
    Marker(&'static str),
}

#[derive(Clone, Default, Debug)]
struct Disk {
    entries: BTreeMap<String, Ino>,
    data: BTreeMap<Ino, Vec<u8>>,
    markers: Vec<&'static str>,
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
            Op::Marker(m) => self.markers.push(m),
        }
    }
    fn read(&self, name: &str) -> Option<&[u8]> {
        self.entries.get(name).map(|i| self.data.get(i).map(|d| d.as_slice()).unwrap_or(&[]))
    }
}

/// The recording filesystem. `durable` is what survives a power loss for
/// certain; `pending` are operations issued but not yet made durable.
struct SimFs {
    durable: Disk,
    view: Disk,
    pending_entries: Vec<Op>,
    pending_data: BTreeMap<Ino, Vec<u8>>,
    next_ino: Ino,
    /// Every crash point's (durable, pending) snapshot, taken before each op.
    points: Vec<(String, Disk, Vec<Op>, BTreeMap<Ino, Vec<u8>>)>,
}

impl SimFs {
    fn new(initial: &[(&str, &[u8])]) -> Self {
        let mut d = Disk::default();
        let mut next = 0;
        for (n, data) in initial {
            d.apply(&Op::Link { name: n.to_string(), ino: next });
            d.apply(&Op::Write { ino: next, data: data.to_vec() });
            next += 1;
        }
        SimFs { durable: d.clone(), view: d, pending_entries: vec![], pending_data: BTreeMap::new(), next_ino: next, points: vec![] }
    }
    fn point(&mut self, label: &str) {
        self.points.push((label.to_string(), self.durable.clone(), self.pending_entries.clone(), self.pending_data.clone()));
    }
    fn create_new(&mut self, name: &str) {
        self.point(&format!("before create {name}"));
        assert!(!self.view.entries.contains_key(name), "create_new: exists");
        let op = Op::Link { name: name.into(), ino: self.next_ino };
        self.next_ino += 1;
        self.view.apply(&op);
        self.pending_entries.push(op);
    }
    fn write_all(&mut self, name: &str, data: &[u8]) {
        self.point(&format!("before write {name}"));
        let ino = self.view.entries[name];
        self.view.apply(&Op::Write { ino, data: data.to_vec() });
        self.pending_data.insert(ino, data.to_vec());
    }
    fn sync_file(&mut self, name: &str) {
        self.point(&format!("before sync_file {name}"));
        let ino = self.view.entries[name];
        if let Some(d) = self.pending_data.remove(&ino) {
            self.durable.apply(&Op::Write { ino, data: d });
        }
        self.durable.data.entry(ino).or_default();
    }
    fn rename(&mut self, from: &str, to: &str) {
        self.point(&format!("before rename {from} -> {to}"));
        let op = Op::Rename { from: from.into(), to: to.into() };
        self.view.apply(&op);
        self.pending_entries.push(op);
    }
    fn sync_dir(&mut self) {
        self.point("before sync_dir");
        let (dir, rest): (Vec<Op>, Vec<Op>) =
            self.pending_entries.drain(..).partition(|o| !matches!(o, Op::Marker(_)));
        self.pending_entries = rest;
        for op in dir { self.durable.apply(&op); }
    }
    /// SQLite's commit in WAL mode with synchronous=NORMAL: atomic, and
    /// survives a killed process, but may roll back on a power loss until
    /// a checkpoint. So it is pending, like a directory entry.
    fn wal_commit(&mut self, m: &'static str) {
        self.point(&format!("before {m}"));
        let op = Op::Marker(m);
        self.view.apply(&op);
        self.pending_entries.push(op);
    }
    /// `PRAGMA wal_checkpoint(FULL)`, completed: the WAL is synced before,
    /// and the database file after, so every commit so far is durable.
    fn checkpoint(&mut self) {
        self.point("before checkpoint");
        let (markers, rest): (Vec<Op>, Vec<Op>) =
            self.pending_entries.drain(..).partition(|o| matches!(o, Op::Marker(_)));
        self.pending_entries = rest;
        for m in markers { self.durable.apply(&m); }
    }
    fn finish(&mut self) {
        self.point("after the last op");
    }
}

/// Every disk a power loss at this point could leave: any subset of the
/// pending directory operations (applied in issue order), and, for each inode
/// with a pending write, either its durable or its written data.
fn power_loss_states(durable: &Disk, pend_e: &[Op], pend_d: &BTreeMap<Ino, Vec<u8>>) -> Vec<Disk> {
    let mut out = vec![];
    let inos: Vec<&Ino> = pend_d.keys().collect();
    for emask in 0u32..(1 << pend_e.len()) {
        for dmask in 0u32..(1 << inos.len()) {
            let mut d = durable.clone();
            for (i, op) in pend_e.iter().enumerate() {
                if emask & (1 << i) != 0 {
                    d.apply(op);
                }
            }
            for (j, ino) in inos.iter().enumerate() {
                if dmask & (1 << j) != 0 {
                    d.apply(&Op::Write { ino: **ino, data: pend_d[*ino].clone() });
                }
            }
            out.push(d);
        }
    }
    out
}

/// A killed process: the kernel keeps every write. One state per point.
fn kill_state(durable: &Disk, pend_e: &[Op], pend_d: &BTreeMap<Ino, Vec<u8>>) -> Vec<Disk> {
    let mut d = durable.clone();
    for op in pend_e { d.apply(op); }
    for (i, data) in pend_d { d.apply(&Op::Write { ino: *i, data: data.clone() }); }
    vec![d]
}

const OLD: &[u8] = b"OLD-KEY";
const NEW: &[u8] = b"NEW-KEY";

/// M2, narrowed to what a crash can lose: once the database is resealed,
/// some file must still hold the new key; before that, some file must still
/// hold the old one. (Which file startup reads is recovery's question,
/// prefix 2, and is checked separately.)
fn m2_holds(d: &Disk) -> bool {
    let want = if d.markers.contains(&"db_resealed") { NEW } else { OLD };
    d.entries.keys().any(|n| d.read(n) == Some(want))
}
/// The stronger property startup needs today, with no recovery: the active
/// path itself holds the key the database is sealed under.
fn active_holds(d: &Disk) -> bool {
    let want = if d.markers.contains(&"db_resealed") { NEW } else { OLD };
    d.read("key") == Some(want)
}

/// `cli.rs` rotation's order as of 0.90.0, with stage 1's syncs optional.
fn rotation(fsyncs: bool) -> SimFs { rotation_dropping(if fsyncs { None } else { Some(usize::MAX) }, false) }

/// The stage 1 order with its four syncs; `drop` removes one of them
/// (0..4), `Some(usize::MAX)` removes all.
fn rotation_dropping(drop: Option<usize>, checkpoint: bool) -> SimFs {
    let keep = |i: usize| match drop { None => true, Some(usize::MAX) => false, Some(d) => d != i };
    let mut fs = SimFs::new(&[("key", OLD)]);
    fs.create_new("key.new");
    fs.write_all("key.new", NEW);
    if keep(0) { fs.sync_file("key.new"); }
    if keep(1) { fs.sync_dir(); }
    fs.wal_commit("db_resealed");
    if checkpoint { fs.checkpoint(); }
    fs.rename("key", "key.bak");
    if keep(2) { fs.sync_dir(); }
    fs.rename("key.new", "key");
    if keep(3) { fs.sync_dir(); }
    fs.finish();
    fs
}

fn run(name: &str, fs: &SimFs, model: fn(&Disk, &[Op], &BTreeMap<Ino, Vec<u8>>) -> Vec<Disk>) {
    let (mut states, mut m2_bad, mut active_bad, mut done_bad) = (0, 0, 0, 0);
    let mut first: Option<String> = None;
    for (label, durable, pe, pd) in &fs.points {
        for d in model(durable, pe, pd) {
            states += 1;
            if !m2_holds(&d) {
                m2_bad += 1;
                first.get_or_insert_with(|| format!("{label}: {:?}", d.entries.keys().map(|n| (n.as_str(), String::from_utf8_lossy(d.read(n).unwrap()).into_owned())).collect::<Vec<_>>()));
            }
            if !active_holds(&d) { active_bad += 1; }
            // Success is printed after the last op: from then on, every
            // power-loss state must have the new key at the active path.
            // ... and must be the key the database is actually sealed under,
            // which a rolled-back WAL commit makes the old one again.
            if label == "after the last op"
                && (!active_holds(&d) || !d.markers.contains(&"db_resealed")) { done_bad += 1; }
        }
    }
    println!("{name}: {} crash points, {states} states; M2 violated in {m2_bad}; active path wrong in {active_bad}; wrong after success in {done_bad}", fs.points.len());
    if let Some(f) = first { println!("    first M2 violation: {f}"); }
}

fn main() {
    let bare = rotation(false);
    let synced = rotation(true);
    run("0.90.0 order, kill model        ", &bare, kill_state);
    run("0.90.0 order, power-loss model  ", &bare, power_loss_states);
    run("stage 1 syncs, kill model       ", &synced, kill_state);
    run("stage 1 syncs, power-loss model ", &synced, power_loss_states);
    let names = ["sync_file(key.new)", "sync_dir before the reseal", "sync_dir after rename 1", "sync_dir after rename 2"];
    for (i, n) in names.iter().enumerate() {
        run(&format!("drop {n:<28}"), &rotation_dropping(Some(i), true), power_loss_states);
    }
    run("stage 1 + checkpoint, power-loss", &rotation_dropping(None, true), power_loss_states);
    run("stage 1 + checkpoint, kill model", &rotation_dropping(None, true), kill_state);
}
```
