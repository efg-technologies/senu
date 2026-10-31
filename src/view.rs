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
    pub site: &'static str,
    pub skip: &'static str,
    pub chapters: &'static str,
    pub pager: &'static str,
    pub previous: &'static str,
    pub next: &'static str,
    pub index_title: &'static str,
    pub intro: &'static str,
    pub footer: &'static str,
    /// The text before and after the number of minutes.
    minutes: (&'static str, &'static str),
}

impl Labels {
    /// The reading-time line, or nothing for an article without words.
    pub fn reading_time(&self, minutes: usize) -> String {
        match minutes {
            0 => String::new(),
            n => format!("{}{n}{}", self.minutes.0, self.minutes.1),
        }
    }
}

pub static ENGLISH: Labels = Labels {
    lang: "en",
    other_lang: "ja",
    other_language: "日本語",
    home: "/",
    site: "Senu notes",
    skip: "Skip to the article",
    chapters: "Chapters",
    pager: "Previous and next",
    previous: "Previous",
    next: "Next",
    index_title: "Build a small notes site with Rust",
    intro: "A course for complete beginners: you run this site on your own computer, \
            add notes to it in Markdown, and then rebuild a small piece of the Rust code \
            behind it. You need no Rust, only a little JavaScript.",
    footer: "Senu is an experimental seed, not production software. \
             Each page here is a Markdown file in content/en/ of the Senu repository.",
    minutes: ("About ", " min read"),
};

pub static JAPANESE: Labels = Labels {
    lang: "ja",
    other_lang: "en",
    other_language: "English",
    home: "/ja/",
    site: "Senu ノート",
    skip: "本文へ移動",
    chapters: "章の一覧",
    pager: "前後の章",
    previous: "前へ",
    next: "次へ",
    index_title: "Rust で小さなノートサイトを作る",
    intro: "まったくの初心者向けの講座です。このサイトを自分のコンピューターで動かし、\
            Markdown でノートを書き足し、最後にサイトを支える Rust のコードを少しだけ作り直します。\
            Rust の知識は要りません。JavaScript を少し知っていれば十分です。",
    footer: "Senu は実験的な試作で、本番用のソフトウェアではありません。\
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

/// What the index and article templates share.
#[derive(Debug)]
pub struct Frame<'a> {
    pub labels: &'static Labels,
    pub title: &'a str,
    /// The language link: the translation, or the other index.
    pub alternate: &'a str,
    pub nav: &'a [NavLink],
    pub previous: Option<&'a NavLink>,
    pub next: Option<&'a NavLink>,
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
