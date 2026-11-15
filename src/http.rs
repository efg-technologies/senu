//! The HTTP adapter: HTML pages and a JSON API over one router.
//!
//! The application lists its routes in an [`Endpoints`] table. Every
//! request to a declared route passes through the same steps, in this
//! order: size limits, routing (unknown paths are 404, methods other than
//! GET are 405), the surface's [`Authenticate`] implementation, which
//! yields the server-side principal, [`Operation::call`] (permission, then
//! the parameter envelope, then the operation), and finally the JSON body
//! or the page with its CSP. The query string is parsed only after
//! authentication succeeds, and it is never searched for credentials. A
//! `custom` route gets the limits and headers but no authentication.
//!
//! [`Endpoints::into_router`] returns a plain [`axum::Router`]. Bind it
//! where you like and add your own tower layers for TLS termination,
//! logging, timeouts and rate limits; Senu adds none of those.
//!
//! With the `demo` feature, `senu site` uses a separate router with the
//! same size limits and response headers but no authentication: one
//! handler looks the raw path up in the pre-rendered pages (spec 0003, §6).
//!
//! Both routers also serve the embedded stylesheet at one exact path under
//! the reserved prefix `/_senu/`, without credentials (spec 0004, §4).

use std::sync::Arc;

use axum::Router;
use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, Uri, header};
use axum::middleware::{self, Next};
#[cfg(feature = "demo")]
use axum::response::Html;
use axum::response::{IntoResponse, Json, Response};
use axum::routing::{MethodRouter, any};
use serde::Serialize;
use serde_json::{Map, Value};

use crate::auth::Authenticate;
#[cfg(feature = "demo")]
use crate::auth::{Basic, Bearer};
#[cfg(feature = "demo")]
use crate::catalogue::Catalogue;
#[cfg(feature = "demo")]
use crate::config::ServeConfig;
#[cfg(feature = "demo")]
use crate::operation::Principal;
use crate::operation::{Args, InvalidDeclaration, OpError, Operation, Param};
#[cfg(feature = "demo")]
use crate::site::Site;

/// Longest accepted request target (path plus query), in bytes.
pub const MAX_TARGET_BYTES: usize = 2048;

/// The `Content-Security-Policy` of every HTML page unless
/// [`Endpoints::html_csp`] replaces it: no scripts, images or frames, only
/// same-origin stylesheets and form targets.
pub const DEFAULT_HTML_CSP: &str = "default-src 'none'; style-src 'self'; form-action 'self'; frame-ancestors 'none'; base-uri 'none'";
#[cfg(feature = "demo")]
const SITE_CSP: &str = "default-src 'none'; style-src 'self'; form-action 'none'; frame-ancestors 'none'; base-uri 'none'";

/// The only path of the stylesheet, which `npm run css` compiles from
/// `assets/ui.css` and Cargo embeds, so nothing is read at run time.
const STYLESHEET_PATH: &str = "/_senu/ui.css";
const STYLESHEET: &str = include_str!("../assets/ui.generated.css");

/// Everything the router needs: the application's state `S` and one
/// authenticator per surface.
pub struct AppState<S> {
    state: S,
    html: Option<Arc<dyn Authenticate>>,
    api: Option<Arc<dyn Authenticate>>,
    html_csp: HeaderValue,
}

impl<S> std::fmt::Debug for AppState<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState")
            .field("html_auth", &self.html.is_some())
            .field("api_auth", &self.api.is_some())
            .finish_non_exhaustive()
    }
}

/// Builds an [`AppState`]: the state, then the authenticators.
pub struct AppStateBuilder<S> {
    app: AppState<S>,
}

impl<S> AppStateBuilder<S> {
    /// Who may open the HTML pages. Without it, a table with an HTML route
    /// is refused.
    pub fn html_auth(mut self, authenticator: impl Authenticate) -> Self {
        self.app.html = Some(Arc::new(authenticator));
        self
    }

