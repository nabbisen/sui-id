//! RFC 100 stage 0: the only path to the key files.
//!
//! One trait, three implementations. **Real** is the only one a release
//! build can reach — `std` only, no new dependency, no `unsafe`. The other
//! two exist for tests: **Failpoint** (process termination and short
//! writes, through the real binary, only under the `test-support`
//! feature) and **Simulated** (`#[cfg(test)]` only: the power-loss model
//! that reproduces the crash-injection harness's prototype against the
//! real code).
//!
//! `rotate.*`'s own operations call through here; nothing else in this
//! crate opens, writes, renames, or removes a key file directly. The
//! "only path" test in `key_adapter::tests` enforces it with a grep over
//! the production source.

use std::io;
use std::path::{Path, PathBuf};

/// What [`KeyFileOps::metadata`] reports: enough to tell a key file from a
/// symlink standing in for one, and to check its owner and mode, without
/// `unsafe` — `std::os::unix::fs::MetadataExt` already reads what
/// `stat(2)` filled in. `symlink_target` is `Some` only when `is_symlink`
/// is true (RFC 100 stage 2, M6: the refusal names the link *and* its
/// target, so simply reporting "this is a symlink" is not enough).
/// `is_dir` (RFC 100 stage 4): recovery must tell the rotation workspace,
/// a real directory, from a file or symlink planted at the same name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyFileMetadata {
    pub uid: u32,
    pub mode: u32,
    pub is_symlink: bool,
    pub is_dir: bool,
    pub symlink_target: Option<PathBuf>,
}

/// The primitives RFC 100 names for touching a key file or its directory.
/// `point` is a call-site label (`"rotate.stage.write"`, and so on) — it
/// does nothing in [`Real`], selects a [`Failpoint`] injection, and labels
/// a crash point for `Simulated`. The two database-durability markers are
/// no-ops here too; [`Simulated`] is where they mean something.
pub trait KeyFileOps {
    fn create_new(&self, point: &str, path: &Path) -> io::Result<()>;
    fn write_all(&self, point: &str, path: &Path, data: &[u8]) -> io::Result<()>;
    fn sync_file(&self, point: &str, path: &Path) -> io::Result<()>;
    /// Atomic replacement: may silently overwrite an existing `to`.
    fn rename(&self, point: &str, from: &Path, to: &Path) -> io::Result<()>;
    /// Atomic **no-replace** publication: `to` must not already exist.
    fn publish_no_replace(&self, point: &str, from: &Path, to: &Path) -> io::Result<()>;
    fn sync_dir(&self, point: &str, dir: &Path) -> io::Result<()>;
    fn read(&self, point: &str, path: &Path) -> io::Result<Vec<u8>>;
    fn remove(&self, point: &str, path: &Path) -> io::Result<()>;
    fn metadata(&self, point: &str, path: &Path) -> io::Result<KeyFileMetadata>;
    /// RFC 100 stage 3: create the rotation workspace -- mode `0700`,
    /// current owner, failing if anything (a file, directory, or symlink)
    /// already exists at `path`. Never follows a symlink: the failure
    /// mode is refusing to create, not creating through one.
    fn create_dir(&self, point: &str, path: &Path) -> io::Result<()>;
    /// The inverse of [`KeyFileOps::create_dir`]. Fails if `path` is not
    /// an empty directory, the same as `rmdir(2)`.
    fn remove_dir(&self, point: &str, path: &Path) -> io::Result<()>;
    /// RFC 100 stage 4: every entry directly inside `dir` (not
    /// recursive), as full paths -- recovery's own classification needs
    /// to see *everything* a workspace holds, to fail closed on anything
    /// it does not bind, not just check for named paths one at a time.
    fn list_dir(&self, point: &str, dir: &Path) -> io::Result<Vec<PathBuf>>;
    /// Rotation calls this right after K07's transaction commits. A no-op
    /// outside [`Simulated`].
    fn db_committed(&self, label: &str);
    /// Rotation calls this right after K07's commit is made durable (its
    /// checkpoint). A no-op outside [`Simulated`].
    fn db_durable(&self, label: &str);
    /// RFC 100 `0.94.0` P2 §1: the liveness lock. `path` is
    /// `RotationPaths::lock`, a permanent, empty, mode-`0600` file never
    /// deleted -- creating it if absent is harmless. `blocking` chooses
    /// `flock(2)`'s own blocking or `LOCK_NB` form; every call site in
    /// this crate passes `false` (rotation and recovery both refuse
    /// immediately rather than wait), but the primitive itself does not
    /// assume that. On success, the lock is held for exactly as long as
    /// the returned guard lives -- dropping it (including by an `Err`
    /// propagating out of the caller's own function, the normal-Rust
    /// analogue of "the process that held it exited") releases it. A
    /// non-blocking call that cannot take the lock returns
    /// [`io::ErrorKind::WouldBlock`], distinguishable from every other
    /// failure a caller must still handle (the file could not be
    /// created at all, say).
    fn lock_exclusive(
        &self,
        point: &str,
        path: &Path,
        blocking: bool,
    ) -> io::Result<Box<dyn LockGuard + '_>>;
}

