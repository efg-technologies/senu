# Questions and comparisons

Short answers about what Senu is, how it compares with Axum, rmcp and Loco, and what version 0.3 can and cannot do.

## What is Senu?

Senu is an open-source Rust library that serves one declared function as an HTML page, a JSON API and an MCP tool. You write the function once with its permission and parameters; Senu runs the same authentication, authorization and validation steps in the same order on all three, and ships the tests that prove it.

## Is Senu ready for production?

Senu is published as a library: the API is documented, the security contract is published as tests, and an external crate can bring its own state, authentication, asynchronous work and, since 0.3, write operations. It is not audited, and as a 0.x crate its API may change in a minor version. Use it after your own review, and run the contract over your own routes.

## How is Senu different from using Axum alone?

Senu is built on Axum and returns an `axum::Router`, so it is an addition, not a replacement. Axum gives you routing and extractors and leaves the order of checks to each handler. Senu fixes that order for declared operations, adds the cross-site check for writes and the MCP side, and tests all of it. Use Axum alone for uploads, streaming, WebSockets and HTML forms, and merge a Senu router in for the operations you also want as tools.

## How is Senu different from using rmcp alone?

rmcp is the official Rust SDK for MCP, and Senu uses it for the MCP transport. With rmcp alone you write tools, and you get the whole protocol: resources, prompts and the HTTP transport. Senu covers less of MCP (tools over stdio) but gives each tool a JSON route, and each read a page, from the same function, with one permission check. Use rmcp alone when MCP is all you need or you need more of the protocol.

## How is Senu different from Loco?

Loco is a full framework in the spirit of Rails: an ORM, migrations, generators, background workers and authentication. Senu is a small library with one idea, the declared operation, and no database layer, generators or workers. Choose Loco to build a whole application quickly; choose Senu to add consistent read access for people, programs and assistants to an application you already have.

## Does Senu support write operations?

Yes, since version 0.3. A write is declared like a read with an effect, served by `POST` with a JSON body of typed values, checked against cross-site requests before authentication, and listed as an MCP tool that is not marked read-only. HTML form posts and file uploads are not declared operations. See [declare a write operation](/writes).

## Does Senu support MCP over HTTP?

Not in version 0.3. MCP tools run over stdio, where the server trusts the local process that starts it. A remote transport must authenticate every request and is planned as a separate specification.

## Can AI assistants call a Senu service safely?

Senu lowers the risk in three ways: each call runs as a principal that holds only the permissions you gave it, the tenant comes from that principal and not from the assistant's arguments, and every tool says in its annotations whether it only reads, so a client can ask before a write. An assistant can still do whatever that principal may do, so give the MCP principal the narrowest permissions that do the job, and no write permission unless you mean it.

## Which authentication methods does Senu support?

Bearer tokens and HTTP Basic are built in. Everything else is an implementation of one trait, `Authenticate`, which receives the request headers and returns a principal: session cookies (with `auth::cookie` and a login redirect), API keys from a database, or a header set by a trusted proxy. See [authenticate callers](/authentication).

## Does Senu need a database?

No. Your state is any type you choose, and Senu passes a reference to it into every operation. With a database, the state holds the pool and the operations are asynchronous.

## Which Rust version does Senu need?

Rust 1.99 or later, with the 2024 edition. It depends on Axum 0.8, Askama 0.16, rmcp 3 and Tokio 1.

## How do you test a Senu application?

Call `senu::contract::check` with your route table and tool list. It sends real requests and verifies twelve rules, from "wrong credentials are 401 before anything else" to "a cross-site write is refused" and "the JSON API and the MCP tool give the same answer". See [test with the contract](/testing).

## What license does Senu use?

Apache-2.0. The code is developed in the open at https://github.com/efg-technologies/senu.

## Where is the API reference?

On https://docs.rs/senu. This guide explains the ideas; the reference documents every type and function.

Back to [the overview](/overview).