    /// Who may call the JSON API. Without it, a table with an API route is
    /// refused.
    pub fn api_auth(mut self, authenticator: impl Authenticate) -> Self {
        self.app.api = Some(Arc::new(authenticator));
        self
    }

    /// The finished state.
    pub fn build(self) -> AppState<S> {
        self.app
    }
}

#[cfg(feature = "demo")]
impl AppState<Catalogue> {
    /// The demo state: `demo-web` behind Basic, `demo-api` behind Bearer.
    pub fn new(catalogue: Catalogue, config: &ServeConfig) -> Self {
        Self::with_principals(
            catalogue,
            config,
            Principal::demo_web(),
            Principal::demo_api(),
        )
    }
}

impl<S> AppState<S> {
    /// Starts from the application's state, with no authenticator.
    pub fn builder(state: S) -> AppStateBuilder<S> {
        AppStateBuilder {
            app: Self {
                state,
                html: None,
                api: None,
                html_csp: HeaderValue::from_static(DEFAULT_HTML_CSP),
            },
        }
    }

    /// Binds principals to the demo's configured credentials: Basic for
    /// HTML and Bearer for the API. Tests use this to inject a principal
    /// without permissions.
    #[cfg(feature = "demo")]
    #[cfg_attr(docsrs, doc(cfg(feature = "demo")))]
    pub fn with_principals(
        state: S,
        config: &ServeConfig,
        web_principal: Principal,
        api_principal: Principal,
    ) -> Self {
        Self::builder(state)
            .html_auth(Basic::new(
                config.web_user.clone(),
                config.web_password.clone(),
                web_principal,
            ))
            .api_auth(Bearer::new(config.api_token.clone(), api_principal))
            .build()
    }

    /// The application's state, for `custom` routes.
    pub fn state(&self) -> &S {
        &self.state
    }

    fn authenticator(&self, surface: Surface) -> Option<&Arc<dyn Authenticate>> {
        match surface {
            Surface::Html => self.html.as_ref(),
            Surface::Api => self.api.as_ref(),
        }
    }
}

/// The state of a `custom` route's handler: `State<Shared<S>>`.
pub type Shared<S> = Arc<AppState<S>>;

/// Builds the demo's router from [`crate::demo::endpoints`]. There is no
/// other endpoint, and in particular no MCP over HTTP.
///
/// # Panics
///
/// Never for the demo's table, which the tests check. `senu serve` uses
/// [`Endpoints::into_router`], which returns the error instead.
#[cfg(feature = "demo")]
#[cfg_attr(docsrs, doc(cfg(feature = "demo")))]
pub fn router(state: AppState<Catalogue>) -> Router {
    crate::demo::endpoints()
        .into_router(state)
        .expect("the demo's route table is valid")
}

/// Renders an operation's result as an HTML page. The arguments are the
/// ones the operation accepted.
pub type View<T> = fn(&Args, &T) -> Result<String, askama::Error>;

/// What a route of an [`Endpoints`] table is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Kind {
    /// An HTML page behind the HTML authenticator.
    Html,
    /// JSON under `/api/` behind the API authenticator.
    Api,
    /// The application's own handler, with no automatic authentication.
    Custom,
    /// A public static file.
    Asset,
}

/// One row of an [`Endpoints`] table, as the contract tests read it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Declared {
    /// The exact path.
    pub path: &'static str,
    /// Which adapter serves it.
    pub kind: Kind,
    /// The operation's name; `None` for a custom route or an asset.
    pub operation: Option<&'static str>,
    /// The operation's parameters; empty for a custom route or an asset.
    pub params: &'static [Param],
}

/// The application's explicit route table. Declaring an operation exposes
/// nothing; only the paths listed here are routed.
pub struct Endpoints<S> {
    routes: Vec<Route<S>>,
    html_csp: &'static str,
}

struct Route<S> {
    declared: Declared,
    invalid: Option<InvalidDeclaration>,
    /// An asset's content type, checked with the table.
    content_type: Option<&'static str>,
    handler: MethodRouter<Shared<S>>,
}

