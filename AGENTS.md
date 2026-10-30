# Notes for coding agents

- Senu is an experimental seed: one pure operation, `list_projects`, over a
  fixed read-only catalogue, served as HTML and JSON by `senu serve`
  (loopback only) and as an opt-in stdio MCP tool. New behaviour, such as
  writes, storage or other authentication, needs a new spec in `docs/specs/`
  first. Demo credentials stay loopback-only.
- Specs: `docs/specs/0001-project-catalogue.md` for the product and
  `docs/specs/0002-tutorial.md` for the tutorial and site.
  `docs/development.md` records only checks that were actually run.
- Tutorial: English in `docs/tutorial/src` is canonical and
  `docs/tutorial/src/ja` mirrors it chapter for chapter. Commands and expected
  output exist only in `docs/tutorial/commands` and `docs/tutorial/exercise`,
  and chapters include them. When a command changes, run it and copy the real
  output into its `.out` section.
- Both READMEs embed `scripts/quickstart.sh` byte for byte.
- Checks: `cargo fmt --check`,
  `cargo clippy --locked --all-targets -- -D warnings`,
  `cargo test --locked -- --test-threads=2`, `scripts/quickstart.sh`,
  `python3 scripts/check_tutorial.py commands`,
  `python3 scripts/check_tutorial.py exercise`, and
  `scripts/build_site.sh` with mdBook 0.5.4 (`scripts/install_mdbook.sh`).
- Generated files stay out of git: builds in `target/`, the site in
  `target/site`, mdBook in `target/tools`. The exercise's renamed catalogue
  exists only in the temporary copy that `check_tutorial.py` makes.
