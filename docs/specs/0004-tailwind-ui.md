# Senu Spec 0004: Tailwind CSS and Typography as the UI Foundation

Status: proposed, 2026-10-31, corrected the same day: the measure of success is the code, configuration, build steps and tests that **application developers** own, not the size of Senu itself, so the stylesheet moved from inline CSS to one fixed, framework-owned route. Stage 1 of two; spec 0005 is stage 2. It changes the look, the markup and the CSP of the HTML pages of specs 0001 and 0003 and adds one route; everything else stays as specified there. Evidence belongs in `docs/development.md`, CI runs and PRs, not in this file.

## 1. Goal and boundaries

- **Goal:** replace every hand-written CSS rule with Tailwind CSS utilities in the templates and the official Typography plugin's `prose` styles, so that Senu offers a standard, widely known UI foundation, much as Bootstrap once did for Rails. Application developers change the look by editing classes or theme tokens and running one documented build command, with no per-page setup.
- **Measure:** the user-owned lines, bytes, files and steps of the concrete samples of §7, before and after. Senu's own size is a separate diagnostic and never a target. No claim goes beyond what §7 measures.
- **Out of scope:** JavaScript at run time, React, Hotwire, Vite or any other JS framework, CDNs, web fonts, a general static-asset server, dark mode, custom breakpoints, databases, authentication changes, and Node in `build.rs` or any Cargo step.

## 2. Why this architecture

Both options considered need zero per-page setup from application developers. The earlier inline-CSS choice served a framework-size metric that the owner has since withdrawn. Inline CSS would also put generated text inside HTML and Askama source (end-tag and delimiter hazards), would need `'unsafe-inline'`, and conflicts with the test that allows only `.html` files in `templates/`. A fixed external route avoids all three. The CSS is compiled into the binary with `include_str!` and served from memory at one exact path, so nothing touches the filesystem at run time, Askama's ERB-like delimiters and the single `HtmlSafe` impl in `src/markdown.rs` stay untouched, and both CSPs drop `'unsafe-inline'` for `style-src 'self'`. The route and the snapshot support add Senu-owned code and tests, which is acceptable: no application repeats them. The generated file is committed, so a Cargo-only clone, the offline binary and the first lessons need no Node, and a CI drift gate keeps it equal to its source. The Markdown renderer emits no `class` attribute (plain `<pre><code>` included), so Typography styles its HTML and Markdown is never scanned.

The official standalone Tailwind CLI binary was considered but not implemented: Senu would have to pin and verify per-platform downloads itself, whereas the npm lockfile records SRI integrity hashes and npm is the official install path.

## 3. Files and toolchain

- `package.json` (Senu-owned): `"private": true`, exactly one script, `"css": "tailwindcss -i assets/ui.css -o assets/ui.generated.css --minify"`, and exact devDependencies without `^` or `~`: `tailwindcss` 4.3.3, `@tailwindcss/cli` 4.3.3 and `@tailwindcss/typography` 0.5.20, all MIT and from registry.npmjs.org, matching the metadata already verified for this spec. There is no JS config file, `.npmrc` or wrapper script.
- `package-lock.json` (generated, committed): lockfile version 3 with `resolved` and SRI `integrity` for every package, including the Linux x64 platform packages that CI needs. `/node_modules` is added to `.gitignore`.
- `assets/ui.css` (the only CSS source, the file where application developers change the theme): a leading `/*! @tailwindcss/typography v0.5.20 | MIT License | … */` notice, then `@import "tailwindcss" source(none);`, `@source "../templates/*.html";`, `@plugin "@tailwindcss/typography";` and an `@theme` block that sets only `--font-sans` and `--font-mono` to today's stacks with their CJK fallbacks. It has no selectors of its own. The one allowed exception is glue for the always-open desktop chapter list (`::details-content`), and only if the pinned variants cannot express it: at most six lines, with a one-line reason.
- `assets/ui.generated.css` (generated, committed, never edited by hand): the minified output, which carries Tailwind's own `/*! tailwindcss v4.3.3 | MIT License … */` banner and the Typography notice.
- `THIRD-PARTY-NOTICES.txt`: the LICENSE texts of both packages at the pinned versions, copied verbatim from the installed packages.
- Node is needed only to change the look. The implementation is verified with Node 24.11.1 and 24.22.0 and their bundled npm, and CI pins 24.22.0 with `actions/setup-node`, pinned by commit SHA like the other actions.

The recipe is the same in both READMEs, AGENTS.md, the course and CI: run `npm ci` once, run `npm run css` after changing classes or the theme, then run Senu as before (`cargo run -- site` or `serve`), and commit the source together with `assets/ui.generated.css`. There is no combined build-and-run script, so the CSS step stays visible and Cargo-only users are unaffected.

## 4. The stylesheet route

