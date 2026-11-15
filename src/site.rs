//! The articles site (spec 0003, §3 and §6).
//!
//! [`Site::load`] is pure: it checks every file name, then renders each
//! article in path order through the Markdown policy, and pre-renders every
//! page into a map from its exact path to its HTML. The HTTP adapter only
//! looks pages up in that map.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde_json::{Value, json};

use crate::discover::MAX_FILE_BYTES;
use crate::markdown::{self, Counting, Outline, TrustedHtml};
use crate::reading::reading_minutes;
use crate::view::{self, ENGLISH, Frame, JAPANESE, Labels, NavGroup, NavLink, Section, Seo};

/// Slugs that would shadow Senu's own routes.
pub const RESERVED_SLUGS: [&str; 4] = ["api", "ja", "mcp", "projects"];

/// The longest slug, in bytes.
pub const MAX_SLUG_BYTES: usize = 48;

/// The longest description of a page, in characters.
pub const MAX_DESCRIPTION_CHARS: usize = 160;

/// The slug of the article whose question headings become a `FAQPage`.
pub const FAQ_SLUG: &str = "faq";

const REPOSITORY: &str = "https://github.com/efg-technologies/senu";
const LICENSE: &str = "https://www.apache.org/licenses/LICENSE-2.0";

/// The public address of the site's snapshot, such as
/// `https://example.org/senu/` (spec 0006, §6). With it, every page names
/// its canonical address, its translations and its structured data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BaseUrl(String);

impl BaseUrl {
    /// Accepts an absolute `https://` address that ends in `/`, of at most
    /// 200 visible ASCII characters, without a query, a fragment, quotes or
    /// angle brackets.
    pub fn new(url: &str) -> Option<Self> {
        let host = url.strip_prefix("https://")?;
        let plain = url
            .bytes()
            .all(|b| b.is_ascii_graphic() && !b"\"'<>?#\\`{}|^".contains(&b));
        let valid = plain
            && url.len() <= 200
            && url.ends_with('/')
            && host.starts_with(|c: char| c.is_ascii_alphanumeric())
            && !host.contains("//");
        valid.then(|| Self(url.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The address of `route` in the snapshot: `/` is the base itself,
    /// `/ja/` a folder and every article a `.html` file, as
    /// `scripts/snapshot_site.py` saves them.
    fn page(&self, route: &str) -> String {
        let path = route.trim_start_matches('/');
        if path.is_empty() || path.ends_with('/') {
            format!("{}{path}", self.0)
        } else {
            format!("{}{path}.html", self.0)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Locale {
    En,
    Ja,
}

impl Locale {
    fn labels(self) -> &'static Labels {
        match self {
            Self::En => &ENGLISH,
            Self::Ja => &JAPANESE,
        }
    }

    fn other(self) -> Self {
        match self {
            Self::En => Self::Ja,
            Self::Ja => Self::En,
        }
    }

    fn counting(self) -> Counting {
        match self {
            Self::En => Counting::Words,
            Self::Ja => Counting::Characters,
        }
    }

    fn route(self, slug: &str) -> String {
        format!("{}{slug}", self.labels().home)
    }
}

/// Why the articles cannot be served. It names the file, and the line when
/// the problem is inside the text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContentError {
    pub path: String,
    pub line: Option<usize>,
    pub reason: String,
}

impl fmt::Display for ContentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let path = self.path.escape_debug();
        match self.line {
            Some(line) => write!(f, "senu: {path}:{line}: {}", self.reason),
            None => write!(f, "senu: {path}: {}", self.reason),
        }
    }
}

impl std::error::Error for ContentError {}

/// Every page of the site, rendered ahead of time.
#[derive(Debug)]
pub struct Site {
    pages: BTreeMap<String, String>,
}

/// One article file with a valid name.
struct Entry<'a> {
    path: &'a str,
    locale: Locale,
    number: &'a str,
    slug: &'a str,
    /// The text without a leading byte order mark.
    text: &'a str,
}

impl Site {
    /// Checks and renders `(path, text)` pairs such as
    /// `("content/en/01-start.md", "# Start\n")`. Checks run in two phases,
    /// each in byte order of path: names, sizes and unique numbers and
    /// slugs, then titles, bodies and links. The first error of the first
    /// failing phase is returned, so a later bad name wins over an earlier
    /// missing title.
    pub fn load(files: &[(&str, &str)]) -> Result<Self, ContentError> {
        Self::load_with(files, None)
    }

