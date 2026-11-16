# Notes for coding agents

- Senu is a library (spec 0006): the core (`src/operation.rs`,
  `src/auth.rs`, `src/http.rs`, `src/mcp.rs`, and `src/contract.rs` behind
  the `contract` feature) must build with `--no-default-features` and name
  nothing from the demo. Everything else in `src/` is the `demo` feature:
  the catalogue application in `src/demo/`, the `senu` binary
  (`senu serve`, loopback only, and opt-in stdio MCP) and `senu site`,
  which serves trusted Markdown articles from `content/` without
  credentials. New behaviour, such as MCP over HTTP, HTML form posts,
  storage or session stores, needs a new spec in `docs/specs/` first. Demo credentials stay
  loopback-only.
- Public API changes go in `CHANGELOG.md` with their migration, keep
  `#![warn(missing_docs)]` clean for the core, and keep `examples/`
  compiling: the guide and both READMEs copy code from `examples/hello.rs`,
  `examples/inventory.rs`, `examples/items.rs` and their tests verbatim,
  which `tests/docs_sync.rs` checks. Claims stay measured: no audit, no
  performance numbers, no search ranking.
- Declared operations (spec 0005): the application is `src/demo/`
  (permissions, principals, `endpoints()`, `tools()` and one file per
  operation); Senu owns `Operation::call`, `http::Endpoints`, `mcp::tool`
  and `tests/contract.rs`. Expose an operation only by listing it, give
  each listed operation a sample in `demo_subject`, and keep the README
  route lists in step (`tests/docs_sync.rs`). `.custom` routes get no
  authentication; their handler owns it.
- Writes (spec 0007): a declared write is served by `POST` only, after the
  cross-site check and before any byte of its body is parsed; keep that
  order in `http::write_route`, and keep CT-8 to CT-12 in `src/contract.rs`
  failing under the mutations that `docs/development.md` lists. The demo
  stays read-only; `examples/items.rs` is the write example.
- Specs: `docs/specs/0001-project-catalogue.md` for the catalogue and
  `docs/specs/0003-articles-and-beginner-tutorial.md` for the articles, the
  site and the course (spec 0002, the mdBook tutorial, is superseded), and
  `docs/specs/0006-library-release-and-public-site.md` for the library,
  the guide (articles 10 to 49) and the public site, and
  `docs/specs/0007-write-operations.md` for writes, typed arguments and
  sessions.
  `docs/development.md` records only checks that were actually run.
- Articles: `content/en` is canonical and `content/ja` mirrors it chapter
  for chapter. `build.rs` embeds every `NN-slug.md` file; there is no
  registry. Commands and expected output live in the articles' fenced
  blocks (`sh check=…`, `text check=…`, `file=…`, `diff patch`). When a
  command changes, commit, run the checker and copy the real output into
  its `text` block. Both languages must run the same checked blocks.
- Article bodies and the JSON-LD element reach templates only as
  `TrustedHtml` from `src/markdown.rs`, the crate's single `HtmlSafe` impl;
  every other template value stays escaped.
- The public site is the snapshot that
  `scripts/snapshot_site.py --base-url <https URL ending in /> <dir>`
  writes; the `pages` workflow deploys it from `main`. Canonical addresses
  follow the snapshot's file names (`/install` is `install.html`).
- Both READMEs embed `scripts/quickstart.sh` byte for byte.
- Look (spec 0004): Tailwind CSS utilities in `templates/*.html` and
  Typography's `prose`; `assets/ui.css` is the only CSS source. After
  changing a class or the theme, run `npm ci` once and `npm run css`, and
  commit `assets/ui.generated.css` with the source; never edit it by hand.
  Both routers serve it at `GET /_senu/ui.css` from memory, and `/_senu/`
  is reserved. Version bumps change `package.json`, the lockfile, the
  generated file and `THIRD-PARTY-NOTICES.txt` together.
- Checks: `cargo fmt --check`,
  `cargo clippy --locked --all-targets -- -D warnings`,
  `cargo clippy --locked --no-default-features --lib -- -D warnings`,
  `cargo test --locked -- --test-threads=2`,
  `RUSTDOCFLAGS="-D warnings" cargo doc --locked --no-deps --all-features`,
  `scripts/quickstart.sh`,
  `python3 scripts/check_tutorial.py` (English, then Japanese; it needs
  Node and the npm registry for chapter 07's last section),
  `python3 scripts/snapshot_site.py <new-or-empty-dir>` and the drift gate
  `npm ci && npm run css && git diff --exit-code -- assets/ui.generated.css`.
- Generated files stay out of git, except `assets/ui.generated.css`:
  builds and snapshots in `target/`, packages in `node_modules/`. The
  learner's notes and edits exist only in the temporary copies that
  `check_tutorial.py` makes.
