<nav class="senu-lang" aria-label="Language"><a href="ja/web.html" hreflang="ja" lang="ja">日本語</a></nav>

# HTML and the JSON API

Both routes in this chapter are served by the `senu serve` process from the
previous chapter, so keep it running. They share one Axum router in
[`src/http.rs`](https://github.com/efg-technologies/senu/blob/main/src/http.rs):
every request passes size limits, then routing, then authentication, and only
then reaches `list_projects`.

## The HTML page escapes markup

The page comes from the Askama template
[`templates/projects.html`](https://github.com/efg-technologies/senu/blob/main/templates/projects.html).
[`askama.toml`](https://github.com/efg-technologies/senu/blob/main/askama.toml)
sets an ERB-like syntax: `<%! %>` for blocks, `<%= %>` for expressions and
`<%# %>` for comments. Each project becomes one list item:

```html
<li data-project-id="<%= p.id %>"><%= p.name %></li>
```

Askama HTML-escapes every `<%= %>` expression in a `.html` template. Fetch
the page with the Basic credentials and keep only the list items:

```sh
{{#include ../commands/web.sh:html}}
```

```text
{{#include ../commands/web.out:html}}
```

The project name contains a `<script>` element, but the page only carries
character references such as `&#60;` (`<`) and `&#34;` (`"`), so a browser
displays the name as text. A search term that goes back into the page, in the
search box's `value` attribute, is escaped the same way:

```sh
{{#include ../commands/web.sh:echoed-search}}
```

```text
{{#include ../commands/web.out:echoed-search}}
```

Escaping cannot be switched off by accident: a test in
[`tests/adapter_http.rs`](https://github.com/efg-technologies/senu/blob/main/tests/adapter_http.rs)
fails if a template or `askama.toml` uses the `safe` filter or an escape
override. The HTML response also sends a restrictive
`Content-Security-Policy` that allows no scripts.

## The JSON API returns data

The same search through the API returns the name as a JSON string. JSON is
data, not markup, so nothing is HTML-escaped here; whoever displays the name
is responsible for escaping it, as the HTML page does.

```sh
{{#include ../commands/web.sh:json}}
```

```text
{{#include ../commands/web.out:json}}
```

## Credentials belong to one transport

The HTML page accepts only HTTP Basic and the API accepts only its Bearer
token. Sending the API token to the HTML page fails with 401, and the
`WWW-Authenticate` header names the scheme that the page expects:

```sh
{{#include ../commands/web.sh:wrong-credential}}
```

```text
{{#include ../commands/web.out:wrong-credential}}
```

Credentials in the query string are never read, and every response carries
`Cache-Control: no-store` and `X-Content-Type-Options: nosniff`. You can stop
the server with Ctrl-C now.

[`tests/adapter_parity.rs`](https://github.com/efg-technologies/senu/blob/main/tests/adapter_parity.rs)
compares this page and this API with a direct `list_projects` call for
several searches. It decodes the HTML items first, so it checks that escaping
changes how a name is written, not what it is.
