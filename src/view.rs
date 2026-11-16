//! HTML rendering with Askama. Every template value is escaped, except the
//! article body, which is `TrustedHtml` from the Markdown policy.

use askama::Template;

use crate::markdown::TrustedHtml;
use crate::operation::ProjectSummary;

#[derive(Template)]
#[template(path = "projects.html")]
struct ProjectsPage<'a> {
    q: &'a str,
    projects: &'a [ProjectSummary],
}

/// Renders the project list page.
pub fn projects_page(q: &str, projects: &[ProjectSummary]) -> Result<String, askama::Error> {
    ProjectsPage { q, projects }.render()
}

#[derive(Template)]
#[template(path = "project.html")]
struct ProjectPage<'a> {
    project: &'a ProjectSummary,
}

/// Renders one project's page.
pub fn project_page(project: &ProjectSummary) -> Result<String, askama::Error> {
    ProjectPage { project }.render()
}

/// The fixed labels of the article pages in one language.
#[derive(Debug)]
pub struct Labels {
    pub lang: &'static str,
    pub other_lang: &'static str,
    /// The name of the other language, in that language.
    pub other_language: &'static str,
    /// The index of this language.
    pub home: &'static str,
    /// The Open Graph locale, such as `en_US`.
    pub locale: &'static str,
    pub site: &'static str,
    pub skip: &'static str,
    pub chapters: &'static str,
    pub pager: &'static str,
    pub previous: &'static str,
    pub next: &'static str,
    pub index_title: &'static str,
    /// What Senu is, in one paragraph: the index's lead.
    pub intro: &'static str,
    /// The same in one sentence of at most 160 characters: the index's
    /// description for search results.
    pub description: &'static str,
    /// The heading above the install command on the index.
    pub install: &'static str,
    /// The sentence before the command that adds Senu from its Git tag.
    pub install_git: &'static str,
    /// The links under the install command: (text, address).
    pub links: [(&'static str, &'static str); 3],
    /// The text of the index's link to `llms.txt`, shown on the public site.
    pub llms: &'static str,
    /// The groups of articles, in the order of [`Section`].
    pub sections: [SectionLabels; 4],
    pub footer: &'static str,
    /// The text before and after the number of minutes.
    minutes: (&'static str, &'static str),
}

/// The heading of a group of articles and its introduction on the index.
#[derive(Debug)]
pub struct SectionLabels {
    pub heading: &'static str,
    pub intro: &'static str,
}

/// The group an article belongs to, by its number, in display order: the
/// library guide (10 to 49), the course (00 to 09), the notes (50 to 89)
/// and the appendices (90 to 99).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Section {
    Guide,
    Course,
    Notes,
    Appendix,
}

impl Section {
    pub const ALL: [Self; 4] = [Self::Guide, Self::Course, Self::Notes, Self::Appendix];

    /// The section of an article number such as `07`.
    pub fn of(number: &str) -> Self {
        match number.parse::<u8>().unwrap_or(0) {
            10..=49 => Self::Guide,
            50..=89 => Self::Notes,
            90.. => Self::Appendix,
            _ => Self::Course,
        }
    }
}

impl Labels {
    /// The reading-time line, or nothing for an article without words.
    pub fn reading_time(&self, minutes: usize) -> String {
        match minutes {
            0 => String::new(),
            n => format!("{}{n}{}", self.minutes.0, self.minutes.1),
        }
    }

    pub fn section(&self, section: Section) -> &SectionLabels {
        &self.sections[section as usize]
    }
}

const REPOSITORY: &str = "https://github.com/efg-technologies/senu";
const DOCS_RS: &str = "https://docs.rs/senu";
const CRATES_IO: &str = "https://crates.io/crates/senu";

