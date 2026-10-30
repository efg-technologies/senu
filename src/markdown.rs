//! The Markdown policy (spec 0003, §4 and §5).
//!
//! pulldown-cmark parses the article; every event then passes an explicit
//! allow-list before `pulldown_cmark::html::push_html` writes HTML, which
//! escapes text and attributes. Raw HTML is shown as text, the info string
//! of a code block is dropped, and links must point at an existing page of
//! the site or at an `https://` address. Anything the policy does not list
//! is an error, so new parser features fail closed.

use std::collections::BTreeSet;
use std::fmt;
use std::ops::Range;

use pulldown_cmark::{
    Alignment, CodeBlockKind, Event, HeadingLevel, LinkType, Options, Parser, Tag, TagEnd,
};

/// The only parser extension: tables. Every other one stays off.
pub const OPTIONS: Options = Options::ENABLE_TABLES;

/// The longest title, in characters.
pub const MAX_TITLE_CHARS: usize = 80;

/// HTML written by [`render`] from allowed events only. The field is
/// private, so no other code can construct one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrustedHtml(String);

impl TrustedHtml {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TrustedHtml {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

// The single trusted-HTML boundary (spec 0003, SAFE-4): Askama inserts this
// type without escaping. Every other template value is escaped.
impl askama::filters::HtmlSafe for TrustedHtml {}

/// How words are counted for the reading time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Counting {
    /// Runs of visible text separated by whitespace, line breaks or block
    /// boundaries, for English. Emphasis, links, inline code and raw HTML
    /// shown as text do not split a run.
    Words,
    /// Non-whitespace characters divided by two, for Japanese, which has
    /// no spaces between words.
    Characters,
}

/// A rendered article.
#[derive(Debug)]
pub struct Article {
    pub title: String,
    pub body: TrustedHtml,
    /// A rough word count of the body outside code blocks.
    pub words: usize,
}

/// Why an article was refused, at a byte offset of its text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Problem {
    pub offset: usize,
    pub reason: String,
}

/// Renders an article whose first block is its level-1 title. `routes` are
/// the exact paths an internal link may point at.
pub fn render(
    text: &str,
    routes: &BTreeSet<String>,
    counting: Counting,
) -> Result<Article, Problem> {
    let mut events = Parser::new_ext(text, OPTIONS).into_offset_iter();
    let title = title(&mut events)?;
    let mut policy = Policy::new(routes, counting);
    let mut allowed = Vec::new();
    for (event, range) in events {
        let event = policy.event(event).map_err(|reason| Problem {
            offset: range.start,
            reason,
        })?;
        allowed.push(event);
    }
    let mut html = String::new();
    pulldown_cmark::html::push_html(&mut html, allowed.into_iter());
    Ok(Article {
        title,
        body: TrustedHtml(html),
        words: policy.words + policy.characters / 2,
    })
}

/// Reads the level-1 heading that must open the article. Inline code and
/// inline HTML in it become literal text.
fn title<'a>(
    events: &mut impl Iterator<Item = (Event<'a>, Range<usize>)>,
) -> Result<String, Problem> {
    let problem = |range: &Range<usize>, reason: &str| Problem {
        offset: range.start,
        reason: reason.to_owned(),
    };
    let heading = match events.next() {
        Some((
            Event::Start(Tag::Heading {
                level: HeadingLevel::H1,
                ..
            }),
            range,
        )) => range,
        other => {
            let range = other.map_or(0..0, |(_, range)| range);
            return Err(problem(
                &range,
                "the first block must be a level-1 heading, such as `# My note`",
            ));
        }
    };
    let mut title = String::new();
    for (event, range) in events.by_ref() {
        match event {
            Event::End(TagEnd::Heading(_)) => break,
            Event::Text(text) | Event::Code(text) | Event::InlineHtml(text) => {
                title.push_str(&text);
            }
            Event::SoftBreak | Event::HardBreak => title.push(' '),
            Event::Start(Tag::Emphasis | Tag::Strong)
            | Event::End(TagEnd::Emphasis | TagEnd::Strong) => {}
            Event::Start(Tag::Link { .. } | Tag::Image { .. }) => {
                return Err(problem(
                    &range,
                    "the title cannot contain a link or an image",
                ));
            }
            _ => {
                return Err(problem(
                    &range,
                    "the title can contain only text, emphasis and inline code",
                ));
            }
        }
    }
    let title = title.trim();
    match title.chars().count() {
        0 => Err(problem(&heading, "the title is empty")),
        n if n > MAX_TITLE_CHARS => {
            Err(problem(&heading, "the title is longer than 80 characters"))
        }
        _ => Ok(title.to_owned()),
    }
}

