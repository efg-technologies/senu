# Development log

This file records how the first Senu slice was built against
[spec 0001](specs/0001-project-catalogue.md), and the red-green evidence that
spec §6 asks for. Each entry lists the command, its exit status, and the test
counts or assertion messages that mattered. Commands run from the repository
root.

Local runs used a resource-bounded wrapper (a single build job with capped
memory and CPU) so the build would not disturb other work on the same machine.
CI runs the same `cargo` commands without that wrapper.

Spec §6 lists 17 red-green steps. The owner asked for a small number of
coherent commits, so the steps are grouped below. Every group that adds
behaviour was run red against stubs before the implementation was written,
and the output quoted here is the real output of those runs.

## Commit 1: scaffold (§6 step 1)

Package, pinned toolchain (Rust 1.99.1, edition 2024), lockfile, CI workflow
and the Apache-2.0 `LICENSE`. There is no behaviour yet, so there is no red
phase.

The resolver locked the current releases: axum 0.8.9, tokio 1.54.1,
serde 1.0.229, serde_json 1.0.154, askama 0.16.1, rmcp 3.8.0, base64 0.23.1,
subtle 2.6.1 and form_urlencoded 1.2.2, plus tower 0.5.3 for tests.

| Command | Exit | Result |
|---|---|---|
| `cargo fmt --check` | 0 | clean |
| `cargo clippy --locked --all-targets -- -D warnings` | 0 | no warnings |
| `cargo test --locked -- --test-threads=2` | 0 | 0 tests |
