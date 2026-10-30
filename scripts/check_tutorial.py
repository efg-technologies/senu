#!/usr/bin/env python3
"""Runs the tutorial articles' commands and checks their output (spec 0003, §8).

Usage: check_tutorial.py [--all-mismatches] [en|ja ...]

With no language, English runs first and then Japanese. Each language runs
in its own fresh copy of the last commit (HEAD), so the learner's notes and
edits of one language never affect the other. The checker refuses to run
while tracked files differ from HEAD, and the real working tree must be
unchanged at the end.

The fenced code blocks of content/<locale>/*.md, in file-name order, are
the script. Their info strings, which the site does not show, say what to do:

  sh check=<name> [exit=<n>] [server]   a command; it must exit with <n> (0)
  text check=<name>                     the exact output of that command,
                                        stdout and stderr merged
  <lang> file=<path>                    the full new content of <path>, which
                                        the paragraph before names in backticks
  diff patch                            a patch applied with `git apply`
  sh manual                             a command the checker cannot run
                                        (cloning, installing); it is listed

Any other `sh` block is an error; other blocks are only shown. A `server`
command runs in the background with SENU_BIND=127.0.0.1:0 until the next
server command or the end, and is then stopped as Ctrl+C would. Later
commands see its real address in place of 127.0.0.1:3000, and its
`text` block is its output up to the listening line. Before comparison the
output loses terminal colours, and the real address, other loopback ports,
test durations, thread ids in panic messages and `deps/senu-<hash>` are
replaced by fixed text. A command that is expected to fail must not print a
compiler error. At the end, src/reading.rs must match HEAD again.

The copy reuses dependency artifacts from target/debug, but never Senu's own
outputs, so a stale binary cannot pass a check. Every command runs in its
own process group with a 1800 s timeout; whatever is left of the group is
killed when it ends. Exit status 0 means every check passed; otherwise the
reasons go to stderr and the status is 1. --all-mismatches keeps going after
an output mismatch and prints every actual output, for authors. Standard
library only.
"""

import difflib
import hashlib
import io
import os
import re
import shutil
import signal
import subprocess
import sys
import tarfile
import tempfile
import time
from pathlib import Path, PurePosixPath

ROOT = Path(__file__).resolve().parent.parent
LOCALES = ("en", "ja")
DOCUMENTED = "127.0.0.1:3000"
TIMEOUT = 1800
FENCE = re.compile(r"^(`{3,})(.*)$")
# Terminal control sequences, such as Cargo's colours under CARGO_TERM_COLOR.
ANSI_ESCAPE = re.compile(r"\x1b\[[0-?]*[ -/]*[@-~]")
OTHER_PORT = re.compile(r"127\.0\.0\.1:(?!3000\b)\d+")
DURATION = re.compile(r"finished in \d+(\.\d+)?s")
THREAD_ID = re.compile(r"(thread '[^']*') \(\d+\) panicked")
SENU_HASH = re.compile(r"deps/senu-[0-9a-f]+")
COMPILER_ERROR = re.compile(r"^error(\[E\d+\])?:")
TEST_FAILED = "error: test failed, to rerun pass"


class CheckFailed(Exception):
    pass


class Block:
    def __init__(self, path, line, info, body, paragraph):
        self.path, self.line, self.body, self.paragraph = path, line, body, paragraph
        words = info.split()
        self.lang = words[0] if words else ""
        self.attrs = {}
        self.flags = set()
        for word in words[1:]:
            key, eq, value = word.partition("=")
            if eq:
                self.attrs[key] = value
            else:
                self.flags.add(key)

    @property
    def where(self):
        return f"{self.path}:{self.line}"

    @property
    def kind(self):
        if self.lang == "sh":
            if "manual" in self.flags:
                return "manual"
            if "check" not in self.attrs:
                raise CheckFailed(f"{self.where}: an sh block needs check=<name> or manual")
            return "server" if "server" in self.flags else "command"
        if "file" in self.attrs:
            return "file"
        if self.lang == "diff" and "patch" in self.flags:
            return "patch"
        if self.lang == "text" and "check" in self.attrs:
            return "expected"
        return "shown"


