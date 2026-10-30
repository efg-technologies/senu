# Senu Spec 0003: Markdown Articles and a Beginner Tutorial

Status: implemented locally, 2026-10-31, pending independent review; proposed and amended the same day after an independent review (reading time instead of section counting, title-only link rule, fail-closed events, UTF-8 at build time, relative snapshot links, installed browsers only). When implemented, this spec supersedes spec 0002 (the mdBook site) and amends spec 0001 only where §7 says so. Evidence belongs in `docs/development.md`, CI runs and PRs, not in this file.

## 1. Goal and boundaries

- **Goal:** a small, reusable, trusted Markdown articles feature, and a new beginner tutorial written as articles and served by Senu itself. A Markdown file placed in `content/en/` or `content/ja/` becomes a page with a route, a title and navigation the next time the usual `cargo build` or `cargo run` runs. No Rust registry needs editing.
- **Audience:** readers who know no Rust and only these parts of JavaScript: `let`, `const`, `function`, `for`, `forEach`, `if`/`else`, `try`/`catch` and `import`/`export`. English is canonical; Japanese must be equally usable.
- **In scope:** build-time discovery and embedding, a Markdown policy, a loopback-only `senu site` mode, the tutorial, its checker, retiring mdBook, and a crawled snapshot of the site as a CI artifact.
- **Out of scope:** runtime file reads, front matter, images and static assets, raw HTML, includes, shortcodes, search, heading anchors, live reload, drafts, more locales, authentication for the site, database, writes, TLS, remote MCP, package publishing and hosting configuration. The fixed catalogue, `senu serve` and `senu mcp` keep their spec 0001 behaviour.

## 2. Why this architecture