/// The event allow-list for the body, which also counts words.
struct Policy<'r> {
    routes: &'r BTreeSet<String>,
    counting: Counting,
    /// Inside a code block or an HTML block, which is shown as code.
    code_depth: usize,
    words: usize,
    characters: usize,
    /// The last counted text ended inside an English word.
    in_word: bool,
}

impl<'r> Policy<'r> {
    fn new(routes: &'r BTreeSet<String>, counting: Counting) -> Self {
        Self {
            routes,
            counting,
            code_depth: 0,
            words: 0,
            characters: 0,
            in_word: false,
        }
    }

    /// The event to write, or why the article is refused.
    fn event<'a>(&mut self, event: Event<'a>) -> Result<Event<'a>, String> {
        // Emphasis and links sit inside a word; every other tag and break
        // ends it.
        match &event {
            Event::Start(Tag::Emphasis | Tag::Strong | Tag::Link { .. })
            | Event::End(TagEnd::Emphasis | TagEnd::Strong | TagEnd::Link) => {}
            Event::Start(_) | Event::End(_) | Event::SoftBreak | Event::HardBreak | Event::Rule => {
                self.in_word = false;
            }
            _ => {}
        }
        Ok(match event {
            Event::Start(tag) => Event::Start(self.start(tag)?),
            Event::End(tag) => Event::End(self.end(tag)?),
            Event::Text(text) => {
                self.count(&text);
                Event::Text(text)
            }
            Event::Code(code) => {
                if self.counting == Counting::Characters || !self.in_word {
                    self.words += 1;
                }
                self.in_word = true;
                Event::Code(code)
            }
            // Raw HTML is shown as text, never passed through.
            Event::Html(html) | Event::InlineHtml(html) => {
                self.count(&html);
                Event::Text(html)
            }
            Event::SoftBreak => Event::SoftBreak,
            Event::HardBreak => Event::HardBreak,
            Event::Rule => Event::Rule,
            Event::InlineMath(_) | Event::DisplayMath(_) => return Err(unsupported("math")),
            Event::FootnoteReference(_) => return Err(unsupported("a footnote")),
            Event::TaskListMarker(_) => return Err(unsupported("a task list")),
        })
    }

    fn start<'a>(&mut self, tag: Tag<'a>) -> Result<Tag<'a>, String> {
        Ok(match tag {
            Tag::Paragraph => Tag::Paragraph,
            Tag::Heading {
                level: HeadingLevel::H1,
                ..
            } => {
                return Err(
                    "only the title can be a level-1 heading; use `## ` for a section".into(),
                );
            }
            Tag::Heading { level, .. } => Tag::Heading {
                level,
                id: None,
                classes: Vec::new(),
                attrs: Vec::new(),
            },
            Tag::BlockQuote(None) => Tag::BlockQuote(None),
            // The info string is dropped, and an HTML block becomes code.
            Tag::CodeBlock(_) | Tag::HtmlBlock => {
                self.code_depth += 1;
                Tag::CodeBlock(CodeBlockKind::Indented)
            }
            Tag::List(start) => Tag::List(start),
            Tag::Item => Tag::Item,
            // Column alignment would become a `style` attribute; it is dropped.
            Tag::Table(columns) => Tag::Table(vec![Alignment::None; columns.len()]),
            Tag::TableHead => Tag::TableHead,
            Tag::TableRow => Tag::TableRow,
            Tag::TableCell => Tag::TableCell,
            Tag::Emphasis => Tag::Emphasis,
            Tag::Strong => Tag::Strong,
            Tag::Link {
                link_type,
                dest_url,
                title,
                id,
            } => {
                // E-mail autolinks would become `mailto:`; wikilinks are off.
                let checked = matches!(
                    link_type,
                    LinkType::Inline
                        | LinkType::Reference
                        | LinkType::Collapsed
                        | LinkType::Shortcut
                        | LinkType::Autolink
                );
                if !(checked
                    && (self.routes.contains(dest_url.as_ref()) || is_https_address(&dest_url)))
                {
                    let shown = match link_type {
                        LinkType::Email => format!("mailto:{dest_url}"),
                        _ => dest_url.to_string(),
                    };
                    return Err(format!(
                        "the link {shown:?} is neither a page of this site, such as \"/\", nor an https:// address"
                    ));
                }
                Tag::Link {
                    link_type,
                    dest_url,
                    title,
                    id,
                }
            }
            Tag::Image { .. } => return Err(unsupported("an image")),
            Tag::BlockQuote(Some(_)) => return Err(unsupported("an alert block quote")),
            Tag::FootnoteDefinition(_) => return Err(unsupported("a footnote")),
            Tag::DefinitionList | Tag::DefinitionListTitle | Tag::DefinitionListDefinition => {
                return Err(unsupported("a definition list"));
            }
            Tag::Strikethrough => return Err(unsupported("strikethrough")),
            Tag::Superscript | Tag::Subscript => {
                return Err(unsupported("superscript or subscript"));
            }
            Tag::MetadataBlock(_) => return Err(unsupported("a metadata block")),
        })
    }

    /// Closes what [`Policy::start`] accepted.
    fn end(&mut self, tag: TagEnd) -> Result<TagEnd, String> {
        Ok(match tag {
            TagEnd::CodeBlock | TagEnd::HtmlBlock => {
                self.code_depth = self.code_depth.saturating_sub(1);
                TagEnd::CodeBlock
            }
            TagEnd::Paragraph
            | TagEnd::Heading(_)
            | TagEnd::BlockQuote(None)
            | TagEnd::List(_)
            | TagEnd::Item
            | TagEnd::Table
            | TagEnd::TableHead
            | TagEnd::TableRow
            | TagEnd::TableCell
            | TagEnd::Emphasis
            | TagEnd::Strong
            | TagEnd::Link => tag,
            TagEnd::BlockQuote(Some(_))
            | TagEnd::Image
            | TagEnd::FootnoteDefinition
            | TagEnd::DefinitionList
            | TagEnd::DefinitionListTitle
            | TagEnd::DefinitionListDefinition
            | TagEnd::Strikethrough
            | TagEnd::Superscript
            | TagEnd::Subscript
            | TagEnd::MetadataBlock(_) => return Err(unsupported("this Markdown")),
        })
    }

    fn count(&mut self, text: &str) {
        if self.code_depth > 0 {
            return;
        }
        match self.counting {
            Counting::Words => {
                for c in text.chars() {
                    if c.is_whitespace() {
                        self.in_word = false;
                    } else if !self.in_word {
                        self.words += 1;
                        self.in_word = true;
                    }
                }
            }
            Counting::Characters => {
                self.characters += text.chars().filter(|c| !c.is_whitespace()).count();
            }
        }
    }
}