def blocks(path, shown_as=None):
    """The fenced blocks of one article, with the paragraph before each."""
    # `paragraph` collects the lines of the paragraph being read, and `last`
    # is the paragraph that ended at the latest blank line.
    found, fence, paragraph, last, current = [], None, [], "", None
    label = shown_as or path
    for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        if fence is None:
            match = FENCE.match(line)
            if match:
                fence = match.group(1)
                current = (number, match.group(2).strip(), [], " ".join(paragraph) or last)
                paragraph, last = [], ""
            elif line.strip():
                paragraph.append(line.strip())
            elif paragraph:
                paragraph, last = [], " ".join(paragraph)
        elif line == fence:
            number0, info, body, before = current
            found.append(Block(label, number0, info, "".join(l + "\n" for l in body), before))
            fence = None
        else:
            current[2].append(line)
    if fence is not None:
        raise CheckFailed(f"{label}: a code block is never closed")
    return found


def articles(root, locale):
    return sorted((root / "content" / locale).glob("*.md"))


def clean_env():
    env = {k: v for k, v in os.environ.items() if not k.startswith("SENU_")}
    for name in ("GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE"):
        env.pop(name, None)
    return env


def exited(job):
    """Whether `job` has exited, without reaping it. Until it is reaped, its
    process id, which is also the id of its process group, stays reserved."""
    flags = os.WEXITED | os.WNOHANG | os.WNOWAIT
    return job.returncode is not None or os.waitid(os.P_PID, job.pid, flags) is not None


def stop(job, first=signal.SIGTERM):
    """Ends every process left in `job`'s process group, then reaps `job`.
    The group is signalled only while `job` is unreaped, so its id cannot
    belong to anyone else; this works even after `job` itself has exited."""
    if job.returncode is not None:
        return
    # Whatever is left of the group gets every later signal too.
    for sig in dict.fromkeys((first, signal.SIGTERM, signal.SIGKILL)):
        try:
            os.killpg(job.pid, sig)
        except ProcessLookupError:
            pass
        deadline = time.monotonic() + 10
        while sig != signal.SIGKILL and not exited(job) and time.monotonic() < deadline:
            time.sleep(0.1)
    job.wait(timeout=10)


def run(script, cwd, env, timeout):
    """Runs `script` with bash; returns its exit status and output."""
    with tempfile.TemporaryFile(mode="w+", encoding="utf-8", errors="replace") as log:
        job = subprocess.Popen(
            ["bash", "-c", script], cwd=cwd, env=env, stdin=subprocess.DEVNULL,
            stdout=log, stderr=log, start_new_session=True,
        )
        try:
            deadline = time.monotonic() + timeout
            while not exited(job):
                if time.monotonic() >= deadline:
                    raise CheckFailed(f"no result within {timeout} s:\n{script}")
                time.sleep(0.1)
        finally:
            stop(job)
        log.seek(0)
        return job.returncode, log.read()


def start_server(script, cwd, env, timeout=TIMEOUT):
    """Starts a server command on port 0 with `bash -e`, so a failed build
    never starts an old binary. Returns (process, address, output, log)."""
    env = dict(env, SENU_BIND="127.0.0.1:0")
    log = tempfile.TemporaryFile(mode="w+", encoding="utf-8", errors="replace")
    server = subprocess.Popen(
        ["bash", "-e", "-c", script], cwd=cwd, env=env, stdin=subprocess.DEVNULL,
        stdout=log, stderr=log, start_new_session=True,
    )
    deadline = time.monotonic() + timeout
    try:
        while time.monotonic() < deadline and not exited(server):
            log.seek(0)
            text = log.read()
            match = re.search(r"^senu: listening on http://(127\.0\.0\.1:\d+)\n", text, re.M)
            if match:
                return server, match.group(1), text[: match.end()], log
            time.sleep(0.2)
    except BaseException:
        stop(server)
        log.close()
        raise
    stop(server)
    log.seek(0)
    text = log.read()
    log.close()
    raise CheckFailed(f"the server command did not start a server:\n{script}--- output\n{text}")


