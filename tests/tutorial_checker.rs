//! Runs `scripts/test_check_tutorial.py`, the regression tests for how
//! `scripts/check_tutorial.py` reads check blocks, cleans up processes,
//! starts and stops servers, and copies the repository (spec 0003, §8 and
//! EX-2), and `scripts/test_snapshot_site.py`, the tests for how
//! `scripts/snapshot_site.py` rewrites links (CI-1). Like `docs_sync`, this
//! checks scripts rather than Senu. It needs Linux, `bash`, `git` and
//! `python3`, but builds nothing and leaves the repository unchanged.

use std::process::Command;

fn run_script(name: &str) {
    let output = Command::new("python3")
        .arg(format!("{}/scripts/{name}", env!("CARGO_MANIFEST_DIR")))
        .output()
        .expect("python3 runs");
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn tutorial_checker_cleans_up_processes_and_copies_only_the_commit() {
    run_script("test_check_tutorial.py");
}

#[test]
fn site_snapshot_rewrites_only_real_links() {
    run_script("test_snapshot_site.py");
}