    /// [`Site::load`] for a site whose public address is `base`.
    pub fn load_with(files: &[(&str, &str)], base: Option<&BaseUrl>) -> Result<Self, ContentError> {
        let mut files = files.to_vec();
        files.sort_unstable_by(|a, b| a.0.cmp(b.0));
        let mut entries: Vec<Entry<'_>> = Vec::with_capacity(files.len());
        for &(path, text) in &files {
            let entry = entry(path, text)?;
            for earlier in entries.iter().filter(|e| e.locale == entry.locale) {
                if earlier.number == entry.number {
                    let reason = format!(
                        "the number {} is already used by {}",
                        entry.number, earlier.path
                    );
                    return Err(whole_file(path, &reason));
                }
                if earlier.slug == entry.slug {
                    let reason = format!(
                        "the slug {} is already used by {}",
                        entry.slug, earlier.path
                    );
                    return Err(whole_file(path, &reason));
                }
            }
            entries.push(entry);
        }

        let mut routes: BTreeSet<String> = [ENGLISH.home, JAPANESE.home].map(str::to_owned).into();
        routes.extend(entries.iter().map(|e| e.locale.route(e.slug)));
        let mut articles = Vec::with_capacity(entries.len());
        for entry in &entries {
            let article = markdown::render(entry.text, &routes, entry.locale.counting()).map_err(
                |problem| ContentError {
                    path: entry.path.to_owned(),
                    line: Some(line_of(entry.text, problem.offset)),
                    reason: problem.reason,
                },
            )?;
            articles.push((entry, article, markdown::outline(entry.text)));
        }

        let mut pages = BTreeMap::new();
        for locale in [Locale::En, Locale::Ja] {
            let labels = locale.labels();
            // Display order: the sections in their order, then the numbers.
            let mut own: Vec<_> = articles
                .iter()
                .filter(|(e, _, _)| e.locale == locale)
                .collect();
            own.sort_by_key(|(e, _, _)| (Section::of(e.number), e.number));
            let links: Vec<NavLink> = own
                .iter()
                .map(|(entry, article, _)| NavLink {
                    href: locale.route(entry.slug),
                    title: article.title.clone(),
                    current: false,
                })
                .collect();
            let groups = |current: Option<usize>| -> Vec<NavGroup> {
                Section::ALL
                    .into_iter()
                    .map(|section| NavGroup {
                        labels: labels.section(section),
                        links: own
                            .iter()
                            .zip(&links)
                            .enumerate()
                            .filter(|(_, ((entry, _, _), _))| Section::of(entry.number) == section)
                            .map(|(i, (_, link))| NavLink {
                                current: current == Some(i),
                                ..link.clone()
                            })
                            .collect(),
                    })
                    .filter(|group| !group.links.is_empty())
                    .collect()
            };
            let other_index = locale.other().labels().home;
            let seo = base.map(|base| {
                let both = [locale, locale.other()].map(|l| l.labels().home);
                index_seo(base, labels, &both)
            });
            let index = Frame {
                labels,
                title: labels.index_title,
                head_title: labels.index_title,
                description: labels.description,
                alternate: other_index,
                nav: &groups(None),
                previous: None,
                next: links.first(),
                seo: seo.as_ref(),
            };
            pages.insert(
                labels.home.to_owned(),
                view::index_page(&index).map_err(unrendered)?,
            );

            let index_link = NavLink {
                href: labels.home.to_owned(),
                title: labels.index_title.to_owned(),
                current: false,
            };
            for (i, (entry, article, outline)) in own.iter().enumerate() {
                let route = &links[i].href;
                let pair = locale.other().route(entry.slug);
                let paired = routes.contains(&pair);
                let description = match outline.summary.as_str() {
                    "" => article.title.clone(),
                    summary => markdown::shorten(summary, MAX_DESCRIPTION_CHARS),
                };
                let head_title = format!("{} · {}", article.title, labels.site);
                let seo = base.map(|base| {
                    let page = Page {
                        labels,
                        route,
                        pair: paired.then_some(pair.as_str()),
                        title: &article.title,
                        description: &description,
                        section: labels.section(Section::of(entry.number)).heading,
                        questions: if entry.slug == FAQ_SLUG {
                            questions(outline)
                        } else {
                            Vec::new()
                        },
                    };
                    article_seo(base, &page)
                });
                let frame = Frame {
                    labels,
                    title: &article.title,
                    head_title: &head_title,
                    description: &description,
                    alternate: if paired { pair.as_str() } else { other_index },
                    nav: &groups(Some(i)),
                    previous: if i == 0 {
                        Some(&index_link)
                    } else {
                        links.get(i - 1)
                    },
                    next: links.get(i + 1),
                    seo: seo.as_ref(),
                };
                let reading = labels.reading_time(reading_minutes(article.words));
                let page =
                    view::article_page(&frame, &reading, &article.body).map_err(unrendered)?;
                pages.insert(route.clone(), page);
            }
        }
        Ok(Self { pages })
    }

    /// The page at exactly `path`, without any decoding or normalization.
    pub fn page(&self, path: &str) -> Option<&str> {
        self.pages.get(path).map(String::as_str)
    }

    /// Every path that has a page, in byte order.
    pub fn paths(&self) -> impl Iterator<Item = &str> {
        self.pages.keys().map(String::as_str)
    }
}

/// What the structured data of an article is built from.
struct Page<'a> {
    labels: &'static Labels,
    route: &'a str,
    /// The route of the translation, when there is one.
    pair: Option<&'a str>,
    title: &'a str,
    description: &'a str,
    section: &'a str,
    questions: Vec<(&'a str, &'a str)>,
}

