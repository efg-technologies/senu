# Questions and comparisons

Short answers about what Senu is, how it compares with Axum, rmcp and Loco, and what version 0.2 can and cannot do.

## What is Senu?

Senu is an open-source Rust library that serves one declared function as an HTML page, a JSON API and an MCP tool. You write the function once with its permission and parameters; Senu runs the same authentication, authorization and validation steps in the same order on all three, and ships the tests that prove it.

## Is Senu ready for production?

Version 0.2 is the first release meant for use as a library: the API is documented, the security contract is published as tests, and an external crate can bring its own state, authentication and asynchronous work. It is not audited, it covers read-only operations only, and as a 0.x crate its API may change in a minor version. Use it for read-only services after your own review.

## How is Senu different from using Axum alone?

Senu is built on Axum and returns an `axum::Router`, so it is an addition, not a replacement. Axum gives you routing and extractors and leaves the order of checks to each handler. Senu fixes that order for declared operations, adds the MCP side, and tests both. Use Axum alone for writes, uploads, streaming and WebSockets, and merge a Senu router in for the read-only operations you also want as tools.

## How is Senu different from using rmcp alone?

rmcp is the official Rust SDK for MCP, and Senu uses it for the MCP transport. With rmcp alone you write tools, and you get the whole protocol: resources, prompts and the HTTP transport. Senu covers less of MCP (read-only tools over stdio) but gives each tool an HTML page and a JSON route from the same function, with one permission check. Use rmcp alone when MCP is all you need or you need more of the protocol.

## How is Senu different from Loco?

Loco is a full framework in the spirit of Rails: an ORM, migrations, generators, background workers and authentication. Senu is a small library with one idea, the declared operation, and no database layer, generators or workers. Choose Loco to build a whole application quickly; choose Senu to add consistent read access for people, programs and assistants to an application you already have.

## Does Senu support write operations?

Not in version 0.2. Every operation is read-only and served by `GET`, and every MCP tool is marked read-only. Writes need rules of their own, such as CSRF protection for browsers and confirmation for assistants, and will come with a separate specification. Until then, write endpoints belong in your own Axum routes.

## Does Senu support MCP over HTTP?

Not in version 0.2. MCP tools run over stdio, where the server trusts the local process that starts it. A remote transport must authenticate every request and is planned as a separate specification.

## Can AI assistants call a Senu service safely?

Senu lowers the risk in three ways: tools are read-only, each call runs as a principal that holds only the permissions you gave it, and the tenant comes from that principal, not from the assistant's arguments. An assistant can still read whatever that principal may read, so give the MCP principal the narrowest permissions that do the job.

## Which authentication methods does Senu support?

Bearer tokens and HTTP Basic are built in. Everything else is an implementation of one trait, `Authenticate`, which receives the request headers and returns a principal: session cookies, API keys from a database, or a header set by a trusted proxy. See [authenticate callers](/authentication).

## Does Senu need a database?

No. Your state is any type you choose, and Senu passes a reference to it into every operation. With a database, the state holds the pool and the operations are asynchronous.

## Which Rust version does Senu need?

Rust 1.99 or later, with the 2024 edition. It depends on Axum 0.8, Askama 0.16, rmcp 3 and Tokio 1.

## How do you test a Senu application?

Call `senu::contract::check` with your route table and tool list. It sends real requests and verifies seven rules, from "wrong credentials are 401 before anything else" to "the JSON API and the MCP tool give the same answer". See [test with the contract](/testing).

## What license does Senu use?

Apache-2.0. The code is developed in the open at https://github.com/efg-technologies/senu.

## Where is the API reference?

On https://docs.rs/senu. This guide explains the ideas; the reference documents every type and function.

Back to [the overview](/overview).
