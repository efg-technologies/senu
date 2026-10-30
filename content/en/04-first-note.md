# Write your first note

Every page of this site is a **Markdown** file in the `content` folder. Markdown is plain text with a few marks that become formatting. In this chapter you change one note, add another, and see how Senu picks them up.

## Markdown in one minute

- A line that starts with `# ` becomes the title. Every note has exactly one, on its first line.
- A line that starts with `## ` becomes a section heading.
- An empty line separates two paragraphs.
- Lines that start with `- ` become a list.
- Text between backticks, like `cargo run`, is shown as code.
- `[words](/start)` becomes a link to a page of this site, and `[words](https://www.rust-lang.org/)` a link to a site on the internet.

## Change the sample note

The site comes with a sample note. Open http://127.0.0.1:3000/sample-note in the browser to see it. Its file is `content/en/50-sample-note.md`.

Open `content/en/50-sample-note.md` in your editor (in Visual Studio Code: File, Open Folder, choose the `senu` folder, then open `content`, `en` and the file in the list on the left). Replace everything in the file with the text below, and save:

```markdown file=content/en/50-sample-note.md
# My edited sample note

This note is a plain Markdown file: `content/en/50-sample-note.md`.
I changed its title and this paragraph.

## What I noticed

- A line that starts with `#` became the page title.
- A line that starts with `-` became an item in this list.
- Words between backticks, like `cargo run`, look like code.

Back to [the first chapter](/start).
```

Reload the page in the browser. Nothing has changed: Senu reads the notes when it is **built**, not while it runs, so a running Senu keeps the pages it was built with. Go to the terminal where Senu runs, press Ctrl+C, and run the same command again; `cargo run` notices the changed file and rebuilds before it starts:

```sh check=edited server
cargo run --quiet -- site
```

```text check=edited
senu: listening on http://127.0.0.1:3000
```

Reload the browser page, or ask from the second terminal:

```sh check=edited-title
curl -s http://127.0.0.1:3000/sample-note | grep "<h1>"
```

```text check=edited-title
<h1>My edited sample note</h1>
```

## Add your own note

The file name of a note decides two things. It must look like `51-my-first-note.md`: two digits, a hyphen, a **slug** made of lowercase letters, digits and single hyphens, and `.md`. The digits set the note's place in the chapter list, and the slug becomes its address, so this file becomes http://127.0.0.1:3000/my-first-note.

In your editor, create a new file named `content/en/51-my-first-note.md` (in Visual Studio Code: right-click the `en` folder, choose New File and type the name). Put this text in it and save:

```markdown file=content/en/51-my-first-note.md
# My first note

Today I ran a Rust program on my own computer and read its pages in a browser.

- The program is called Senu.
- It keeps running until I press Ctrl+C in its terminal.
```

Restart Senu as before: Ctrl+C in its terminal, then the same command.

```sh check=added server
cargo run --quiet -- site
```

```text check=added
senu: listening on http://127.0.0.1:3000
```

Open http://127.0.0.1:3000/my-first-note. Under the title, Senu shows an estimate of the reading time; you will rebuild the function behind it in a later chapter. Your note also appears in the chapter list, right after the sample note, because 51 comes after 50. From the second terminal:

```sh check=added-title
curl -s http://127.0.0.1:3000/my-first-note | grep "<h1>"
```

```text check=added-title
<h1>My first note</h1>
```

```sh check=added-list
curl -s http://127.0.0.1:3000/ | grep my-first-note
```

The start page lists your note twice, once in the chapter list and once in its table of contents:

```text check=added-list
<li><a href="/my-first-note">My first note</a></li>
<li><a href="/my-first-note">My first note</a></li>
```

You did not register the note anywhere. Senu finds every `.md` file in `content/en` when it is built, so adding a file and restarting is enough.

## Exercise

Add a link from your note to the Rust website, https://www.rust-lang.org/, restart Senu, and follow the link in the browser.

**Solution.** Add a line with a link, such as the last line below, to `content/en/51-my-first-note.md`, so that the file reads:

```markdown file=content/en/51-my-first-note.md
# My first note

Today I ran a Rust program on my own computer and read its pages in a browser.

- The program is called Senu.
- It keeps running until I press Ctrl+C in its terminal.

Next I want to read more on [the Rust website](https://www.rust-lang.org/).
```

```sh check=linked server
cargo run --quiet -- site
```

```text check=linked
senu: listening on http://127.0.0.1:3000
```

```sh check=linked-href
curl -s http://127.0.0.1:3000/my-first-note | grep rust-lang.org
```

```text check=linked-href
<p>Next I want to read more on <a href="https://www.rust-lang.org/">the Rust website</a>.</p>
```

## If something goes wrong

- **The page still shows the old text**: Senu was not restarted. Press Ctrl+C in its terminal and run `cargo run --quiet -- site` again.
- **"Not Found" instead of your note**: compare the address with the file name. `51-my-first-note.md` is served at `/my-first-note`, without the number and without `.md`.
- **Senu stops with a message instead of starting**: the next chapter explains those messages.

Next: [when Senu says no](/when-senu-says-no).
