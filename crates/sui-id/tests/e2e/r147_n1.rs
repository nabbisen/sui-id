//! RFC 147 §3 (N1's dispatch): a missing `sui-id.toml` says how to make
//! one, through the real binary, in an empty directory -- not just
//! `Config::load` in isolation (`sui-id-store`'s own unit tests already
//! cover that).

use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_sui-id");

fn run_in_empty_dir(args: &[&str]) -> (tempfile::TempDir, Output) {
    let dir = tempfile::tempdir().expect("tempdir");
    let out = Command::new(BIN)
        .args(args)
        .current_dir(dir.path())
        .output()
        .expect("run");
    (dir, out)
}

fn assert_names_the_three_parts(out: &Output, expected_path: &str) {
    assert!(
        !out.status.success(),
        "a missing config must refuse, not start"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains(&format!("no configuration file at {expected_path}")),
        "part 1 (what, and its path): {stderr}"
    );
    assert!(
        stderr.contains("sui-id --print-sample-config > sui-id.toml"),
        "part 2 (how to make a local trial): {stderr}"
    );
    assert!(
        stderr.contains("sui-id.example.toml"),
        "part 3 (the production template): {stderr}"
    );
    assert!(
        stderr.contains(&format!(
            "https://github.com/nabbisen/sui-id/blob/{}/docs/src/reference/configuration.md",
            env!("CARGO_PKG_VERSION")
        )),
        "part 3's reference must be pinned to this binary's own version: {stderr}"
    );
}

/// No argument: the default `./sui-id.toml`, relative to an empty cwd.
#[test]
fn no_argument_in_an_empty_directory() {
    let (_dir, out) = run_in_empty_dir(&[]);
    assert_names_the_three_parts(&out, "./sui-id.toml");
}

/// `setup --admin-username admin`: the headless-setup subcommand reaches
/// the same `Config::load` after its own `--admin-username` check.
#[test]
fn setup_subcommand_in_an_empty_directory() {
    let (_dir, out) = run_in_empty_dir(&["setup", "--admin-username", "admin"]);
    assert_names_the_three_parts(&out, "./sui-id.toml");
}

/// An explicit `--config` path that also does not exist.
#[test]
fn explicit_missing_config_path() {
    let (_dir, out) = run_in_empty_dir(&["--config", "./missing.toml"]);
    assert_names_the_three_parts(&out, "./missing.toml");
}
