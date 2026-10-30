<nav class="senu-lang" aria-label="Language"><a href="ja/operation.html" hreflang="ja" lang="ja">日本語</a></nav>

# One shared operation

## Read `list_projects`

Open [`src/operation.rs`](https://github.com/efg-technologies/senu/blob/main/src/operation.rs).
`list_projects` takes the catalogue, a principal and the decoded request
arguments, and returns either a `ProjectList` or an `OpError`. It does no I/O
and reads no clock or global state, so a unit test can call it directly. Its
numbered comments match these steps:

1. **Authorize first.** A principal without the `ListProjects` permission
   gets `Forbidden` before its input is read, so a caller without the
   permission learns nothing about which input would be valid.
2. **Decode strictly.** The only accepted argument is an optional string `q`.
   Any other field, such as `tenant`, returns `InvalidRequest`.
3. **Normalize and bound the search.** `q` is trimmed. An empty search means
   no search; a control character, or more than 80 characters after
   trimming, returns `InvalidSearch`.
4. **Stay inside the tenant.** The search reads only
   `catalogue.for_tenant(principal.tenant())`.
5. **Sort by id**, so every adapter returns the same order.

Principals are never built from a request. The three that the adapters use,
`demo_web`, `demo_api` and `demo_mcp`, are constants in the same file, each in
tenant `acme` with the one permission. The catalogue itself is a constant
array in [`src/catalogue.rs`](https://github.com/efg-technologies/senu/blob/main/src/catalogue.rs).

## Start the server

Open a second terminal in the clone and start `senu serve` on port 3000 with
the demo credentials. They are fixed demo values for a loopback-only server;
choose your own for anything else. Senu refuses to bind anything but a
loopback address such as `127.0.0.1`.

```sh
{{#include ../commands/serve.sh:serve}}
```

After the build, the server prints one line and keeps running:

```text
{{#include ../commands/serve.out:serve}}
```

Leave it running for this chapter and the next, and stop it with Ctrl-C
when you are done.

## Call it from outside

Back in the first terminal, search for `launch` through the JSON API. The
`-w` option prints the HTTP status after the body.

```sh
{{#include ../commands/operation.sh:launch}}
```

```text
{{#include ../commands/operation.out:launch}}
```

**Tenant isolation.** "Globex Launch" matches `launch` too, but it belongs to
tenant `globex`. Searching for its name directly still finds nothing, because
the API principal is in `acme`:

```sh
{{#include ../commands/operation.sh:other-tenant}}
```

```text
{{#include ../commands/operation.out:other-tenant}}
```

The request cannot choose a tenant either. `tenant` is not an argument of
`list_projects`, so step 2 rejects it:

```sh
{{#include ../commands/operation.sh:pick-tenant}}
```

```text
{{#include ../commands/operation.out:pick-tenant}}
```

**Validation.** `%07` is the BEL control character, which step 3 rejects.
The error is a fixed code and does not repeat your input:

```sh
{{#include ../commands/operation.sh:bad-search}}
```

```text
{{#include ../commands/operation.out:bad-search}}
```

**Authentication.** Without a token the adapter never calls `list_projects`:

```sh
{{#include ../commands/operation.sh:no-token}}
```

```text
{{#include ../commands/operation.out:no-token}}
```

You cannot reach `Forbidden` from outside, because every demo principal has
the permission. The unit tests at the bottom of `src/operation.rs` build a
principal without it, and the HTTP and MCP adapter tests inject one, to check
that each transport maps the error correctly.