impl<S: Send + Sync + 'static> Default for Endpoints<S> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S: Send + Sync + 'static> Endpoints<S> {
    /// An empty table.
    pub fn new() -> Self {
        Self {
            routes: Vec::new(),
            html_csp: DEFAULT_HTML_CSP,
        }
    }

    /// An HTML page at `path`, behind the HTML authenticator, rendered by
    /// `view` with the page CSP.
    pub fn html<T: Send + 'static>(
        self,
        path: &'static str,
        operation: &'static Operation<S, T>,
        view: View<T>,
    ) -> Self {
        let handler = move |State(app): State<Shared<S>>,
                            method: Method,
                            uri: Uri,
                            headers: HeaderMap| async move {
            html_route(&app, &method, &uri, &headers, operation, view).await
        };
        self.push(path, Kind::Html, operation, any(handler))
    }

    /// JSON at `path`, which must be under `/api/`, behind the API
    /// authenticator.
    pub fn api<T: Serialize + Send + 'static>(
        self,
        path: &'static str,
        operation: &'static Operation<S, T>,
    ) -> Self {
        let handler = move |State(app): State<Shared<S>>,
                            method: Method,
                            uri: Uri,
                            headers: HeaderMap| async move {
            api_route(&app, &method, &uri, &headers, operation).await
        };
        self.push(path, Kind::Api, operation, any(handler))
    }

    /// A public static file at `path`, such as the application's own
    /// stylesheet: `GET` only, no credentials, served from memory with
    /// `content_type`. It must hold no private data.
    pub fn asset(
        mut self,
        path: &'static str,
        content_type: &'static str,
        body: &'static [u8],
    ) -> Self {
        let handler = move |method: Method| async move {
            if method != Method::GET {
                return Surface::Html.error(HttpError::MethodNotAllowed);
            }
            match HeaderValue::from_str(content_type) {
                Ok(content_type) => ([(header::CONTENT_TYPE, content_type)], body).into_response(),
                Err(_) => Surface::Html.error(HttpError::Internal),
            }
        };
        self.routes.push(Route {
            declared: Declared {
                path,
                kind: Kind::Asset,
                operation: None,
                params: &[],
            },
            invalid: None,
            content_type: Some(content_type),
            handler: any(handler),
        });
        self
    }

    /// Replaces the `Content-Security-Policy` of every HTML page of this
    /// table, for example to allow images. The default,
    /// [`DEFAULT_HTML_CSP`], allows only same-origin stylesheets and form
    /// targets; anything you add here is yours to justify.
    pub fn html_csp(mut self, policy: &'static str) -> Self {
        self.html_csp = policy;
        self
    }

    /// The override hatch: the application's own handler at `path`. It gets
    /// the size limits and response headers, but no authentication, method
    /// check or CSP; the handler is responsible for those.
    pub fn custom(mut self, path: &'static str, handler: MethodRouter<Shared<S>>) -> Self {
        self.routes.push(Route {
            declared: Declared {
                path,
                kind: Kind::Custom,
                operation: None,
                params: &[],
            },
            invalid: None,
            content_type: None,
            handler,
        });
        self
    }

    fn push<T>(
        mut self,
        path: &'static str,
        kind: Kind,
        operation: &'static Operation<S, T>,
        handler: MethodRouter<Shared<S>>,
    ) -> Self {
        self.routes.push(Route {
            declared: Declared {
                path,
                kind,
                operation: Some(operation.name),
                params: operation.params,
            },
            invalid: operation.check().err(),
            content_type: None,
            handler,
        });
        self
    }

    /// The `Content-Security-Policy` that this table's HTML pages carry.
    pub fn csp(&self) -> &'static str {
        self.html_csp
    }

    /// The table's rows, in order.
    pub fn declared(&self) -> Vec<Declared> {
        self.routes.iter().map(|route| route.declared).collect()
    }

    /// Checks the whole table, then builds the router: the listed routes,
    /// the stylesheet, the 404 fallback, the limits and the headers, so
    /// every route has the same layers. An invalid table is an error, never
    /// a panic.
    pub fn into_router(self, mut state: AppState<S>) -> Result<Router, RouteError> {
        self.check(&state)?;
        state.html_csp =
            HeaderValue::from_str(self.html_csp).map_err(|_| RouteError::InvalidCsp)?;
        let mut router = Router::new();
        for route in self.routes {
            router = router.route(route.declared.path, route.handler);
        }
        Ok(router
            .route(STYLESHEET_PATH, any(stylesheet))
            .fallback(not_found)
            .with_state(Arc::new(state))
            .layer(middleware::from_fn(limits))
            .layer(middleware::from_fn(response_headers)))
    }

    fn check(&self, state: &AppState<S>) -> Result<(), RouteError> {
        let visible = |text: &str| text.bytes().all(|b| b == b' ' || b.is_ascii_graphic());
        if self.html_csp.trim().is_empty() || !visible(self.html_csp) {
            return Err(RouteError::InvalidCsp);
        }
        for (i, route) in self.routes.iter().enumerate() {
            let Declared { path, kind, .. } = route.declared;
            if !is_exact_path(path) {
                return Err(RouteError::InvalidPath(path));
            }
            if path == "/_senu" || path.starts_with("/_senu/") {
                return Err(RouteError::Reserved(path));
            }
            let wrong_prefix = match kind {
                Kind::Api => !path.starts_with("/api/"),
                Kind::Html | Kind::Asset => Surface::for_path(path) == Surface::Api,
                Kind::Custom => false,
            };
            if wrong_prefix {
                return Err(RouteError::WrongPrefix(path));
            }
            if self.routes[..i].iter().any(|r| r.declared.path == path) {
                return Err(RouteError::Duplicate(path));
            }
            if let Some(invalid) = route.invalid {
                return Err(RouteError::Operation(invalid));
            }
            let unguarded = match kind {
                Kind::Html => state.html.is_none(),
                Kind::Api => state.api.is_none(),
                Kind::Custom | Kind::Asset => false,
            };
            if unguarded {
                return Err(RouteError::NoAuthenticator(path));
            }
            if let Some(content_type) = route.content_type
                && (content_type.trim().is_empty() || !visible(content_type))
            {
                return Err(RouteError::InvalidAsset(path));
            }
        }
        Ok(())
    }
}