fn unsupported(what: &str) -> String {
    format!("{what} is not supported in articles")
}

/// `https://`, a lowercase host name with a dot, then nothing or a tail
/// that starts with `/`, `?` or `#` and holds only visible ASCII without
/// `"<>\^`{|}`. A port, user information or a percent sign in the host is
/// therefore impossible.
pub fn is_https_address(dest: &str) -> bool {
    let Some(rest) = dest.strip_prefix("https://") else {
        return false;
    };
    let (host, tail) = rest.split_at(rest.find(['/', '?', '#']).unwrap_or(rest.len()));
    let host_ok = (1..=253).contains(&host.len())
        && host
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'-')
        && host.contains('.')
        && !host.starts_with(['.', '-'])
        && !host.ends_with(['.', '-']);
    let tail_ok = tail
        .bytes()
        .all(|b| (b'!'..=b'~').contains(&b) && !b"\"<>\\^`{|}".contains(&b));
    host_ok && tail_ok
}

#[cfg(test)]
mod tests {
    use super::*;
    use pulldown_cmark::{BlockQuoteKind, CowStr};

    const ALLOWED_TAGS: [&str; 23] = [
        "p",
        "h2",
        "h3",
        "h4",
        "h5",
        "h6",
        "blockquote",
        "ul",
        "ol",
        "li",
        "table",
        "thead",
        "tbody",
        "tr",
        "th",
        "td",
        "em",
        "strong",
        "code",
        "pre",
        "a",
        "br",
        "hr",
    ];

