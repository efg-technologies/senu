#!/usr/bin/env python3
"""Regression tests for scripts/check_tutorial.py.

They run real child and grandchild processes, real git repositories and
harmless fake `cargo` and `senu` commands in temporary directories; nothing
is built and the repository is not changed. They read /proc to see whether a
process still runs, so they need Linux, plus bash, sh, sleep and git.
`tests/tutorial_checker.rs` runs them as part of `cargo test`.

Usage: python3 scripts/test_check_tutorial.py
Standard library only.
"""

import io
import os
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path
from unittest import mock

# No __pycache__ in the tree: the exercise check needs its copy to end clean.
sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
import check_tutorial  # noqa: E402

REAL_ROOT = check_tutorial.ROOT
CheckFailed = check_tutorial.CheckFailed


def running(pid):
    """Whether `pid` is a live process; a zombie counts as ended."""
    try:
        stat = Path(f"/proc/{pid}/stat").read_text()
    except (FileNotFoundError, ProcessLookupError):  # gone, or going while read
        return False
    return stat.rsplit(")", 1)[1].split()[0] != "Z"


def ends_soon(pid, seconds=5):
    deadline = time.monotonic() + seconds
    while running(pid):
        if time.monotonic() > deadline:
            return False
        time.sleep(0.05)
    return True


def git(cwd, *args):
    subprocess.run(
        ["git", "-c", "user.name=t", "-c", "user.email=t@example.invalid",
         "-c", "commit.gpgsign=false", "-c", "core.hooksPath=/dev/null", *args],
        cwd=cwd, env=check_tutorial.clean_env(), check=True, capture_output=True,
    )


