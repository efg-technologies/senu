//! HTML rendering with Askama.

use askama::Template;

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
        assert!(!page.contains("<li"));
    }

    #[test]
    fn template_comments_are_not_rendered() {
        let page = projects_page("", &[]).unwrap();
        assert!(!page.contains("<%"), "{page}");
        assert!(!page.contains("Askama"), "{page}");
    }
}
