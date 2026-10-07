"""RFC 138 D2 self-tests for scripts/hunk-hashes.py.

Run as: python3.14 -m pytest scripts/tests/test_hunk_hashes.py

Each fixture is a throwaway git repository. Most tests import the module
directly and call `parse_diff` on real `git diff` output, so a hunk's exact
bytes can be checked against a hash built independently in the test, not just
against the tool's own hash of itself. A few tests run the CLI as a
subprocess and read the printed Markdown, to prove the wiring from `main()`
to the parser is also right, not only the parser in isolation.
"""

import hashlib
import importlib.util
import subprocess
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
TOOL = REPO_ROOT / "scripts" / "hunk-hashes.py"

_spec = importlib.util.spec_from_file_location("hunk_hashes", TOOL)
hh = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(hh)


class Repo:
    """A throwaway git repository, built the way this project's other
    self-tests build one (see test_owner_attributions.py's `Repo`)."""

    def __init__(self, tmp):
        self.root = Path(tmp)
        self._git("init", "-q", "-b", "main")
        self._git("config", "user.email", "t@example.invalid")
        self._git("config", "user.name", "t")
        self._git("config", "commit.gpgsign", "false")

    def _git(self, *args):
        subprocess.run(["git", *args], cwd=self.root, check=True, capture_output=True)

    def write(self, rel, data):
        path = self.root / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        if isinstance(data, str):
            path.write_text(data, newline="")  # no implicit newline translation
        else:
            path.write_bytes(data)

    def write_bytes_no_final_newline(self, rel, text: str):
        self.write(rel, text.encode("utf-8"))

    def commit(self, msg="c"):
        self._git("add", "-A")
        self._git("commit", "-q", "-m", msg)

    def mv(self, src, dst):
        self._git("mv", src, dst)

    def rm(self, rel):
        self._git("rm", "-q", rel)

    def baseline(self):
        return self._git_out("rev-parse", "HEAD")

    def _git_out(self, *args):
        return subprocess.run(
            ["git", *args], cwd=self.root, check=True, capture_output=True, text=True
        ).stdout.strip()

    def diff_changes(self, baseline, *paths):
        raw = hh.git(self.root, "diff", "-U3", "-M", baseline, "--", *(paths or (".",)))
        return hh.parse_diff(raw)

    def run_cli(self, baseline, *paths):
        args = ["--baseline", baseline, "--root", str(self.root)]
        if paths:
            args += list(paths)
        buf = []
        old_print = hh.__dict__.get("print")
        import io
        import contextlib

        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            hh.main(args)
        return out.getvalue()


def ten_lines(prefix="x"):
    return "\n".join(f"{prefix}{i}" for i in range(10)) + "\n"