pub static ENGLISH: Labels = Labels {
    lang: "en",
    other_lang: "ja",
    other_language: "日本語",
    home: "/",
    locale: "en_US",
    site: "Senu",
    skip: "Skip to the article",
    chapters: "Contents",
    pager: "Previous and next",
    previous: "Previous",
    next: "Next",
    index_title: "Senu: one Rust function as a web page, a JSON API and an MCP tool",
    intro: "Senu is an open-source Rust library. You declare a function once, a read or a write, \
            with its permission and parameters, and Senu serves it as an HTML page, \
            a JSON API and an MCP tool for AI assistants, with the same authentication order, \
            the same errors and one contract test suite for all three. \
            It is built on Axum, Askama and the official rmcp SDK.",
    description: "Senu is a Rust library that serves one declared function as an HTML page, \
                  a JSON API and an MCP tool, with shared authentication, errors and contract tests.",
    install: "Add it to a project",
    install_git: "Or build it from the Git tag instead of crates.io:",
    links: [
        ("Source on GitHub", REPOSITORY),
        ("API reference on docs.rs", DOCS_RS),
        ("Crate on crates.io", CRATES_IO),
    ],
    llms: "llms.txt: this site as plain text for AI assistants",
    sections: [
        SectionLabels {
            heading: "Library guide",
            intro: "How to declare an operation, authenticate callers, expose MCP tools, \
                    deploy and test. Start here if you already write Rust.",
        },
        SectionLabels {
            heading: "Course: learn Rust with Senu",
            intro: "A course for complete beginners: you run this site on your own computer, \
                    add notes to it in Markdown, and then rebuild a small piece of the Rust code \
                    behind it. You need no Rust, only a little JavaScript.",
        },
        SectionLabels {
            heading: "Notes",
            intro: "Sample notes, and the place where the course has you add your own.",
        },
        SectionLabels {
            heading: "Appendices",
            intro: "Optional tours of the demo catalogue, its JSON API and MCP.",
        },
    ],
    footer: "Senu is open source under the Apache-2.0 license and is not audited security software. \
             Each page here is a Markdown file in content/en/ of the Senu repository.",
    minutes: ("About ", " min read"),
};

pub static JAPANESE: Labels = Labels {
    lang: "ja",
    other_lang: "en",
    other_language: "English",
    home: "/ja/",
    locale: "ja_JP",
    site: "Senu",
    skip: "本文へ移動",
    chapters: "目次",
    pager: "前後のページ",
    previous: "前へ",
    next: "次へ",
    index_title: "Senu: Rust の関数ひとつを Web ページ・JSON API・MCP ツールに",
    intro: "Senu はオープンソースの Rust ライブラリです。読み取りでも書き込みでも、関数を権限とパラメータと一緒に一度だけ宣言すると、\
            Senu がそれを HTML ページ、JSON API、AI アシスタント向けの MCP ツールとして提供します。\
            認証の順序もエラーも 3 つで同じで、同じ契約テストで確かめられます。\
            Axum、Askama、公式の rmcp SDK の上に作られています。",
    description: "Senu は、一度宣言した関数を HTML ページ・JSON API・MCP ツールとして提供する Rust ライブラリです。\
                  認証・エラー・契約テストを 3 つで共有します。",
    install: "プロジェクトに追加する",
    install_git: "crates.io の代わりに、Git のタグからビルドすることもできます。",
    links: [
        ("GitHub のソースコード", REPOSITORY),
        ("docs.rs の API リファレンス", DOCS_RS),
        ("crates.io のクレート", CRATES_IO),
    ],
    llms: "llms.txt: AI アシスタント向けに、このサイトをプレーンテキストにしたもの (英語)",
    sections: [
        SectionLabels {
            heading: "ライブラリガイド",
            intro: "操作の宣言、呼び出し元の認証、MCP ツールの公開、デプロイ、テストの方法です。\
                    すでに Rust を書く人はここから始めてください。",
        },
        SectionLabels {
            heading: "講座: Senu で学ぶ Rust",
            intro: "まったくの初心者向けの講座です。このサイトを自分のコンピューターで動かし、\
                    Markdown でノートを書き足し、最後にサイトを支える Rust のコードを少しだけ作り直します。\
                    Rust の知識は要りません。JavaScript を少し知っていれば十分です。",
        },
        SectionLabels {
            heading: "ノート",
            intro: "サンプルのノートです。講座では、ここに自分のノートを足します。",
        },
        SectionLabels {
            heading: "付録",
            intro: "デモのカタログ、JSON API、MCP を見て回る任意の章です。",
        },
    ],
    footer: "Senu は Apache-2.0 ライセンスのオープンソースで、監査済みのセキュリティソフトウェアではありません。\
             ここの各ページは、Senu リポジトリの content/ja/ にある Markdown ファイルです。",
    minutes: ("約 ", " 分で読めます"),
};

