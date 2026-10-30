//! DOC-2: both READMEs embed `scripts/quickstart.sh` byte for byte, and the
//! tutorial languages share chapters and snippets (spec 0002).

fn read(path: &str) -> String {
    let full = format!("{}/{path}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&full).unwrap_or_else(|err| panic!("{path}: {err}"))
}

#[test]
fn readmes_embed_the_quickstart_script_exactly() {
    let script = read("scripts/quickstart.sh");
    assert!(script.ends_with('\n'), "the script must end with a newline");
    let block = format!("```sh\n{script}```\n");
    for readme in ["README.md", "README.ja.md"] {
        let text = read(readme);
        assert_eq!(
            text.matches(&block).count(),
            1,
            "{readme} must embed scripts/quickstart.sh exactly once, byte for byte"
        );
    }
}

/// The chapters of one tutorial language, in SUMMARY.md order.
fn tutorial_chapters(dir: &str) -> Vec<String> {
    read(&format!("{dir}/SUMMARY.md"))
        .lines()
        .filter_map(|line| line.split_once("](")?.1.strip_suffix(')'))
        .map(str::to_owned)
        .collect()
}

/// The repository files and sections a chapter includes, in order. Every
/// line of a shell block must be an include, so no command is written out
/// by hand in either language.
fn tutorial_includes(dir: &str, chapter: &str) -> Vec<String> {
    let root = std::fs::canonicalize(env!("CARGO_MANIFEST_DIR")).unwrap();
    let base = root.join(dir);
    let mut includes = Vec::new();
    let mut in_shell = false;
    for line in read(&format!("{dir}/{chapter}")).lines() {
        if line.starts_with("```") {
            in_shell = line == "```sh";
        }
        let directive = line
            .strip_prefix("{{#include ")
            .and_then(|rest| rest.strip_suffix("}}"));
        if in_shell && !line.starts_with("```") {
            assert!(directive.is_some(), "{dir}/{chapter}: shell line {line:?}");
        }
        if let Some(target) = directive {
            let (path, section) = target.split_once(':').unwrap_or((target, ""));
            let full = std::fs::canonicalize(base.join(path))
                .unwrap_or_else(|err| panic!("{dir}/{chapter}: {path}: {err}"));
            let rel = full.strip_prefix(&root).unwrap().display();
            includes.push(format!("{rel}:{section}"));
        }
    }
    includes
}

#[test]
fn tutorial_languages_share_chapters_and_snippets() {
    let (en, ja) = ("docs/tutorial/src", "docs/tutorial/src/ja");
    let chapters = tutorial_chapters(en);
    assert_eq!(chapters.len(), 6, "{chapters:?}");
    assert_eq!(tutorial_chapters(ja), chapters, "chapter files and order");
    for chapter in &chapters {
        assert_eq!(
            tutorial_includes(ja, chapter),
            tutorial_includes(en, chapter),
            "{chapter}: the Japanese page must include the same snippets in the same order"
        );
    }
}
