//! DOC-2: both READMEs embed `scripts/quickstart.sh` byte for byte. NAV-1
//! (spec 0003): both languages have every chapter of the course, each
//! chapter links to its translation, and the two languages run the same
//! checked commands.

use senu::content::EMBEDDED;
use senu::site::Site;

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

/// The course's chapters, as in the table of spec 0003, §8. Learner notes
/// outside this table are ignored.
const CHAPTERS: [&str; 10] = [
    "01-start",
    "02-install",
    "03-first-page",
    "04-first-note",
    "05-when-senu-says-no",
    "06-first-function",
    "07-next-steps",
    "50-sample-note",
    "90-appendix-catalogue",
    "91-appendix-mcp",
];

fn slug(chapter: &str) -> &str {
    &chapter[3..]
}

#[test]
fn both_languages_have_every_chapter() {
    for locale in ["en", "ja"] {
        let names: Vec<String> = EMBEDDED
            .iter()
            .filter_map(|(path, _)| {
                path.strip_prefix(&format!("content/{locale}/"))?
                    .strip_suffix(".md")
            })
            .filter(|name| CHAPTERS.contains(name))
            .map(str::to_owned)
            .collect();
        assert_eq!(names, CHAPTERS, "content/{locale}");
    }
}

#[test]
fn every_chapter_links_to_its_translation() {
    let site = Site::load(EMBEDDED).expect("the embedded articles load");
    for chapter in CHAPTERS {
        let slug = slug(chapter);
        for (page, pair) in [
            (format!("/{slug}"), format!("/ja/{slug}")),
            (format!("/ja/{slug}"), format!("/{slug}")),
        ] {
            let html = site.page(&page).unwrap_or_else(|| panic!("no page {page}"));
            assert!(
                html.contains(&format!("<a href=\"{pair}\" hreflang=")),
                "{page} does not link to {pair}"
            );
        }
    }
}

/// The checked blocks of one article: the info string and, for commands,
/// patches and Rust files, the text. Japanese paths are mapped to English.
fn checked_blocks(locale: &str, chapter: &str) -> Vec<(String, Option<String>)> {
    let english = |text: &str| {
        text.replace("content/ja/", "content/en/")
            .replace("/ja/", "/")
    };
    let mut blocks = Vec::new();
    let mut open: Option<(String, String)> = None;
    for line in read(&format!("content/{locale}/{chapter}.md")).lines() {
        match open.take() {
            None => {
                if let Some(info) = line.strip_prefix("```") {
                    open = Some((info.to_owned(), String::new()));
                }
            }
            Some((info, body)) if line == "```" => {
                let words: Vec<&str> = info.split_whitespace().collect();
                let checked = words
                    .iter()
                    .any(|w| w.starts_with("check=") || w.starts_with("file="))
                    || words == ["diff", "patch"]
                    || words == ["sh", "manual"];
                let compared = words
                    .first()
                    .is_some_and(|lang| ["sh", "diff", "rust"].contains(lang));
                if checked {
                    blocks.push((english(&info), compared.then(|| english(&body))));
                }
            }
            Some((info, mut body)) => {
                body.push_str(line);
                body.push('\n');
                open = Some((info, body));
            }
        }
    }
    blocks
}

#[test]
fn both_languages_run_the_same_checked_blocks() {
    for chapter in CHAPTERS {
        let english = checked_blocks("en", chapter);
        let japanese = checked_blocks("ja", chapter);
        assert_eq!(
            japanese.iter().map(|b| &b.0).collect::<Vec<_>>(),
            english.iter().map(|b| &b.0).collect::<Vec<_>>(),
            "{chapter}: block kinds, names, exit statuses and paths"
        );
        assert_eq!(
            japanese, english,
            "{chapter}: commands, patches and Rust files"
        );
    }
}