/// A route table that Senu refuses. `senu serve` prints it and exits with
/// status 2 before binding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum RouteError {
    /// Not an exact path: it must start with `/`, hold only visible ASCII,
    /// and contain no `{`, `}`, `*`, `?`, `#`, `%` or `\`, no dot segment and
    /// no segment that starts with `:`.
    InvalidPath(&'static str),
    /// `/_senu` and everything under `/_senu/` belong to Senu.
    Reserved(&'static str),
    /// An API route outside `/api/`, or an HTML route or asset under it.
    WrongPrefix(&'static str),
    /// The path is listed twice.
    Duplicate(&'static str),
    /// The route's operation has invalid metadata.
    Operation(InvalidDeclaration),
    /// An HTML or API route whose surface has no authenticator in the
    /// [`AppState`]. Senu refuses the table rather than serve it open.
    NoAuthenticator(&'static str),
    /// An asset whose content type cannot be a header value.
    InvalidAsset(&'static str),
    /// The CSP of [`Endpoints::html_csp`] is empty or cannot be a header
    /// value.
    InvalidCsp,
}

impl std::fmt::Display for RouteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidPath(path) => write!(f, "senu: route {path:?} is not an exact path"),
            Self::Reserved(path) => write!(f, "senu: route {path:?} is reserved for Senu"),
            Self::WrongPrefix(path) => write!(
                f,
                "senu: route {path:?}: API routes must be under /api/ and HTML routes must not"
            ),
            Self::Duplicate(path) => write!(f, "senu: route {path:?} is listed twice"),
            Self::Operation(invalid) => write!(f, "senu: {invalid}"),
            Self::NoAuthenticator(path) => write!(
                f,
                "senu: route {path:?} has no authenticator; set html_auth or api_auth on the AppState"
            ),
            Self::InvalidAsset(path) => {
                write!(f, "senu: asset {path:?} has an invalid content type")
            }
            Self::InvalidCsp => {
                f.write_str("senu: the HTML Content-Security-Policy is empty or not visible ASCII")
            }
        }
    }
}

impl std::error::Error for RouteError {}

fn is_exact_path(path: &str) -> bool {
    path.starts_with('/')
        && path
            .bytes()
            .all(|b| b.is_ascii_graphic() && !b"{}*?#%\\".contains(&b))
        && path
            .split('/')
            .all(|segment| segment != "." && segment != ".." && !segment.starts_with(':'))
}

/// Builds the `senu site` router: every path is looked up exactly, without
/// percent-decoding or normalization, and the query string and
/// `Authorization` are never read. `/projects`, `/api/*` and `/mcp` do not
/// exist here.
#[cfg(feature = "demo")]
#[cfg_attr(docsrs, doc(cfg(feature = "demo")))]
pub fn site_router(site: Site) -> Router {
    Router::new()
        .route(STYLESHEET_PATH, any(stylesheet))
        .fallback(site_page)
        .with_state(Arc::new(site))
        .layer(middleware::from_fn(site_limits))
        .layer(middleware::from_fn(response_headers))
}

/// A public, fixed error. None of these repeat the request or a credential.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum HttpError {
    /// 401: the authenticator found no principal.
    Unauthorized,
    /// 403: the principal lacks the permission.
    Forbidden,
    /// 400: the arguments do not fit the declared parameters.
    InvalidRequest,
    /// 404: no such route, or the operation found nothing.
    NotFound,
    /// 405: any method but `GET`.
    MethodNotAllowed,
    /// 414: the target is longer than [`MAX_TARGET_BYTES`].
    UriTooLong,
    /// 413: the request has a body.
    PayloadTooLarge,
    /// 500: the operation or the view failed. No detail is sent.
    Internal,
    /// An operation's own 400 error, from [`OpError::Invalid`].
    Invalid {
        /// The JSON error code.
        code: &'static str,
        /// The plain-text body.
        phrase: &'static str,
    },
}

impl HttpError {
    /// The response status.
    pub fn status(self) -> StatusCode {
        match self {
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Forbidden => StatusCode::FORBIDDEN,
            Self::InvalidRequest | Self::Invalid { .. } => StatusCode::BAD_REQUEST,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::MethodNotAllowed => StatusCode::METHOD_NOT_ALLOWED,
            Self::UriTooLong => StatusCode::URI_TOO_LONG,
            Self::PayloadTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            Self::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// The JSON error code used under `/api/*`.
    pub fn code(self) -> &'static str {
        match self {
            Self::Unauthorized => "unauthorized",
            Self::Forbidden => "forbidden",
            Self::InvalidRequest => "invalid_request",
            Self::NotFound => "not_found",
            Self::MethodNotAllowed => "method_not_allowed",
            Self::UriTooLong => "uri_too_long",
            Self::PayloadTooLarge => "payload_too_large",
            Self::Internal => "internal",
            Self::Invalid { code, .. } => code,
        }
    }

    /// The plain-text body used everywhere else.
    pub fn phrase(self) -> &'static str {
        match self {
            Self::Unauthorized => "Unauthorized",
            Self::Forbidden => "Forbidden",
            Self::InvalidRequest => "Invalid request",
            Self::NotFound => "Not Found",
            Self::MethodNotAllowed => "Method Not Allowed",
            Self::UriTooLong => "URI Too Long",
            Self::PayloadTooLarge => "Payload Too Large",
            Self::Internal => "Internal Server Error",
            Self::Invalid { phrase, .. } => phrase,
        }
    }
}

impl From<OpError> for HttpError {
    fn from(err: OpError) -> Self {
        match err {
            OpError::Forbidden => Self::Forbidden,
            OpError::InvalidRequest => Self::InvalidRequest,
            OpError::NotFound => Self::NotFound,
            OpError::Internal => Self::Internal,
            OpError::Invalid { code, phrase } => Self::Invalid { code, phrase },
        }
    }
}

/// Which error format and challenge a path uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Surface {
    Api,
    Html,
}

