# Authenticate callers

Authentication in Senu is one trait: it turns a request's headers into a server-side principal, or into nothing. HTML pages and the JSON API each get their own authenticator.

## What is a principal?

A `Principal` is who is calling, as the server sees it: a name for your logs, a tenant and a list of permissions. No request can carry a tenant or a permission. They exist only in the principal, and only your authenticator creates it.

```rust excerpt=examples/inventory.rs
pub const VIEW_STOCK: Permission = Permission::new("view_stock");

pub const NORTH_CLERK: Principal =
    Principal::new("north-clerk", TenantId::from_static("north"), &[VIEW_STOCK]);
```

`Principal::new` builds a constant, for a service account. `Principal::owned(name, tenant, permissions)` builds one at run time, for a user whose record you just loaded. Map stored roles to your permission constants in your own code; Senu never reads role names from a request.

## Which authenticators are built in?

- `auth::Bearer::new(token, principal)` accepts `Authorization: Bearer <token>`. Add more tokens with `.and(token, principal)`, each with its own principal.
- `auth::Basic::new(user, password, principal)` accepts HTTP Basic. It sends the password with every request, so use it only over TLS, and prefer your own sessions for people.

Both accept exactly one `Authorization` header (two are treated as none), compare secrets in constant time for equal lengths, and never log or echo a credential. A `Secret` holds 1 to 256 visible ASCII characters and prints as `Secret(<redacted>)`.

You attach them when you build the state:

```rust excerpt=examples/inventory.rs
    let state = AppState::builder(Inventory::sample())
        .html_auth(Basic::new(
            Secret::new("clerk").map_err(|err| err.to_string())?,
            secret("INVENTORY_WEB_PASSWORD")?,
            NORTH_CLERK,
        ))
        .api_auth(ApiKeys::new(vec![(
            secret("INVENTORY_API_KEY")?,
            NORTH_CLERK,
        )]))
        .build();
```

## How do you write your own authenticator?

Implement `Authenticate`. This one reads an API key from a custom header:

```rust excerpt=examples/inventory.rs
impl Authenticate for ApiKeys {
    fn authenticate<'a>(&'a self, headers: &'a HeaderMap) -> AuthFuture<'a> {
        Box::pin(async move {
            // Exactly one header, as for `Authorization`.
            let mut values = headers.get_all("x-api-key").iter();
            let (Some(presented), None) = (values.next(), values.next()) else {
                return None;
            };
            // Compare with every key, so the time does not show which one
            // matched.
            let mut found = None;
            for (key, principal) in &self.0 {
                if key.matches(presented.as_bytes()) && found.is_none() {
                    found = Some(principal.clone());
                }
            }
            found
        })
    }

    fn challenge(&self) -> HeaderValue {
        HeaderValue::from_static("ApiKey realm=\"inventory\"")
    }
}
```

`authenticate` is asynchronous, so it can ask a database or a cache. It receives only the headers: the path, the query string and the body are never passed in, so a credential in a URL cannot be read even by mistake. `challenge` is the `WWW-Authenticate` value sent with the 401.

The same trait covers the common production cases:

- **Session cookies:** read the `Cookie` header, look the session up, and build the principal with `Principal::owned`.
- **API keys in a database:** hash the presented key, look the hash up, and map the stored scopes to permissions.
- **A trusted proxy:** read the header that your gateway sets after it verified a JWT, and accept it only from that gateway's network.

## What happens when an authenticator is missing?

The table is refused. `into_router` returns `RouteError::NoAuthenticator` for an HTML route without `html_auth` or an API route without `api_auth`, so a forgotten line cannot open a route. Assets and `custom` routes never authenticate; a `custom` handler that needs it must do it itself.

## Does Senu protect against CSRF?

Version 0.2 has no write operations, so there is nothing for a forged request to change. Reads still deserve care: Senu sends no CORS headers, so a script on another origin cannot read a response, and every response is sent with `Cache-Control: no-store`. When write operations arrive in a later version, they will come with their own rules.

## What should you check before going live?

1. Secrets come from the environment or a secret store, are long and random, and differ per client.
2. TLS terminates in front of the router, and plain HTTP is redirected or closed.
3. Your authenticator takes the same path for a wrong user and a wrong password, and does not log credentials.
4. Each client's principal holds only the permissions it needs.
5. [The contract tests](/testing) pass on your table.

Next: [expose operations as MCP tools](/mcp-tools).
