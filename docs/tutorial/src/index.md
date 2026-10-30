<nav class="senu-lang" aria-label="Language"><a href="ja/index.html" hreflang="ja" lang="ja">日本語</a></nav>

# The finished app

This tutorial walks through [Senu](https://github.com/efg-technologies/senu),
a small experimental Rust program, from cloning it to changing it with a
failing test first. You run it, read the one operation it shares, see the
same result as an HTML page, a JSON API and an MCP tool, and then rename a
project and follow the tests from red to green.

## What you are looking at

Senu has one pure function, `list_projects`. It lists the projects of a fixed
demo catalogue: five projects in two tenants (`acme` and `globex`), compiled
into the binary. Three adapters call it. Each one checks its own credential,
picks a principal (the caller identity) on the server side, calls the same
function and renders the result in its own format.

<figure class="senu-flow" aria-labelledby="senu-flow-caption">
<div class="senu-flow-core">
<p class="senu-flow-title"><code>list_projects</code> in <code>src/operation.rs</code></p>
<p>Checks the permission, decodes the optional search <code>q</code>, searches only the principal's tenant of the fixed catalogue, and sorts by id.</p>
</div>
<ul class="senu-flow-adapters">
<li><p class="senu-flow-title">HTML page</p><p><code>GET /projects</code>, HTTP Basic, an escaped Askama template</p></li>
<li><p class="senu-flow-title">JSON API</p><p><code>GET /api/projects</code>, Bearer token, <code>{"projects": [...]}</code></p></li>
<li class="senu-flow-optin"><p class="senu-flow-title">MCP tool, opt-in</p><p><code>senu mcp --enable-mcp</code> over stdio, token from the environment, off unless enabled</p></li>
</ul>
<figcaption id="senu-flow-caption">One operation, three adapters. <code>senu serve</code> runs the HTML page and the JSON API on a loopback address. MCP runs only as a separate <code>senu mcp --enable-mcp</code> process that speaks over its standard input and output.</figcaption>
</figure>

A test, [`tests/adapter_parity.rs`](https://github.com/efg-technologies/senu/blob/main/tests/adapter_parity.rs),
checks that all three adapters return exactly what `list_projects` returns,
in the same order.

## What you will do

1. Install the prerequisites, clone the repository and run the quickstart.
2. Read `list_projects` and see authorization, input validation and tenant
   isolation from the outside.
3. Compare the HTML page, which escapes markup, with the JSON API, which
   returns data.
4. Start the MCP tool, which is off unless you ask for it, and check its reply.
5. Rename one project: change the test first, watch it fail, change the
   catalogue, and follow the other tests that pin the old name.

Every command and expected output in this tutorial comes from files in
[`docs/tutorial/commands`](https://github.com/efg-technologies/senu/tree/main/docs/tutorial/commands)
and [`docs/tutorial/exercise`](https://github.com/efg-technologies/senu/tree/main/docs/tutorial/exercise).
The English and Japanese pages include the same files, and
`scripts/check_tutorial.py` runs them and compares the output, so what you
read is what was last checked.

## What Senu is not

Senu is a runnable seed, not a product, and it is not audited security
software. The catalogue is fixed and read-only: there is no database, no
persistence and no way to change data. HTTP Basic here is a demo credential,
not a production login, and there are no sessions, users or roles. The server
has no TLS and listens only on a loopback address, so credentials travel in
cleartext on your own machine. This tutorial site is served over HTTPS by a
static host, but that does not add TLS to the demo you run locally. MCP runs
only over local stdio; there is no remote MCP and no OAuth. There are no
generators, scaffolding or ORM, and no published crate.

The [README](https://github.com/efg-technologies/senu/blob/main/README.md)
lists every route and setting, and
[spec 0001](https://github.com/efg-technologies/senu/blob/main/docs/specs/0001-project-catalogue.md)
is the specification the code was built and tested against.