def normalize(output, address=None):
    output = ANSI_ESCAPE.sub("", output)
    if address:
        output = output.replace(address, DOCUMENTED)
    output = OTHER_PORT.sub("127.0.0.1:PORT", output)
    output = DURATION.sub("finished in 0.00s", output)
    output = THREAD_ID.sub(r"\1 (ID) panicked", output)
    return SENU_HASH.sub("deps/senu-HASH", output)


def git(args, cwd):
    done = subprocess.run(["git", *args], cwd=cwd, env=clean_env(), capture_output=True, check=False)
    if done.returncode != 0:
        raise CheckFailed(f"git {' '.join(args)} failed: {done.stderr.decode(errors='replace')}")
    return done.stdout


def snapshot():
    """A digest of HEAD, the status and the tracked diff of the real tree."""
    digest = hashlib.sha256()
    for args in (
        ["rev-parse", "HEAD"],
        ["status", "--porcelain=v1", "-z", "--untracked-files=all"],
        ["diff", "--binary", "HEAD"],
    ):
        digest.update(git(args, ROOT))
    return digest.hexdigest()


def own_outputs(root):
    """Names of build outputs that belong to Senu itself rather than to a
    dependency: the package and every integration test target."""
    return {"senu"} | {path.stem for path in (root / "tests").glob("*.rs")}


def copy_tree(work):
    """Unpacks the committed HEAD, and nothing else, into a new repository,
    and seeds it with dependency artifacts only."""
    changed = git(["status", "--porcelain", "--untracked-files=no"], ROOT)
    if changed:
        raise CheckFailed(
            "the tutorial check runs on the last commit, but tracked files differ"
            " from it; commit or stash them first:\n" + changed.decode(errors="replace")
        )
    archive = git(["archive", "--format=tar", "HEAD"], ROOT)
    work.mkdir(parents=True)
    try:
        with tarfile.open(fileobj=io.BytesIO(archive)) as tar:
            tar.extractall(work, filter="data")
    except tarfile.FilterError as err:
        raise CheckFailed(f"HEAD has an entry that cannot be unpacked safely: {err}") from None
    quiet = ["-c", "commit.gpgsign=false", "-c", "core.hooksPath=/dev/null"]
    identity = ["-c", "user.name=tutorial", "-c", "user.email=tutorial@example.invalid"]
    git(["init", "-q", "-b", "main"], work)
    git(["add", "-A"], work)
    git([*quiet, *identity, "commit", "-q", "-m", "tutorial base"], work)

    cache = Path(os.environ.get("CARGO_TARGET_DIR") or "target")
    cache = (cache if cache.is_absolute() else ROOT / cache) / "debug"
    if not cache.is_dir():
        print("note: no target/debug to reuse; the copy builds everything")
        return
    seeded = work / "target" / "debug"
    seeded.mkdir(parents=True)
    own = own_outputs(work)
    for part in ("deps", "build", ".fingerprint"):
        if not (cache / part).is_dir():
            continue
        copied = subprocess.run(
            ["cp", "-a", "--reflink=auto", str(cache / part), str(seeded)], capture_output=True
        )
        if copied.returncode != 0:
            shutil.copytree(cache / part, seeded / part, symlinks=True, dirs_exist_ok=True)
        for entry in (seeded / part).iterdir():
            if entry.name.removeprefix("lib").rsplit("-", 1)[0] in own:
                if entry.is_dir() and not entry.is_symlink():
                    shutil.rmtree(entry)
                else:
                    entry.unlink()


def check_file_path(block):
    path = block.attrs["file"]
    pure = PurePosixPath(path)
    if pure.is_absolute() or ".." in pure.parts or pure.parts[0] not in ("content", "src"):
        raise CheckFailed(f"{block.where}: file={path} must stay under content/ or src/")
    if f"`{path}`" not in block.paragraph:
        raise CheckFailed(f"{block.where}: the paragraph before must name `{path}`")
    return pure


