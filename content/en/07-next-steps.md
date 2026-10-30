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