/// What [`KeyFileOps::lock_exclusive`] returns: nothing to call, only to
/// hold and eventually drop. Every implementation's own guard releases
/// the lock on [`Drop`].
pub trait LockGuard {}

/// Production. The only implementation a release build links.
pub struct Real;

impl KeyFileOps for Real {
    fn create_new(&self, _point: &str, path: &Path) -> io::Result<()> {
        use std::fs::OpenOptions;
        use std::os::unix::fs::OpenOptionsExt;
        OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(path)?;
        Ok(())
    }

    fn write_all(&self, _point: &str, path: &Path, data: &[u8]) -> io::Result<()> {
        use std::fs::OpenOptions;
        use std::io::Write;
        OpenOptions::new().write(true).open(path)?.write_all(data)
    }

    fn sync_file(&self, _point: &str, path: &Path) -> io::Result<()> {
        std::fs::OpenOptions::new()
            .write(true)
            .open(path)?
            .sync_all()
    }

    fn rename(&self, _point: &str, from: &Path, to: &Path) -> io::Result<()> {
        std::fs::rename(from, to)
    }

    fn publish_no_replace(&self, _point: &str, from: &Path, to: &Path) -> io::Result<()> {
        // `link(2)` refuses an existing name at `to`, unlike `rename(2)`,
        // which would silently replace it.
        std::fs::hard_link(from, to)?;
        std::fs::remove_file(from)
    }

    fn sync_dir(&self, _point: &str, dir: &Path) -> io::Result<()> {
        std::fs::File::open(dir)?.sync_all()
    }

    fn read(&self, _point: &str, path: &Path) -> io::Result<Vec<u8>> {
        std::fs::read(path)
    }

    fn remove(&self, _point: &str, path: &Path) -> io::Result<()> {
        std::fs::remove_file(path)
    }

    fn metadata(&self, _point: &str, path: &Path) -> io::Result<KeyFileMetadata> {
        use std::os::unix::fs::MetadataExt;
        let m = std::fs::symlink_metadata(path)?;
        let is_symlink = m.file_type().is_symlink();
        Ok(KeyFileMetadata {
            uid: m.uid(),
            mode: m.mode() & 0o777,
            is_symlink,
            is_dir: m.file_type().is_dir(),
            symlink_target: is_symlink.then(|| std::fs::read_link(path).ok()).flatten(),
        })
    }

    fn create_dir(&self, _point: &str, path: &Path) -> io::Result<()> {
        use std::os::unix::fs::DirBuilderExt;
        // No `.recursive(true)`: a plain `mkdir(2)`, which fails with
        // `AlreadyExists` on anything already at `path` -- a file, a
        // directory, or a symlink -- and never resolves a symlink for
        // its own, final-component creation.
        std::fs::DirBuilder::new().mode(0o700).create(path)
    }

    fn remove_dir(&self, _point: &str, path: &Path) -> io::Result<()> {
        std::fs::remove_dir(path)
    }

    fn list_dir(&self, _point: &str, dir: &Path) -> io::Result<Vec<PathBuf>> {
        std::fs::read_dir(dir)?
            .map(|entry| entry.map(|e| e.path()))
            .collect()
    }

    fn db_committed(&self, _label: &str) {}
    fn db_durable(&self, _label: &str) {}

    fn lock_exclusive(
        &self,
        _point: &str,
        path: &Path,
        blocking: bool,
    ) -> io::Result<Box<dyn LockGuard + '_>> {
        use rustix::fs::FlockOperation;
        use std::fs::OpenOptions;
        use std::os::unix::fs::OpenOptionsExt;
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .mode(0o600)
            .open(path)?;
        let op = if blocking {
            FlockOperation::LockExclusive
        } else {
            FlockOperation::NonBlockingLockExclusive
        };
        rustix::fs::flock(&file, op).map_err(io::Error::from)?;
        Ok(Box::new(RealLockGuard(file)))
    }
}

/// Held only so its `Drop` closes the file descriptor, which is what
/// releases the `flock` -- never read again once acquired.
#[allow(dead_code)]
struct RealLockGuard(std::fs::File);

impl LockGuard for RealLockGuard {}

/// The adapter every production call site uses: [`Real`], unless the
/// `test-support` feature is compiled in **and** `SUI_ID_FAILPOINT` names
/// a point in this exact process — a release build checks neither the
/// feature nor the variable, so this costs nothing there.
pub fn default_adapter() -> Box<dyn KeyFileOps> {
    #[cfg(feature = "test-support")]
    {
        if let Ok(spec) = std::env::var(failpoint::ENV_VAR) {
            return Box::new(failpoint::Failpoint::from_spec(&spec));
        }
    }
    Box::new(Real)
}

#[cfg(feature = "test-support")]
pub mod failpoint {
    //! Process termination and short writes, driven through the real
    //! binary (`SUI_ID_FAILPOINT=<point>:<action>`, read once per
    //! process). Never active unless `test-support` is compiled in.
    use super::{KeyFileMetadata, KeyFileOps, LockGuard, Real};
    use std::io;
    use std::path::Path;

