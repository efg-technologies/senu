# Senu Spec 0002: Tutorial and Documentation Site

Status: implemented, 2026-10-30. This spec covers the tutorial in `docs/tutorial` and the scripts that check and build it. It does not change Senu's behaviour: the product source and its tests keep the spec 0001 catalogue.

## 1. Boundaries

- **In scope:** six short chapters in English (canonical, `docs/tutorial/src`) and Japanese (`docs/tutorial/src/ja`), a checked rename exercise, and one static mdBook site with English at the root and Japanese under `ja/`.
- **Out of scope:** new operations, new transports or sample apps, framework features, automatic deployment, hosting configuration, a custom domain, and any change to how Senu authenticates or serves MCP.
- The tutorial describes Senu as it is: fixed read-only demo data, loopback-only HTTP without TLS, demo credentials, MCP over local stdio only when enabled, and no database, persistence, production login, remote MCP, scaffolding or ORM. Serving the site over HTTPS does not add TLS to the demo.

## 2. Single sources

- Commands live in `docs/tutorial/commands/*.sh` and `docs/tutorial/exercise/steps.sh`, cut into mdBook anchor sections. Expected output lives in same-named `.out` files with the same sections. Diffs live in `docs/tutorial/exercise/*.patch`. `scripts/quickstart.sh` is included as it is.
- Both languages include those files. Shell blocks in chapters contain only `{{#include}}` lines.
- `scripts/check_mcp.py` stays the only MCP client.

## 3. The exercise

Rename `acme-001` from "Apollo Launch" to "Artemis Launch" on a branch: change the expected tuple in `demo_catalogue_matches_the_spec_table` and see that test fail on its assertion; change `DEMO_PROJECTS` and see it pass; run the whole suite and see the two maintained HTTP contract fixtures in `tests/adapter_http.rs` fail (the JSON body for `launch` and the `apollo+launch` search); update them; see the whole suite, `adapter_parity` and the real quickstart pass with ids `acme-001` and `acme-003`; restore the files. Synthetic fixtures in `src/view.rs` and `tests/quickstart_check.rs` do not read the catalogue and stay unchanged.

## 4. Acceptance criteria

- **TUT-1** The English and Japanese books have the same chapter files in the same order, and each chapter includes the same snippet files and sections in the same order (`tests/docs_sync.rs`).
- **TUT-2** `scripts/check_tutorial.py commands` starts the server from the `serve` section on a free loopback port and runs every other command section except `git clone`; each exits 0 and prints exactly its expected section.
- **TUT-3** `scripts/check_tutorial.py exercise` runs every exercise section, in order, in a temporary git copy that reuses a copy of `target/debug`. Each step has its listed exit status and expected lines; the red steps fail on test assertions, not on patch, compile or setup errors; nothing in the parity output still says "Apollo"; the copy ends clean on `main`; and the real working tree is unchanged.
- **TUT-4** `scripts/build_site.sh` accepts only mdBook 0.5.4, refuses an output directory that exists and is neither empty nor an earlier build, accepts a base path that starts and ends with `/` (default `/`), and writes `.nojekyll` and `source-revision.txt`. `scripts/install_mdbook.sh` downloads only the official x86_64 Linux tarball and checks its SHA-256 before unpacking, without sudo or a global install.
- **TUT-5** `scripts/check_site.py` fails on a missing required file, a wrong `lang`, an unexpanded directive or anchor, a missing local file or fragment, differing chapters or tables of contents, a missing language link, a non-https or unlisted external host, and a repository link that is not `blob/main` or `tree/main` to a path that exists (`tests/site_scripts.rs`).
- **TUT-6** CI keeps `permissions: contents: read` and runs TUT-1 to TUT-5.

## 5. Design

The site uses mdBook's own navigation, search, print page and repository links, with one extra stylesheet (`docs/tutorial/assets/senu.css`) that sets the palette, a 70ch measure, Japanese font fallbacks, visible focus and wrapping. The only custom component is the architecture figure in chapter 1: plain HTML (one operation above three adapters, MCP drawn dashed because it is opt-in) that stacks on narrow screens. An SVG was rejected because its text would shrink at phone widths and need translating inside the image. The language link is one line at the top of each chapter, because mdBook's templates cannot turn a chapter's `.md` path into its `.html` path; the site check verifies every pair.

## 6. Known limits

- The Japanese pages use mdBook's built-in search index. How well it finds Japanese words has not been checked in a browser.
- Line numbers in the patches must match the source. If `src/catalogue.rs` or `tests/adapter_http.rs` changes, TUT-3 fails until the patches and expected output are updated.
- No public site URL exists yet; deployment is a separate, manual step.