impl Surface {
    fn for_path(path: &str) -> Self {
        if path == "/api" || path.starts_with("/api/") {
            Self::Api
        } else {
            Self::Html
        }
    }

    fn error(self, err: HttpError) -> Response {
        let mut response = match self {
            Self::Api => (
                err.status(),
                Json(serde_json::json!({ "error": err.code() })),
            )
                .into_response(),
            Self::Html => (
                err.status(),
                [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                err.phrase(),
            )
                .into_response(),
        };
        if err == HttpError::MethodNotAllowed {
            response
                .headers_mut()
                .insert(header::ALLOW, HeaderValue::from_static("GET"));
        }
        response
    }

    /// 401 with the authenticator's challenge.
    fn unauthorized(self, challenge: HeaderValue) -> Response {
        let mut response = self.error(HttpError::Unauthorized);
        response
            .headers_mut()
            .insert(header::WWW_AUTHENTICATE, challenge);
        response
    }
}

/// The steps every declared route shares: the method, then the surface's
/// authenticator, then the operation with the arguments of the query.
async fn run_route<S, T>(
    app: &AppState<S>,
    surface: Surface,
    method: &Method,
    uri: &Uri,
    headers: &HeaderMap,
    operation: &Operation<S, T>,
) -> Result<(Args, T), Box<Response>> {
    if method != Method::GET {
        return Err(Box::new(surface.error(HttpError::MethodNotAllowed)));
    }
    // `Endpoints::check` refused the table if this is missing; fail closed
    // all the same.
    let Some(authenticator) = app.authenticator(surface) else {
        return Err(Box::new(surface.error(HttpError::Internal)));
    };
    let Some(principal) = authenticator.authenticate(headers).await else {
        return Err(Box::new(surface.unauthorized(authenticator.challenge())));
    };
    let args = query_arguments(uri.query());
    match operation.call(&app.state, &principal, &args).await {
        Ok(result) => Ok((args, result)),
        Err(err) => Err(Box::new(surface.error(err.into()))),
    }
}

/// The shared JSON handler of every declared API route.
async fn api_route<S, T: Serialize>(
    app: &AppState<S>,
    method: &Method,
    uri: &Uri,
    headers: &HeaderMap,
    operation: &Operation<S, T>,
) -> Response {
    match run_route(app, Surface::Api, method, uri, headers, operation).await {
        Ok((_args, result)) => Json(result).into_response(),
        Err(response) => *response,
    }
}

/// The shared page handler of every declared HTML route.
async fn html_route<S, T>(
    app: &AppState<S>,
    method: &Method,
    uri: &Uri,
    headers: &HeaderMap,
    operation: &Operation<S, T>,
    view: View<T>,
) -> Response {
    let (args, result) = match run_route(app, Surface::Html, method, uri, headers, operation).await
    {
        Ok(done) => done,
        Err(response) => return *response,
    };
    match view(&args, &result) {
        Ok(page) => (
            [
                (header::CONTENT_SECURITY_POLICY, app.html_csp.clone()),
                (
                    header::CONTENT_TYPE,
                    HeaderValue::from_static("text/html; charset=utf-8"),
                ),
            ],
            page,
        )
            .into_response(),
        Err(_) => Surface::Html.error(HttpError::Internal),
    }
}

#[cfg(feature = "demo")]
async fn site_page(State(site): State<Arc<Site>>, request: Request) -> Response {
    let Some(page) = site.page(request.uri().path()) else {
        return Surface::Html.error(HttpError::NotFound);
    };
    if request.method() != Method::GET {
        return Surface::Html.error(HttpError::MethodNotAllowed);
    }
    (
        [(header::CONTENT_SECURITY_POLICY, SITE_CSP)],
        Html(page.to_owned()),
    )
        .into_response()
}

/// The embedded stylesheet. It holds no data, so no credential is read.
async fn stylesheet(method: Method) -> Response {
    if method != Method::GET {
        return Surface::Html.error(HttpError::MethodNotAllowed);
    }
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        STYLESHEET,
    )
        .into_response()
}

