//! RFC 094's mandatory negative fixture: a new, repository-shaped
//! function that calls `Connection::execute("UPDATE ...")` with no
//! `declare_write_command!` anywhere near it, and is nowhere on
//! `contracts/write-authority.toml`'s allow-list. Both controls must
//! reject it, in different ways: this package must `cargo check`
//! cleanly (it is valid, compiling Rust -- the compiler cannot reject
//! an unregistered write, which is why the AST control exists at all),
//! and `cargo xtask audit-structure --negative-fixtures` must flag it
//! by name as an unregistered write. A fixture that fails to build
//! proves nothing; a fixture the structural gate does not reject
//! proves the gate inspected nothing.

pub fn mark_user_disabled_within_tx(
    tx: &rusqlite::Transaction<'_>,
    user_id: &str,
) -> Result<(), rusqlite::Error> {
    tx.execute(
        "UPDATE users SET is_disabled = 1 WHERE id = ?1",
        rusqlite::params![user_id],
    )?;
    Ok(())
}