/// A link in the chapter list or the previous and next links.
#[derive(Clone, Debug)]
pub struct NavLink {
    pub href: String,
    pub title: String,
    pub current: bool,
}

/// One group of the contents: a section's labels and its articles.
#[derive(Clone, Debug)]
pub struct NavGroup {
    pub labels: &'static SectionLabels,
    pub links: Vec<NavLink>,
}

/// The absolute addresses and structured data of a page, present only when
/// the site knows its public base URL (spec 0006, §6).
#[derive(Debug)]
pub struct Seo {
    pub canonical: String,
    /// `hreflang` and address of every language version, then `x-default`.
    pub alternates: Vec<(&'static str, String)>,
    /// The Open Graph type: `website` or `article`.
    pub kind: &'static str,
    /// The JSON-LD script element.
    pub json_ld: TrustedHtml,
    /// The article's Markdown source in the snapshot; `None` on an index.
    pub markdown: Option<String>,
    /// The snapshot's `llms.txt`.
    pub llms: String,
    /// The snapshot's icon, so that a browser does not ask the host for
    /// `/favicon.ico`.
    pub icon: String,
}

/// What the index and article templates share.
#[derive(Debug)]
pub struct Frame<'a> {
    pub labels: &'static Labels,
    pub title: &'a str,
    /// The text of the `<title>` element.
    pub head_title: &'a str,
    /// The page's description, as plain text.
    pub description: &'a str,
    /// The language link: the translation, or the other index.
    pub alternate: &'a str,
    pub nav: &'a [NavGroup],
    pub previous: Option<&'a NavLink>,
    pub next: Option<&'a NavLink>,
    pub seo: Option<&'a Seo>,
}

#[derive(Template)]
#[template(path = "site_index.html")]
struct IndexPage<'a> {
    frame: &'a Frame<'a>,
}

#[derive(Template)]
#[template(path = "site_article.html")]
struct ArticlePage<'a> {
    frame: &'a Frame<'a>,
    reading: &'a str,
    body: &'a TrustedHtml,
}

/// Renders the index of one language.
pub fn index_page(frame: &Frame<'_>) -> Result<String, askama::Error> {
    IndexPage { frame }.render()
}

