"""RFC 093 cycle H item 4 self-tests for scripts/local-matrix.sh.

Run as: python3.14 -m unittest scripts.tests.test_local_matrix

scripts/local-matrix.sh derives its full set of checks from
`.github/workflows/ci.yml` rather than a hand-written list, because a
hand-written list is exactly what went wrong twice in four consecutive
`0.85.0` rounds: a dispatch's own count came back thirteen of the fourteen
`python3.14 -m unittest scripts.tests.test_*` steps, missing
`test_markdown_links`, and before that none of the four rounds ran those
fourteen steps at all. This module's own derivation (`ci_yml_derived_set`)
is written independently of the shell script's own awk/grep -- in Python,
against the same real `.github/workflows/ci.yml` -- rather than by
importing or re-using the script's logic, so a bug shared between the two
would not go undetected here.

Fixtures are synthetic `.github/workflows/ci.yml` files in a tempdir,
following this project's existing convention (test_generate_ci_workflow.py)
of mutating real project content rather than freestanding look-alikes:
the "missing lane" fixture is built from the real file's own unittest
lines, minus one, rather than a hand-typed count retyped from scratch.
"""

import re
import subprocess
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
SCRIPT = REPO_ROOT / "scripts" / "local-matrix.sh"
REAL_CI_YML = REPO_ROOT / ".github" / "workflows" / "ci.yml"
REAL_GATE_INPUTS = REPO_ROOT / "contracts" / "gate-inputs.toml"

GATE_RE = re.compile(r"bash scripts/ci-gate\.sh ([A-Za-z0-9]+)")
CMD_RE = re.compile(r'echo "command=(.*)"')
UNITTEST_CMD_RE = re.compile(r"python3\.14 -m unittest scripts\.tests\.\w+")


def ci_yml_derived_set(path: Path) -> set[str]:
    """Independent re-derivation of what `ci.yml` runs: every
    `bash scripts/ci-gate.sh <ID>` gate id (prefixed `GATE:`) and every
    `echo "command=<CMD>"` literal command (prefixed `CMD:`) -- the two
    markers the dispatch's own measurement (its §2) found cover everything
    ci.yml runs that `contracts/gate-inputs.toml`'s `[gates]` table alone
    does not."""
    text = path.read_text(encoding="utf-8")
    out = {f"GATE:{m}" for m in GATE_RE.findall(text)}
    out |= {f"CMD:{m}" for m in CMD_RE.findall(text)}
    return out


STEP_RE = re.compile(r"^      - name:.*$", re.MULTILINE)


def inverted_step_count(path: Path) -> int:
    """How many of `ci.yml`'s own steps would be read as INVERTED by
    `local-matrix.sh`'s polarity heuristic (`-eq 0` present anywhere in
    the step's body) -- an independent re-derivation of that heuristic,
    in Python, against the real file, the same reason `ci_yml_derived_set`
    exists separately from the shell script's own awk. Carried from the
    local-matrix round's own review (2026-10-28): the heuristic's unsafe
    direction (an ordinary step that happens to contain `-eq 0` is
    silently treated as inverted, and a real failure then reports PASS)
    had no guard. One assertion on the real file closes both directions
    at once -- a second genuine inverted fixture, or an accidental
    `-eq 0`, both change this count and a person then looks."""
    text = path.read_text(encoding="utf-8")
    starts = [m.start() for m in STEP_RE.finditer(text)] + [len(text)]
    steps = [text[starts[i] : starts[i + 1]] for i in range(len(starts) - 1)]
    return sum(1 for step in steps if 'echo "command=' in step and "-eq 0" in step)


