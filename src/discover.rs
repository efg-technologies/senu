//! Finds the article files under `content/` (spec 0003, §3).
//!
//! `build.rs` includes this file with `#[path]` to embed the articles, so it
//! uses only std and nothing else from the crate. The library compiles it
//! too, so the rules are unit-tested and the embedded table can be compared
//! with a fresh listing.

use std::fs;
use std::path::Path;

/// The locale directories under `content/`, English (canonical) first.
pub const LOCALES: [&str; 2] = ["en", "ja"];

/// The largest article, in bytes.
pub const MAX_FILE_BYTES: u64 = 128 * 1024;

/// Lists the articles under `root`, the `content` directory, as
/// `content/<locale>/<name>` paths in byte order.
///
/// Names that start with `.` or do not end in `.md` are ignored, such as
/// editor swap and backup files. Every other entry must be a regular file
/// with a UTF-8 name, at most [`MAX_FILE_BYTES`] long and valid UTF-8, and
/// `root` and its locale directories must be real directories, not
/// symlinks; all other rules are checked when the site loads. Errors read
/// `senu: <path>: <reason>`, with control characters escaped.
pub fn discover(root: &Path) -> Result<Vec<String>, String> {
    let meta =
        fs::symlink_metadata(root).map_err(|err| format!("senu: content: {}", err.kind()))?;
    if !meta.is_dir() {
        return Err("senu: content: must be a directory, not a symlink or a file".into());
    }
    let mut paths = Vec::new();
    for locale in LOCALES {
        let dir = format!("content/{locale}");
        let problem = |reason: &dyn std::fmt::Display| format!("senu: {dir}: {reason}");
        let meta = fs::symlink_metadata(root.join(locale)).map_err(|err| problem(&err.kind()))?;
        if !meta.is_dir() {
            return Err(problem(&"must be a directory, not a symlink or a file"));
        }
        for entry in fs::read_dir(root.join(locale)).map_err(|err| problem(&err.kind()))? {
            let entry = entry.map_err(|err| problem(&err.kind()))?;
            let raw = entry.file_name();
            let bytes = raw.as_encoded_bytes();
            if bytes.starts_with(b".") || !bytes.ends_with(b".md") {
                continue;
            }
            let Some(name) = raw.to_str() else {
                let shown = raw.to_string_lossy();
                return Err(problem(&format_args!(
                    "{}: the file name is not valid UTF-8",
                    shown.escape_debug()
                )));
            };
            let path = format!("{dir}/{name}");
            check_file(&entry.path())
                .map_err(|reason| format!("senu: {}: {reason}", path.escape_debug()))?;
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths)
}

/// Checks one selected entry without following a symlink.
fn check_file(path: &Path) -> Result<(), String> {
    let meta = fs::symlink_metadata(path).map_err(|err| err.kind().to_string())?;
    if !meta.file_type().is_file() {
        return Err("must be a regular file, not a symlink or a directory".into());
    }
    if meta.len() > MAX_FILE_BYTES {
        return Err("is larger than 128 KiB".into());
    }
    let bytes = fs::read(path).map_err(|err| err.kind().to_string())?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err("is larger than 128 KiB".into());
    }
    match std::str::from_utf8(&bytes) {
        Ok(_) => Ok(()),
        Err(err) => Err(format!(
            "is not valid UTF-8 (first bad byte at offset {})",
            err.valid_up_to()
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// A fresh `content` directory with empty `en` and `ja` directories.
    fn scratch() -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir()
            .join(format!("senu-discover-{}-{n}", std::process::id()))
            .join("content");
        let _ = fs::remove_dir_all(root.parent().unwrap());
        for locale in LOCALES {
            fs::create_dir_all(root.join(locale)).unwrap();
        }
        root
    }

    fn write(root: &Path, rel: &str, text: &[u8]) {
        fs::write(root.join(rel), text).unwrap();
    }

    #[test]
    fn lists_markdown_files_of_both_locales_in_byte_order() {
        let root = scratch();
        for rel in ["ja/02-b.md", "en/10-z.md", "en/02-b.md", "ja/01-a.md"] {
            write(&root, rel, b"# T\n");
        }
        assert_eq!(
            discover(&root).unwrap(),
            [
                "content/en/02-b.md",
                "content/en/10-z.md",
                "content/ja/01-a.md",
                "content/ja/02-b.md"
            ]
        );
    }

    #[test]
    fn the_order_does_not_depend_on_creation_order() {
        let names = ["03-c.md", "01-a.md", "02-b.md", "20-x.md", "10-y.md"];
        let first = scratch();
        let second = scratch();
        for name in names {
            write(&first, &format!("en/{name}"), b"# T\n");
        }
        for name in names.iter().rev() {
            write(&second, &format!("en/{name}"), b"# T\n");
        }
        assert_eq!(discover(&first).unwrap(), discover(&second).unwrap());
    }

    #[test]
    fn hidden_and_other_names_are_ignored() {
        let root = scratch();
        for rel in [
            "en/.01-a.md.swp",
            "en/01-a.md~",
            "en/.hidden.md",
            "en/notes.txt",
            "en/README",
            "ja/x.md.bak",
        ] {
            write(&root, rel, b"\xff not even UTF-8");
        }
        fs::create_dir(root.join("en/drafts")).unwrap();
        assert_eq!(discover(&root).unwrap(), Vec::<String>::new());
    }

    #[test]
    fn a_missing_locale_directory_is_an_error() {
        let root = scratch();
        fs::remove_dir(root.join("ja")).unwrap();
        let err = discover(&root).unwrap_err();
        assert!(err.starts_with("senu: content/ja: "), "{err}");
    }

    #[test]
    fn a_directory_named_like_an_article_is_an_error() {
        let root = scratch();
        fs::create_dir(root.join("en/x.md")).unwrap();
        let err = discover(&root).unwrap_err();
        assert_eq!(
            err,
            "senu: content/en/x.md: must be a regular file, not a symlink or a directory"
        );
    }

    #[test]
    fn oversized_and_non_utf8_files_are_errors() {
        let root = scratch();
        write(&root, "en/01-a.md", &vec![b'a'; MAX_FILE_BYTES as usize]);
        assert!(discover(&root).is_ok(), "128 KiB exactly is allowed");
        write(
            &root,
            "en/01-a.md",
            &vec![b'a'; MAX_FILE_BYTES as usize + 1],
        );
        assert_eq!(
            discover(&root).unwrap_err(),
            "senu: content/en/01-a.md: is larger than 128 KiB"
        );
        write(&root, "en/01-a.md", b"# ok\n\xff\n");
        assert_eq!(
            discover(&root).unwrap_err(),
            "senu: content/en/01-a.md: is not valid UTF-8 (first bad byte at offset 5)"
        );
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_are_errors() {
        use std::os::unix::fs::symlink;
        let root = scratch();
        write(&root, "en/01-a.md", b"# T\n");
        symlink(root.join("en/01-a.md"), root.join("ja/01-a.md")).unwrap();
        assert_eq!(
            discover(&root).unwrap_err(),
            "senu: content/ja/01-a.md: must be a regular file, not a symlink or a directory"
        );

        let linked = scratch();
        let real = linked.parent().unwrap().join("real-en");
        fs::create_dir(&real).unwrap();
        fs::remove_dir(linked.join("en")).unwrap();
        symlink(&real, linked.join("en")).unwrap();
        assert_eq!(
            discover(&linked).unwrap_err(),
            "senu: content/en: must be a directory, not a symlink or a file"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_symlinked_content_directory_is_an_error() {
        use std::os::unix::fs::symlink;
        let outside = scratch();
        write(&outside, "en/01-a.md", b"# Outside\n");
        write(&outside, "ja/01-a.md", b"# Outside\n");
        let linked = scratch();
        fs::remove_dir_all(&linked).unwrap();
        symlink(&outside, &linked).unwrap();
        assert_eq!(
            discover(&linked).unwrap_err(),
            "senu: content: must be a directory, not a symlink or a file"
        );
        assert_eq!(
            discover(&outside).unwrap().len(),
            2,
            "the real tree is fine"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_non_utf8_name_is_an_error_with_escaped_text() {
        use std::os::unix::ffi::OsStrExt;
        let root = scratch();
        let name = std::ffi::OsStr::from_bytes(b"x\xff\x1b.md");
        fs::write(root.join("en").join(name), b"# T\n").unwrap();
        assert_eq!(
            discover(&root).unwrap_err(),
            "senu: content/en: x\u{FFFD}\\u{1b}.md: the file name is not valid UTF-8"
        );
    }

    #[test]
    fn the_embedded_table_matches_a_fresh_listing() {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        let listed = discover(&manifest.join("content")).unwrap();
        let embedded: Vec<&str> = crate::content::EMBEDDED.iter().map(|(p, _)| *p).collect();
        assert_eq!(embedded, listed, "rebuild, or run `cargo clean -p senu`");
        for (path, text) in crate::content::EMBEDDED {
            let on_disk = fs::read_to_string(manifest.join(path)).unwrap();
            assert_eq!(*text, on_disk, "{path} changed since the last build");
        }
    }
}