    fn routes() -> BTreeSet<String> {
        ["/", "/ja/", "/first-note", "/ja/first-note"]
            .into_iter()
            .map(str::to_owned)
            .collect()
    }

    fn rendered(text: &str) -> Result<Article, Problem> {
        render(text, &routes(), Counting::Words)
    }

    /// The body HTML of an article titled `T`.
    fn body(markdown: &str) -> String {
        let text = format!("# T\n\n{markdown}");
        match rendered(&text) {
            Ok(article) => article.body.as_str().to_owned(),
            Err(problem) => panic!("{markdown:?} was refused: {problem:?}"),
        }
    }

    /// The 1-based line and reason of a refused article.
    fn refusal(text: &str) -> (usize, String) {
        let problem = rendered(text).expect_err("refused");
        let line = text[..problem.offset].matches('\n').count() + 1;
        (line, problem.reason)
    }

    /// Tags and attribute names in rendered HTML. Every `<` in the output
    /// starts a tag, because text is escaped.
    fn tags(html: &str) -> Vec<(String, Vec<String>)> {
        let mut found = Vec::new();
        let mut rest = html;
        while let Some(start) = rest.find('<') {
            let end = start + rest[start..].find('>').expect("a closed tag");
            let inner = rest[start + 1..end]
                .trim_start_matches('/')
                .trim_end_matches('/')
                .trim_end();
            let name_end = inner.find(char::is_whitespace).unwrap_or(inner.len());
            let mut attributes = Vec::new();
            let mut attrs = inner[name_end..].trim_start();
            while !attrs.is_empty() {
                let name_end = attrs.find(['=', ' ']).unwrap_or(attrs.len());
                attributes.push(attrs[..name_end].to_owned());
                attrs = &attrs[name_end..];
                if let Some(value) = attrs.strip_prefix("=\"") {
                    let close = value.find('"').expect("a quoted value");
                    attrs = &value[close + 1..];
                }
                attrs = attrs.trim_start();
            }
            found.push((inner[..name_end].to_owned(), attributes));
            rest = &rest[end + 1..];
        }
        found
    }

    fn assert_only_allowed_markup(html: &str) {
        for (tag, attributes) in tags(html) {
            assert!(ALLOWED_TAGS.contains(&tag.as_str()), "<{tag}> in {html}");
            for attribute in attributes {
                assert!(
                    ["href", "title", "start"].contains(&attribute.as_str()),
                    "{attribute} on <{tag}> in {html}"
                );
            }
        }
    }

    #[test]
    fn options_enable_tables_and_nothing_else() {
        assert_eq!(OPTIONS, Options::ENABLE_TABLES);
    }

    #[test]
    fn ordinary_markdown_becomes_standard_html() {
        let html = body(
            "Some *emphasis*, **strong** and `code`.\nNext line.  \nBreak.\n\n\
             ## Section\n\n###### Small\n\n> Quoted\n\n- one\n- two\n\n3. three\n4. four\n\n\
             | a | b |\n|:--|--:|\n| 1 | 2 |\n\n---\n",
        );
        assert_eq!(
            html,
            "<p>Some <em>emphasis</em>, <strong>strong</strong> and <code>code</code>.\nNext line.<br />\nBreak.</p>\n\
             <h2>Section</h2>\n<h6>Small</h6>\n<blockquote>\n<p>Quoted</p>\n</blockquote>\n\
             <ul>\n<li>one</li>\n<li>two</li>\n</ul>\n<ol start=\"3\">\n<li>three</li>\n<li>four</li>\n</ol>\n\
             <table><thead><tr><th>a</th><th>b</th></tr></thead><tbody>\n<tr><td>1</td><td>2</td></tr>\n</tbody></table>\n<hr />\n"
        );
    }

