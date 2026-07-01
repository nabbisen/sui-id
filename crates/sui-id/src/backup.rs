//! Encrypted backup and restore for sui-id (RFC 040, refactored RFC 075).

mod ops;
mod tar;
#[cfg(test)]
mod tests;
mod types;

pub use ops::{run_backup, run_restore, run_verify};
pub use types::{BackupOptions, Manifest, RestoreOptions, VerifyReport};

/// Archive format version. Incremented on incompatible format changes.
pub const FORMAT_VERSION: u32 = 1;