    pub const ENV_VAR: &str = "SUI_ID_FAILPOINT";

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Action {
        Abort,
        Error,
        ShortWrite,
    }

    pub struct Failpoint {
        point: String,
        action: Action,
    }

    impl Failpoint {
        /// Parse `<point>:<action>`. An unrecognized action is not a
        /// matchable point: every call compares its own point name
        /// against `self.point`, so a malformed spec simply never fires.
        pub fn from_spec(spec: &str) -> Self {
            let (point, action) = spec.split_once(':').unwrap_or((spec, ""));
            let action = match action {
                "abort" => Action::Abort,
                "short-write" => Action::ShortWrite,
                _ => Action::Error,
            };
            Self {
                point: point.to_owned(),
                action,
            }
        }

        /// `true`, and acts, only when `point` is the one named by the
        /// environment variable.
        fn fire(&self, point: &str) -> Option<Action> {
            (point == self.point).then_some(self.action)
        }
    }

    fn denied(point: &str) -> io::Error {
        io::Error::other(format!("sui-id test failpoint at {point}"))
    }

    impl KeyFileOps for Failpoint {
        fn create_new(&self, point: &str, path: &Path) -> io::Result<()> {
            match self.fire(point) {
                Some(Action::Abort) => std::process::abort(),
                Some(_) => Err(denied(point)),
                None => Real.create_new(point, path),
            }
        }

        fn write_all(&self, point: &str, path: &Path, data: &[u8]) -> io::Result<()> {
            match self.fire(point) {
                Some(Action::Abort) => std::process::abort(),
                Some(Action::ShortWrite) => {
                    let half = data.len() / 2;
                    Real.write_all(point, path, &data[..half])?;
                    Err(denied(point))
                }
                Some(Action::Error) => Err(denied(point)),
                None => Real.write_all(point, path, data),
            }
        }

        fn sync_file(&self, point: &str, path: &Path) -> io::Result<()> {
            match self.fire(point) {
                Some(Action::Abort) => std::process::abort(),
                Some(_) => Err(denied(point)),
                None => Real.sync_file(point, path),
            }
        }

        fn rename(&self, point: &str, from: &Path, to: &Path) -> io::Result<()> {
            match self.fire(point) {
                Some(Action::Abort) => std::process::abort(),
                Some(_) => Err(denied(point)),
                None => Real.rename(point, from, to),
            }
        }

        fn publish_no_replace(&self, point: &str, from: &Path, to: &Path) -> io::Result<()> {
            match self.fire(point) {
                Some(Action::Abort) => std::process::abort(),
                Some(_) => Err(denied(point)),
                None => Real.publish_no_replace(point, from, to),
            }
        }

        fn sync_dir(&self, point: &str, dir: &Path) -> io::Result<()> {
            match self.fire(point) {
                Some(Action::Abort) => std::process::abort(),
                Some(_) => Err(denied(point)),
                None => Real.sync_dir(point, dir),
            }
        }

        fn read(&self, point: &str, path: &Path) -> io::Result<Vec<u8>> {
            match self.fire(point) {
                Some(Action::Abort) => std::process::abort(),
                Some(_) => Err(denied(point)),
                None => Real.read(point, path),
            }
        }

        fn remove(&self, point: &str, path: &Path) -> io::Result<()> {
            match self.fire(point) {
                Some(Action::Abort) => std::process::abort(),
                Some(_) => Err(denied(point)),
                None => Real.remove(point, path),
            }
        }

        fn metadata(&self, point: &str, path: &Path) -> io::Result<KeyFileMetadata> {
            match self.fire(point) {
                Some(Action::Abort) => std::process::abort(),
                Some(_) => Err(denied(point)),
                None => Real.metadata(point, path),
            }
        }

        fn create_dir(&self, point: &str, path: &Path) -> io::Result<()> {
            match self.fire(point) {
                Some(Action::Abort) => std::process::abort(),
                Some(_) => Err(denied(point)),
                None => Real.create_dir(point, path),
            }
        }

        fn remove_dir(&self, point: &str, path: &Path) -> io::Result<()> {
            match self.fire(point) {
                Some(Action::Abort) => std::process::abort(),
                Some(_) => Err(denied(point)),
                None => Real.remove_dir(point, path),
            }
        }

        fn list_dir(&self, point: &str, dir: &Path) -> io::Result<Vec<std::path::PathBuf>> {
            match self.fire(point) {
                Some(Action::Abort) => std::process::abort(),
                Some(_) => Err(denied(point)),
                None => Real.list_dir(point, dir),
            }
        }

        fn db_committed(&self, _label: &str) {}
        fn db_durable(&self, _label: &str) {}

        fn lock_exclusive(
            &self,
            point: &str,
            path: &Path,
            blocking: bool,
        ) -> io::Result<Box<dyn LockGuard + '_>> {
            match self.fire(point) {
                Some(Action::Abort) => std::process::abort(),
                Some(_) => Err(denied(point)),
                None => Real.lock_exclusive(point, path, blocking),
            }
        }
    }
}

#[cfg(test)]
#[path = "key_adapter/simulated.rs"]
pub mod simulated;

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
#[path = "key_adapter/tests.rs"]
mod tests;
