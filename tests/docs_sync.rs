//! DOC-2: both READMEs embed `scripts/quickstart.sh` byte for byte. NAV-1
//! (spec 0003): both languages have every chapter of the course, each
//! chapter links to its translation, and the two languages run the same
//! checked commands. BLD-1 (spec 0004): the CSS toolchain is pinned.

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

/// Spec 0004, §3 and BLD-1: exact versions, one script, and a lockfile
/// whose every downloaded package comes from the npm registry with an SRI
/// hash. An entry npm marks `inBundle` has neither, because it ships inside
/// the tarball of its bundling ancestor, whose own hash is checked here.
#[test]
fn the_css_toolchain_is_pinned_to_the_npm_registry() {
    let json = |path: &str| serde_json::from_str::<serde_json::Value>(&read(path)).unwrap();
    let pinned = [
        ("@tailwindcss/cli", "4.3.3"),
        ("@tailwindcss/typography", "0.5.20"),
        ("tailwindcss", "4.3.3"),
    ];
    let package = json("package.json");
    assert_eq!(package["private"], true);
    assert_eq!(
        package["scripts"],
        serde_json::json!({"css": "tailwindcss -i assets/ui.css -o assets/ui.generated.css --minify"})
    );
    // GHSA-rj75-hqrm-r3gf: Typography 0.5.20 pins the parser to 6.0.10.
    assert_eq!(
        package["overrides"],
        serde_json::json!({"@tailwindcss/typography": {"postcss-selector-parser": "6.1.5"}})
    );
    assert_eq!(
        package["devDependencies"],
        serde_json::Value::Object(
            pinned
                .iter()
                .map(|(name, version)| (name.to_string(), (*version).into()))
                .collect()
        )
    );
    let lock = json("package-lock.json");
    assert_eq!(lock["lockfileVersion"], 3);
    let packages = lock["packages"].as_object().unwrap();
    let mut counts = (0, 0);
    for (path, entry) in packages.iter().filter(|(path, _)| !path.is_empty()) {
        if entry["inBundle"] == true {
            let (ancestor, _) = path.rsplit_once("/node_modules/").unwrap();
            assert_ne!(packages[ancestor]["inBundle"], true, "{path}");
            assert!(packages[ancestor]["integrity"].is_string(), "{path}");
            counts.1 += 1;
            continue;
        }
        counts.0 += 1;
        let resolved = entry["resolved"].as_str().unwrap_or("");
        assert!(
            resolved.starts_with("https://registry.npmjs.org/"),
            "{path}: {resolved}"
        );
        let integrity = entry["integrity"].as_str().unwrap_or("");
        assert!(integrity.starts_with("sha512-"), "{path}: {integrity}");
    }
    // Recorded in docs/development.md: registry tarballs and bundled entries.
    assert_eq!(counts, (70, 6));
    assert_eq!(
        packages["node_modules/postcss-selector-parser"]["version"],
        "6.1.5"
    );
    let notices = read("THIRD-PARTY-NOTICES.txt");
    for (name, version) in pinned {
        let entry = &packages[&format!("node_modules/{name}")];
        assert_eq!(entry["version"], version, "{name}");
        assert_eq!(entry["license"], "MIT", "{name}");
        assert!(notices.contains(&format!("{name} {version}")), "{name}");
    }
    assert_eq!(notices.matches("MIT License").count(), 2);
}
