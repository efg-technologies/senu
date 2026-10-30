#!/usr/bin/env python3
"""Runs the tutorial's shared command snippets and checks their output.

Usage: check_tutorial.py commands
       check_tutorial.py exercise

Both tutorial languages include the same snippet files, so this checks the
commands and expected output that readers see in English and in Japanese.
A snippet section is the text between `ANCHOR: name` and `ANCHOR_END: name`
lines, the syntax that mdBook's `{{#include file:name}}` uses.

`commands` runs from the repository root. It starts the server as the `serve`
section of docs/tutorial/commands/serve.sh does, but on a free loopback port,
then runs every other section of docs/tutorial/commands/*.sh, replacing the
documented http://127.0.0.1:3000 with the real address. Each section must
exit 0 and print exactly the same-named section of the matching `.out` file.
Other loopback ports in the output are compared as `PORT`.

`exercise` copies the repository's tracked and unignored files into a new git
repository in a temporary directory, and reuses a copy of target/debug when
one exists, so only Senu itself is rebuilt there. It then runs the sections
of docs/tutorial/exercise/steps.sh in order. Each must exit with the status
listed in STEPS below, and the lines of its `.out` section must appear in its
output in that order; a line ending in "..." matches by prefix. A failing
step must fail on a test assertion, never on a patch, compiler or setup
error. The temporary directory is removed, and the original working tree
must be unchanged at the end.

Exit status 0 means every check passed; on failure the reason, and the
output of the failing step, go to stderr and the exit status is 1.
Standard library only.
"""

import difflib
import hashlib
import os
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
COMMANDS = Path("docs/tutorial/commands")
STEPS_FILE = Path("docs/tutorial/exercise/steps.sh")
DOCUMENTED_URL = "http://127.0.0.1:3000"
# Sections that are shown but not run here: cloning needs the network.
UNCHECKED = {("setup.sh", "clone")}
# The exercise sections, in order, with the exit status each must have.
STEPS = {
    "branch": 0,
    "expect": 0,
    "red": 101,
    "rename": 0,
    "green": 0,
    "suite-red": 101,
    "fixtures": 0,
    "suite-green": 0,
    "parity": 0,
    "restore": 0,
}
ANCHOR = re.compile(r"ANCHOR(_END)?:\s*([A-Za-z0-9_-]+)")
OTHER_PORT = re.compile(r"http://127\.0\.0\.1:(?!3000\b)\d+")


class CheckFailed(Exception):
    pass


def sections(path):
    """The anchored sections of a snippet file, in file order."""
    if not path.is_file():
        raise CheckFailed(f"missing snippet file {path}")
    found, current = {}, None
    for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        match = ANCHOR.search(line)
        if not match:
            if current is not None:
                found[current].append(line)
            continue
        end, name = match.groups()
        if end and name != current or not end and (current is not None or name in found):
            raise CheckFailed(f"{path}:{number}: unbalanced or repeated anchor {name}")
        if end:
            current = None
        else:
            found[name] = []
            current = name
    if current is not None:
        raise CheckFailed(f"{path}: anchor {current} is never closed")
    return {name: "".join(line + "\n" for line in lines) for name, lines in found.items()}


def clean_env():
    env = dict(os.environ)
    for name in ("GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE"):
        env.pop(name, None)
    return env


def run(script, cwd, env, timeout):
    try:
        done = subprocess.run(
            ["bash", "-c", script],
            cwd=cwd,
            env=env,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            errors="replace",
            timeout=timeout,
        )
    except subprocess.TimeoutExpired:
        raise CheckFailed(f"no result within {timeout} s:\n{script}") from None
    return done.returncode, done.stdout


def normalize(output, url=None):
    if url:
        output = output.replace(url, DOCUMENTED_URL)
    return OTHER_PORT.sub("http://127.0.0.1:PORT", output)


def report(label, script, output):
    return f"{label}\n--- commands\n{script}--- output\n{output}"


def start_server(env):
    """Runs the `serve` section on port 0 and returns (process, url, log)."""
    serve = sections(ROOT / COMMANDS / "serve.sh")["serve"]
    script = serve.replace("127.0.0.1:3000", "127.0.0.1:0")
    if script == serve:
        raise CheckFailed("serve.sh no longer binds 127.0.0.1:3000")
    log = tempfile.TemporaryFile(mode="w+", encoding="utf-8", errors="replace")
    server = subprocess.Popen(
        ["bash", "-c", script],
        cwd=ROOT,
        env=env,
        stdin=subprocess.DEVNULL,
        stdout=log,
        stderr=log,
        start_new_session=True,
    )
    deadline = time.monotonic() + 600
    while time.monotonic() < deadline and server.poll() is None:
        log.seek(0)
        for line in log.read().splitlines():
            if line.startswith("senu: listening on http://127.0.0.1:"):
                return server, line.removeprefix("senu: listening on "), log
        time.sleep(0.2)
    log.seek(0)
    stop(server)
    raise CheckFailed(f"the serve section did not start a server:\n{log.read()}")


def stop(server):
    if server.poll() is None:
        os.killpg(server.pid, signal.SIGTERM)
        try:
            server.wait(timeout=10)
        except subprocess.TimeoutExpired:
            os.killpg(server.pid, signal.SIGKILL)
            server.wait(timeout=10)


