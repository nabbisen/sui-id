//! RFC 148 cycle O2b §7: `sui-id init` and the first-run conversation,
//! through the real binary. The interactive prompt itself (RFC 148 §4's
//! own worked example) is driven through a real pseudo-terminal by hand
//! for this return (the transcript is pasted into the review package,
//! per §8) rather than automated here: this workspace has no PTY-capable
//! crate today, and adding one for a single test is not a trade worth
//! making (§7's own escape clause: "if you cannot allocate one in the
//! test environment, say so and test the decision function directly").
//! `cli::first_run::tests::decide_covers_all_eight_cases` is that test,
//! exhaustive over every input the prompt-or-not choice depends on.

use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

const BIN: &str = env!("CARGO_BIN_EXE_sui-id");

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}
fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

/// Runs `sui-id` with its stdin closed (never a terminal) and a
/// wall-clock bound, so a regression that makes it wait reads as a
/// failure rather than a stalled suite.
fn run_bounded(args: &[&str], dir: &std::path::Path) -> Output {
    let mut child = Command::new(BIN)
        .args(args)
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    let start = Instant::now();
    loop {
        if let Some(_status) = child.try_wait().expect("wait") {
            return child.wait_with_output().expect("output");
        }
        if start.elapsed() > Duration::from_secs(10) {
            let _ = child.kill();
            panic!("{args:?} did not exit promptly -- it must never wait without a terminal");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn init_never_overwrites() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("sui-id.toml");

    let first = run_bounded(&["init"], dir.path());
    assert_eq!(first.status.code(), Some(0), "{}", stderr(&first));
    assert!(path.exists());
    let first_bytes = std::fs::read(&path).expect("read");

    let second = run_bounded(&["init"], dir.path());
    assert!(
        !second.status.success(),
        "a second `init` must refuse, not overwrite"
    );
    assert!(
        stderr(&second).contains("./sui-id.toml"),
        "the refusal must name the existing path: {}",
        stderr(&second)
    );

    let after_bytes = std::fs::read(&path).expect("read after the refused second run");
    assert_eq!(
        first_bytes, after_bytes,
        "the file must be byte-identical after the refused overwrite"
    );
}

/// **Mutation (run and reverted):** `run_init`'s `OpenOptions` was
/// temporarily changed from `.create_new(true)` to plain `.create(true)`
/// (write, truncate, no "already exists" check) -- confirmed red (the
/// second run exited 0 and the file's content changed to the fresh
/// render, losing any edits); reverted.
#[test]
fn init_second_run_names_the_path_and_exits_nonzero() {
    let dir = tempfile::tempdir().expect("tempdir");
    assert_eq!(run_bounded(&["init"], dir.path()).status.code(), Some(0));
    let second = run_bounded(&["init"], dir.path());
    assert_ne!(second.status.code(), Some(0));
    assert!(stderr(&second).contains("./sui-id.toml"));
}

#[test]
fn init_and_config_sample_agree_byte_for_byte_both_profiles() {
    let dir = tempfile::tempdir().expect("tempdir");

    let sample = run_bounded(&["config", "sample"], dir.path());
    assert_eq!(sample.status.code(), Some(0));
    let init = run_bounded(&["init", "--output", "local.toml"], dir.path());
    assert_eq!(init.status.code(), Some(0), "{}", stderr(&init));
    let written = std::fs::read_to_string(dir.path().join("local.toml")).expect("read");
    assert_eq!(stdout(&sample), written, "local-trial profile must agree");

    let sample_prod = run_bounded(&["config", "sample", "--production"], dir.path());
    assert_eq!(sample_prod.status.code(), Some(0));
    let init_prod = run_bounded(
        &["init", "--production", "--output", "prod.toml"],
        dir.path(),
    );
    assert_eq!(init_prod.status.code(), Some(0), "{}", stderr(&init_prod));
    let written_prod = std::fs::read_to_string(dir.path().join("prod.toml")).expect("read");
    assert_eq!(
        stdout(&sample_prod),
        written_prod,
        "production profile must agree"
    );
}

/// RFC 148 §4: without a terminal, never prompt, never wait. `stdin` is
/// closed above (never a terminal) and `run_bounded`'s own wall-clock
/// bound is the "never wait" half of this assertion.
#[test]
fn no_terminal_no_prompt_exits_one_promptly_with_the_guidance() {
    let dir = tempfile::tempdir().expect("tempdir");
    let out = run_bounded(&[], dir.path());
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    let err = stderr(&out);
    assert!(err.starts_with("error: no configuration found at"), "{err}");
    assert!(err.contains("sui-id init"), "{err}");
    assert!(!dir.path().join("sui-id.toml").exists(), "nothing written");
}

/// RFC 148 §6 answer 3: an explicit `--config` that is missing never
/// prompts either, even though this case is otherwise indistinguishable
/// from the no-terminal one at the point the decision is made.
#[test]
fn explicit_missing_config_never_prompts_even_with_stdin_open() {
    let dir = tempfile::tempdir().expect("tempdir");
    // stdin is a real pipe here (not closed), so this is not the
    // no-terminal case -- a pipe is also not a terminal, which is
    // exactly why §6 answer 3 is its own rule: an operator could in
    // principle run this with stdin attached to something other than a
    // TTY and it must still never wait.
    let out = Command::new(BIN)
        .args(["--config", "./missing.toml"])
        .current_dir(dir.path())
        .stdin(Stdio::piped())
        .output()
        .expect("run");
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    let err = stderr(&out);
    assert!(err.contains("./missing.toml"), "{err}");
    assert!(err.contains("sui-id init"), "{err}");
}

#[test]
fn serve_with_no_configuration_and_no_terminal_behaves_the_same_as_bare() {
    let dir = tempfile::tempdir().expect("tempdir");
    let out = run_bounded(&["serve"], dir.path());
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert!(stderr(&out).starts_with("error: no configuration found at"));
}