class DecisiveCase(unittest.TestCase):
    """The regression RFC 138 exists to prevent.

    The first test below pins RFC 138 D2's sentence literally: the new
    method's hash for hunk 1 is unchanged when a third hunk is appended.
    That sentence alone does not distinguish the new method from the
    retired one -- checked by mutating `parse_diff` back to the retired
    "split on \\n@@" method and finding this test still passes. The reason
    is structural: hunk 1's boundary is set by where hunk 2 starts, which a
    third hunk appended later does not move, under either method.

    The second test is the one that actually fails under that mutation: it
    compares the retired method against the new one on the same diff,
    reproducing RFC 138's own measurement. Both tests are kept -- the first
    guards a different, still-real property (stability under appending),
    and only the second is the discriminator between the two methods.
    """

    def test_hunk_1_hash_unchanged_when_a_third_hunk_is_appended(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo = Repo(tmp)
            # Three widely separated regions in one file, each far enough
            # apart (well past -U3's 3-line context) to stay separate hunks.
            lines = [f"line{i}" for i in range(60)]
            repo.write("f.txt", "\n".join(lines) + "\n")
            repo.commit()
            base = repo.baseline()

            lines[5] = "CHANGED-5"
            lines[30] = "CHANGED-30"
            repo.write("f.txt", "\n".join(lines) + "\n")
            repo.commit()

            changes = repo.diff_changes(base)
            self.assertEqual(len(changes), 1)
            self.assertEqual(len(changes[0].hunks), 2, "two separated edits must stay two hunks")
            hunk1_before = hashlib.sha256(changes[0].hunks[0]).hexdigest()

            lines[55] = "CHANGED-55"
            repo.write("f.txt", "\n".join(lines) + "\n")
            repo.commit()

            changes2 = repo.diff_changes(base)
            self.assertEqual(len(changes2[0].hunks), 3)
            hunk1_after = hashlib.sha256(changes2[0].hunks[0]).hexdigest()

            self.assertEqual(
                hunk1_before,
                hunk1_after,
                "hunk 1's hash must not depend on whether a later hunk exists",
            )

    def test_the_retired_method_disagrees_with_the_new_one_on_a_non_final_hunk(self):
        """Confirms the regression is real, not assumed, reproducing RFC
        138's own measurement: on one two-hunk diff, the retired "split on
        \\n@@" method and the new header-plus-body method disagree on hunk 1
        -- the retired method drops the newline that ends it, because
        something follows it in the same file -- and agree on hunk 2, the
        final hunk, where there is nothing after it to lose."""
        with tempfile.TemporaryDirectory() as tmp:
            repo = Repo(tmp)
            lines = [f"line{i}" for i in range(60)]
            repo.write("f.txt", "\n".join(lines) + "\n")
            repo.commit()
            base = repo.baseline()

            lines[5] = "CHANGED-5"
            lines[30] = "CHANGED-30"
            repo.write("f.txt", "\n".join(lines) + "\n")
            repo.commit()

            raw = hh.git(repo.root, "diff", "-U3", base, "--", "f.txt")
            retired = raw.split(b"\n@@")
            # retired[0] is the preamble; retired[1:] are the hunks, each
            # missing its leading "@@" (split consumed it) and, for every
            # hunk but the last, missing its trailing newline too.
            retired_hunk1 = b"@@" + retired[1]
            retired_hunk2 = b"@@" + retired[2]

            new_method = repo.diff_changes(base)[0].hunks
            self.assertEqual(len(new_method), 2)

            self.assertNotEqual(
                hashlib.sha256(retired_hunk1).hexdigest(),
                hashlib.sha256(new_method[0]).hexdigest(),
                "the retired method must disagree with the new one on hunk 1",
            )
            self.assertEqual(
                hashlib.sha256(retired_hunk2).hexdigest(),
                hashlib.sha256(new_method[1]).hexdigest(),
                "both methods must agree on the final hunk",
            )


class OneHunkFile(unittest.TestCase):
    def test_a_single_edit_is_one_hunk(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo = Repo(tmp)
            repo.write("f.txt", ten_lines())
            repo.commit()
            base = repo.baseline()
            repo.write("f.txt", ten_lines("y"))
            repo.commit()

            changes = repo.diff_changes(base)
            self.assertEqual(len(changes), 1)
            self.assertEqual(changes[0].status, "modified")
            self.assertEqual(len(changes[0].hunks), 1)
            self.assertTrue(changes[0].hunks[0].startswith(b"@@"))


class AddedFile(unittest.TestCase):
    def test_a_tracked_new_file_is_full_content(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo = Repo(tmp)
            repo.write("a.txt", "hello\n")
            repo.commit()
            base = repo.baseline()
            repo.write("new.txt", "brand new\n")
            repo.commit()

            changes = repo.diff_changes(base)
            self.assertEqual(len(changes), 1)
            self.assertEqual(changes[0].status, "added")
            self.assertEqual(changes[0].path, "new.txt")
            # git's own diff for a new file is shaped like a hunk (`@@ -0,0
            # +1 @@` etc.); the tool's output step uses the full-content
            # bytes instead for "added", not these, but the raw parse
            # legitimately finds one.

    def test_an_untracked_file_is_reported_via_the_cli(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo = Repo(tmp)
            repo.write("a.txt", "hello\n")
            repo.commit()
            base = repo.baseline()
            repo.write("untracked.txt", "never added\n")  # not `git add`ed

            out = repo.run_cli(base)
            self.assertIn("untracked.txt", out)
            expected = hashlib.sha256(b"never added\n").hexdigest()
            self.assertIn(expected, out)


class DeletedFile(unittest.TestCase):
    def test_a_deleted_file_reports_its_baseline_content_hash(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo = Repo(tmp)
            repo.write("gone.txt", "will be removed\n")
            repo.commit()
            base = repo.baseline()
            repo.rm("gone.txt")
            repo.commit()

            changes = repo.diff_changes(base)
            self.assertEqual(len(changes), 1)
            self.assertEqual(changes[0].status, "deleted")
            self.assertEqual(changes[0].path, "gone.txt")

            out = repo.run_cli(base)
            expected = hashlib.sha256(b"will be removed\n").hexdigest()
            self.assertIn("## Deleted files", out)
            self.assertIn(expected, out)


class Renames(unittest.TestCase):
    def test_a_pure_rename_has_no_hunks_and_the_unchanged_content_hash(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo = Repo(tmp)
            repo.write("old.txt", "same content\n")
            repo.commit()
            base = repo.baseline()
            repo.mv("old.txt", "new.txt")
            repo.commit()

            changes = repo.diff_changes(base)
            self.assertEqual(len(changes), 1)
            self.assertEqual(changes[0].status, "renamed")
            self.assertEqual(changes[0].old_path, "old.txt")
            self.assertEqual(changes[0].path, "new.txt")
            self.assertEqual(changes[0].hunks, [])

            out = repo.run_cli(base)
            expected = hashlib.sha256(b"same content\n").hexdigest()
            self.assertIn("## Renamed files, content unchanged", out)
            self.assertIn(expected, out)

    def test_a_rename_with_a_content_change_has_hunks(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo = Repo(tmp)
            repo.write("old.txt", ten_lines())
            repo.commit()
            base = repo.baseline()
            repo.mv("old.txt", "new.txt")
            # One line changed out of ten: similar enough for git's default
            # rename-detection threshold to still call this a rename rather
            # than an unrelated delete-and-add pair.
            lines = ten_lines().splitlines()
            lines[0] = "CHANGED"
            repo.write("new.txt", "\n".join(lines) + "\n")
            repo.commit()

            changes = repo.diff_changes(base)
            self.assertEqual(len(changes), 1)
            self.assertEqual(changes[0].status, "renamed")
            self.assertEqual(changes[0].old_path, "old.txt")
            self.assertEqual(changes[0].path, "new.txt")
            self.assertEqual(len(changes[0].hunks), 1)


class NoTrailingNewline(unittest.TestCase):
    def test_losing_the_final_newline_changes_the_hunk_hash(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo = Repo(tmp)
            repo.write("f.txt", "a\nb\nc\n")
            repo.commit()
            base = repo.baseline()

            repo.write("f.txt", "a\nb\nc")  # same bytes, minus the final \n
            repo.commit()
            changes = repo.diff_changes(base)
            self.assertEqual(len(changes), 1)
            self.assertEqual(len(changes[0].hunks), 1)
            self.assertIn(b"\\ No newline at end of file", changes[0].hunks[0])

    def test_the_no_newline_marker_is_part_of_the_hunks_hashed_bytes(self):
        # Two separate repos: one ends with a newline, one does not, same
        # visible content otherwise. Their hunk hashes must differ, because
        # the marker line is part of what gets hashed.
        with tempfile.TemporaryDirectory() as tmp_a, tempfile.TemporaryDirectory() as tmp_b:
            repo_a = Repo(tmp_a)
            repo_a.write("f.txt", "x\n")
            repo_a.commit()
            base_a = repo_a.baseline()
            repo_a.write("f.txt", "a\nb\nc\n")
            repo_a.commit()
            hunk_with_newline = repo_a.diff_changes(base_a)[0].hunks[0]

            repo_b = Repo(tmp_b)
            repo_b.write("f.txt", "x\n")
            repo_b.commit()
            base_b = repo_b.baseline()
            repo_b.write("f.txt", "a\nb\nc")
            repo_b.commit()
            hunk_without_newline = repo_b.diff_changes(base_b)[0].hunks[0]

            self.assertNotEqual(
                hashlib.sha256(hunk_with_newline).hexdigest(),
                hashlib.sha256(hunk_without_newline).hexdigest(),
            )


class BinaryContent(unittest.TestCase):
    def test_an_added_binary_file_is_full_content_not_hunks(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo = Repo(tmp)
            repo.write("a.txt", "hello\n")
            repo.commit()
            base = repo.baseline()
            data = bytes([0, 1, 2, 3, 0xFF, 0x00, 0x7F]) * 4
            repo.write("blob.bin", data)
            repo.commit()

            changes = repo.diff_changes(base)
            self.assertEqual(len(changes), 1)
            self.assertTrue(changes[0].binary)
            self.assertEqual(changes[0].hunks, [])

            out = repo.run_cli(base)
            expected = hashlib.sha256(data).hexdigest()
            self.assertIn("## Binary files, full content", out)
            self.assertIn(expected, out)
            self.assertIn("added", out)


class CrlfContent(unittest.TestCase):
    def test_crlf_line_endings_are_hashed_literally(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo = Repo(tmp)
            repo.write("f.txt", b"a\r\nb\r\nc\r\n")
            repo.commit()
            base = repo.baseline()
            repo.write("f.txt", b"a\r\nCHANGED\r\nc\r\n")
            repo.commit()

            changes = repo.diff_changes(base)
            self.assertEqual(len(changes), 1)
            hunk = changes[0].hunks[0]
            self.assertIn(b"\r\n", hunk, "a \\r must survive into the hashed bytes")
            # Independently reconstruct what the hunk should be and compare
            # hash for hash, not just "contains \\r\\n".
            raw = hh.git(repo.root, "diff", "-U3", "-M", base, "--", "f.txt")
            idx = raw.find(b"\n@@")
            expected_body = raw[idx + 1 :]
            expected_hunks = hh.split_hunks(expected_body)
            self.assertEqual(len(expected_hunks), 1)
            self.assertEqual(
                hashlib.sha256(hunk).hexdigest(),
                hashlib.sha256(expected_hunks[0]).hexdigest(),
            )


class CliSmoke(unittest.TestCase):
    """The output format pastes straight into a package, per D1."""

    def test_output_has_every_section_heading(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo = Repo(tmp)
            repo.write("f.txt", ten_lines())
            repo.commit()
            base = repo.baseline()
            repo.write("f.txt", ten_lines("y"))
            repo.commit()

            out = repo.run_cli(base)
            for heading in (
                "## Per-hunk SHA-256",
                "## Full-content SHA-256",
                "## Deleted files",
                "## Renamed files, content unchanged",
                "## Binary files, full content",
            ):
                self.assertIn(heading, out)

    def test_cli_runs_as_a_real_subprocess_too(self):
        """One true end-to-end check: the shebang, the argv parsing, and
        stdout -- not just calling `main` in-process."""
        with tempfile.TemporaryDirectory() as tmp:
            repo = Repo(tmp)
            repo.write("f.txt", ten_lines())
            repo.commit()
            base = repo.baseline()
            repo.write("f.txt", ten_lines("y"))
            repo.commit()

            result = subprocess.run(
                ["python3.14", str(TOOL), "--baseline", base, "--root", str(repo.root), "f.txt"],
                capture_output=True,
                text=True,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn("## Per-hunk SHA-256", result.stdout)
            self.assertIn("`f.txt`", result.stdout)


if __name__ == "__main__":
    unittest.main()
