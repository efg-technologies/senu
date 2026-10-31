# Notes for coding agents

- Senu is an experimental seed: one pure operation, `list_projects`, over a
  fixed read-only catalogue, served as HTML and JSON by `senu serve`
  (loopback only) and as an opt-in stdio MCP tool, plus `senu site`, which
  serves trusted Markdown articles from `content/` without credentials. New
  behaviour, such as writes, storage or other authentication, needs a new
  spec in `docs/specs/` first. Demo credentials stay loopback-only.
- Specs: `docs/specs/0001-project-catalogue.md` for the catalogue and
  `docs/specs/0003-articles-and-beginner-tutorial.md` for the articles, the
  site and the course (spec 0002, the mdBook tutorial, is superseded).
  `docs/development.md` records only checks that were actually run.
- Articles: `content/en` is canonical and `content/ja` mirrors it chapter
  for chapter. `build.rs` embeds every `NN-slug.md` file; there is no
  registry. Commands and expected output live in the articles' fenced
  blocks (`sh check=…`, `text check=…`, `file=…`, `diff patch`). When a
  command changes, commit, run the checker and copy the real output into
  its `text` block. Both languages must run the same checked blocks.
- Article bodies reach templates only as `TrustedHtml` from
  `src/markdown.rs`, the crate's single `HtmlSafe` impl; every other
  template value stays escaped.
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
  `cargo test --locked -- --test-threads=2`, `scripts/quickstart.sh`,
  `python3 scripts/check_tutorial.py` (English, then Japanese; it needs
  Node and the npm registry for chapter 07's last section),
  `python3 scripts/snapshot_site.py <new-or-empty-dir>` and the drift gate
  `npm ci && npm run css && git diff --exit-code -- assets/ui.generated.css`.
- Generated files stay out of git, except `assets/ui.generated.css`:
  builds and snapshots in `target/`, packages in `node_modules/`. The
  learner's notes and edits exist only in the temporary copies that
  `check_tutorial.py` makes.
