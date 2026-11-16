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

    fn challenge(&self) -> Option<HeaderValue> {
        Some(HeaderValue::from_static("ApiKey realm=\"inventory\""))
    }
}
```

`authenticate` is asynchronous, so it can ask a database or a cache. It receives only the headers: the path, the query string and the body are never passed in, so a credential in a URL cannot be read even by mistake. `challenge` is the `WWW-Authenticate` value sent with the 401; it is optional, and a session authenticator leaves it out.

The same trait covers the common production cases:

- **Session cookies:** `auth::cookie(headers, "sid")` returns the cookie when it appears exactly once; look the session up and build the principal with `Principal::owned`. The next section shows it.
- **API keys in a database:** hash the presented key, look the hash up, and map the stored scopes to permissions.
- **A trusted proxy:** read the header that your gateway sets after it verified a JWT, and accept it only from that gateway's network.

## How do cookie sessions work?

`examples/items.rs` signs people in with a session cookie. The authenticator is a few lines, because Senu parses the cookie and the application owns the sessions:

```rust excerpt=examples/items.rs
pub struct Sessions(pub Store);

impl Authenticate for Sessions {
    fn authenticate<'a>(&'a self, headers: &'a HeaderMap) -> AuthFuture<'a> {
        Box::pin(async move {
            let session = auth::cookie(headers, "sid")?;
            self.0.lock().sessions.get(session).cloned()
        })
    }
}
```

A page request without a session should go to a login page, not get a 401. `html_login` does that with `303 See Other` to one fixed path, so it cannot become an open redirect; the JSON API still answers 401:

```rust excerpt=examples/items.rs
pub fn state(store: Store) -> AppState<Store> {
    AppState::builder(store.clone())
        .html_auth(Sessions(store.clone()))
        .api_auth(Sessions(store))
        .html_login("/login")
        .build()
}
```

The login page itself, the logout and an OAuth callback are routes of your own. List them with `.custom`, or with `.custom_with_body` when they take a form, so that they get Senu's limits and response headers:

```rust excerpt=examples/items.rs
async fn logout(State(app): State<Shared<Store>>, headers: HeaderMap) -> impl IntoResponse {
    if !auth::same_origin(&headers, &[]) {
        return (StatusCode::FORBIDDEN, "Cross-origin request refused\n");
    }
    if let Some(session) = auth::cookie(&headers, "sid") {
        app.state().sign_out(session);
    }
    (StatusCode::OK, "Signed out\n")
}
```

Set the session cookie with `HttpOnly`, `Secure` and `SameSite=Lax`, keep the session id long and random, and store only its hash.

## What happens when an authenticator is missing?

The table is refused. `into_router` returns `RouteError::NoAuthenticator` for an HTML route without `html_auth` or an API route without `api_auth`, so a forgotten line cannot open a route. Assets and `custom` routes never authenticate; a `custom` handler that needs it must do it itself.

## Does Senu protect against CSRF?

Yes, for every declared write. A browser attaches cookies and Basic credentials to requests that another site triggers, so Senu checks each write with `Sec-Fetch-Site`, or with `Origin` against `Host`, before authentication, and refuses a cross-site request with 403 `cross_origin`; [the writes page](/writes) has the rules. A `custom` route that changes something must call `auth::same_origin` itself, as the logout above does. Senu sends no CORS headers, so a script on another origin cannot read a response, and every response is sent with `Cache-Control: no-store`.

## What should you check before going live?

1. Secrets come from the environment or a secret store, are long and random, and differ per client.
2. TLS terminates in front of the router, and plain HTTP is redirected or closed.
3. Your authenticator takes the same path for a wrong user and a wrong password, and does not log credentials.
4. Each client's principal holds only the permissions it needs.
5. [The contract tests](/testing) pass on your table.

Next: [expose operations as MCP tools](/mcp-tools).