- `GET /_senu/ui.css` exists on both routers, `senu site` and `senu serve`, and answers 200 with the embedded bytes and `Content-Type: text/css; charset=utf-8`, plus the shared `nosniff` and `no-store` headers. Every other method answers 405 with `Allow: GET`. The existing limits run first (413 for a body, 414 for an oversized target). The query string is ignored. Only that exact path matches: a trailing slash, other case, percent-encoding, dot segments and other names under `/_senu/` answer 404. The prefix `/_senu/` is reserved for Senu; site slugs already cannot contain `_`.
- The stylesheet holds no data and no secret, so neither router asks for credentials and neither reads `Authorization` for it. `/projects`, `/api/projects`, their authentication and MCP are unchanged.
- `HTML_CSP` and `SITE_CSP` change only `style-src 'unsafe-inline'` to `style-src 'self'`.
- The `<link>` carries no `integrity` attribute: the stylesheet is same-origin and compiled into the binary, and package integrity is pinned by the lockfile.

## 5. Templates

- `site_base.html` and `projects.html` each have exactly one `<link rel="stylesheet" href="/_senu/ui.css">` and no `<style>` element or `style` attribute. Every hand-written rule, the `:root` variables and the class names that existed only for that CSS (`skip`, `top`, `toc`, `meta`, `pager`, `banner` and the rest) are removed.
- `prose` wraps the text of every page: `<main>` on the site pages and the body of the projects page. Typography's defaults stay. The only `prose-*` modifiers are for overflow: tables and code blocks scroll inside the article, and long words and URLs wrap. The frame (header, chapter list, pager, footer, banner and form) uses utilities, with the default palette and breakpoints; the two-column layout starts at `lg`.
- Utility class names appear only as complete literal strings in `templates/*.html`.
- Behaviour and semantics that stay: one `<h1>` per page; the `<li data-project-id>` items; `id="main"`; the skip link, visible on focus; a visible focus indicator on every focusable element, including links inside articles; the chapter list, collapsible on mobile and sticky and open on desktop, marking the current chapter with `aria-current="page"`; the language switch, pager, footer, `lang` and `hreflang`; strict line breaking and a looser line height on Japanese pages through `[&:lang(ja)]:` variants; and visible borders on form controls, which preflight removes. In every tag, `class` is the last attribute, so tests can match stable prefixes such as `<a href="#main"`.

## 6. Snapshot, CI and course

- **Snapshot:** `scripts/snapshot_site.py` fetches `/_senu/ui.css` as an asset (it requires `text/css`, saves `_senu/ui.css` byte for byte and parses no links in it), rewrites each real `<link href>` to a relative path such as `../_senu/ui.css` from `ja/`, and copies `THIRD-PARTY-NOTICES.txt`. Verification fails if the stylesheet or the notices file is missing, if the saved stylesheet differs from the served one, or if either package's notice is absent from the stylesheet. Escaped code examples stay unchanged. The pages count stays 22, plus the two files.
- **CI drift gate,** in the `tutorial` job, which needs Node for the course anyway: set up Node, run `npm ci` and `npm run css`, then `git diff --exit-code -- assets/ui.generated.css`.
- **Course:** a final optional section of `content/en/07-next-steps.md` and its Japanese mirror, at most 30 lines each. It first explains that Tailwind generates rules only for the classes it finds in `templates/*.html` and that Cargo embeds the committed result. A `sh manual` block names the Node install. Then a `diff patch` changes the header's `bg-white` to `bg-rose-100` (no template uses `rose` before); `grep -c bg-rose-100 assets/ui.generated.css` prints `0` with `exit=1`; `npm ci --silent` must print nothing; `npm run css` runs; `grep -o` prints the new `.bg-rose-100{…}` rule; and `cargo run --quiet -- site` serves the changed header for the reader to see. If the pinned CLI prints a build duration, `check_tutorial.py` gains exactly one normalization for it, with a unit test. Chapters 01 to 06 never mention Node.

## 7. Measured samples

Each sample is applied to a temporary copy at the baseline (1163895) and at the final head, built and served by the real binary, and rendered in a browser. `docs/development.md` records its diff and the user-owned lines, bytes, files and commands on each side, counting non-blank lines.

- **S1, a new Markdown page:** a 3 to 5 line article in `content/en/` (with its `content/ja/` mirror if the site requires one). The expected result, to be confirmed, is the same user cost on both sides (no CSS, Rust or configuration) and Typography styling after.
- **S2, a utility card in an existing template:** a short note box in `site_index.html`. The baseline adds its HTML and the hand-written rules in `site_base.html`'s `<style>`; after, the HTML carries utilities and `npm run css` runs once. The rendered boxes are compared side by side.
- **S3, a theme override:** change the sans-serif font stack and the link colour. The baseline edits `:root` variables; after, `assets/ui.css` gains `@theme` tokens and `npm run css` runs once. No page or template changes.
- **Diagnostics, reported but not targets:** bytes and `gzip -9` bytes of the old inline CSS and of the generated stylesheet; bytes of `/`, `/ja/`, the largest article and `/projects`, and of the whole snapshot, before and after; the npm package count and `node_modules` size; confirmation that `Cargo.lock` is unchanged; and the lines Senu itself gained.