async fn not_found(uri: Uri) -> Response {
    Surface::for_path(uri.path()).error(HttpError::NotFound)
}

/// Rejects oversized targets and any request body before routing.
async fn limits(request: Request, next: Next) -> Response {
    match over_limits(&request) {
        Some(err) => Surface::for_path(request.uri().path()).error(err),
        None => next.run(request).await,
    }
}

/// The same limits for `senu site`, whose errors are always plain text.
#[cfg(feature = "demo")]
async fn site_limits(request: Request, next: Next) -> Response {
    match over_limits(&request) {
        Some(err) => Surface::Html.error(err),
        None => next.run(request).await,
    }
}

fn over_limits(request: &Request) -> Option<HttpError> {
    let target = request
        .uri()
        .path_and_query()
        .map_or(0, |target| target.as_str().len());
    if target > MAX_TARGET_BYTES {
        Some(HttpError::UriTooLong)
    } else if has_body(request.headers()) {
        Some(HttpError::PayloadTooLarge)
    } else {
        None
    }
}

async fn response_headers(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    response
}

fn has_body(headers: &HeaderMap) -> bool {
    headers.contains_key(header::TRANSFER_ENCODING)
        || headers
            .get_all(header::CONTENT_LENGTH)
            .iter()
            .any(|length| !is_decimal_zero(length.as_bytes()))
}