def check_commands():
    env = clean_env()
    server, url, log = start_server(env)
    try:
        expected_serve = sections(ROOT / COMMANDS / "serve.out")["serve"]
        log.seek(0)
        listening = [line for line in log.read().splitlines() if line.startswith("senu: ")]
        got = "\n".join(listening).replace(url, DOCUMENTED_URL) + "\n"
        if got != expected_serve:
            raise CheckFailed(f"serve.sh:serve printed\n{got}expected\n{expected_serve}")
        print("ok: serve.sh:serve")
        for script_path in sorted((ROOT / COMMANDS).glob("*.sh")):
            scripts = sections(script_path)
            expected = sections(script_path.with_suffix(".out"))
            for name, script in scripts.items():
                label = f"{script_path.name}:{name}"
                if (script_path.name, name) in UNCHECKED or label == "serve.sh:serve":
                    continue
                if name not in expected:
                    raise CheckFailed(f"{label} has no expected output")
                status, output = run(script.replace(DOCUMENTED_URL, url), ROOT, env, 600)
                output = normalize(output, url)
                if status != 0 or output != expected[name]:
                    diff = "".join(
                        difflib.unified_diff(
                            expected[name].splitlines(True),
                            output.splitlines(True),
                            "expected",
                            "actual",
                        )
                    )
                    raise CheckFailed(f"{label} exited {status}; output differs:\n{diff}")
                print(f"ok: {label}")
            extra = set(expected) - set(scripts)
            if extra:
                raise CheckFailed(f"{script_path.with_suffix('.out').name}: no commands for {sorted(extra)}")
    finally:
        stop(server)
        log.close()


def appears_in_order(expected, output):
    """The output lines that match `expected`, or the first missing line."""
    lines, found = iter(output.splitlines()), []
    for want in expected.splitlines():
        prefix = want[:-3] if want.endswith("...") else None
        line = next(
            (l for l in lines if (l.startswith(prefix) if prefix is not None else l == want)),
            None,
        )
        if line is None:
            return want, found
        found.append(line)
    return None, found


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


def git(args, cwd):
    done = subprocess.run(
        ["git", *args], cwd=cwd, env=clean_env(), capture_output=True, check=False
    )
    if done.returncode != 0:
        raise CheckFailed(f"git {' '.join(args)} failed: {done.stderr.decode(errors='replace')}")
    return done.stdout


def copy_tree(work):
    listed = git(["ls-files", "-z", "--cached", "--others", "--exclude-standard"], ROOT)
    for name in sorted({n for n in listed.decode().split("\0") if n}):
        source = ROOT / name
        if source.is_file():
            target = work / name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source, target)
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
    for part in ("deps", "build", ".fingerprint"):
        if (cache / part).is_dir():
            copied = subprocess.run(
                ["cp", "-a", "--reflink=auto", str(cache / part), str(seeded)],
                capture_output=True,
            )
            if copied.returncode != 0:
                shutil.copytree(cache / part, seeded / part, symlinks=True, dirs_exist_ok=True)


def check_exercise():
    before = snapshot()
    scripts = sections(ROOT / STEPS_FILE)
    expected = sections((ROOT / STEPS_FILE).with_suffix(".out"))
    if list(scripts) != list(STEPS) or set(expected) != set(STEPS):
        raise CheckFailed(f"{STEPS_FILE} and its .out must have the sections {list(STEPS)}")
    with tempfile.TemporaryDirectory(prefix="senu-tutorial-") as tmp:
        work = Path(tmp) / "senu"
        copy_tree(work)
        env = clean_env()
        env["CARGO_TARGET_DIR"] = str(work / "target")
        for name, status in STEPS.items():
            got, output = run(scripts[name], work, env, 1800)
            output = normalize(output)
            label = f"steps.sh:{name}"
            failure = None
            missing, found = appears_in_order(expected[name], output)
            if got != status:
                failure = f"{label} exited {got}, expected {status}"
            elif missing is not None:
                failure = f"{label}: expected line not found in order: {missing}"
            elif status != 0 and any(
                line.startswith("error") and not line.startswith("error: test failed, to rerun pass")
                for line in output.splitlines()
            ):
                failure = f"{label} failed for a reason other than a test assertion"
            elif status == 0 and name.startswith("suite"):
                results = [l for l in output.splitlines() if l.startswith("test result: ")]
                if not results or not all(l.startswith("test result: ok.") for l in results):
                    failure = f"{label}: not every test binary passed"
            elif name == "parity" and "Apollo" in output:
                failure = f"{label}: an output still shows the old name"
            if failure:
                raise CheckFailed(report(failure, scripts[name], output))
            print(f"ok: {label} (exit {got})")
            if status != 0:
                print("".join(f"    {line}\n" for line in found), end="")
        if git(["status", "--porcelain"], work) or git(["branch", "--show-current"], work) != b"main\n":
            raise CheckFailed("the restore step did not return the copy to its original state")
    if snapshot() != before:
        raise CheckFailed("the original working tree changed during the exercise check")
    print("ok: the original working tree is unchanged")


def main(argv):
    modes = {"commands": check_commands, "exercise": check_exercise}
    if len(argv) != 1 or argv[0] not in modes:
        print("usage: check_tutorial.py commands|exercise", file=sys.stderr)
        return 2
    try:
        modes[argv[0]]()
        return 0
    except CheckFailed as err:
        print(f"check_tutorial: {err}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
