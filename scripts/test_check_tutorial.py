#!/usr/bin/env python3
"""Regression tests for scripts/check_tutorial.py (spec 0003, §8 and EX-2).

They run real child and grandchild processes, real git repositories and
harmless fake `cargo` commands in temporary directories; nothing is built
and the repository is not changed. They read /proc to see whether a process
still runs, so they need Linux, plus bash, sh, sleep and git.
`tests/tutorial_checker.rs` runs them as part of `cargo test`.

Usage: python3 scripts/test_check_tutorial.py
Standard library only.
"""

import io
import os
import signal
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path
from unittest import mock

# No __pycache__ in the tree: the checker needs its copy to end clean.
sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
import check_tutorial  # noqa: E402

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


def write(path, text):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text)
    return path


class Scratch(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory(prefix="check-tutorial-test-")
        self.addCleanup(tmp.cleanup)
        self.tmp = Path(tmp.name)
        self.env = check_tutorial.clean_env()

    def use_root(self, root):
        patch = mock.patch.object(check_tutorial, "ROOT", root)
        patch.start()
        self.addCleanup(patch.stop)

    def repo(self, files):
        """A committed repository at self.tmp/repo, used as the checker's ROOT."""
        root = self.tmp / "repo"
        for rel, text in files.items():
            write(root / rel, text)
        git(root, "init", "-q", "-b", "main")
        git(root, "add", "-A")
        git(root, "commit", "-q", "-m", "base")
        self.use_root(root)
        environ = mock.patch.dict(os.environ)
        environ.start()
        self.addCleanup(environ.stop)
        os.environ.pop("CARGO_TARGET_DIR", None)
        return root

    def article(self, text, name="01-a.md"):
        path = write(self.tmp / "article" / name, text)
        return check_tutorial.blocks(path)


@unittest.skipUnless(Path("/proc/self/stat").is_file(), "needs Linux /proc")
class Processes(Scratch):
    def setUp(self):
        super().setUp()
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
        fake_bin = self.tmp / "bin"
        old = self.tmp / "target" / "debug" / "senu"
        for path, text in (
            (fake_bin / "cargo", "echo 'error: fake build failure' >&2\nexit 9\n"),
            (old, "touch old-binary-ran\necho 'senu: listening on http://127.0.0.1:4321'\nexec sleep 30\n"),
        ):
            write(path, "#!/bin/sh\n" + text).chmod(0o755)
        env = dict(self.env, PATH=f"{fake_bin}{os.pathsep}{self.env['PATH']}")
        script = "cargo build --quiet\n./target/debug/senu site\n"
        with self.assertRaisesRegex(CheckFailed, "did not start a server(.|\n)*fake build failure"):
            check_tutorial.start_server(script, self.tmp, env, timeout=30)
        self.assertFalse((self.tmp / "old-binary-ran").exists(), "the old binary was started")

    def test_a_server_gets_ctrl_c_first_and_its_address_is_used(self):
        script = (
            "trap 'echo interrupted > got-int; exit 0' INT\n"
            "echo $$ > server.pid\n"
            "echo \"senu: listening on http://$SENU_BIND\" | sed 's/:0$/:4321/'\n"
            "while :; do sleep 0.1; done\n"
        )
        process, address, output, log = check_tutorial.start_server(script, self.tmp, self.env, 30)
        self.addCleanup(log.close)
        self.assertEqual(address, "127.0.0.1:4321")
        self.assertEqual(output, "senu: listening on http://127.0.0.1:4321\n")
        check_tutorial.stop(process, first=signal.SIGINT)
        self.assertEqual((self.tmp / "got-int").read_text(), "interrupted\n")
        self.assertTrue(ends_soon(self.pid("server.pid")))
        self.assertTrue(running(self.bystander.pid))


class Blocks(Scratch):
    def test_info_strings_name_the_block_kinds(self):
        found = self.article(
            "# A\n\nRun:\n\n```sh check=one exit=2\nfalse\n```\n\n```text check=one\nx\n```\n\n"
            "```sh check=web server\ncargo run\n```\n\nIn `src/x.rs` write:\n\n"
            "```rust file=src/x.rs\nfn f() {}\n```\n\n```diff patch\n-a\n+b\n```\n\n"
            "```sh manual\ngit clone x\n```\n\n```text\nshown only\n```\n"
        )
        self.assertEqual(
            [b.kind for b in found],
            ["command", "expected", "server", "file", "patch", "manual", "shown"],
        )
        self.assertEqual(found[0].attrs, {"check": "one", "exit": "2"})
        self.assertEqual(found[3].body, "fn f() {}\n")
        self.assertEqual(found[3].paragraph, "In `src/x.rs` write:")

    def test_an_sh_block_without_a_check_is_refused(self):
        (block,) = self.article("# A\n\n```sh\nls\n```\n")
        with self.assertRaisesRegex(CheckFailed, "01-a.md:3: an sh block needs check="):
            block.kind

    def test_file_paths_stay_under_content_or_src_and_are_named_before(self):
        for path in ("../x.md", "/etc/passwd", "README.md", "content/../src/x.rs", "target/x"):
            (block,) = self.article(f"Edit `{path}`:\n\n```md file={path}\nx\n```\n")
            with self.assertRaisesRegex(CheckFailed, "must stay under content/ or src/"):
                check_tutorial.check_file_path(block)
        (block,) = self.article("Edit the note:\n\n```md file=content/en/51-a.md\nx\n```\n")
        with self.assertRaisesRegex(CheckFailed, "must name `content/en/51-a.md`"):
            check_tutorial.check_file_path(block)
        (block,) = self.article("Edit `content/en/51-a.md`, then:\n\n```md file=content/en/51-a.md\nx\n```\n")
        self.assertEqual(str(check_tutorial.check_file_path(block)), "content/en/51-a.md")

    def test_an_unclosed_block_is_refused(self):
        with self.assertRaisesRegex(CheckFailed, "never closed"):
            self.article("# A\n\n```sh check=x\nls\n")

    def test_normalization_covers_colours_addresses_ports_durations_and_thread_ids(self):
        raw = ("\x1b[1m\x1b[91merror\x1b[0m: x\nhttp://127.0.0.1:4321/ and 127.0.0.1:5555\n"
               "thread 'a::b' (98765) panicked at src/a.rs:1:2:\nfinished in 0.37s\ndeps/senu-0a1b2c\n")
        self.assertEqual(
            check_tutorial.normalize(raw, "127.0.0.1:4321"),
            "error: x\nhttp://127.0.0.1:3000/ and 127.0.0.1:PORT\n"
            "thread 'a::b' (ID) panicked at src/a.rs:1:2:\nfinished in 0.00s\ndeps/senu-HASH\n",
        )


class Copies(Scratch):
    def setUp(self):
        super().setUp()
        self.root = self.repo({".gitignore": "target/\n", "README.md": "committed\n",
                               "tests/adapter_x.rs": "", "content/en/01-a.md": "# A\n"})
        write(self.tmp / "outside.txt", "outside sentinel\n")
        write(self.root / "untracked.txt", "untracked sentinel\n")
        write(self.root / "content/en/51-untracked-note.md", "# Untracked\n")
        (self.root / "outside-link.txt").symlink_to(self.tmp / "outside.txt")
        debug = self.root / "target" / "debug"
        for rel in ("deps/libdep-1.rlib", "deps/senu-0a1b", "deps/libsenu-0a1b.rlib",
                    "deps/adapter_x-0a1b", ".fingerprint/dep-1/x", ".fingerprint/senu-0a1b/x",
                    "build/senu-0a1b/out/content.rs", "build/dep-2/out/x", "senu"):
            write(debug / rel, "stale\n")
        self.work = self.tmp / "work"

    def copied(self):
        return sorted(
            str(p.relative_to(self.work)) for p in self.work.rglob("*")
            if ".git" not in p.relative_to(self.work).parts and not p.is_dir()
        )

    def test_only_the_commit_and_dependency_artifacts_are_copied(self):
        before = check_tutorial.snapshot()
        with mock.patch("sys.stdout", new_callable=io.StringIO):
            check_tutorial.copy_tree(self.work)
        self.assertEqual(self.copied(), [
            ".gitignore", "README.md", "content/en/01-a.md",
            "target/debug/.fingerprint/dep-1/x", "target/debug/build/dep-2/out/x",
            "target/debug/deps/libdep-1.rlib", "tests/adapter_x.rs",
        ])
        self.assertEqual(check_tutorial.snapshot(), before)
        self.assertTrue((self.root / "outside-link.txt").is_symlink())

    def test_tracked_changes_are_refused(self):
        write(self.root / "README.md", "edited\n")
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


class Languages(Scratch):
    """check_language on a small repository whose commands need no Cargo."""

    ARTICLE = (
        "# A\n\nCreate `content/{locale}/51-b.md`:\n\n```md file=content/{locale}/51-b.md\n# B\n```\n\n"
        "```sh check=list\nls content/{locale}\n```\n\n```text check=list\n{expected}```\n\n"
        "```sh check=fails exit=101\nprintf '\\033[1m\\033[91merror\\033[0m: test failed, to rerun pass `--lib`\\n'\nexit 101\n```\n\n"
        "```text check=fails\nerror: test failed, to rerun pass `--lib`\n```\n"
    )

    def setUp(self):
        super().setUp()
        files = {"src/reading.rs": "fn f() {}\n"}
        for locale in check_tutorial.LOCALES:
            files[f"content/{locale}/01-a.md"] = self.ARTICLE.format(
                locale=locale, expected="01-a.md\n51-b.md\n")
        self.root = self.repo(files)

    def check(self, locale):
        with mock.patch("sys.stdout", new_callable=io.StringIO) as printed:
            check_tutorial.check_language(locale)
        return printed.getvalue()

    def test_each_language_runs_in_its_own_copy_and_leaves_the_tree_alone(self):
        before = check_tutorial.snapshot()
        for locale in check_tutorial.LOCALES:
            printed = self.check(locale)
            self.assertIn(f"ok: content/{locale}/01-a.md:13: list output matches", printed)
            self.assertIn(f"ok: {locale}: the original working tree is unchanged", printed)
        self.assertFalse((self.root / "content/en/51-b.md").exists())
        self.assertEqual(check_tutorial.snapshot(), before)

    def test_a_different_output_fails_with_a_diff(self):
        write(self.root / "content/en/01-a.md",
              self.ARTICLE.format(locale="en", expected="01-a.md\n"))
        git(self.root, "commit", "-q", "-am", "wrong expectation")
        with self.assertRaisesRegex(CheckFailed, "list output differs(.|\n)*\\+51-b.md"):
            self.check("en")

    def test_a_coloured_compiler_error_is_not_an_expected_failure(self):
        compiler = r"\033[0m\033[1m\033[38;5;9merror[E0425]\033[0m: cannot find value"
        text = self.ARTICLE.format(locale="en", expected="01-a.md\n51-b.md\n").replace(
            "printf '", f"printf '{compiler}\\n")
        write(self.root / "content/en/01-a.md", text)
        git(self.root, "commit", "-q", "-am", "compiler error")
        with self.assertRaisesRegex(CheckFailed, "other than the expected one"):
            self.check("en")

    def commit_article(self, text):
        write(self.root / "content/en/01-a.md", "# A\n\n" + text)
        git(self.root, "commit", "-q", "-am", "article")

    def test_output_without_its_text_block_fails_before_the_next_block(self):
        one = "```sh check=one\nprintf 'a\\n'\n```\n\n"
        for following in (
            "```sh check=two\nprintf 'b\\n'\n```\n\n```text check=two\nb\n```\n",
            "Create `content/en/51-b.md`:\n\n```md file=content/en/51-b.md\n# B\n```\n",
        ):
            self.commit_article(one + following)
            with self.assertRaisesRegex(
                    CheckFailed, r"^content/en/01-a.md:3: one printed output that no `text check=one`"):
                self.check("en")

    def test_output_without_its_text_block_fails_at_the_end(self):
        self.commit_article("```sh check=one\nprintf 'a\\n'\n```\n")
        with self.assertRaisesRegex(
                CheckFailed, r"^content/en/01-a.md:3: one printed output that no `text check=one`"):
            self.check("en")

    def test_complete_pairs_and_silent_commands_pass(self):
        self.commit_article(
            "```sh check=quiet\ntrue\n```\n\n"
            "```sh check=one\nprintf 'a\\n'\n```\n\n```text check=one\na\n```\n\n"
            "```sh check=last-quiet\ntrue\n```\n")
        printed = self.check("en")
        self.assertIn("ok: content/en/01-a.md:11: one output matches", printed)
        self.assertIn("ok: content/en/01-a.md:15: last-quiet (exit 0)", printed)

    def test_a_changed_reading_rs_fails(self):
        text = self.ARTICLE.format(locale="en", expected="01-a.md\n51-b.md\n")
        text += "\nEdit `src/reading.rs`:\n\n```rust file=src/reading.rs\nfn g() {}\n```\n"
        write(self.root / "content/en/01-a.md", text)
        git(self.root, "commit", "-q", "-am", "edit reading.rs")
        with self.assertRaisesRegex(CheckFailed, "src/reading.rs differs from HEAD"):
            self.check("en")


if __name__ == "__main__":
    unittest.main()
