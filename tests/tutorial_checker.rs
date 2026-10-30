//! Runs `scripts/test_check_tutorial.py`, the regression tests for how
//! `scripts/check_tutorial.py` cleans up processes, starts the server and
//! copies the repository (spec 0002, TUT-2 and TUT-3). Like `site_scripts`,
//! this checks a script rather than Senu. It needs Linux, `bash`, `git` and
//! `python3`, but builds nothing and leaves the repository unchanged.

use std::process::Command;

#[test]
fn tutorial_checker_cleans_up_processes_and_copies_only_the_commit() {
    let output = Command::new("python3")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/scripts/test_check_tutorial.py"
        ))
        .output()
        .expect("python3 runs");
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
