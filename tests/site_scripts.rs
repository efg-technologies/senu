//! `scripts/check_site.py` and the refusals of `scripts/build_site.sh`,
//! against small fake sites and directories. Like `docs_sync`, these tests
//! check scripts rather than Senu, so they are not part of the test pyramid.
//! They need `bash` and `python3`, but not mdBook.

mod support;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use support::process;

const REPO_LINK: &str = "https://github.com/efg-technologies/senu/blob/main/Cargo.toml";

/// A fresh directory under Cargo's per-target temporary directory.
fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write(root: &Path, rel: &str, text: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

/// The smallest site that passes: one chapter in each language.
fn good_site(root: &Path) {
    write(
        root,
        "index.html",
        &format!(
            r##"<html lang="en"><body><h1 id="top">Start</h1>
<a href="ja/index.html" hreflang="ja">日本語</a> <a href="#top">top</a>
<a href="{REPO_LINK}">source</a> <link href="style.css"></body></html>"##
        ),
    );
    write(
        root,
        "ja/index.html",
        r#"<html lang="ja"><a href="../index.html" hreflang="en">English</a></html>"#,
    );
    write(
        root,
        "toc.html",
        r#"<html lang="en"><a href="index.html">1</a></html>"#,
    );
    write(
        root,
        "ja/toc.html",
        r#"<html lang="ja"><a href="index.html">1</a></html>"#,
    );
    for rel in [
        ".nojekyll",
        "source-revision.txt",
        "style.css",
        "searchindex.js",
        "ja/searchindex.js",
    ] {
        write(root, rel, "");
    }
}

fn check_site(root: &Path) -> (Option<i32>, Vec<String>, Vec<String>) {
    let mut command = Command::new("python3");
    command
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/scripts/check_site.py"
        ))
        .arg(root);
    let (status, stdout, stderr) = process::run(command);
    (status.code(), stdout, stderr)
}

#[test]
fn site_check_passes_a_good_site_and_names_each_defect() {
    let root = scratch("site-check-good");
    good_site(&root);
    let (code, stdout, stderr) = check_site(&root);
    assert_eq!(code, Some(0), "{stderr:?}");
    assert_eq!(stdout.len(), 1, "{stdout:?}");

    // (file, text to replace, replacement, expected reason)
    let cases = [
        (
            "ja/index.html",
            r#"lang="ja""#,
            r#"lang="en""#,
            "lang is 'en', expected 'ja'",
        ),
        (
            "index.html",
            "</body>",
            "{{#include x.sh}}</body>",
            "unexpanded mdBook directive",
        ),
        (
            "index.html",
            "</body>",
            "ANCHOR: x</body>",
            "snippet anchor",
        ),
        (
            "index.html",
            r##"href="#top""##,
            r##"href="#gone""##,
            "#gone -> no such id",
        ),
        (
            "index.html",
            "style.css",
            "missing.css",
            "-> missing missing.css",
        ),
        (
            "index.html",
            r#"href="ja/index.html""#,
            r#"href="ja/index.html#x""#,
            "no id 'x' in ja/index.html",
        ),
        (
            "index.html",
            "Cargo.toml",
            "no-such-file",
            "not a file here",
        ),
        (
            "index.html",
            REPO_LINK,
            "http://example.com/",
            "is not https",
        ),
        (
            "index.html",
            REPO_LINK,
            "https://example.com/",
            "host that is not allowed",
        ),
        (
            "index.html",
            REPO_LINK,
            "https://github.com/someone/else",
            "not a blob/main",
        ),
        (
            "ja/index.html",
            r#"hreflang="en""#,
            r#"hreflang="fr""#,
            "no hreflang=en link",
        ),
        (
            "toc.html",
            "index.html",
            "other.html",
            "-> missing other.html",
        ),
        (
            "extra.html",
            "",
            r#"<html lang="en"></html>"#,
            "chapters differ",
        ),
    ];
    for (rel, from, to, reason) in cases {
        let root = scratch("site-check-bad");
        good_site(&root);
        let path = root.join(rel);
        let text = if from.is_empty() {
            to.to_owned()
        } else {
            let text = fs::read_to_string(&path).unwrap();
            assert!(text.contains(from), "{rel} lacks {from}");
            text.replacen(from, to, 1)
        };
        fs::write(&path, text).unwrap();
        let (code, stdout, stderr) = check_site(&root);
        assert_eq!(code, Some(1), "{rel}: {to}");
        assert!(stdout.is_empty(), "{stdout:?}");
        assert!(
            stderr.iter().any(|line| line.contains(reason)),
            "{rel}: expected {reason:?} in {stderr:?}"
        );
    }

    let root = scratch("site-check-empty");
    let (code, _, stderr) = check_site(&root);
    assert_eq!(code, Some(1));
    assert!(
        stderr.iter().any(|line| line.contains("missing .nojekyll")),
        "{stderr:?}"
    );
}

#[test]
fn site_build_refuses_foreign_outputs_bad_base_paths_and_other_mdbook_versions() {
    let dir = scratch("site-build");
    let fake = dir.join("mdbook");
    fs::write(&fake, "#!/bin/sh\necho 'mdbook v0.5.3'\n").unwrap();
    let chmod = Command::new("chmod")
        .arg("755")
        .arg(&fake)
        .status()
        .unwrap();
    assert!(chmod.success());
    let foreign = dir.join("foreign");
    write(&foreign, "keep.txt", "not a site");
    let a_file = dir.join("a-file");
    fs::write(&a_file, "").unwrap();
    let empty = dir.join("empty");
    fs::create_dir(&empty).unwrap();

    let cases: [(&[&str], &Path, &str); 5] = [
        (&[], &foreign, "is not empty and is not an earlier build"),
        (&[], &a_file, "exists and is not a directory"),
        (
            &["--base-path", "docs/"],
            &empty,
            "must start and end with /",
        ),
        (&["--base-path", "/a b/"], &empty, "may only use letters"),
        (&[], &empty, "need mdbook v0.5.4, but"),
    ];
    for (args, out, reason) in cases {
        let mut command = Command::new("bash");
        command
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/scripts/build_site.sh"
            ))
            .args(args)
            .arg(out)
            .env("MDBOOK", &fake);
        let (status, stdout, stderr) = process::run(command);
        assert_eq!(status.code(), Some(1), "{args:?} {out:?}: {stderr:?}");
        assert!(stdout.is_empty(), "{stdout:?}");
        assert!(
            stderr.iter().any(|line| line.contains(reason)),
            "expected {reason:?} in {stderr:?}"
        );
    }
    assert_eq!(
        fs::read_to_string(foreign.join("keep.txt")).unwrap(),
        "not a site"
    );
    assert_eq!(fs::read_dir(&empty).unwrap().count(), 0);
}
