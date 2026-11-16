# Deploy and operate

Senu gives you an ordinary `axum::Router`. You bind it, put TLS in front of it and add the layers your service needs; Senu adds none of those and listens nowhere by itself.

## What does Senu do on every request?

- Refuses a request target longer than 2048 bytes (414), any body on a read (413) and a body over the limit on a write (413, 64 KiB by default), before routing.
- Answers unknown paths with 404. A read answers only `GET` and a write only `POST`; anything else is 405 with `Allow`.
- Refuses a write that a browser sends from another site (403), before authentication.
- Sends `Cache-Control: no-store` and `X-Content-Type-Options: nosniff` on every response.
- Sends a Content-Security-Policy on every HTML page: by default no scripts, images or frames, only same-origin stylesheets and form targets.
- Never reads a credential from the query string, and never repeats input or secrets in an error.

## What stays with you?

| Concern | Where it belongs |
|---|---|
| TLS and HSTS | A reverse proxy or load balancer in front of the router |
| Request logging and tracing | A tower layer, such as `tower_http::trace::TraceLayer` |
| Timeouts | A tower layer, such as `tower_http::timeout::TimeoutLayer` |
| Rate limiting and lockout | Your gateway, or a tower layer |
| Session storage, login pages and OAuth | Your [authenticator](/authentication) and your own routes |
| Audit logging | Your operations; `Principal::name` is there for it |
| Health checks | A `custom` route |
| Graceful shutdown | `axum::serve(...).with_graceful_shutdown(...)` |

## How do you bind and serve the router?

```rust excerpt=examples/inventory.rs
    let router = endpoints()
        .into_router(state)
        .map_err(|err| err.to_string())?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .map_err(|err| format!("inventory: cannot listen: {}", err.kind()))?;
    eprintln!("inventory: listening on http://127.0.0.1:3000");
    axum::serve(listener, router)
        .await
        .map_err(|_| "inventory: the server stopped".to_owned())
```

Layers go around the router in the usual Axum way, for example `router.layer(TraceLayer::new_for_http())`. To serve Senu's routes next to your own Axum routes, merge the routers with `Router::merge`. Layers you add must not log the `Authorization` or `Cookie` headers. The cross-site check compares `Origin` with `Host`, so a proxy in front must pass the original `Host` on, or you name your public origin with `Endpoints::trusted_origin`.

## How should secrets be handled?

Read them from the environment or a secret store at startup and turn them into `Secret` values at once. `Secret::new` refuses anything but 1 to 256 visible ASCII characters, and a `Secret` never appears in `Debug` output. When configuration is wrong, name the variable in the message and never its value, as the example does. Fail before binding: a service that cannot authenticate should not start.

## How do you serve several tenants?

Put the tenant in the principal when you authenticate, and filter by `caller.tenant()` inside every operation. Do not accept a tenant as a parameter. The contract tests check that an unknown parameter such as `tenant=other` is refused, and your own tests should check that one tenant's key cannot read another tenant's data, as `tests/example_inventory.rs` does.

## What should a production checklist contain?

1. TLS in front; no plain HTTP reachable from outside.
2. Long, random, per-client secrets, loaded from a secret store.
3. Each principal holds only the permissions it needs.
4. Timeouts, request logging and rate limits are in place as tower layers or at the gateway.
5. Operations that wait are `Run::Async`; nothing blocks the executor.
6. Writes that touch several records use a transaction, and a retried delete does no harm.
7. `OpError::Internal` is logged with its cause on the server.
8. [The contract tests](/testing) and your own tenant-isolation tests run in CI.
9. Dependencies are pinned with `Cargo.lock` and checked with `cargo audit` or a similar tool.

## Is Senu audited?

No. Senu is tested, and its contract is published as tests you can run, but it has had no independent security audit, and it makes no performance claim. Review it as you would any dependency that sits on your authentication path. Report a vulnerability privately, as `SECURITY.md` in the repository describes.

Next: [test with the contract](/testing).