/// The headings that are questions, each with its answer.
fn questions(outline: &Outline) -> Vec<(&str, &str)> {
    outline
        .sections
        .iter()
        .filter(|(heading, answer)| heading.ends_with(['?', '？']) && !answer.is_empty())
        .map(|(heading, answer)| (heading.as_str(), answer.as_str()))
        .collect()
}

fn publisher() -> Value {
    json!({
        "@type": "Organization",
        "name": "EFG technologies",
        "url": "https://efg-technologies.com",
    })
}

/// The language versions of a page, then `x-default`, which is English.
fn alternates(base: &BaseUrl, routes: &[(&'static str, &str)]) -> Vec<(&'static str, String)> {
    let mut alternates: Vec<(&'static str, String)> = routes
        .iter()
        .map(|(lang, route)| (*lang, base.page(route)))
        .collect();
    alternates.sort_by_key(|(lang, _)| *lang);
    if let Some((_, english)) = alternates.iter().find(|(lang, _)| *lang == "en") {
        alternates.push(("x-default", english.clone()));
    }
    alternates
}

/// The index describes the site and the software.
fn index_seo(base: &BaseUrl, labels: &'static Labels, homes: &[&'static str; 2]) -> Seo {
    let canonical = base.page(labels.home);
    let data = json!({
        "@context": "https://schema.org",
        "@graph": [
            {
                "@type": "WebSite",
                "@id": format!("{}#website", base.as_str()),
                "url": canonical,
                "name": labels.site,
                "description": labels.description,
                "inLanguage": labels.lang,
                "publisher": publisher(),
            },
            {
                "@type": "SoftwareSourceCode",
                "@id": format!("{}#software", base.as_str()),
                "name": "Senu",
                "description": labels.intro,
                "url": base.as_str(),
                "codeRepository": REPOSITORY,
                "programmingLanguage": "Rust",
                "license": LICENSE,
                "author": publisher(),
            },
        ],
    });
    Seo {
        alternates: alternates(
            base,
            &[(labels.lang, homes[0]), (labels.other_lang, homes[1])],
        ),
        canonical,
        kind: "website",
        json_ld: TrustedHtml::json_ld(&data),
        markdown: None,
        llms: format!("{}llms.txt", base.as_str()),
        icon: format!("{}icon.svg", base.as_str()),
    }
}

/// An article is a `TechArticle` under the index, and the FAQ also lists
/// its questions.
fn article_seo(base: &BaseUrl, page: &Page<'_>) -> Seo {
    let canonical = base.page(page.route);
    let home = base.page(page.labels.home);
    let mut graph = vec![
        json!({
            "@type": "TechArticle",
            "headline": page.title,
            "description": page.description,
            "inLanguage": page.labels.lang,
            "url": canonical,
            "mainEntityOfPage": canonical,
            "articleSection": page.section,
            "isPartOf": { "@id": format!("{}#website", base.as_str()) },
            "about": { "@id": format!("{}#software", base.as_str()) },
            "author": publisher(),
            "publisher": publisher(),
        }),
        json!({
            "@type": "BreadcrumbList",
            "itemListElement": [
                { "@type": "ListItem", "position": 1, "name": page.labels.site, "item": home },
                { "@type": "ListItem", "position": 2, "name": page.title, "item": canonical },
            ],
        }),
    ];
    if !page.questions.is_empty() {
        let questions: Vec<Value> = page
            .questions
            .iter()
            .map(|(question, answer)| {
                json!({
                    "@type": "Question",
                    "name": question,
                    "acceptedAnswer": { "@type": "Answer", "text": answer },
                })
            })
            .collect();
        graph.push(json!({ "@type": "FAQPage", "mainEntity": questions }));
    }
    let data = json!({ "@context": "https://schema.org", "@graph": graph });
    let mut routes = vec![(page.labels.lang, page.route)];
    routes.extend(page.pair.map(|pair| (page.labels.other_lang, pair)));
    Seo {
        alternates: alternates(base, &routes),
        markdown: canonical
            .strip_suffix(".html")
            .map(|page| format!("{page}.md")),
        canonical,
        kind: "article",
        json_ld: TrustedHtml::json_ld(&data),
        llms: format!("{}llms.txt", base.as_str()),
        icon: format!("{}icon.svg", base.as_str()),
    }
}

/// Checks the path, name and size, and removes a byte order mark.
fn entry<'a>(path: &'a str, text: &'a str) -> Result<Entry<'a>, ContentError> {
    let (locale, name) = if let Some(name) = path.strip_prefix("content/en/") {
        (Locale::En, name)
    } else if let Some(name) = path.strip_prefix("content/ja/") {
        (Locale::Ja, name)
    } else {
        return Err(whole_file(
            path,
            "articles must be in content/en/ or content/ja/",
        ));
    };
    let bad_name = || {
        whole_file(
            path,
            "the file name must be two digits, a hyphen, a slug of lowercase letters and digits \
             joined by single hyphens, then .md, such as 51-my-note.md",
        )
    };
    let stem = name.strip_suffix(".md").ok_or_else(bad_name)?;
    let (number, rest) = stem.split_at_checked(2).ok_or_else(bad_name)?;
    let slug = rest.strip_prefix('-').ok_or_else(bad_name)?;
    let word = |w: &str| {
        !w.is_empty()
            && w.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
    };
    if !number.bytes().all(|b| b.is_ascii_digit()) || !slug.split('-').all(word) {
        return Err(bad_name());
    }
    if slug.len() > MAX_SLUG_BYTES {
        return Err(whole_file(
            path,
            "the slug, the part after the number, is longer than 48 characters",
        ));
    }
    if RESERVED_SLUGS.contains(&slug) {
        return Err(whole_file(
            path,
            &format!("the slug {slug} is reserved for Senu's own routes"),
        ));
    }
    if text.len() as u64 > MAX_FILE_BYTES {
        return Err(whole_file(path, "is larger than 128 KiB"));
    }
    Ok(Entry {
        path,
        locale,
        number,
        slug,
        text: text.strip_prefix('\u{feff}').unwrap_or(text),
    })
}

fn whole_file(path: &str, reason: &str) -> ContentError {
    ContentError {
        path: path.to_owned(),
        line: None,
        reason: reason.to_owned(),
    }
}

fn unrendered(_: askama::Error) -> ContentError {
    whole_file("templates", "a page could not be rendered")
}

/// The 1-based line that contains the byte at `offset`.
fn line_of(text: &str, offset: usize) -> usize {
    text.as_bytes()[..offset.min(text.len())]
        .iter()
        .filter(|&&b| b == b'\n')
        .count()
        + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILES: [(&str, &str); 5] = [
        (
            "content/ja/01-first-note.md",
            "# 最初のノート\n\n本文です。\n",
        ),
        (
            "content/en/03-only-english.md",
            "# Only English\n\nThe end.\n",
        ),
        ("content/en/02-second-note.md", "# Second note\n\nText.\n"),
        (
            "content/en/01-first-note.md",
            "# First note\n\nSee [the second note](/second-note).\n",
        ),
        (
            "content/ja/02-second-note.md",
            "# 二つ目のノート\n\n[英語版](/second-note)\n",
        ),
    ];

    fn site() -> Site {
        Site::load(&FILES).expect("valid fixture")
    }

    fn page(site: &Site, path: &str) -> String {
        site.page(path)
            .unwrap_or_else(|| panic!("no page {path}"))
            .to_owned()
    }

    fn load_error(files: &[(&str, &str)]) -> String {
        Site::load(files).expect_err("refused").to_string()
    }

    /// The chapter list as (href, current) pairs.
    fn chapter_list(page: &str) -> Vec<(String, bool)> {
        let list = &page[page.find("<details").unwrap()..page.find("</details>").unwrap()];
        list.lines()
            .filter_map(|line| {
                let href = line.strip_prefix("<li><a href=\"")?;
                let end = href.find('"')?;
                Some((
                    href[..end].to_owned(),
                    line.contains("aria-current=\"page\""),
                ))
            })
            .collect()
    }

    /// The href of the first link whose start tag continues with `rest`
    /// right after the href, such as ` rel="next"`.
    fn link_followed_by(page: &str, rest: &str) -> Option<String> {
        page.split("<a href=\"").skip(1).find_map(|tag| {
            let end = tag.find('"')?;
            tag[end + 1..]
                .starts_with(rest)
                .then(|| tag[..end].to_owned())
        })
    }

    #[test]
    fn every_route_has_a_page() {
        assert_eq!(
            site().paths().collect::<Vec<_>>(),
            [
                "/",
                "/first-note",
                "/ja/",
                "/ja/first-note",
                "/ja/second-note",
                "/only-english",
                "/second-note"
            ]
        );
    }

    #[test]
    fn names_are_checked_and_each_error_names_the_file() {
        let rule = "the file name must be two digits";
        let cases = [
            ("content/en/1-a.md", rule),
            ("content/en/001-a.md", rule),
            ("content/en/ab-a.md", rule),
            ("content/en/01a.md", rule),
            ("content/en/01-.md", rule),
            ("content/en/01-A.md", rule),
            ("content/en/01_a.md", rule),
            ("content/en/01-a_b.md", rule),
            ("content/en/01-a--b.md", rule),
            ("content/en/01--a.md", rule),
            ("content/en/01-a-.md", rule),
            ("content/en/01-a b.md", rule),
            ("content/en/01-é.md", rule),
            ("content/en/01-a.markdown", rule),
            ("content/en/sub/01-a.md", rule),
            (
                "content/fr/01-a.md",
                "articles must be in content/en/ or content/ja/",
            ),
            (
                "content/01-a.md",
                "articles must be in content/en/ or content/ja/",
            ),
            ("content/en/01-api.md", "the slug api is reserved"),
            ("content/en/01-ja.md", "the slug ja is reserved"),
            ("content/ja/01-mcp.md", "the slug mcp is reserved"),
            ("content/en/01-projects.md", "the slug projects is reserved"),
        ];
        for (path, reason) in cases {
            let err = load_error(&[(path, "# T\n")]);
            assert!(err.starts_with(&format!("senu: {path}: {reason}")), "{err}");
        }
        let long = format!("content/en/01-{}.md", "a".repeat(49));
        assert!(load_error(&[(&long, "# T\n")]).contains("longer than 48 characters"));
        let longest = format!("content/en/01-{}.md", "a".repeat(48));
        for path in [
            longest.as_str(),
            "content/en/00-a.md",
            "content/ja/99-a1-b2-3c.md",
        ] {
            assert!(Site::load(&[(path, "# T\n")]).is_ok(), "{path}");
        }
        let err = load_error(&[("content/en/01-x\u{1b}.md", "# T\n")]);
        assert!(
            err.starts_with("senu: content/en/01-x\\u{1b}.md: "),
            "{err}"
        );
    }

    #[test]
    fn numbers_and_slugs_are_unique_within_a_language() {
        assert_eq!(
            load_error(&[
                ("content/en/01-b.md", "# B\n"),
                ("content/en/01-a.md", "# A\n")
            ]),
            "senu: content/en/01-b.md: the number 01 is already used by content/en/01-a.md"
        );
        assert_eq!(
            load_error(&[
                ("content/ja/02-a.md", "# B\n"),
                ("content/ja/01-a.md", "# A\n")
            ]),
            "senu: content/ja/02-a.md: the slug a is already used by content/ja/01-a.md"
        );
        assert!(
            Site::load(&[
                ("content/en/01-a.md", "# A\n"),
                ("content/ja/01-a.md", "# A\n")
            ])
            .is_ok()
        );
    }

    #[test]
    fn size_and_byte_order_mark() {
        let largest = format!("# T\n{}", "a".repeat(MAX_FILE_BYTES as usize - 4));
        assert!(Site::load(&[("content/en/01-a.md", &largest)]).is_ok());
        let larger = format!("{largest}a");
        assert_eq!(
            load_error(&[("content/en/01-a.md", &larger)]),
            "senu: content/en/01-a.md: is larger than 128 KiB"
        );
        let site = Site::load(&[("content/en/01-a.md", "\u{feff}# Marked\n")]).unwrap();
        assert!(page(&site, "/a").contains("<h1>Marked</h1>"));
        assert_eq!(
            load_error(&[("content/en/01-a.md", "\u{feff}Text\n")]),
            "senu: content/en/01-a.md:1: the first block must be a level-1 heading, such as `# My note`"
        );
    }

    #[test]
    fn content_errors_have_a_line_and_the_first_path_wins() {
        let files = [
            ("content/ja/01-b.md", "# B\n\n[x](javascript:alert(1))\n"),
            ("content/en/02-b.md", "# B\n\nText.\n\n[x](/ja/missing)\n"),
            ("content/en/01-a.md", "# A\n"),
        ];
        assert_eq!(
            load_error(&files),
            "senu: content/en/02-b.md:5: the link \"/ja/missing\" is neither a page of this site, \
             such as \"/\", nor an https:// address"
        );
        let names_first = [
            ("content/en/01-a.md", "no title\n"),
            ("content/en/02-B.md", "# B\n"),
        ];
        assert!(load_error(&names_first).starts_with("senu: content/en/02-B.md: the file name"));
    }

    #[test]
    fn links_may_point_to_any_page_of_either_language() {
        let site = site();
        assert!(
            page(&site, "/first-note").contains("<a href=\"/second-note\">the second note</a>")
        );
        assert!(page(&site, "/ja/second-note").contains("<a href=\"/second-note\">英語版</a>"));
    }

    #[test]
    fn the_chapter_list_follows_the_numbers_and_marks_the_current_page() {
        let site = site();
        let current = |path: &str| chapter_list(&page(&site, path));
        let english = ["/first-note", "/second-note", "/only-english"];
        assert_eq!(
            current("/second-note"),
            english.map(|href| (href.to_owned(), href == "/second-note"))
        );
        assert_eq!(current("/"), english.map(|href| (href.to_owned(), false)));
        assert_eq!(
            current("/ja/first-note"),
            [
                ("/ja/first-note".to_owned(), true),
                ("/ja/second-note".to_owned(), false)
            ]
        );
    }

    #[test]
    fn previous_and_next_links_at_both_ends() {
        let site = site();
        let pager = |path: &str| {
            let page = page(&site, path);
            (
                link_followed_by(&page, " rel=\"prev\""),
                link_followed_by(&page, " rel=\"next\""),
            )
        };
        let some = |href: &str| Some(href.to_owned());
        assert_eq!(pager("/"), (None, some("/first-note")));
        assert_eq!(pager("/first-note"), (some("/"), some("/second-note")));
        assert_eq!(
            pager("/second-note"),
            (some("/first-note"), some("/only-english"))
        );
        assert_eq!(pager("/only-english"), (some("/second-note"), None));
        assert_eq!(pager("/ja/"), (None, some("/ja/first-note")));
        assert_eq!(pager("/ja/second-note"), (some("/ja/first-note"), None));
    }

    #[test]
    fn the_language_link_goes_to_the_pair_or_the_other_index() {
        let site = site();
        let cases = [
            ("/", "/ja/", "en"),
            ("/ja/", "/", "ja"),
            ("/first-note", "/ja/first-note", "en"),
            ("/ja/first-note", "/first-note", "ja"),
            ("/only-english", "/ja/", "en"),
        ];
        for (path, alternate, lang) in cases {
            let page = page(&site, path);
            assert_eq!(
                link_followed_by(&page, " hreflang=").as_deref(),
                Some(alternate),
                "{path}"
            );
            assert!(
                page.trim_start()
                    .starts_with(&format!("<!DOCTYPE html>\n<html lang=\"{lang}\">")),
                "{path}"
            );
        }
    }

    #[test]
    fn labels_are_localized() {
        let site = site();
        let english = page(&site, "/first-note");
        let japanese = page(&site, "/ja/first-note");
        for label in [
            ENGLISH.skip,
            ENGLISH.chapters,
            ENGLISH.footer,
            ENGLISH.other_language,
        ] {
            assert!(english.contains(label), "{label}");
        }
        for label in [
            JAPANESE.skip,
            JAPANESE.chapters,
            JAPANESE.footer,
            JAPANESE.other_language,
        ] {
            assert!(japanese.contains(label), "{label}");
        }
        assert!(english.contains("<title>First note · Senu</title>"));
        assert!(japanese.contains("<title>最初のノート · Senu</title>"));
    }

    #[test]
    fn each_page_has_one_h1_main_and_chapter_nav() {
        let site = site();
        for path in site.paths() {
            let page = page(&site, path);
            assert_eq!(page.matches("<h1>").count(), 1, "{path}");
            assert_eq!(page.matches("<main id=\"main\"").count(), 1, "{path}");
            assert!(page.contains("<a href=\"#main\""), "{path}");
            assert!(page.contains("<meta name=\"viewport\""), "{path}");
        }
    }

    #[test]
    fn the_reading_time_is_shown_only_for_articles_with_words() {
        let long = format!("# Long\n\n{}\n", "word ".repeat(201));
        let site = Site::load(&[
            ("content/en/01-long.md", &long),
            ("content/en/02-empty.md", "# Empty\n"),
            (
                "content/ja/01-long.md",
                &format!("# 長い\n\n{}\n", "あ".repeat(402)),
            ),
        ])
        .unwrap();
        assert!(page(&site, "/long").contains(">About 2 min read</p>"));
        assert!(!page(&site, "/empty").contains("min read"));
        assert!(page(&site, "/ja/long").contains(">約 2 分で読めます</p>"));
    }

    #[test]
    fn the_body_is_inserted_once_and_the_title_is_escaped_once() {
        let site = Site::load(&[(
            "content/en/01-a.md",
            "# Tags <b> & co\n\nA *zz-body-marker* here.\n",
        )])
        .unwrap();
        let page = page(&site, "/a");
        assert_eq!(page.matches("<em>zz-body-marker</em>").count(), 1, "{page}");
        // Askama writes numeric character references.
        assert!(
            page.contains("<h1>Tags &#60;b&#62; &#38; co</h1>"),
            "{page}"
        );
        assert!(!page.contains("&#38;#"), "escaped twice: {page}");
        assert!(!page.contains("<b>"), "{page}");
    }

    // Spec 0006, §6: sections, descriptions and the public address.

    const SECTIONED: [(&str, &str); 7] = [
        ("content/en/01-start.md", "# Start\n\nThe course begins.\n"),
        ("content/en/10-overview.md", "# Overview\n\nWhat Senu is.\n"),
        (
            "content/en/11-faq.md",
            "# Questions & </script> answers\n\nShort answers.\n\n\
             ## Is it <b>safe</b>?\n\nIt is *not* audited & says so.\n\n\
             ## Limits\n\nNot a question.\n\n## Empty?\n\n## Why?\n\nBecause.\n",
        ),
        ("content/en/50-note.md", "# Note\n\n```sh\nonly code\n```\n"),
        ("content/en/90-appendix.md", "# Appendix\n\nExtra.\n"),
        ("content/ja/10-overview.md", "# 概要\n\nSenu とは何か。\n"),
        ("content/ja/12-only-japanese.md", "# 日本語だけ\n\n本文。\n"),
    ];

    fn base() -> BaseUrl {
        BaseUrl::new("https://example.org/senu/").expect("a valid base")
    }

    fn count(page: &str, needle: &str) -> usize {
        page.matches(needle).count()
    }

    /// The JSON-LD of a page, parsed.
    fn json_ld(page: &str) -> Value {
        let open = "<script type=\"application/ld+json\">";
        assert_eq!(count(page, open), 1, "{page}");
        let rest = &page[page.find(open).unwrap() + open.len()..];
        serde_json::from_str(&rest[..rest.find("</script>").unwrap()]).expect("valid JSON-LD")
    }

    fn of_type<'a>(data: &'a Value, kind: &str) -> Option<&'a Value> {
        data["@graph"]
            .as_array()
            .unwrap()
            .iter()
            .find(|node| node["@type"] == kind)
    }

    #[test]
    fn sections_order_the_contents_and_the_pager() {
        let site = Site::load(&SECTIONED).unwrap();
        let order = ["/overview", "/faq", "/start", "/note", "/appendix"];
        for path in ["/", "/start"] {
            let hrefs: Vec<String> = chapter_list(&page(&site, path))
                .into_iter()
                .map(|(href, _)| href)
                .collect();
            assert_eq!(hrefs, order, "{path}");
        }
        let start = page(&site, "/start");
        assert_eq!(
            link_followed_by(&start, " rel=\"prev\""),
            Some("/faq".to_owned())
        );
        assert_eq!(
            link_followed_by(&start, " rel=\"next\""),
            Some("/note".to_owned())
        );
        let index = page(&site, "/");
        let positions: Vec<usize> = Section::ALL
            .iter()
            .map(|section| {
                let heading = format!("<h2>{}</h2>", ENGLISH.section(*section).heading);
                index
                    .find(&heading)
                    .unwrap_or_else(|| panic!("no {heading}"))
            })
            .collect();
        assert!(positions.is_sorted(), "{positions:?}");
        // A section without articles has no heading.
        let japanese = page(&site, "/ja/");
        assert!(japanese.contains(JAPANESE.section(Section::Guide).heading));
        assert!(!japanese.contains(JAPANESE.section(Section::Course).heading));
        assert_eq!(Section::of("09"), Section::Course);
        assert_eq!(Section::of("10"), Section::Guide);
        assert_eq!(Section::of("49"), Section::Guide);
        assert_eq!(Section::of("50"), Section::Notes);
        assert_eq!(Section::of("89"), Section::Notes);
        assert_eq!(Section::of("90"), Section::Appendix);
    }

    #[test]
    fn every_page_has_one_description_from_its_first_paragraph() {
        let long = format!("# Long\n\n{}\n", "word ".repeat(80));
        let mut files = SECTIONED.to_vec();
        files.push(("content/en/13-long.md", &long));
        let site = Site::load(&files).unwrap();
        for path in site.paths() {
            let page = page(&site, path);
            assert_eq!(count(&page, "<meta name=\"description\""), 1, "{path}");
        }
        let description = |path: &str| {
            let page = page(&site, path);
            let open = "<meta name=\"description\" content=\"";
            let rest = &page[page.find(open).unwrap() + open.len()..];
            rest[..rest.find("\">").unwrap()].to_owned()
        };
        assert_eq!(description("/overview"), "What Senu is.");
        assert_eq!(description("/ja/overview"), "Senu とは何か。");
        // Without a paragraph, the title; and markup is escaped.
        assert_eq!(description("/note"), "Note");
        assert_eq!(description("/faq"), "Short answers.");
        let cut = description("/long");
        assert!(cut.chars().count() <= MAX_DESCRIPTION_CHARS, "{cut}");
        assert!(cut.ends_with("word…"), "{cut}");
        for labels in [&ENGLISH, &JAPANESE] {
            let length = labels.description.chars().count();
            assert!((50..=MAX_DESCRIPTION_CHARS).contains(&length), "{length}");
            assert!(page(&site, labels.home).contains(labels.description));
        }
    }

    #[test]
    fn without_a_base_url_no_page_names_an_absolute_address() {
        let site = Site::load(&SECTIONED).unwrap();
        for path in site.paths() {
            let page = page(&site, path);
            for absent in [
                "rel=\"canonical\"",
                "hreflang=\"x-default\"",
                "og:",
                "ld+json",
                "text/markdown",
                "llms.txt",
                "rel=\"icon\"",
            ] {
                assert!(!page.contains(absent), "{path} has {absent}");
            }
        }
    }

    #[test]
    fn with_a_base_url_pages_name_their_address_translations_and_data() {
        let site = Site::load_with(&SECTIONED, Some(&base())).unwrap();
        let link = |page: &str, lang: &str| {
            let open = format!("<link rel=\"alternate\" hreflang=\"{lang}\" href=\"");
            page.find(&open).map(|at| {
                let rest = &page[at + open.len()..];
                rest[..rest.find('"').unwrap()].to_owned()
            })
        };
        let some = |url: &str| Some(url.to_owned());
        for path in site.paths() {
            let page = page(&site, path);
            assert_eq!(count(&page, "<link rel=\"canonical\""), 1, "{path}");
            assert_eq!(count(&page, "property=\"og:url\""), 1, "{path}");
            let icon = "<link rel=\"icon\" type=\"image/svg+xml\" \
                        href=\"https://example.org/senu/icon.svg\">";
            assert_eq!(count(&page, icon), 1, "{path}");
            json_ld(&page);
        }

        let index = page(&site, "/");
        assert!(index.contains("<link rel=\"canonical\" href=\"https://example.org/senu/\">"));
        assert_eq!(link(&index, "en"), some("https://example.org/senu/"));
        assert_eq!(link(&index, "ja"), some("https://example.org/senu/ja/"));
        assert_eq!(link(&index, "x-default"), some("https://example.org/senu/"));
        assert!(index.contains("property=\"og:type\" content=\"website\""));
        assert!(!index.contains("type=\"text/markdown\""));
        let llms = format!(
            "<li><a href=\"https://example.org/senu/llms.txt\">{}</a></li>",
            ENGLISH.llms
        );
        assert!(index.contains(&llms), "{index}");
        let data = json_ld(&index);
        let software = of_type(&data, "SoftwareSourceCode").expect("the software");
        assert_eq!(software["codeRepository"], REPOSITORY);
        assert_eq!(software["programmingLanguage"], "Rust");
        assert!(of_type(&data, "WebSite").is_some());

        let japanese = page(&site, "/ja/overview");
        let canonical = "https://example.org/senu/ja/overview.html";
        assert!(japanese.contains(&format!("<link rel=\"canonical\" href=\"{canonical}\">")));
        assert_eq!(link(&japanese, "ja"), some(canonical));
        assert_eq!(
            link(&japanese, "en"),
            some("https://example.org/senu/overview.html")
        );
        assert_eq!(
            link(&japanese, "x-default"),
            some("https://example.org/senu/overview.html")
        );
        assert!(japanese.contains("property=\"og:locale\" content=\"ja_JP\""));
        assert!(japanese.contains(
            "<link rel=\"alternate\" type=\"text/markdown\" \
             href=\"https://example.org/senu/ja/overview.md\">"
        ));
        let data = json_ld(&japanese);
        let article = of_type(&data, "TechArticle").expect("an article");
        assert_eq!(article["headline"], "概要");
        assert_eq!(article["inLanguage"], "ja");
        assert_eq!(article["url"], canonical);
        assert_eq!(article["articleSection"], "ライブラリガイド");
        let crumbs = &of_type(&data, "BreadcrumbList").unwrap()["itemListElement"];
        assert_eq!(crumbs[0]["item"], "https://example.org/senu/ja/");
        assert_eq!(crumbs[1]["item"], canonical);
        assert!(of_type(&data, "FAQPage").is_none());

        // A page without a translation names only itself.
        let alone = page(&site, "/ja/only-japanese");
        assert_eq!(link(&alone, "en"), None);
        assert_eq!(link(&alone, "x-default"), None);
        assert!(link(&alone, "ja").is_some());
        let alone = page(&site, "/start");
        assert_eq!(link(&alone, "ja"), None);
        assert_eq!(
            link(&alone, "x-default"),
            some("https://example.org/senu/start.html")
        );
    }

    #[test]
    fn the_faq_lists_its_questions_and_no_text_can_leave_the_script() {
        let site = Site::load_with(&SECTIONED, Some(&base())).unwrap();
        let faq = page(&site, "/faq");
        let data = json_ld(&faq);
        let questions = &of_type(&data, "FAQPage").expect("the FAQ")["mainEntity"];
        let pairs: Vec<(&str, &str)> = questions
            .as_array()
            .unwrap()
            .iter()
            .map(|q| {
                (
                    q["name"].as_str().unwrap(),
                    q["acceptedAnswer"]["text"].as_str().unwrap(),
                )
            })
            .collect();
        assert_eq!(
            pairs,
            [
                ("Is it <b>safe</b>?", "It is not audited & says so."),
                ("Why?", "Because."),
            ]
        );
        let article = of_type(&data, "TechArticle").unwrap();
        assert_eq!(article["headline"], "Questions & </script> answers");
        // One script element, and the hostile title appears only escaped.
        assert_eq!(count(&faq, "</script>"), 1, "{faq}");
        assert_eq!(count(&faq, "<script"), 1, "{faq}");
        assert!(!faq.contains("<b>safe</b>"), "{faq}");
    }

    #[test]
    fn base_urls_are_checked_and_map_routes_to_the_saved_files() {
        let base = base();
        assert_eq!(base.page("/"), "https://example.org/senu/");
        assert_eq!(base.page("/ja/"), "https://example.org/senu/ja/");
        assert_eq!(
            base.page("/install"),
            "https://example.org/senu/install.html"
        );
        assert_eq!(
            base.page("/ja/install"),
            "https://example.org/senu/ja/install.html"
        );
        assert!(BaseUrl::new("https://example.org/").is_some());
        for bad in [
            "",
            "https://",
            "https:///",
            "http://example.org/",
            "https://example.org",
        ] {
            assert!(BaseUrl::new(bad).is_none(), "{bad}");
        }
    }
}
