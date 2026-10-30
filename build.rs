//! Embeds the Markdown articles under `content/` into the binary (spec 0003,
//! §3). The generated `$OUT_DIR/content.rs` lists `(path, text)` pairs, each
//! read with `include_str!` from this package's own directory, so the binary
//! reads no files at run time and a reused target directory cannot embed
//! another checkout's files. Only the filesystem rules are checked here;
//! `senu::site::Site::load` checks everything else.

#[path = "src/discover.rs"]
mod discover;

use std::fmt::Write as _;
use std::path::PathBuf;

fn main() {
    // For a directory, Cargo reruns this script when any file below it is
    // added, removed or modified.
    println!("cargo::rerun-if-changed=content");
    println!("cargo::rerun-if-changed=build.rs");
    let manifest = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("set by Cargo"));
    let paths = match discover::discover(&manifest.join("content")) {
        Ok(paths) => paths,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(1);
        }
    };
    let mut code = String::from("pub static EMBEDDED: &[(&str, &str)] = &[\n");
    for path in &paths {
        let from_manifest = format!("/{path}");
        writeln!(
            code,
            "    ({path:?}, include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), {from_manifest:?}))),"
        )
        .expect("writing to a String");
    }
    code.push_str("];\n");
    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("set by Cargo"));
    std::fs::write(out.join("content.rs"), code).expect("write content.rs");
}