def run_list(root: Path) -> set[str]:
    result = subprocess.run(
        ["bash", str(SCRIPT), "--root", str(root), "--list"],
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0, result.stderr
    out = set()
    for line in result.stdout.splitlines():
        kind, _, value = line.partition("\t")
        out.add(f"{kind}:{value}")
    return out


def _write_gate_inputs(root: Path, extra_gates: str = "") -> None:
    """A minimal, valid [gates]/[lane_profiles] table -- real gate-inputs.toml
    content is not needed for a ci.yml-only fixture; only a parseable file."""
    (root / "contracts").mkdir(parents=True, exist_ok=True)
    (root / "contracts" / "gate-inputs.toml").write_text(
        "[gates]\n" + extra_gates + "\n[lane_profiles]\n",
        encoding="utf-8",
    )


def _write_ci_yml(root: Path, body: str) -> None:
    dest = root / ".github" / "workflows" / "ci.yml"
    dest.parent.mkdir(parents=True, exist_ok=True)
    dest.write_text(body, encoding="utf-8")


class LocalMatrixDerivation(unittest.TestCase):
    def test_the_scripts_derived_set_matches_the_real_ci_yml(self):
        self.assertEqual(run_list(REPO_ROOT), ci_yml_derived_set(REAL_CI_YML))

    def test_a_lane_missing_from_ci_yml_is_absent_from_the_derived_set(self):
        # Modelled on the historical defect itself (dispatch §1): a synthetic
        # ci.yml carrying thirteen of the real fourteen unittest lines,
        # missing exactly the one an actual dispatch's hand count missed.
        # Built from the real file's own lines, not retyped, so this fixture
        # cannot itself drift from what ci.yml actually contains.
        real_text = REAL_CI_YML.read_text(encoding="utf-8")
        unittest_lines = sorted(set(UNITTEST_CMD_RE.findall(real_text)))
        self.assertIn(
            "python3.14 -m unittest scripts.tests.test_markdown_links",
            unittest_lines,
            "the real ci.yml no longer names test_markdown_links -- update this fixture",
        )
        omitted = "python3.14 -m unittest scripts.tests.test_markdown_links"
        kept = [line for line in unittest_lines if line != omitted]

        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            _write_gate_inputs(root)
            body = "".join(f'      - name: dummy\n        run: echo "command={cmd}"\n' for cmd in kept)
            _write_ci_yml(root, body)
            derived = run_list(root)

        self.assertNotIn(f"CMD:{omitted}", derived, "a lane absent from ci.yml must not appear in the derived set")
        for cmd in kept:
            self.assertIn(f"CMD:{cmd}", derived)

    def test_a_newly_added_lane_is_picked_up_without_editing_the_script(self):
        # The flip side, and the property this whole round exists for: a
        # lane genuinely present in ci.yml must appear in the derived set,
        # proving the script re-derives from the file rather than from a
        # cached or hand-written list.
        new_lane = "python3.14 -m unittest scripts.tests.test_a_brand_new_lane_not_known_today"
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            _write_gate_inputs(root)
            _write_ci_yml(root, f'      - name: dummy\n        run: echo "command={new_lane}"\n')
            derived = run_list(root)
        self.assertIn(f"CMD:{new_lane}", derived)

    def test_a_gate_id_not_yet_declared_anywhere_is_still_derived_by_name(self):
        # Gate-id derivation reads only ci.yml's own `bash scripts/ci-gate.sh
        # <ID>` lines, never a fixed G01..G21 range, so a 25th gate (G99, not
        # a real id) is still picked up by name.
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            _write_gate_inputs(root)
            _write_ci_yml(root, "      - name: dummy\n        run: bash scripts/ci-gate.sh G99\n")
            derived = run_list(root)
        self.assertIn("GATE:G99", derived)

    def test_the_real_ci_yml_has_exactly_one_inverted_check(self):
        # Carried from the local-matrix round's own review
        # (2026-10-28, §3): the polarity heuristic's dangerous direction
        # — an ordinary step containing `-eq 0` silently becomes
        # INVERTED, and a real failure then reports PASS — had no
        # guard. This closes both directions: a second genuine inverted
        # fixture, or an accidental `-eq 0` in an unrelated step, both
        # move this count away from the one known-correct value, and a
        # person then looks rather than the matrix silently reporting a
        # passed check that was actually a hidden failure.
        count = inverted_step_count(REAL_CI_YML)
        self.assertEqual(
            count,
            1,
            f"expected exactly one INVERTED step in the real ci.yml (the mdBook "
            f"missing-chapter negative fixture), found {count} -- review what "
            f"changed before trusting the local matrix's own PASS/FAIL split",
        )


SCOPED_LANE_IDS = ("G01", "G02", "G03", "G04", "G05", "G06", "G07", "G07b", "G08", "G09a", "G09b")
RUST_SCOPE_PATHS = [
    "crates/**",
    "Cargo.toml",
    "Cargo.lock",
    "clippy.toml",
    ".cargo/config.toml",
    "rust-toolchain*",
    "scripts/ci-gate.sh",
    "contracts/gate-inputs.toml",
    "contracts/workflow-template.toml",
    ".github/workflows/ci.yml",
]


def _git(root: Path, *args: str) -> subprocess.CompletedProcess:
    return subprocess.run(["git", "-C", str(root), *args], capture_output=True, text=True, check=True)


def _gate_inputs_for_scope(scoped_lanes: tuple[str, ...] = SCOPED_LANE_IDS) -> str:
    # `[gates]` entries are trivial `true` commands, not real Rust
    # commands: these fixtures run through `--changed` without `--list`
    # only to assert the summary line's own arithmetic, never to prove a
    # real gate's command.
    lines = ["[gates]"]
    for lane in (*scoped_lanes, "G10a"):
        lines.append(f'{lane} = "true"')
    lines.append("")
    lines.append("[lane_profiles]")
    for lane in scoped_lanes:
        lines.append(f"{lane} = {{ paths = {RUST_SCOPE_PATHS!r} }}")
    lines.append('G10a = { paths = ["**"] }')
    return "\n".join(lines) + "\n"


def _ci_yml_for_scope(scoped_lanes: tuple[str, ...] = SCOPED_LANE_IDS) -> str:
    parts = ["jobs:"]
    for lane in scoped_lanes:
        parts.append(f"  {lane}:")
        parts.append("    if: needs.changes.outputs.rust == 'true'")
        parts.append("    steps:")
        parts.append(f"      - run: bash scripts/ci-gate.sh {lane}")
    parts.append("  G10a:")
    parts.append("    steps:")
    parts.append("      - run: bash scripts/ci-gate.sh G10a")
    return "\n".join(parts) + "\n"


class LocalMatrixChangedChooser(unittest.TestCase):
    """RFC 093 cycle J stage 7d, §4: the ten chooser cases, black-box
    against real throwaway git repositories, driven through `--list
    --changed` (a seam that needs no cargo). `--base <commit>` is used
    explicitly in every case except the no-upstream one (7), so that case
    is the only one depending on the default `@{upstream}` resolution --
    every other case's own property (the changed-set union, `--no-renames`,
    the ignored-file exclusion) does not depend on how the base ref itself
    was found.
    """

    def _init_repo(self, root: Path, scoped_lanes: tuple[str, ...] = SCOPED_LANE_IDS) -> str:
        _git(root, "init", "-q", "-b", "main")
        _git(root, "config", "gc.auto", "0")
        _git(root, "config", "maintenance.auto", "false")
        _git(root, "-c", "user.email=t@example.invalid", "-c", "user.name=t", "config", "commit.gpgsign", "false")
        (root / "contracts").mkdir()
        (root / "contracts" / "gate-inputs.toml").write_text(
            _gate_inputs_for_scope(scoped_lanes), encoding="utf-8"
        )
        (root / ".github" / "workflows").mkdir(parents=True)
        (root / ".github" / "workflows" / "ci.yml").write_text(
            _ci_yml_for_scope(scoped_lanes), encoding="utf-8"
        )
        (root / "README.md").write_text("hello\n", encoding="utf-8")
        (root / ".gitignore").write_text("*.sqlite\n", encoding="utf-8")
        return self._commit_all(root, "base")

    def _commit_all(self, root: Path, message: str) -> str:
        _git(root, "add", "-A")
        _git(root, "-c", "user.email=t@example.invalid", "-c", "user.name=t", "commit", "-q", "-m", message)
        return _git(root, "rev-parse", "HEAD").stdout.strip()

    def _write(self, root: Path, rel: str, content: str = "x\n") -> Path:
        path = root / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")
        return path

    def _run_list_changed(self, root: Path, base: str | None = None) -> subprocess.CompletedProcess:
        args = ["bash", str(SCRIPT), "--root", str(root), "--list", "--changed"]
        if base is not None:
            args += ["--base", base]
        return subprocess.run(args, capture_output=True, text=True)

    def _parse(self, stdout: str) -> tuple[str | None, list[tuple[str, ...]]]:
        scope = None
        entries: list[tuple[str, ...]] = []
        for line in stdout.splitlines():
            parts = tuple(line.split("\t"))
            if parts[0] == "SCOPE":
                scope = parts[1]
            else:
                entries.append(parts)
        return scope, entries

    def _skipped_gates(self, entries: list[tuple[str, ...]]) -> set[str]:
        return {v for k1, k2, v in entries if k1 == "SKIP" and k2 == "GATE"}

    def _run_gates(self, entries: list[tuple[str, ...]]) -> set[str]:
        return {v for k1, k2, v in entries if k1 == "RUN" and k2 == "GATE"}

    # ── 1 ──────────────────────────────────────────────────────────────

    def test_docs_only_change_skips_the_scoped_lanes(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            base = self._init_repo(root)
            self._write(root, "docs/guide.md")
            self._commit_all(root, "docs")
            r = self._run_list_changed(root, base=base)
        self.assertEqual(r.returncode, 0, r.stderr)
        scope, entries = self._parse(r.stdout)
        self.assertEqual(self._skipped_gates(entries), set(SCOPED_LANE_IDS))
        self.assertIn("G10a", self._run_gates(entries))
        self.assertIn("no Rust-scope path", scope)

    # ── 2 ──────────────────────────────────────────────────────────────

    def test_a_committed_unpushed_rust_change_skips_nothing(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            base = self._init_repo(root)
            self._write(root, "crates/x/lib.rs")
            self._commit_all(root, "rust")
            r = self._run_list_changed(root, base=base)
        scope, entries = self._parse(r.stdout)
        self.assertEqual(self._skipped_gates(entries), set())
        self.assertIn("Rust scope touched", scope)

    # ── 3 ──────────────────────────────────────────────────────────────

    def test_a_staged_only_rust_change_skips_nothing(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            base = self._init_repo(root)
            self._write(root, "crates/x/lib.rs")
            _git(root, "add", "crates/x/lib.rs")  # staged, not committed
            r = self._run_list_changed(root, base=base)
        _, entries = self._parse(r.stdout)
        self.assertEqual(self._skipped_gates(entries), set())

    def test_an_unstaged_only_rust_change_skips_nothing(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            self._write(root, "crates/x/lib.rs", "original\n")
            base = self._init_repo(root)
            # Modify a tracked file without staging the change.
            self._write(root, "crates/x/lib.rs", "modified\n")
            r = self._run_list_changed(root, base=base)
        _, entries = self._parse(r.stdout)
        self.assertEqual(self._skipped_gates(entries), set())

    # ── 4 ──────────────────────────────────────────────────────────────

    def test_an_untracked_new_rust_file_skips_nothing(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            base = self._init_repo(root)
            self._write(root, "crates/x/new_untracked.rs")  # never `git add`ed
            r = self._run_list_changed(root, base=base)
        _, entries = self._parse(r.stdout)
        self.assertEqual(self._skipped_gates(entries), set())

    def test_a_non_ascii_rust_path_skips_nothing_committed_or_untracked(self) -> None:
        # git quotes such a path by default (`core.quotePath`); read without
        # `-z`, the quoted form matched no glob and both listings missed it.
        for committed in (True, False):
            with self.subTest(committed=committed), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                base = self._init_repo(root)
                self._write(root, "crates/x/\u00e9.rs")
                if committed:
                    self._commit_all(root, "non-ascii")
                r = self._run_list_changed(root, base=base)
                _, entries = self._parse(r.stdout)
                self.assertEqual(self._skipped_gates(entries), set())

    # ── 5 ──────────────────────────────────────────────────────────────

    def test_a_deleted_rust_file_skips_nothing(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            self._write(root, "crates/x/lib.rs")
            base = self._init_repo(root)
            _git(root, "rm", "-q", "crates/x/lib.rs")
            self._commit_all(root, "delete")
            r = self._run_list_changed(root, base=base)
        _, entries = self._parse(r.stdout)
        self.assertEqual(self._skipped_gates(entries), set())

    def test_a_rename_out_of_crates_skips_nothing(self) -> None:
        # The property `--no-renames` exists for: without it, `git diff`
        # would report only the new path (docs/lib.rs), which matches
        # nothing in the Rust scope, making the file leaving `crates/`
        # invisible to this check.
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            self._write(root, "crates/x/lib.rs")
            base = self._init_repo(root)
            (root / "docs").mkdir(exist_ok=True)
            _git(root, "mv", "crates/x/lib.rs", "docs/lib.rs")
            self._commit_all(root, "move out of crates")
            r = self._run_list_changed(root, base=base)
        _, entries = self._parse(r.stdout)
        self.assertEqual(self._skipped_gates(entries), set())

    # ── 6 ──────────────────────────────────────────────────────────────

    def test_cargo_lock_alone_skips_nothing(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            base = self._init_repo(root)
            self._write(root, "Cargo.lock")
            self._commit_all(root, "lock")
            r = self._run_list_changed(root, base=base)
        _, entries = self._parse(r.stdout)
        self.assertEqual(self._skipped_gates(entries), set())

    def test_cargo_config_toml_alone_skips_nothing(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            base = self._init_repo(root)
            self._write(root, ".cargo/config.toml")
            self._commit_all(root, "cargo config")
            r = self._run_list_changed(root, base=base)
        _, entries = self._parse(r.stdout)
        self.assertEqual(self._skipped_gates(entries), set())

    def test_ci_yml_alone_skips_nothing(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            base = self._init_repo(root)
            # Touch ci.yml itself -- still must count as a Rust-scope path.
            (root / ".github" / "workflows" / "ci.yml").write_text(
                _ci_yml_for_scope() + "\n# touched\n", encoding="utf-8"
            )
            self._commit_all(root, "ci.yml")
            r = self._run_list_changed(root, base=base)
        _, entries = self._parse(r.stdout)
        self.assertEqual(self._skipped_gates(entries), set())

    # ── 7 ──────────────────────────────────────────────────────────────

    def test_no_upstream_and_no_base_runs_the_complete_matrix(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            self._init_repo(root)
            self._write(root, "docs/guide.md")
            self._commit_all(root, "docs")
            r = self._run_list_changed(root, base=None)  # no --base, no upstream
        self.assertEqual(r.returncode, 0, r.stderr)
        scope, entries = self._parse(r.stdout)
        self.assertEqual(self._skipped_gates(entries), set())
        self.assertIn("could not determine the changed set", scope)

    # ── 8 ──────────────────────────────────────────────────────────────

    def test_an_unresolvable_base_is_a_usage_error(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            self._init_repo(root)
            r = self._run_list_changed(root, base="does-not-exist-anywhere")
        self.assertEqual(r.returncode, 2)

    # ── 9 ──────────────────────────────────────────────────────────────

    def test_a_ci_yml_scoped_set_mismatch_runs_the_complete_matrix(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            # ci.yml conditions only 10 of the 11 canonical lanes -- a
            # drift `rust_scope.SCOPED_LANES` does not know about.
            drifted = SCOPED_LANE_IDS[:-1]
            base = self._init_repo(root, scoped_lanes=drifted)
            self._write(root, "docs/guide.md")
            self._commit_all(root, "docs")
            r = self._run_list_changed(root, base=base)
        self.assertEqual(r.returncode, 0, r.stderr)
        scope, entries = self._parse(r.stdout)
        self.assertEqual(self._skipped_gates(entries), set())
        self.assertIn("scoped-lane sets disagree", scope)
        self.assertIn("G09b", scope)  # the dropped lane, named

    # ── 10 ─────────────────────────────────────────────────────────────

    def test_an_ignored_sqlite_file_under_crates_skips_the_scoped_lanes(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            base = self._init_repo(root)
            # Never committed, never added, and ignored -- CI never sees
            # it either (`git ls-files --others --exclude-standard`
            # excludes it the same way).
            self._write(root, "crates/x/dev.sqlite")
            r = self._run_list_changed(root, base=base)
        scope, entries = self._parse(r.stdout)
        self.assertEqual(
            self._skipped_gates(entries),
            set(SCOPED_LANE_IDS),
            "an ignored file under crates/ must not be seen as a Rust-scope change",
        )

    # ── §4's own output assertions ───────────────────────────────────────

    def test_changed_summary_counts_add_up_and_skipped_is_counted_apart(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            base = self._init_repo(root)
            self._write(root, "docs/guide.md")
            self._commit_all(root, "docs")
            result = subprocess.run(
                ["bash", str(SCRIPT), "--root", str(root), "--changed", "--base", base],
                capture_output=True,
                text=True,
            )
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        summary = next(
            line for line in result.stdout.splitlines() if line.startswith("local-matrix: ") and "check(s):" in line
        )
        m = re.match(
            r"local-matrix: (\d+) check\(s\): (\d+) run, (\d+) passed, (\d+) failed, (\d+) skipped",
            summary,
        )
        self.assertIsNotNone(m, summary)
        total, run, passed, failed, skipped = (int(x) for x in m.groups())
        self.assertEqual(total, run + skipped, summary)
        self.assertEqual(run, passed + failed, summary)
        self.assertEqual(skipped, len(SCOPED_LANE_IDS), summary)
        self.assertIn("not verified green", summary)

    def test_default_mode_summary_format_is_unchanged(self) -> None:
        # Deliberately not `_init_repo` (which builds the 12-lane chooser
        # fixture): the point of this test is the default, non-`--changed`
        # summary line's own format, so it reuses the minimal single-check
        # fixture `LocalMatrixExecution` already relies on, not a git
        # repository at all -- the default path never calls the chooser.
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            _write_gate_inputs(root)
            _write_ci_yml(root, '      - name: x\n        run: echo "command=true"\n')
            result = subprocess.run(
                ["bash", str(SCRIPT), "--root", str(root)],
                capture_output=True,
                text=True,
            )
        self.assertIn("local-matrix: 1 check(s) run, 1 passed, 0 failed.", result.stdout)
        self.assertNotIn("skipped", result.stdout)


class LocalMatrixExecution(unittest.TestCase):
    """End-to-end: the script actually runs what it derives, applies the
    [gates] command and [lane_profiles] tz for a gate id, and respects the
    inverted-polarity idiom (`-eq 0` present in a command's own step)."""

    def _run(self, root: Path) -> subprocess.CompletedProcess:
        return subprocess.run(
            ["bash", str(SCRIPT), "--root", str(root)],
            capture_output=True,
            text=True,
        )

    def test_gate_command_and_tz_are_read_from_gate_inputs_and_applied(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "contracts").mkdir()
            (root / "contracts" / "gate-inputs.toml").write_text(
                '[gates]\nZZZ = "echo \\"TZ=$TZ\\""\n\n'
                '[lane_profiles]\nZZZ = { tz = "Pacific/Kiritimati" }\n',
                encoding="utf-8",
            )
            _write_ci_yml(root, "      - name: dummy\n        run: bash scripts/ci-gate.sh ZZZ\n")
            result = self._run(root)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("TZ=Pacific/Kiritimati", result.stdout)

    def test_pass_fail_counting_and_polarity_inversion(self):
        # Four checks: a normal pass (`true`), a normal fail (`false`), an
        # inverted pass (a command that fails, under a step whose body
        # contains the `-eq 0` idiom), and an inverted fail (a command that
        # succeeds under that same idiom) -- one of each of the four
        # (polarity x outcome) combinations the script must tell apart.
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            _write_gate_inputs(root)
            body = (
                '      - name: normal pass\n        run: echo "command=true"\n'
                '      - name: normal fail\n        run: echo "command=false"\n'
                '      - name: inverted pass\n'
                '        run: |\n'
                '          echo "command=false"\n'
                '          if [[ "$status" -eq 0 ]]; then exit 1; fi\n'
                '      - name: inverted fail\n'
                '        run: |\n'
                '          echo "command=true"\n'
                '          if [[ "$status" -eq 0 ]]; then exit 1; fi\n'
            )
            _write_ci_yml(root, body)
            result = self._run(root)

        self.assertEqual(result.returncode, 1, "one normal and one inverted check must fail overall")
        self.assertIn("local-matrix: 4 check(s) run, 2 passed, 2 failed.", result.stdout)
        self.assertIn("  - false", result.stdout.splitlines(), "the normal `false` check must be listed as failed")
        self.assertIn("  - true", result.stdout.splitlines(), "the inverted `true` check must be listed as failed")

    def test_mutation_an_unconditional_pass_rule_would_hide_the_normal_failure(self):
        # Mutation evidence for the counting logic itself: if `run_one`
        # treated every exit status as a pass (the bug a matrix whose value
        # is completeness must not have), this fixture's "normal fail" check
        # would be missing from the failure list. Asserted here on the
        # actual, unmutated script, which must show it.
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            _write_gate_inputs(root)
            _write_ci_yml(root, '      - name: normal fail\n        run: echo "command=false"\n')
            result = self._run(root)
        self.assertEqual(result.returncode, 1)
        self.assertIn("result=FAIL", result.stdout)
        self.assertIn("local-matrix: 1 check(s) run, 0 passed, 1 failed.", result.stdout)

    def test_the_not_level_b_banner_is_in_the_output_not_only_the_docs(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            _write_gate_inputs(root)
            _write_ci_yml(root, '      - name: x\n        run: echo "command=true"\n')
            result = self._run(root)
        self.assertIn("NOT Level B", result.stdout)
        self.assertIn("+stable", result.stdout)


if __name__ == "__main__":
    unittest.main()