    #[test]
    fn markup_in_the_source_is_shown_as_text() {
        let cases = [
            "<script>alert(1)</script>",
            "<img src=x onerror=alert(1)>",
            "<div onclick=\"x()\">block</div>\n",
            "Inline <b onmouseover=x>bold</b> and <!-- comment -->",
            "`<script>in code</script>`",
            "```\n<script>fenced</script>\n```",
            "    <script>indented</script>",
        ];
        for markdown in cases {
            let html = body(markdown);
            assert_only_allowed_markup(&html);
            assert!(!html.contains("<script"), "{html}");
            assert!(html.contains("&lt;"), "{markdown:?} lost its text: {html}");
        }
        let html = body("<img src=x onerror=alert(1)>");
        assert_eq!(
            html,
            "<pre><code>&lt;img src=x onerror=alert(1)&gt;</code></pre>\n"
        );
    }

    #[test]
    fn the_info_string_of_a_code_block_is_dropped() {
        let html = body(
            "```\"><script>alert(1)</script>\nlet x = 1;\n```\n\n```rust file=src/x.rs\nfn f() {}\n```",
        );
        assert_eq!(
            html,
            "<pre><code>let x = 1;\n</code></pre>\n<pre><code>fn f() {}\n</code></pre>\n"
        );
    }

    #[test]
    fn images_and_a_second_title_are_refused() {
        assert_eq!(
            refusal("# T\n\n![alt](https://example.com/a.png)\n"),
            (3, "an image is not supported in articles".into())
        );
        let second = "only the title can be a level-1 heading; use `## ` for a section";
        assert_eq!(refusal("# T\n\ntext\n\n# Again\n"), (5, second.into()));
        assert_eq!(refusal("# T\n\nAgain\n=====\n"), (3, second.into()));
    }

    #[test]
    fn disabled_extensions_stay_literal_text() {
        let html = body(
            "Note[^1] and $x^2$ and ~~gone~~ and ^up^ and ~down~ and [[Wiki]].\n\n\
             [^1]: The footnote.\n\n- [ ] open\n- [x] done\n\n> [!NOTE]\n> Alert.\n\n\
             Term\n: Definition\n\n---\ntitle: x\n---\n",
        );
        assert_only_allowed_markup(&html);
        for absent in ["<input", "<sup", "<math", "<del", "<dl", "class="] {
            assert!(!html.contains(absent), "{absent} in {html}");
        }
        for literal in [
            "[^1]",
            "$x^2$",
            "~~gone~~",
            "[ ] open",
            "[x] done",
            "[!NOTE]",
            ": Definition",
        ] {
            assert!(html.contains(literal), "{literal} missing from {html}");
        }
    }

    #[test]
    fn unsupported_events_fail_closed() {
        let routes = routes();
        let events = [
            Event::FootnoteReference(CowStr::from("1")),
            Event::TaskListMarker(false),
            Event::InlineMath(CowStr::from("x")),
            Event::DisplayMath(CowStr::from("x")),
            Event::Start(Tag::FootnoteDefinition(CowStr::from("1"))),
            Event::Start(Tag::Strikethrough),
            Event::Start(Tag::Superscript),
            Event::Start(Tag::DefinitionList),
            Event::Start(Tag::BlockQuote(Some(BlockQuoteKind::Note))),
            Event::Start(Tag::Image {
                link_type: LinkType::Inline,
                dest_url: CowStr::from("https://example.com/a.png"),
                title: CowStr::from(""),
                id: CowStr::from(""),
            }),
            Event::Start(Tag::Link {
                link_type: LinkType::WikiLink { has_pothole: false },
                dest_url: CowStr::from("/first-note"),
                title: CowStr::from(""),
                id: CowStr::from(""),
            }),
            Event::End(TagEnd::Strikethrough),
        ];
        for event in events {
            let shown = format!("{event:?}");
            assert!(
                Policy::new(&routes, Counting::Words).event(event).is_err(),
                "{shown} was accepted"
            );
        }
    }

    #[test]
    fn link_titles_are_kept_escaped() {
        let html = body("[x](/first-note \"a \\\" onmouseover=alert(1) <b>\")");
        assert_eq!(
            html,
            "<p><a href=\"/first-note\" title=\"a &quot; onmouseover=alert(1) &lt;b&gt;\">x</a></p>\n"
        );
        assert_only_allowed_markup(&html);
    }