class Scratch(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory(prefix="check-tutorial-test-")
        self.addCleanup(tmp.cleanup)
        self.tmp = Path(tmp.name)

    def use_root(self, root):
        patch = mock.patch.object(check_tutorial, "ROOT", root)
        patch.start()
        self.addCleanup(patch.stop)


@unittest.skipUnless(Path("/proc/self/stat").is_file(), "needs Linux /proc")
class Processes(Scratch):
    def setUp(self):
        super().setUp()
        self.env = check_tutorial.clean_env()
        # Not started by the checker, so the checker must never signal it.
        self.bystander = subprocess.Popen(["sleep", "60"], start_new_session=True)
        self.addCleanup(self.bystander.wait)
        self.addCleanup(self.bystander.kill)

    def pid(self, name):
        path = self.tmp / name
        deadline = time.monotonic() + 5
        while not path.is_file() or not path.read_text().strip():
            self.assertLess(time.monotonic(), deadline, f"{name} was never written")
            time.sleep(0.05)
        pid = int(path.read_text())
        # Only matters while the checker leaks it; never outlives this test.
        self.addCleanup(lambda: running(pid) and os.kill(pid, signal.SIGKILL))
        return pid

    def test_a_timeout_ends_the_command_and_its_grandchild(self):
        script = (
            "echo $$ > child.pid\n"
            "sh -c 'sleep 120 & echo $! > grandchild.pid; wait'\n"
            "echo unreachable\n"
        )
        with self.assertRaisesRegex(CheckFailed, "no result within 1 s"):
            check_tutorial.run(script, self.tmp, self.env, 1)
        for name in ("child.pid", "grandchild.pid"):
            self.assertTrue(ends_soon(self.pid(name)), f"{name} still runs")
        self.assertTrue(running(self.bystander.pid))

    def test_a_finished_command_leaves_no_background_process(self):
        script = "sleep 120 >/dev/null 2>&1 & echo $! > grandchild.pid\necho done\nexit 3\n"
        self.assertEqual(check_tutorial.run(script, self.tmp, self.env, 30), (3, "done\n"))
        self.assertTrue(ends_soon(self.pid("grandchild.pid")), "grandchild still runs")
        self.assertTrue(running(self.bystander.pid))

    def test_a_failed_build_never_starts_the_old_binary(self):
        root = self.tmp / "repo"
        serve = root / check_tutorial.COMMANDS / "serve.sh"
        serve.parent.mkdir(parents=True)
        shutil.copy(REAL_ROOT / check_tutorial.COMMANDS / "serve.sh", serve)
        fake_bin = self.tmp / "bin"
        fake_bin.mkdir()
        old = root / "target" / "debug"
        old.mkdir(parents=True)
        for path, text in (
            (fake_bin / "cargo", "echo 'error: fake build failure' >&2\nexit 9\n"),
            (old / "senu", "touch old-binary-ran\n"
             "echo 'senu: listening on http://127.0.0.1:4321'\nexec sleep 30\n"),
        ):
            path.write_text("#!/bin/sh\n" + text)
            path.chmod(0o755)
        self.use_root(root)
        env = dict(self.env, PATH=f"{fake_bin}{os.pathsep}{self.env['PATH']}")
        with self.assertRaisesRegex(CheckFailed, "did not start a server(.|\n)*fake build failure"):
            check_tutorial.start_server(env)
        self.assertFalse((root / "old-binary-ran").exists(), "the old binary was started")


class Snapshot(Scratch):
    def setUp(self):
        super().setUp()
        self.root = self.tmp / "repo"
        self.root.mkdir()
        (self.root / ".gitignore").write_text("target/\n")
        (self.root / "README.md").write_text("committed\n")
        git(self.root, "init", "-q", "-b", "main")
        git(self.root, "add", "-A")
        git(self.root, "commit", "-q", "-m", "base")
        (self.tmp / "outside.txt").write_text("outside sentinel\n")
        (self.root / "untracked.txt").write_text("untracked sentinel\n")
        (self.root / "outside-link.txt").symlink_to(self.tmp / "outside.txt")
        self.use_root(self.root)
        environ = mock.patch.dict(os.environ)
        environ.start()
        self.addCleanup(environ.stop)
        os.environ.pop("CARGO_TARGET_DIR", None)
        self.work = self.tmp / "work"

    def copied(self):
        """Every file in the copy outside .git, and what git tracks there."""
        files = sorted(
            str(p.relative_to(self.work)) for p in self.work.rglob("*")
            if ".git" not in p.relative_to(self.work).parts and not p.is_dir()
        )
        listed = subprocess.run(
            ["git", "ls-files"], cwd=self.work, env=check_tutorial.clean_env(),
            check=True, capture_output=True, text=True,
        ).stdout.split()
        return files, listed

    def test_only_the_committed_tree_is_copied(self):
        before = check_tutorial.snapshot()
        check_tutorial.copy_tree(self.work)
        self.assertEqual(self.copied(), ([".gitignore", "README.md"], [".gitignore", "README.md"]))
        self.assertEqual(check_tutorial.snapshot(), before)
        self.assertEqual((self.root / "untracked.txt").read_text(), "untracked sentinel\n")
        self.assertTrue((self.root / "outside-link.txt").is_symlink())

    def test_tracked_changes_are_refused(self):
        (self.root / "README.md").write_text("edited\n")
        with self.assertRaisesRegex(CheckFailed, "commit or stash(.|\n)*README.md"):
            check_tutorial.copy_tree(self.work)
        self.assertFalse(self.work.exists())
        self.assertEqual((self.root / "README.md").read_text(), "edited\n")

    def test_a_committed_link_out_of_the_tree_is_refused(self):
        (self.root / "out.txt").symlink_to("../outside.txt")
        git(self.root, "add", "out.txt")
        git(self.root, "commit", "-q", "-m", "link out")
        with self.assertRaisesRegex(CheckFailed, "out.txt"):
            check_tutorial.copy_tree(self.work)
        for path in self.work.rglob("*") if self.work.exists() else ():
            if path.is_file() and not path.is_symlink():
                self.assertNotIn(b"outside sentinel", path.read_bytes(), path)


class ColouredExercise(Scratch):
    """CI sets CARGO_TERM_COLOR=always, so Cargo colours its diagnostics."""

    TEST_FAILED = r"\033[1m\033[91merror\033[0m: test failed, to rerun pass `--test adapter_http`"

    def setUp(self):
        super().setUp()
        self.root = self.tmp / "repo"
        self.root.mkdir()
        git(self.root, "init", "-q", "-b", "main")
        self.use_root(self.root)
        environ = mock.patch.dict(os.environ)
        environ.start()
        self.addCleanup(environ.stop)
        os.environ.pop("CARGO_TARGET_DIR", None)

    def exercise(self, output):
        steps = self.root / check_tutorial.STEPS_FILE
        steps.parent.mkdir(parents=True)
        steps.write_text(f"# ANCHOR: suite-red\nprintf '{output}\\n' >&2\nexit 101\n"
                         "# ANCHOR_END: suite-red\n")
        steps.with_suffix(".out").write_text(
            "# ANCHOR: suite-red\nerror: test failed, to rerun pass `--test adapter_http`\n"
            "# ANCHOR_END: suite-red\n")
        git(self.root, "add", "-A", "docs")
        git(self.root, "commit", "-q", "-m", "steps")
        with mock.patch.object(check_tutorial, "STEPS", {"suite-red": 101}), \
                mock.patch("sys.stdout", new_callable=io.StringIO) as printed:
            check_tutorial.check_exercise()
        return printed.getvalue()

    def test_a_coloured_test_failure_matches_the_plain_expected_line(self):
        printed = self.exercise(self.TEST_FAILED)
        self.assertIn("ok: steps.sh:suite-red (exit 101)\n"
                      "    error: test failed, to rerun pass `--test adapter_http`\n", printed)

    def test_a_coloured_compiler_error_is_not_a_test_failure(self):
        compiler = r"\033[0m\033[1m\033[38;5;9merror[E0425]\033[0m\033[0m\033[1m: cannot find value"
        with self.assertRaisesRegex(CheckFailed, "for a reason other than a test assertion"):
            self.exercise(f"{compiler}\\n{self.TEST_FAILED}")


if __name__ == "__main__":
    unittest.main()