## 8. Acceptance criteria

- **UI-1** Rendered pages use real utilities and `prose`. The generated CSS contains rules for those classes and the Typography rules, and no template keeps a `<style>`, a `style` attribute or an old rule.
- **UI-2** A Rust test checks the generated CSS: both notices are present, `.prose` rules exist, and outside the notices there is no `@import`, `url(` or `://`. Another checks that each page has exactly one stylesheet `<link>` to `/_senu/ui.css`. Each was shown RED first.
- **ROUTE-1** Adapter tests on both routers cover §4: the 200 bytes and headers; 405 with `Allow` for HEAD, POST, PUT, DELETE and OPTIONS; 413; 414; an ignored query; 404 for every variant listed in §4; no credentials needed; and both CSPs.
- **SAFE-1** `src/markdown.rs` keeps the only `HtmlSafe` impl, and the template guard (`.html` only, no `safe` or `escape`) passes unchanged. The Markdown policy, the other routes, headers, limits, authentication, site isolation and MCP are unchanged; existing tests change only where a CSP string or a class-based selector changes. No tracked file gains secrets, machine paths or private data.
- **BLD-1** The versions in `package.json` are exact, and every lockfile entry has SRI `integrity` and a `resolved` URL on registry.npmjs.org. The output of `npm audit signatures` is recorded. Two runs of `npm ci && npm run css`, and runs under Node 24.11.1 and 24.22.0, produce bytes identical to the committed file.
- **BLD-2** Only templates are scanned: `bg-red-500` added to an article's Markdown leaves the generated CSS unchanged. This is recorded, then reverted.
- **BLD-3** A fresh clone with no Node on `PATH` passes `cargo build`, `cargo test` and the quickstart. The built binary, run from an empty temporary directory without network access, serves the stylesheet and styled pages.
- **CI-1** The drift gate is shown failing after a template class edit without `npm run css`, and passing after it.
- **SNAP-1** The snapshot of §6 passes, and its unit tests cover the rewritten `<link>`, the missing-stylesheet case and the missing-notice case. The saved pages are styled when opened from disk, make no network request, and the CI artifact matches.
- **CRS-1** `python3 scripts/check_tutorial.py` passes English, then Japanese, including §6's section.
- **SMP-1** S1 to S3 and the diagnostics of §7 are recorded with real numbers.
- **BRW-1** In locally installed Chrome and Firefox at 1280×800, 375×812 and 320×568, the following are checked and recorded: no horizontal page scroll; the skip link, visible focus under Tab, the language switch, the current chapter, the collapsible and sticky chapter list, the pager and the projects form work; code blocks and tables scroll inside the article; Japanese text wraps; text and focus colours meet WCAG 2.2 AA by computed values; and no request leaves loopback. 200 % zoom is reported only if it was really exercised, and otherwise as not run.
- **DOC-1** Both READMEs and AGENTS.md give the recipe of §3, the drift gate, the route and the notices. The READMEs still embed `scripts/quickstart.sh` byte for byte.
- **TST-1** At the final head, all of these pass: fmt, clippy, every Rust test (the 177 present at 1163895 plus the new ones, with the count recorded), the quickstart, the course in both languages, the snapshot, the Python unit tests and CI.

## 9. Delivery

Each commit passes all checks on its own and records its RED and GREEN evidence (the command, its exit status and the decisive lines) in `docs/development.md`.

1. `test:` find elements by semantics, with the templates reordered so that `class` comes last (no visual change).
2. `ui:` the toolchain, `assets/`, the notices, the route, the CSPs, and the templates with utilities and `prose` (UI-1, UI-2, ROUTE-1, SAFE-1, BLD-1, BLD-2).
3. `snapshot:` carry, rewrite and verify the stylesheet and the notices (SNAP-1).
4. `ci:` the drift gate (CI-1).
5. `docs:` the course section, the checker normalization if it is needed, the READMEs and AGENTS.md (CRS-1, DOC-1).
6. `docs:` the samples, diagnostics and browser results, and this spec's status (SMP-1, BRW-1, BLD-3, TST-1).

## 10. Risks and known limits

- Because of `no-store`, browsers fetch the stylesheet again with each page. A cache policy is a separate decision.
- The generated file is one minified line, so reviewers read the source diff and rely on the drift gate.
- Tailwind's scanner also reads template words such as `block` as candidates. The few extra rules are harmless and drift-checked.
- Typography's defaults, such as backticks around inline code and dark code blocks, are accepted as the de facto look.
- A version upgrade is an ordinary PR that changes `package.json`, the lockfile, the generated file and the notices together.
- Running the full course check needs Node, npm and access to the npm registry. Learners need them only for the optional section of chapter 07.