The inspected code already compiles everything it serves into the binary: the catalogue is a constant (`src/catalogue.rs`), and Askama checks the templates at compile time. Embedding articles with a build script and `include_str!` extends that model, so the binary reads no files at run time, does not depend on its working directory, and has no directory traversal or symlink race to guard against. `src/http.rs` already lists routes explicitly, applies its `limits` and `response_headers` layers and returns fixed plain-text errors. The site reuses those layers around one exact-lookup handler and adds no framework layer: there are no collections, plugins, front matter or generic content types. `src/app.rs` validates configuration before binding (exit 2 for configuration, 1 at run time), and `src/cli.rs` is an exact-match pure parser, so a third mode costs one match arm and one startup function. pulldown-cmark is a mature CommonMark parser whose pull events allow an explicit allow-list ([Event docs](https://docs.rs/pulldown-cmark/latest/pulldown_cmark/enum.Event.html)). Because raw HTML is never emitted, no HTML sanitizer is needed. Three alternatives were rejected. Rendering inside `build.rs` would bury errors in Cargo's build-script output, which contains hashed paths, and would make unit testing harder. Reading the content directory at run time would bring back working-directory dependence and filesystem risk. A separate static generator would duplicate the backend.

## 3. Content conventions

- `content/en/` (canonical) and `content/ja/` must both be real directories, not symlinks. Senu reads every entry in them whose name ends in `.md` and does not start with `.`, and ignores every other name, such as editor swap and backup files. An entry it reads must be a regular file (not a symlink, directory or device) with a UTF-8 name.
- Names are `NN-slug.md`. `NN` is two ASCII digits, unique within a locale, and sets the order. `slug` is 1 to 48 bytes of lowercase ASCII words (`[a-z0-9]+`) joined by single hyphens, unique within a locale, and not `api`, `ja`, `mcp` or `projects`. A file holds at most 128 KiB of valid UTF-8; a leading UTF-8 BOM is removed.
- The first block must be a level-1 heading. Its text, with inline code and inline HTML kept as literal text, becomes the title: 1 to 80 characters after trimming. A link or image inside the level-1 heading is an error, and so is an image or a second level-1 heading anywhere. Links in the body are allowed under §5. The template renders the title, so the body never contains an `<h1>`.
- Routes: `/` is the English index and `/ja/` the Japanese index; `/<slug>` and `/ja/<slug>` are articles. The same slug in both locales makes a translation pair.
- **Discovery:** `build.rs` uses only std. It calls a discovery function in `src/discover.rs`, included through `#[path]` and unit-tested in the library, lists the entries in byte order, and writes `$OUT_DIR/content.rs` as `(path, include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/<path>")))` pairs, so a reused target directory cannot embed another tree's files. It prints `cargo::rerun-if-changed=content` and `cargo::rerun-if-changed=build.rs`. For a directory, Cargo scans the whole tree for changed mtimes, which catches additions and deletions ([build scripts](https://doc.rust-lang.org/cargo/reference/build-scripts.html)). Build time checks the filesystem rules of the first bullet with one exception: each selected file must be at most 128 KiB and valid UTF-8, because `include_str!` would otherwise stop the build with a compiler error that does not name the rule. All other rules run in `Site::load`, which checks the size again so that it stays a complete pure check.
- **Load:** `Site::load(&[(&str, &str)]) -> Result<Site, ContentError>` is pure. It checks in two phases, each in byte order of path: first every file's path, name and size and the uniqueness of numbers and slugs, then each file's title, body and links (against the complete route set). It pre-renders every page into a map from exact path to HTML. The first error of the first failing phase is reported, so a bad name in a later file wins over a missing title in an earlier one, as `senu: <path>:<line>: <reason>`, or `senu: <path>: <reason>` for a problem with the whole file such as its name, with offending text in Rust debug quoting so control characters cannot reach the terminal.

## 4. Markdown policy

Options: `Options::ENABLE_TABLES` only. The crate is the current pulldown-cmark 0.13 (0.13.6 when checked) with `default-features = false, features = ["html"]`. Every `Event` and `Tag` is matched explicitly, and any event the policy does not list fails closed. Because those extensions are disabled, their syntax normally parses as ordinary CommonMark: `[^1]`, `- [ ] x`, `$x$`, `~~x~~` and `> [!NOTE]` become escaped literal text inside ordinary paragraphs, lists and block quotes. The policy therefore does not promise to reject syntax the parser does not recognize; it promises that no event outside the table reaches the output, so the HTML never carries footnote, checkbox, math, strikethrough or definition-list markup. The policy hands only allowed events to `pulldown_cmark::html::push_html`, which escapes text and attributes.

| Input | Output |
|---|---|
| paragraphs, H2–H6, block quotes without a kind, lists, tables, emphasis, strong, text, inline code, soft and hard breaks, rules | standard HTML |
| fenced or indented code | `<pre><code>` with escaped text; the info string is dropped |
| HTML block | its source, escaped, inside `<pre><code>` |
| inline HTML | escaped text |
| link | only per §5; a link title is kept as an escaped attribute |
| image, a second H1 | error |
| any other event or tag (footnotes, task-list markers, math, metadata, definition lists, strikethrough, super- and subscript, wikilinks, alert block quotes, unknown variants) | error |

While rendering, the policy counts words for the reading time of §8: whitespace-separated runs of visible text outside code blocks for English, where line breaks and block boundaries also end a run but emphasis, links, inline code and raw HTML shown as text do not, so `foo**bar**baz` is one word, and non-whitespace characters divided by two, rounded down, for Japanese, which has no spaces between words. Inline code counts as one word. This is a rough estimate and the tutorial says so.

The output is `TrustedHtml`, a newtype with a private field that only this renderer constructs. It carries the only `askama::filters::HtmlSafe` impl in `src/`. That impl is the single trusted-HTML boundary; templates still contain no `safe` filter and no escape override.

## 5. Link policy

- **Internal:** the destination equals an existing route exactly (`/`, `/ja/`, `/<slug>` or `/ja/<slug>`). Cross-locale links are allowed.
- **External:** `https://`, then a host of 1 to 253 bytes of `[a-z0-9.-]` that contains a dot and does not start or end with `.` or `-`, then nothing or a tail that starts with `/`, `?` or `#` and is made of ASCII `!`–`~` excluding `"<>\`{|}^` and the backtick. A port or userinfo is therefore impossible, and a percent sign is allowed only in the tail.
- **Everything else is an error.** The required test table includes `javascript:alert(1)`, `JavaScript:x`, `java&#115;cript:x` (the parser decodes entities first), `data:text/html,x`, `file:///etc/passwd`, `http://example.com`, `//example.com`, `/\example.com`, `https:example.com`, `https://user@example.com`, `https://example.com:8443`, `https://%65xample.com`, `/first%2Dnote`, `/first-note?x=1`, `/first-note#top`, `#top`, `/../first-note`, `/./first-note`, `/ja/../x`, `first-note`, `./first-note`, `/first-note/`, `/ja`, `/missing`, the empty destination, and destinations containing a space, a tab, U+0000–U+001F, U+007F or `\`.

## 6. `senu site`

- The CLI accepts `senu site`. The only setting is `SENU_BIND`, with the same loopback rule and default as `serve`. Any other `SENU_*` variable, including the serve and MCP credentials, exits 2, names the variable and prints no value, so nobody mistakes the site for a protected one.
- Startup checks the configuration, then runs `Site::load` over the embedded table (an error exits 2 before binding), binds, and prints `senu: listening on http://<addr>`.
- The router has one handler that does an exact lookup of the raw request path in the pre-rendered map. No percent-decoding or normalization happens, and the query string and `Authorization` are never read. It keeps the `limits` and `response_headers` layers. An unknown path returns 404 `text/plain` `Not Found`, and a known path with a method other than GET returns 405 with `Allow: GET`. Pages are `text/html; charset=utf-8` with `Content-Security-Policy: default-src 'none'; style-src 'unsafe-inline'; form-action 'none'; frame-ancestors 'none'; base-uri 'none'`. `/projects`, `/api/*` and `/mcp` do not exist in this mode.
- Pages are Askama templates (`templates/site_index.html` and `templates/site_article.html`, sharing a base) with escaping on. Each page has `<html lang>`, a viewport meta tag, a header that links to its locale's index, a language link (to the translation pair, otherwise to the other index), `<nav>` with the locale's articles in order and `aria-current="page"`, `<main>` with one `<h1>`, previous/next links, and a footer noting an experimental seed. A fixed English and Japanese label table lives in `src/view.rs`. Styles are inline, as in `projects.html`, and include Japanese font fallbacks, visible focus, wrapping, and `pre` that scrolls on its own.

## 7. Changes to existing behaviour and files

- Spec 0001 CFG-3 and the help text gain `senu site`. Its usage line becomes `usage: senu serve | senu site | senu mcp --enable-mcp | senu --help`. WEB-3 is amended only by §4's single `HtmlSafe` boundary. `serve` routes, credentials, errors and MCP output stay byte-identical.
- The following are deleted in the same branch once the new tutorial passes, so main never maintains two tutorials: `docs/tutorial/` (book, chapters, assets, snippets, patches), `scripts/build_site.sh`, `scripts/install_mdbook.sh`, `scripts/check_site.py`, `tests/site_scripts.rs` and the CI `site` job. `scripts/check_tutorial.py`, `scripts/test_check_tutorial.py`, `tests/tutorial_checker.rs` and the tutorial half of `tests/docs_sync.rs` are rewritten. Spec 0002's status becomes "superseded by spec 0003"; its history stays.
- Both READMEs describe `senu site`, the content rules and the tutorial entry points, and still embed `scripts/quickstart.sh` byte for byte. `AGENTS.md` lists the new checks. `docs/research.md` records the four sources cited here with their check date.

## 8. Tutorial

Pedagogy borrows only the learning structure of the [Rails getting-started guide](https://guides.rubyonrails.org/getting_started.html) and the table of contents of the [Ruby on Rails Tutorial](https://www.railstutorial.org/book) (a first app with a visible running result, then a small app, then static pages built red-green-refactor, with language concepts taught inside the application), and copies no text from either: an early visible success, every concept explained before its first use, short edit-run-observe loops, exercises, and troubleshooting. One app grows across the chapters: the learner's own field-notes site, which starts as the Senu-served pages they are reading. Chapters 01 to 07 never lecture on traits, generics, lifetimes or macros; `#[test]` and `assert_eq!` each get one plain sentence. For every action the text says where to type (terminal, editor or browser), which file to edit, what should appear, and what to do when something else appears. Chapter 03 reaches the first page in the browser before any Rust is shown.

| NN-slug | Learner does | Concepts introduced |
|---|---|---|
| 01-start | sees the goal, opens a terminal | terminal, prompt, command, `pwd`, `ls`, `cd` |
| 02-install | follows the official installation steps, installs git, clones, runs `cargo --version` | rustup, cargo, git clone, linker |
| 03-first-page | runs `cargo run --quiet -- site`, opens `http://127.0.0.1:3000/`, stops it with Ctrl+C | build, run, server, URL, port, loopback |
| 04-first-note | edits `50-sample-note.md`, adds `51-my-note.md`, reruns, sees the page and navigation | Markdown basics, name rule, embedding at build time |
| 05-when-senu-says-no | triggers missing-H1, bad-name and `javascript:` errors, sees raw HTML shown as text, adds and deletes `52-scratch.md`; troubleshooting | error messages, exit status, rebuild |
| 06-first-function | rebuilds `reading_minutes` in `src/reading.rs`, the function behind the reading time on every article page: replaces the file with a first attempt that compiles (`words / 200`) and four tests (0, 1, 200 and 201 words), sees an assertion fail (red), fixes the rounding (green), and sees the minutes on the note's page | `fn`, parameters and return type, `usize`, `let` vs `const`/`let`, `if`/`else` as a value, `/` and `%`, overflow, `#[test]`, `assert_eq!`, `mod`/`use` vs `import`/`export` |
| 07-next-steps | reviews what they built; finds further reading | — |
| 50-sample-note | starter field note edited in 04 | — |
| 90-appendix-catalogue | optional: `list_projects`, HTML/JSON adapters, the rename exercise migrated from spec 0002 | principals, tenants, HTTP auth |
| 91-appendix-mcp | optional: MCP over stdio | MCP |

**Reading time.** `reading_minutes(words: usize) -> usize` returns the words divided by 200, rounded up, so 0 words give 0, 1 to 200 give 1 and 201 give 2. It computes the quotient and remainder and never adds before dividing, so `usize::MAX` cannot overflow. The article template shows "About N min read" or "約 N 分で読めます" under the title when N is greater than 0. The repository ships the finished function and its tests; at the end of chapter 06 the learner's file matches it byte for byte, and the checker verifies that. The chapter states the estimate's limits for Japanese (§4) and shows `(words + 199) / 200` only as the overflow pitfall in an exercise.

`content/ja/` has the same `NN-slug` set. Japanese learners create their notes under `content/ja/`. Chapter 02 links the [Rust Book installation steps](https://doc.rust-lang.org/book/ch01-01-installation.html) for Linux and macOS, including the C linker or `xcode-select --install`. WSL users install a Linux distribution by following Microsoft's documentation and then the Linux steps; native Windows is not covered. The chapter says plainly that only GitHub-hosted Ubuntu is tested, and it lists `rustup toolchain install 1.99.1` as the fix when the pinned toolchain is missing. The tutorial never runs installers or changes system settings for the learner.

**Check blocks.** Fenced info strings carry the checks. The renderer drops them, so readers see plain code. Block kinds:

- `sh check=<name> [exit=<n>] [server]` is a command to run.
- `text check=<name>` is that command's exact expected output, stdout and stderr merged.
- `<lang> file=<path>` is the full content of a file the learner creates or replaces. Paths stay under `content/` or `src/` and contain no `..`, and the preceding paragraph must name the path in backticks.
- `diff patch` is a unified diff applied with `git apply`; only the appendix uses it.
- `sh manual` is a command the checker cannot run, such as cloning; it is listed as not run.

A `server` command runs in the background with `SENU_BIND=127.0.0.1:0`. Later blocks see the real address in place of `127.0.0.1:3000`, and the checker stops the server as Ctrl+C would before the next server block and at the end. Learners look at pages in the browser; to make the same observation checkable, the tutorial also shows `curl` in a second terminal, explained before its first use, and the checker runs those commands. Before comparison the output is normalized for ANSI colours, ports, `finished in <t>s`, the thread id that Rust prints in a panic message, and `deps/senu-<hash>`, and for nothing else. Builds use `--quiet`, and the tutorial warns that the first build is silent for several minutes.

**Checker.** `python3 scripts/check_tutorial.py` runs the English and then the Japanese articles' blocks, in order, each language in its own fresh copy unpacked from HEAD. It refuses to run while tracked files differ from HEAD. It reuses only dependency artifacts and removes Senu's own build outputs from the copied cache. Each command runs in its own process group with a 1800 s timeout. The real tree's digest must be unchanged at the end. No step resets, restores or deletes a file that the same chapter did not create. CI runs the checker in its own job.

## 9. Acceptance criteria

- **CNT-1** A test compares the embedded table with a fresh listing of `content/` (selection rule of §3, byte order). Discovery unit tests refuse a symlinked `content` or `content/en`, a symlink or directory named `x.md` and a non-UTF-8 name, ignore `.x.md.swp` and `x.md~`, and return the same order for any creation order.
- **CNT-2** In the checker's copy, without `cargo clean`, rerunning the documented command after adding a note serves it and lists it in the navigation, after editing it serves the change, and after deleting it returns 404 and drops it from the navigation.
- **ART-1** Name, size, BOM, duplicate-number, duplicate-slug and reserved-slug cases are table-tested; each error names the file.
- **ART-2** Title rules: missing or late H1, an empty or 81-character title, a second H1, and a link or image in the H1 fail with file and line. Inline HTML in a title renders as escaped text.
- **ART-3** Navigation follows `NN`, marks the current page, and has correct previous/next links at both ends. The language link goes to the pair or to the other index, `lang` matches the locale, and labels are localized.
- **ART-4** Exact routing: each of `/First-note`, `/first-note/`, `/first%2Dnote`, `/ja`, `//first-note`, `/en/first-note`, `/index.html` and `/content/en/51-x.md` returns 404. POST on a page returns 405. A query string does not change the response.
- **SAFE-1** `<script>`, `<img onerror=…>`, an HTML block, inline HTML, and markup inside code spans and fenced code all render as escaped text, and an info string such as `"><script>` is dropped from the output. A test parses the output into tags and attributes and finds only allow-listed tags and no attribute whose name starts with `on`; escaped example text such as `&lt;img onerror=x&gt;` is allowed.
- **SAFE-2** Every destination in §5's table fails with file, line and the destination in debug quoting; the allowed internal and https forms pass.
- **SAFE-3** Images and a second H1 fail. A unit test feeds unsupported events (for example a footnote reference and a task-list marker) straight into the policy and sees them fail closed. The disabled extensions' syntax renders as literal text with only allow-listed tags, so no `<input`, `<sup`, `<math`, `<del`, `<dl` or `class=` appears. Enabling another `Options` flag fails a test.
- **SAFE-4** `HtmlSafe` appears exactly once in `src/`, on `TrustedHtml`, whose field is private. The WEB-3 template and config test still passes. A page contains the body HTML unescaped exactly once and the title escaped exactly once.
- **SITE-1** E2E: the real binary, started from a new empty temporary directory with a cleared environment and `SENU_BIND=127.0.0.1:0`, serves `/`, `/ja/`, an English and a Japanese article with the expected titles; `/projects`, `/api/projects` and `/mcp` are 404 plain text; no response has `WWW-Authenticate`; sending `Authorization` changes nothing; `Cache-Control`, `X-Content-Type-Options` and the CSP are set.
- **SITE-2** `senu site` with `SENU_WEB_USER`, `SENU_API_TOKEN`, `SENU_MCP_TOKEN`, `SENU_X` or a non-loopback bind exits 2, names the variable, prints no value and no listening line. An occupied port exits 1, `senu site extra` exits 2, and `--help` lists `site`.
- **SITE-3** The startup function, given invalid fixture content, exits 2 with the first error in the phase and path order of **Load** (§3) and never binds.
- **REG-1** All spec 0001 tests pass with only the §7 changes to usage and help text. `GET /` on `serve` is still 404, `tools/list` is unchanged, and the quickstart passes.
- **TP-1** Unit tests outnumber adapter tests, which outnumber E2E tests. This slice adds at most two E2E test functions.
- **NAV-1** Both locales contain the §8 table's `NN-slug` set, and every chapter has a working link to its pair. A sync test checks that both languages have the same sequence of block kinds, names, exit statuses and file paths, and that `sh`, `diff` and Rust `file` blocks are identical once `content/ja/` and `/ja/` are mapped to their English forms. Learner notes outside the table are ignored.
- **EX-1** The checker passes in both languages. Every output matches exactly after §8's normalization, and every expected failure has its listed exit status and exact message. The red steps of chapter 06 fail on assertions, not on compile errors.
- **EX-2** Checker tests show that an untracked note and a stale `senu` artifact in the real tree never reach the copy, that a dirty tracked file is refused, and that the real tree is unchanged.
- **UI-1** A person checks the site in the installed Chromium-based browser and the installed Firefox; a browser that is not installed is recorded as not run, and none is installed for this check. The check covers 1280×800 and responsive mode at 375 px and 320 px wide: landmarks and headings in the accessibility tree, keyboard order and visible focus, no horizontal page scroll outside `pre`, Japanese wrapping, both language links, and 200 % zoom. The result goes in `docs/development.md` with the browser versions. Physical phones and other browsers are listed as untested.
- **TUT-1** A reviewer other than the implementer records a concept ledger proving each concept is introduced before use, the first visible success in chapter 03 before any Rust is read, original prose in both languages, and natural Japanese.
- **CI-1** CI keeps `contents: read` and SHA-pinned actions and runs fmt, clippy, tests, the quickstart and the tutorial checker. `scripts/snapshot_site.py <empty-dir>` starts the site with `cargo run --quiet --locked -- site` on a free loopback port, crawls it from `/` and `/ja/`, fails on any internal link that does not return 200, writes `index.html`, `<slug>.html` and `ja/…`, rewrites each internal route in an `href` to the relative `.html` file, and verifies that every rewritten target and `#fragment` exists, so the directory can be browsed with any plain static file server on localhost. CI uploads that directory as an artifact. There is no deploy step.

## 10. Delivery

Each behaviour commit starts with a failing test and records concise RED and GREEN evidence in `docs/development.md`: command, exit status and the decisive lines. Local builds use a resource-bounded serial wrapper; CI uses plain `cargo`. Commits are small, in English, and carry only generic attribution. The order is:

1. discovery and `build.rs` (CNT-1)
2. names and titles (ART-1, ART-2)
3. Markdown policy and `TrustedHtml` (SAFE-1, SAFE-3, SAFE-4)
4. links (SAFE-2)
5. site router and views (ART-3, ART-4)
6. `senu site` wiring (SITE-1 to SITE-3)
7. English chapters, Japanese chapters and the checker (NAV-1, EX-1, EX-2, CNT-2)
8. mdBook retirement, READMEs, snapshot and CI (CI-1, REG-1)
9. browser check (UI-1)

Independent code and pedagogy reviews happen before merge, and their fixes follow the same red-green rule.

## 11. Decisions, risks and known limits

- **R1:** Askama documents `HtmlSafe` as a speed-up hint that may still escape. That would fail safe, and SAFE-4 detects it. If it happens, `src/view.rs` renders the page in two Askama halves around the fragment, which is still the one boundary.
- **R2:** If the pinned pulldown-cmark API differs from §4, keep §4's behaviour and record the difference.
- **R3:** Cargo detects changes by mtime. If a coarse clock misses one, CNT-1 fails in `cargo test`, and troubleshooting suggests `cargo clean -p senu`.
- **R4:** Two language runs lengthen CI; the tutorial job gets its own timeout.
- **Limits:** no search, print view, images or heading anchors. Content changes need a restart. External links are checked for form, not liveness. The snapshot is verified as local files only, not on any host. Public hosting is out of scope; if a host refuses deployment, the slice is complete with the local site and the CI artifact, and no workaround is attempted.
