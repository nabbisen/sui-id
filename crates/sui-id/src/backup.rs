//! Encrypted backup and restore for sui-id (RFC 040, refactored RFC 075).

mod types;
mod ops;
mod tar;
#[cfg(test)] mod tests;

pub use types::{Manifest, BackupOptions, RestoreOptions, VerifyReport};
pub use ops::{run_backup, run_restore, run_verify};



/// Archive format version. Incremented on incompatible format changes.
pub const FORMAT_VERSION: u32 = 1;