    #[test]
    fn allowed_link_destinations_pass() {
        for dest in [
            "/",
            "/ja/",
            "/first-note",
            "/ja/first-note",
            "https://example.com",
            "https://example.com/",
            "https://doc.rust-lang.org/book/ch01-01-installation.html",
            "https://a-b.example.org/x?y=1&z=%20#top",
            "https://example.com#top",
        ] {
            let html = body(&format!("[x](<{dest}>)"));
            assert!(html.contains("<a href=\""), "{dest}: {html}");
        }
        let html = body("<https://example.com/a>");
        assert_eq!(
            html,
            "<p><a href=\"https://example.com/a\">https://example.com/a</a></p>\n"
        );
    }

    #[test]
    fn other_link_destinations_are_refused_with_line_and_quoted_text() {
        let cases = [
            ("[x](javascript:alert(1))", "javascript:alert(1)"),
            ("[x](JavaScript:x)", "JavaScript:x"),
            ("[x](java&#115;cript:x)", "javascript:x"),
            ("[x](data:text/html,x)", "data:text/html,x"),
            ("[x](file:///etc/passwd)", "file:///etc/passwd"),
            ("[x](http://example.com)", "http://example.com"),
            ("[x](//example.com)", "//example.com"),
            ("[x](</\\example.com>)", "/\\example.com"),
            ("[x](https:example.com)", "https:example.com"),
            ("[x](https://user@example.com)", "https://user@example.com"),
            ("[x](https://example.com:8443)", "https://example.com:8443"),
            ("[x](https://%65xample.com)", "https://%65xample.com"),
            ("[x](/first%2Dnote)", "/first%2Dnote"),
            ("[x](/first-note?x=1)", "/first-note?x=1"),
            ("[x](/first-note#top)", "/first-note#top"),
            ("[x](#top)", "#top"),
            ("[x](/../first-note)", "/../first-note"),
            ("[x](/./first-note)", "/./first-note"),
            ("[x](/ja/../x)", "/ja/../x"),
            ("[x](first-note)", "first-note"),
            ("[x](./first-note)", "./first-note"),
            ("[x](/first-note/)", "/first-note/"),
            ("[x](/ja)", "/ja"),
            ("[x](/missing)", "/missing"),
            ("[x]()", ""),
            ("[x](<https://example.com/a b>)", "https://example.com/a b"),
            (
                "[x](<https://example.com/a\tb>)",
                "https://example.com/a\tb",
            ),
            (
                "[x](<https://example.com/\u{1}>)",
                "https://example.com/\u{1}",
            ),
            (
                "[x](<https://example.com/\u{1f}>)",
                "https://example.com/\u{1f}",
            ),
            (
                "[x](<https://example.com/\u{7f}>)",
                "https://example.com/\u{7f}",
            ),
            ("[x](<https://example.com/\\x>)", "https://example.com/\\x"),
            ("[x](<https://EXAMPLE.com>)", "https://EXAMPLE.com"),
            (
                "[x][ref]\n\n[ref]: mailto:a@example.com",
                "mailto:a@example.com",
            ),
            ("<a@example.com>", "mailto:a@example.com"),
        ];
        for (markdown, dest) in cases {
            let (line, reason) = refusal(&format!("# T\n\ntext\n{markdown}\n"));
            assert_eq!(line, 4, "{markdown:?}");
            let quoted = format!("the link {dest:?} is neither");
            assert!(reason.starts_with(&quoted), "{markdown:?}: {reason}");
            assert!(!reason.chars().any(char::is_control), "{reason:?}");
        }
    }

    #[test]
    fn destinations_the_parser_cannot_link_are_still_refused_by_the_rule() {
        for dest in [
            "https://example.com/\n",
            "https://example.com/\r",
            "https://example.com/\0",
            "https://exa_mple.com",
            "https://.example.com",
            "https://example.com.",
            "https://-example.com",
            "https://localhost",
            "https://example.com/{x}",
            "https://example.com/|",
            "https://example.com/^",
            "https://example.com/`",
            "https://example.com/\"",
            "https://example.com/<",
            "https://example.com/é",
            "",
        ] {
            assert!(!is_https_address(dest), "{dest:?}");
        }
        let longest = format!("https://{}.com", "a".repeat(249));
        assert!(is_https_address(&longest));
        assert!(!is_https_address(&format!(
            "https://{}.com",
            "a".repeat(250)
        )));
    }

