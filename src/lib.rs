//! Senu: one read-only operation served as HTML, a JSON API and MCP, and a
//! small site of Markdown articles embedded at build time.
//!
//! This is an experimental runnable seed. It is not audited security software.

pub mod app;
pub mod auth;
pub mod catalogue;
pub mod cli;
pub mod config;
pub mod content;
pub mod discover;
pub mod http;
pub mod markdown;
pub mod mcp;
pub mod operation;
pub mod reading;
pub mod site;
pub mod view;
