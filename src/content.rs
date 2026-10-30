//! The articles embedded by `build.rs` (spec 0003, §3).

/// Every article under `content/` as `(path, text)`, in byte order of path.
/// `path` is relative to the package, such as `content/en/01-start.md`.
#[rustfmt::skip]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/content.rs"));
}

pub use generated::EMBEDDED;