class Session:
    """Runs one language's blocks in a copy."""

    def __init__(self, work, env, all_mismatches):
        self.work, self.env, self.all_mismatches = work, env, all_mismatches
        self.server = None
        self.address = None
        self.last = None  # (name, output) of the last command
        self.mismatches = []
        self.manual = []

    def stop_server(self):
        if self.server is not None:
            process, log = self.server
            stop(process, first=signal.SIGINT)
            log.close()
            self.server = None
            self.address = None

    def step(self, block):
        kind = block.kind
        if kind in ("command", "server"):
            self.command(block, kind)
        elif kind == "expected":
            self.compare(block)
        elif kind == "file":
            target = self.work / check_file_path(block)
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text(block.body, encoding="utf-8")
            print(f"ok: {block.where}: wrote {block.attrs['file']}")
        elif kind == "patch":
            with tempfile.NamedTemporaryFile("w", suffix=".patch", delete=False) as patch:
                patch.write(block.body)
            try:
                status, output = run(f"git apply '{patch.name}'", self.work, self.env, 60)
            finally:
                os.unlink(patch.name)
            if status != 0:
                raise CheckFailed(f"{block.where}: the patch does not apply:\n{output}")
            print(f"ok: {block.where}: applied the patch")
        elif kind == "manual":
            self.manual.append(block.where)

    def command(self, block, kind):
        name = block.attrs["check"]
        want = int(block.attrs.get("exit", "0"))
        script = block.body
        if kind == "server":
            self.stop_server()
            process, self.address, output, log = start_server(script, self.work, self.env)
            self.server = (process, log)
            status = 0
        else:
            if self.address:
                script = script.replace(DOCUMENTED, self.address)
            status, output = run(script, self.work, self.env, TIMEOUT)
        output = normalize(output, self.address)
        label = f"{block.where}: {name}"
        if status != want:
            raise CheckFailed(f"{label} exited {status}, expected {want}\n--- commands\n{block.body}--- output\n{output}")
        if want != 0 and any(
            COMPILER_ERROR.match(line) and not line.startswith(TEST_FAILED)
            for line in output.splitlines()
        ):
            raise CheckFailed(f"{label} failed for a reason other than the expected one:\n{output}")
        self.last = (name, output)
        print(f"ok: {label} (exit {status})")

    def compare(self, block):
        name = block.attrs["check"]
        if self.last is None or self.last[0] != name:
            raise CheckFailed(f"{block.where}: text check={name} does not follow its command")
        output = self.last[1]
        self.last = None
        if output == block.body:
            print(f"ok: {block.where}: {name} output matches")
            return
        diff = "".join(difflib.unified_diff(
            block.body.splitlines(True), output.splitlines(True), "expected", "actual"))
        message = f"{block.where}: {name} output differs:\n{diff}--- actual output\n{output}--- end"
        if not self.all_mismatches:
            raise CheckFailed(message)
        self.mismatches.append(message)


def check_language(locale, all_mismatches=False):
    before = snapshot()
    with tempfile.TemporaryDirectory(prefix=f"senu-tutorial-{locale}-") as tmp:
        work = Path(tmp) / "senu"
        copy_tree(work)
        env = clean_env()
        env["CARGO_TARGET_DIR"] = str(work / "target")
        session = Session(work, env, all_mismatches)
        try:
            for path in articles(work, locale):
                for block in blocks(path, shown_as=path.relative_to(work)):
                    session.step(block)
        finally:
            session.stop_server()
        for where in session.manual:
            print(f"not run: {where} (manual)")
        if git(["status", "--porcelain", "--", "src/reading.rs"], work):
            raise CheckFailed("src/reading.rs differs from HEAD after the tutorial")
        print(f"ok: {locale}: src/reading.rs matches HEAD")
        if session.mismatches:
            raise CheckFailed("\n\n".join(session.mismatches))
    if snapshot() != before:
        raise CheckFailed("the original working tree changed during the tutorial check")
    print(f"ok: {locale}: the original working tree is unchanged")


def main(argv):
    all_mismatches = "--all-mismatches" in argv
    locales = [arg for arg in argv if arg != "--all-mismatches"] or list(LOCALES)
    if any(locale not in LOCALES for locale in locales):
        print("usage: check_tutorial.py [--all-mismatches] [en|ja ...]", file=sys.stderr)
        return 2
    try:
        for locale in locales:
            check_language(locale, all_mismatches)
        return 0
    except CheckFailed as err:
        print(f"check_tutorial: {err}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
