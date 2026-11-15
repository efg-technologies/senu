//! Senu: declare a function once and serve it as a web page, a JSON API and
//! an MCP tool.
//!
//! An [`operation::Operation`] is a read-only function over your state with
//! a name, a description, a permission and its string parameters. You list
//! it in an [`http::Endpoints`] table and in an MCP tool list; Senu's
//! adapters then do the same steps for every route and tool, in the same
//! order: size limits, the method, authentication, the permission, the
//! parameter envelope, your function, and the error mapping. Declaring an
//! operation exposes nothing: only what you list is reachable.
//!
//! ```
//! use senu::auth::{Bearer, Secret};
//! use senu::http::{AppState, Endpoints};
//! use senu::mcp::{self, McpServer};
//! use senu::operation::{
//!     Args, OpError, Operation, Param, Permission, Principal, Run, TenantId, decode,
//! };
//!
//! // Your state, your permission and your function.
//! struct Stock(Vec<(&'static str, u32)>);
//!
//! const VIEW_STOCK: Permission = Permission::new("view_stock");
//!
//! #[derive(serde::Deserialize)]
//! #[serde(deny_unknown_fields)]
//! struct StockArgs {
//!     sku: String,
//! }
//!
//! #[derive(serde::Serialize)]
//! struct Level {
//!     sku: String,
//!     units: u32,
//! }
//!
//! fn get_stock(stock: &Stock, _caller: &Principal, args: &Args) -> Result<Level, OpError> {
//!     let StockArgs { sku } = decode(args)?;
//!     let (sku, units) = stock.0.iter().find(|(known, _)| *known == sku).ok_or(OpError::NotFound)?;
//!     Ok(Level { sku: (*sku).to_owned(), units: *units })
//! }
//!
//! // The declaration: nothing is exposed yet.
//! const GET_STOCK: Operation<Stock, Level> = Operation {
//!     name: "get_stock",
//!     description: "Units in stock for one SKU.",
//!     permission: VIEW_STOCK,
//!     params: &[Param::required("sku", "The SKU, such as SKU-1.")],
//!     run: Run::Sync(get_stock),
//! };
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // A JSON route behind a Bearer token. `router` is an `axum::Router`.
//! let clerk = Principal::new("clerk", TenantId::from_static("main"), &[VIEW_STOCK]);
//! let state = AppState::builder(Stock(vec![("SKU-1", 12)]))
//!     .api_auth(Bearer::new(Secret::new("a-long-random-token")?, clerk.clone()))
//!     .build();
//! let router = Endpoints::new().api("/api/stock", &GET_STOCK).into_router(state)?;
//!
//! // The same operation as an MCP tool.
//! let server = McpServer::with_tools(Stock(vec![("SKU-1", 12)]), clerk, vec![mcp::tool(&GET_STOCK)])?;
//! # let _ = (router, server);
//! # Ok(())
//! # }
//! ```
//!
//! # Modules
//!
//! - [`operation`]: `Operation`, `Principal`, `Permission`, `OpError`.
//! - [`auth`]: the [`auth::Authenticate`] trait and the built-in Basic and
//!   Bearer authenticators.
//! - [`http`]: the route table and the Axum router.
//! - [`mcp`]: the tool list and the stdio server.
//! - `contract` (feature `contract`): Senu's contract tests as a kit that
//!   you run over your own routes and tools.
//!
//! # Features
//!
//! - `demo` (default): the catalogue demo, the `senu` binary and the article
//!   site that holds the beginner course. A library user turns it off with
//!   `default-features = false`.
//! - `contract`: the test kit. Enable it in `[dev-dependencies]`.
//!
//! # Limits
//!
//! Operations are read-only and served by `GET`; parameters are strings;
//! paths are exact. MCP runs over stdio. Writes, MCP over HTTP, sessions and
//! OAuth are not part of this version. Senu is not audited security
//! software; read `SECURITY.md` before you rely on it.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![cfg_attr(docsrs, feature(doc_cfg))]

// The modules behind `demo` are the application and the course, not the
// library's API, so `missing_docs` is not required of them.

pub mod auth;
#[cfg(feature = "contract")]
#[cfg_attr(docsrs, doc(cfg(feature = "contract")))]
pub mod contract;
pub mod http;
pub mod mcp;
pub mod operation;

#[cfg(feature = "demo")]
#[cfg_attr(docsrs, doc(cfg(feature = "demo")))]
#[allow(missing_docs)]
pub mod app;
#[cfg(feature = "demo")]
#[cfg_attr(docsrs, doc(cfg(feature = "demo")))]
#[allow(missing_docs)]
pub mod catalogue;
#[cfg(feature = "demo")]
#[cfg_attr(docsrs, doc(cfg(feature = "demo")))]
#[allow(missing_docs)]
pub mod cli;
#[cfg(feature = "demo")]
#[cfg_attr(docsrs, doc(cfg(feature = "demo")))]
#[allow(missing_docs)]
pub mod config;
#[cfg(feature = "demo")]
#[cfg_attr(docsrs, doc(cfg(feature = "demo")))]
#[allow(missing_docs)]
pub mod content;
#[cfg(feature = "demo")]
#[cfg_attr(docsrs, doc(cfg(feature = "demo")))]
#[allow(missing_docs)]
pub mod demo;
#[cfg(feature = "demo")]
#[cfg_attr(docsrs, doc(cfg(feature = "demo")))]
#[allow(missing_docs)]
pub mod discover;
#[cfg(feature = "demo")]
#[cfg_attr(docsrs, doc(cfg(feature = "demo")))]
#[allow(missing_docs)]
pub mod markdown;
#[cfg(feature = "demo")]
#[cfg_attr(docsrs, doc(cfg(feature = "demo")))]
#[allow(missing_docs)]
pub mod reading;
#[cfg(feature = "demo")]
#[cfg_attr(docsrs, doc(cfg(feature = "demo")))]
#[allow(missing_docs)]
pub mod site;
#[cfg(feature = "demo")]
#[cfg_attr(docsrs, doc(cfg(feature = "demo")))]
#[allow(missing_docs)]
pub mod view;
