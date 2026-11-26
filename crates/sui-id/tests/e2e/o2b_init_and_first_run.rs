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
        // A hang guard, not a speed assertion: stdin is closed, so a
        // command that waited for input would wait forever, and one that
        // works exits as soon as it has printed. Generous for a loaded
        // runner (see o2a_cli_registry's listening waits).
        if start.elapsed() > Duration::from_secs(60) {
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

// ---------- RFC 148 closure, 0.95.0 Q1: the first run, in a real terminal ----------
//
// `script` (util-linux) gives the child a real pseudo-terminal for stdin,
// stdout and stderr, so `stdin_and_stderr_are_terminals()` reads true the
// same way it would for an operator at a keyboard. No pty crate is added
// (O2b's review rejected one for a single test; that reasoning still
// holds). If `script` is not on this machine, the test below fails,
// naming it -- never skips, which is exactly how the terminal case went
// untested for a release.

/// Drives bare `sui-id` through a real pty via `script -q -e -c <bin>
/// /dev/null`. `answer`, if given, is written to the child's stdin and
/// followed by EOF (closing the handle); `None` means EOF with no bytes
/// at all -- the "end of input with no answer" case. `-e` makes
/// `script`'s own exit code the child's. The same wall-clock hang guard
/// `run_bounded` uses: stdin always reaches EOF one way or another, so a
/// build that waits anyway is a regression, not a slow pass.
fn run_in_pty(dir: &std::path::Path, answer: Option<&str>) -> Output {
    assert!(
        Command::new("script").arg("--version").output().is_ok(),
        "script (util-linux) is not on this machine -- required, not optional, for this test"
    );
    let mut child = Command::new("script")
        .args(["-q", "-e", "-c", BIN, "/dev/null"])
        .current_dir(dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn script");
    if let Some(answer) = answer {
        use std::io::Write as _;
        child
            .stdin
            .as_mut()
            .expect("stdin")
            .write_all(answer.as_bytes())
            .expect("write the answer to script's stdin");
    }
    drop(child.stdin.take()); // EOF, with or without a prior answer.

    let start = Instant::now();
    loop {
        if let Some(_status) = child.try_wait().expect("wait") {
            return child.wait_with_output().expect("output");
        }
        if start.elapsed() > Duration::from_secs(60) {
            let _ = child.kill();
            panic!("script-driven run did not exit promptly -- it must never wait past EOF");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// `script` merges the pty's stdout and stderr into one stream; read
/// both captured handles regardless, so a future change in which stream
/// a line lands on cannot make an assertion here silently stop matching.
fn pty_text(o: &Output) -> String {
    format!("{}{}", stdout(o), stderr(o))
}

#[test]
fn the_offer_appears_and_n_writes_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let out = run_in_pty(dir.path(), Some("n\n"));
    let text = pty_text(&out);
    assert!(text.contains("Create one now?"), "{text}");
    assert!(text.contains("choice [l/p/n]"), "{text}");
    assert_eq!(out.status.code(), Some(1), "{text}");
    assert!(!dir.path().join("sui-id.toml").exists(), "nothing written");
}

#[test]
fn l_writes_the_local_trial_and_does_not_serve() {
    let dir = tempfile::tempdir().expect("tempdir");
    let out = run_in_pty(dir.path(), Some("l\n"));
    assert_eq!(out.status.code(), Some(0), "{}", pty_text(&out));

    let written_path = dir.path().join("sui-id.toml");
    assert!(written_path.exists());
    let written = std::fs::read(&written_path).expect("read");

    // The same content `sui-id init` itself writes in an empty
    // directory -- one function behind both (first_run.rs's own doc
    // comment), proved here rather than assumed.
    let reference_dir = tempfile::tempdir().expect("tempdir");
    let reference_out = run_bounded(&["init"], reference_dir.path());
    assert_eq!(
        reference_out.status.code(),
        Some(0),
        "{}",
        stderr(&reference_out)
    );
    let reference = std::fs::read(reference_dir.path().join("sui-id.toml")).expect("read");
    assert_eq!(
        written, reference,
        "byte-identical to `sui-id init`'s own output"
    );
}

#[test]
fn p_writes_the_production_template() {
    let dir = tempfile::tempdir().expect("tempdir");
    let out = run_in_pty(dir.path(), Some("p\n"));
    assert_eq!(out.status.code(), Some(0), "{}", pty_text(&out));

    let written = std::fs::read(dir.path().join("sui-id.toml")).expect("read");

    let reference_dir = tempfile::tempdir().expect("tempdir");
    let reference_out = run_bounded(&["init", "--production"], reference_dir.path());
    assert_eq!(
        reference_out.status.code(),
        Some(0),
        "{}",
        stderr(&reference_out)
    );
    let reference = std::fs::read(reference_dir.path().join("sui-id.toml")).expect("read");
    assert_eq!(
        written, reference,
        "byte-identical to `sui-id init --production`'s own output"
    );
}

#[test]
fn an_unexpected_answer_is_not_now() {
    let dir = tempfile::tempdir().expect("tempdir");
    let out = run_in_pty(dir.path(), Some("x\n"));
    // Q1 review: exit 1 with nothing written is also what the no-terminal
    // path does. Asserting the offer was shown is what makes this a test
    // of the prompt's answer, not of whether a terminal was seen at all.
    assert!(
        pty_text(&out).contains("choice [l/p/n]"),
        "{}",
        pty_text(&out)
    );
    assert_eq!(out.status.code(), Some(1), "{}", pty_text(&out));
    assert!(!dir.path().join("sui-id.toml").exists(), "nothing written");
}

#[test]
fn end_of_input_with_no_answer_is_not_now() {
    let dir = tempfile::tempdir().expect("tempdir");
    let out = run_in_pty(dir.path(), None);
    // Q1 review: exit 1 with nothing written is also what the no-terminal
    // path does. Asserting the offer was shown is what makes this a test
    // of the prompt's answer, not of whether a terminal was seen at all.
    assert!(
        pty_text(&out).contains("choice [l/p/n]"),
        "{}",
        pty_text(&out)
    );
    assert_eq!(out.status.code(), Some(1), "{}", pty_text(&out));
    assert!(!dir.path().join("sui-id.toml").exists(), "nothing written");
}