    #[test]
    fn the_title_is_the_opening_level_1_heading() {
        let article = rendered("# Field *notes* with `code` and <b>tags</b>\n\nBody.\n").unwrap();
        assert_eq!(article.title, "Field notes with code and <b>tags</b>");
        assert_eq!(article.body.as_str(), "<p>Body.</p>\n");
        let setext = rendered("Setext title\n============\n").unwrap();
        assert_eq!(setext.title, "Setext title");
        assert_eq!(
            rendered(&format!("# {}\n", "é".repeat(80)))
                .unwrap()
                .title
                .chars()
                .count(),
            80
        );
    }

    #[test]
    fn title_rules_are_enforced_with_a_line() {
        let first = "the first block must be a level-1 heading, such as `# My note`";
        assert_eq!(refusal(""), (1, first.into()));
        assert_eq!(refusal("Text first.\n\n# Late\n"), (1, first.into()));
        assert_eq!(refusal("\n\n## Section\n\n# Late\n"), (3, first.into()));
        assert_eq!(refusal("#\n\nBody\n"), (1, "the title is empty".into()));
        assert_eq!(refusal("# \u{3000} \n"), (1, "the title is empty".into()));
        assert_eq!(
            refusal(&format!("\n# {}\n", "x".repeat(81))),
            (2, "the title is longer than 80 characters".into())
        );
        let link = "the title cannot contain a link or an image";
        assert_eq!(refusal("# A [link](/first-note)\n"), (1, link.into()));
        assert_eq!(
            refusal("# A ![picture](https://example.com/a.png)\n"),
            (1, link.into())
        );
        assert_eq!(refusal("# A <https://example.com>\n"), (1, link.into()));
    }

    #[test]
    fn english_counts_words_outside_code() {
        let article = rendered(
            "# Not counted in the title\n\nOne two  three\nfour `five six` seven.\n\n```\nnot counted at all\n```\n\n<p>\nhtml block\n</p>\n",
        )
        .unwrap();
        assert_eq!(article.words, 6);
    }

    #[test]
    fn formatting_inside_a_word_does_not_split_it() {
        let text = format!("# T\n\n{}\n", "foo**bar**baz ".repeat(100));
        let words = rendered(&text).unwrap().words;
        assert_eq!(words, 100);
        assert_eq!(crate::reading::reading_minutes(words), 1);
    }

    #[test]
    fn whitespace_breaks_and_blocks_separate_words_but_inline_markup_does_not() {
        let words = |markdown: &str| rendered(&format!("# T\n\n{markdown}")).unwrap().words;
        // Inline markup, links, inline code and raw HTML shown as text join.
        assert_eq!(words("*a*b [c](/)d e`f`g h<b>i</b>j\n"), 4);
        assert_eq!(words("[one two](/first-note), three\n"), 3);
        // Whitespace, line breaks and block boundaries separate.
        assert_eq!(words("a *b* [c](/) `d` <b>e</b>\n"), 5);
        assert_eq!(words("a\nb  \nc\n"), 3);
        assert_eq!(words("a\n\nb\n\n## c\n\n- d\n- e\n\n> f\n"), 6);
        assert_eq!(words("| a | b |\n|---|---|\n| c | d |\n"), 4);
        assert_eq!(words("a\n\n```\nnot counted\n```\nb\n"), 2);
    }

    #[test]
    fn japanese_counts_two_characters_as_a_word() {
        let text = "# 題名\n\nこれは 日本語の文です。\n\n`code` と、\n\n```\n数えない\n```\n";
        let article = render(text, &routes(), Counting::Characters).unwrap();
        // 11 characters in the first paragraph and 2 in the second, halved
        // and rounded down, plus one inline code span.
        assert_eq!(article.words, 13 / 2 + 1);
    }
}