/// `Content-Length` is `1*DIGIT` (RFC 9110 §8.6), so `0`, `00` and `000` all
/// mean an empty body. Positive and malformed values count as a body.
fn is_decimal_zero(value: &[u8]) -> bool {
    !value.is_empty() && value.iter().all(|&byte| byte == b'0')
}

/// Decodes a query string into operation arguments. Repeated keys become
/// arrays, so the parameter envelope rejects them as non-string values.
pub fn query_arguments(query: Option<&str>) -> Map<String, Value> {
    let mut args = Map::new();
    for (key, value) in form_urlencoded::parse(query.unwrap_or("").as_bytes()) {
        let value = Value::String(value.into_owned());
        match args.get_mut(key.as_ref()) {
            None => {
                args.insert(key.into_owned(), value);
            }
            Some(Value::Array(values)) => values.push(value),
            Some(existing) => {
                let first = existing.take();
                *existing = Value::Array(vec![first, value]);
            }
        }
    }
    args
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn query_arguments_decode_plus_and_percent_escapes() {
        assert_eq!(
            Value::Object(query_arguments(Some("q=caf%C3%A9+roadmap"))),
            json!({"q": "café roadmap"})
        );
    }

    #[test]
    fn query_arguments_turn_repeated_keys_into_arrays() {
        assert_eq!(
            Value::Object(query_arguments(Some("q=a&q=b&q=c"))),
            json!({"q": ["a", "b", "c"]})
        );
    }

    #[test]
    fn query_arguments_keep_unknown_keys_for_the_operation_to_reject() {
        assert_eq!(
            Value::Object(query_arguments(Some("tenant=globex&q"))),
            json!({"tenant": "globex", "q": ""})
        );
        assert!(query_arguments(None).is_empty());
        assert!(query_arguments(Some("")).is_empty());
    }

    #[test]
    fn query_arguments_replace_invalid_utf8() {
        assert_eq!(
            Value::Object(query_arguments(Some("q=%FF"))),
            json!({"q": "\u{FFFD}"})
        );
    }

    #[test]
    fn errors_map_to_spec_status_codes() {
        let table = [
            (HttpError::Unauthorized, 401, "unauthorized"),
            (HttpError::Forbidden, 403, "forbidden"),
            (HttpError::InvalidRequest, 400, "invalid_request"),
            (HttpError::NotFound, 404, "not_found"),
            (HttpError::MethodNotAllowed, 405, "method_not_allowed"),
            (HttpError::UriTooLong, 414, "uri_too_long"),
            (HttpError::PayloadTooLarge, 413, "payload_too_large"),
            (HttpError::Internal, 500, "internal"),
        ];
        for (err, status, code) in table {
            assert_eq!(err.status().as_u16(), status);
            assert_eq!(err.code(), code);
            assert!(!err.phrase().is_empty());
        }
    }

    #[test]
    fn operation_errors_keep_their_codes() {
        for err in [
            OpError::Forbidden,
            OpError::InvalidRequest,
            OpError::NotFound,
            OpError::Internal,
            OpError::Invalid {
                code: "invalid_sku",
                phrase: "Invalid SKU",
            },
        ] {
            assert_eq!(HttpError::from(err).code(), err.code());
        }
        assert_eq!(HttpError::from(OpError::Internal).status().as_u16(), 500);
    }

    #[cfg(feature = "demo")]
    #[test]
    fn the_demo_errors_keep_their_codes_as_constants() {
        assert_eq!(HttpError::InvalidSearch.status().as_u16(), 400);
        assert_eq!(HttpError::InvalidSearch.code(), "invalid_search");
        assert_eq!(HttpError::InvalidSearch.phrase(), "Invalid search");
        assert_eq!(
            HttpError::from(OpError::InvalidSearch),
            HttpError::InvalidSearch
        );
    }

    #[cfg(feature = "demo")]
    #[test]
    fn get_project_errors_keep_their_codes_and_statuses() {
        assert_eq!(HttpError::InvalidId.status().as_u16(), 400);
        assert_eq!(HttpError::InvalidId.code(), "invalid_id");
        assert_eq!(HttpError::InvalidId.phrase(), "Invalid id");
        for (err, status) in [(OpError::InvalidId, 400), (OpError::NotFound, 404)] {
            assert_eq!(HttpError::from(err).code(), err.code());
            assert_eq!(HttpError::from(err).status().as_u16(), status);
        }
    }

    #[test]
    fn api_surface_covers_the_api_prefix_only() {
        assert_eq!(Surface::for_path("/api/projects"), Surface::Api);
        assert_eq!(Surface::for_path("/api"), Surface::Api);
        assert_eq!(Surface::for_path("/api/"), Surface::Api);
        assert_eq!(Surface::for_path("/apiary"), Surface::Html);
        assert_eq!(Surface::for_path("/projects"), Surface::Html);
    }

    #[test]
    fn body_detection_treats_zero_length_as_no_body() {
        let mut headers = HeaderMap::new();
        assert!(!has_body(&headers));
        headers.insert(header::CONTENT_LENGTH, HeaderValue::from_static("0"));
        assert!(!has_body(&headers));
        headers.insert(header::CONTENT_LENGTH, HeaderValue::from_static("1"));
        assert!(has_body(&headers));
        let mut chunked = HeaderMap::new();
        chunked.insert(
            header::TRANSFER_ENCODING,
            HeaderValue::from_static("chunked"),
        );
        assert!(has_body(&chunked));
    }

    #[test]
    fn body_detection_accepts_any_decimal_spelling_of_zero() {
        let lengths = |values: &[&'static str]| {
            let mut headers = HeaderMap::new();
            for value in values {
                headers.append(header::CONTENT_LENGTH, HeaderValue::from_static(value));
            }
            headers
        };
        for zero in [&["00"][..], &["000"], &["0", "00"]] {
            assert!(!has_body(&lengths(zero)), "{zero:?}");
        }
        for other in [
            &["01"][..],
            &["10"],
            &["0", "1"],
            &[""],
            &[" 0"],
            &["+0"],
            &["-0"],
            &["0x0"],
            &["0, 0"],
        ] {
            assert!(has_body(&lengths(other)), "{other:?}");
        }
    }
}
