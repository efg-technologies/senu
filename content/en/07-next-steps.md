# What you built, and where to go next

You started a Rust program, read its pages in a browser, wrote notes in Markdown, met Senu's rules through its messages, and rebuilt one of its functions test first. Every step used the same small program.

## What to remember

- `cargo run --quiet -- site` builds and starts the site, and Ctrl+C stops it. Restart it after every change.
- Notes live in `content/en/`, or `content/ja/` for Japanese, and are named like `51-my-first-note.md`.
- When Senu refuses to start, its message names the file, usually the line, and the rule.
- A test that fails before a fix, for the reason you expected, and passes after it, shows that the fix works.

## Honest limits

Senu is an experimental seed, not a product. Its site runs only on your own computer, it reads the notes only when it is built, and it has no accounts, no database and no way to edit a note from the browser. It has been tested on Linux only.

## Where to go next

- [The Rust Programming Language](https://doc.rust-lang.org/book/), the official book, explains everything this course touched, in depth and at your own pace.
- [Rust by Example](https://doc.rust-lang.org/rust-by-example/) teaches with short programs you can run.
- [The CommonMark reference](https://commonmark.org/help/) lists the Markdown you can use in notes. Senu adds tables and leaves out images.
- Two optional appendices show the rest of Senu: [a project catalogue served as HTML and JSON](/appendix-catalogue), with a test-first rename, and [the same catalogue over MCP](/appendix-mcp). They assume that you have finished this course.

## Exercise

Write one more note in your own words about something you learned here. Give it the number 53, and link to it from your first note.

**Solution.** Any file named like `content/en/53-what-i-learned.md` whose first line is a title works. Link to it from `content/en/51-my-first-note.md` with a line such as `Read [what I learned](/what-i-learned).`, restart Senu, and follow the link.

## Optional: change the look

Senu's pages get their look from [Tailwind CSS](https://tailwindcss.com/), small classes such as `bg-white` (a white background) in `templates/*.html`. Tailwind writes a CSS rule only for the classes it finds there, into `assets/ui.generated.css`, and Cargo builds that committed file into Senu, so nothing else was needed so far. Changing a class needs [Node.js](https://nodejs.org/) 24 with its npm, once; install it from its website, then check:

```sh manual
node --version
```

Stop Senu with Ctrl+C. This patch gives the top bar a light rose background; no template uses `rose` yet:

```diff patch
--- a/templates/site_base.html
+++ b/templates/site_base.html
@@ -11,3 +11,3 @@
 <a href="#main" class="absolute -top-16 left-2 z-10 border-2 border-blue-700 bg-white px-3 py-2 text-blue-700 underline focus:top-2"><%= frame.labels.skip %></a>
-<header class="border-b border-gray-200 bg-white">
+<header class="border-b border-gray-200 bg-rose-100">
 <div class="mx-auto flex max-w-6xl flex-wrap items-baseline justify-between gap-x-6 gap-y-2 px-5 py-3">
```

The stylesheet has no rule for the new class yet, so `grep -c`, which counts matching lines, prints 0 and exits 1:

```sh check=no-rule exit=1
grep -c bg-rose-100 assets/ui.generated.css
```

```text check=no-rule
0
```

`npm ci` installs, once, the exact tools that `package-lock.json` lists, and `--silent` hides its report. `npm run css` rebuilds the stylesheet after each change of a class:

```sh check=tools
npm ci --silent
```

```sh check=css
npm run css
```

```text check=css

> css
> tailwindcss -i assets/ui.css -o assets/ui.generated.css --minify

≈ tailwindcss v4.3.3

Done in 0ms
```

`grep -o` prints only the matching part, here the new rule:

```sh check=new-rule
grep -o '\.bg-rose-100{[^}]*}' assets/ui.generated.css
```

```text check=new-rule
.bg-rose-100{background-color:var(--color-rose-100)}
```

Start Senu again and reload http://127.0.0.1:3000/ to see the rose top bar:

```sh check=rose server
cargo run --quiet -- site
```

```text check=rose
senu: listening on http://127.0.0.1:3000
```

Commit the template together with `assets/ui.generated.css`, so that readers without Node see the same look. To change the fonts or colours of every page instead, edit the `@theme` block in `assets/ui.css` and run `npm run css` again.