/// Renders one article around its pre-rendered body.
pub fn article_page(
    frame: &Frame<'_>,
    reading: &str,
    body: &TrustedHtml,
) -> Result<String, askama::Error> {
    ArticlePage {
        frame,
        reading,
        body,
    }
    .render()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(id: &str, name: &str) -> ProjectSummary {
        ProjectSummary {
            id: id.to_owned(),
            name: name.to_owned(),
        }
    }

    /// Decodes the entities Askama may emit, without assuming which form
    /// (named or numeric) it picks.
    fn unescape(text: &str) -> String {
        let mut out = String::new();
        let mut rest = text;
        while let Some(amp) = rest.find('&') {
            out.push_str(&rest[..amp]);
            let tail = &rest[amp..];
            let decoded = tail.find(';').and_then(|semi| {
                let entity = &tail[1..semi];
                let c = match entity {
                    "amp" => Some('&'),
                    "lt" => Some('<'),
                    "gt" => Some('>'),
                    "quot" => Some('"'),
                    "apos" => Some('\''),
                    _ => entity
                        .strip_prefix("#x")
                        .or_else(|| entity.strip_prefix("#X"))
                        .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                        .or_else(|| entity.strip_prefix('#').and_then(|dec| dec.parse().ok()))
                        .and_then(char::from_u32),
                };
                c.map(|c| (c, semi + 1))
            });
            match decoded {
                Some((c, len)) => {
                    out.push(c);
                    rest = &tail[len..];
                }
                None => {
                    out.push('&');
                    rest = &tail[1..];
                }
            }
        }
        out.push_str(rest);
        out
    }

    fn value_attribute(page: &str) -> String {
        let start = page.find("value=\"").expect("value attribute") + "value=\"".len();
        let end = start + page[start..].find('"').expect("closing quote");
        unescape(&page[start..end])
    }

    #[test]
    fn markup_in_project_names_is_escaped() {
        let evil = "<script>alert('x')</script> & \"Launch\" Notes";
        let page = projects_page("", &[project("acme-003", evil)]).unwrap();
        assert!(!page.contains("<script>"), "{page}");
        let start = page.find("<li data-project-id=\"acme-003\">").unwrap();
        let name_start = start + "<li data-project-id=\"acme-003\">".len();
        let name_end = name_start + page[name_start..].find("</li>").unwrap();
        let raw = &page[name_start..name_end];
        assert!(!raw.contains(['<', '>', '"']), "{raw}");
        assert_eq!(unescape(raw), evil);
    }

    #[test]
    fn each_project_is_a_list_item_with_its_id() {
        let page = projects_page(
            "",
            &[
                project("acme-001", "Apollo Launch"),
                project("acme-002", "Café Roadmap"),
            ],
        )
        .unwrap();
        assert!(page.contains("<li data-project-id=\"acme-001\">Apollo Launch</li>"));
        assert!(page.contains("<li data-project-id=\"acme-002\">Café Roadmap</li>"));
        assert!(page.find("acme-001").unwrap() < page.find("acme-002").unwrap());
    }

    #[test]
    fn search_is_escaped_inside_the_value_attribute() {
        let q = "<b>\"x\"";
        let page = projects_page(q, &[]).unwrap();
        assert!(!page.contains(q), "{page}");
        assert!(!page.contains("<b>"), "{page}");
        assert_eq!(value_attribute(&page), q);
    }

    #[test]
    fn search_appears_only_in_the_value_attribute() {
        let q = "zz-unique-search-marker";
        let page = projects_page(q, &[]).unwrap();
        assert_eq!(page.matches(q).count(), 1, "{page}");
        assert!(page.contains(&format!("value=\"{q}\"")), "{page}");
    }

    #[test]
    fn page_has_language_title_banner_and_labelled_search() {
        let page = projects_page("", &[]).unwrap();
        for needle in [
            "<!DOCTYPE html>",
            "<html lang=\"en\">",
            "<meta charset=\"utf-8\">",
            "<title>",
            "Local demo, not production",
            "<label for=\"q\">",
            "id=\"q\"",
            "name=\"q\"",
            "<form method=\"get\" action=\"/projects\"",
        ] {
            assert!(page.contains(needle), "page lacks {needle}");
        }
    }

    /// The search input's opening tag.
    fn search_input(page: &str) -> &str {
        let start = page.find("<input id=\"q\"").expect("search input");
        let end = start + page[start..].find('>').expect("end of input tag");
        &page[start..=end]
    }

    #[test]
    fn search_input_leaves_length_rules_to_the_server() {
        // Browsers count `maxlength`/`minlength` in UTF-16 code units of the
        // untrimmed value, but `list_projects` counts Unicode scalars after
        // trimming, so any client-side rule would reject valid searches.
        let page = projects_page("", &[]).unwrap();
        let input = search_input(&page);
        for attribute in ["maxlength", "minlength", "pattern", "required"] {
            assert!(!input.contains(attribute), "{input}");
        }
    }

    #[test]
    fn empty_results_are_announced() {
        let page = projects_page("zephyr", &[]).unwrap();
        assert!(page.contains("No projects match"));
        // `<li` alone would also match the stylesheet's `<link`.
        assert!(!page.contains("<li ") && !page.contains("<li>"), "{page}");
    }

    #[test]
    fn template_comments_are_not_rendered() {
        let page = projects_page("", &[]).unwrap();
        assert!(!page.contains("<%"), "{page}");
        assert!(!page.contains("Askama"), "{page}");
    }
}
