# When Senu says no

Senu checks every note before it starts serving. When a note breaks a rule, Senu prints what is wrong and where, and stops, so a broken page never reaches the browser. In this chapter you make a scratch note, break it on purpose in several ways, read each message, and finally delete it.

First stop Senu: press Ctrl+C in the terminal where it runs. Keep the second terminal open.

## A note without a title

In your editor, create the file `content/en/52-scratch.md` with this text, which has no title line:

```markdown file=content/en/52-scratch.md
This scratch note forgot its title.
```

In the first terminal, start Senu:

```sh check=no-title exit=2
cargo run --quiet -- site
```

Instead of the listening line, Senu prints:

```text check=no-title
senu: content/en/52-scratch.md:1: the first block must be a level-1 heading, such as `# My note`
```

Read it from left to right: the program that complains (`senu`), the file, the line in that file (`1`), and what is wrong. The prompt comes back at once, because Senu stopped. Programs also report success or failure with a number called the **exit status**: 0 means success and anything else means failure. Senu uses 2 for problems that you can fix in its input; typing `echo $?` right after a command prints that command's exit status.

## A link that is not allowed

Give the note a title, and add a link that would run code instead of opening a page. Replace the content of `content/en/52-scratch.md` with:

```markdown file=content/en/52-scratch.md
# Scratch

[Click me](javascript:alert(1))
```

```sh check=bad-link exit=2
cargo run --quiet -- site
```

```text check=bad-link
senu: content/en/52-scratch.md:3: the link "javascript:alert(1)" is neither a page of this site, such as "/", nor an https:// address
```

Senu accepts two kinds of link only: the exact address of a page of this site, such as `/start` or `/ja/start`, and an address that starts with `https://`. Everything else is refused, because a link is the easiest way to smuggle something unexpected into a page.

## HTML is shown, not run

Markdown files may contain HTML tags, but Senu never hands them to the browser as HTML. Replace the content of `content/en/52-scratch.md` with:

```markdown file=content/en/52-scratch.md
# Scratch

<script>alert("hi")</script>
```

This time Senu starts:

```sh check=html server
cargo run --quiet -- site
```

```text check=html
senu: listening on http://127.0.0.1:3000
```

Open http://127.0.0.1:3000/scratch. The tag appears as text in a code box and does not run. The second terminal shows how: the angle brackets became `&lt;` and `&gt;`, which a browser displays as `<` and `>` but never runs.

```sh check=html-shown
curl -s http://127.0.0.1:3000/scratch | grep script
```

```text check=html-shown
<pre><code>&lt;script&gt;alert("hi")&lt;/script&gt;
```

## A name that breaks the rule

Stop Senu with Ctrl+C. Rename the file to `52-Scratch.md`, with a capital S: in your editor, right-click the file and choose Rename. You can do the same in the terminal with `mv`, which moves a file and so can also rename it:

```sh check=rename
mv content/en/52-scratch.md content/en/52-Scratch.md
```

```sh check=bad-name exit=2
cargo run --quiet -- site
```

```text check=bad-name
senu: content/en/52-Scratch.md: the file name must be two digits, a hyphen, a slug of lowercase letters and digits joined by single hyphens, then .md, such as 51-my-note.md
```

## Delete the scratch note

Delete `content/en/52-Scratch.md` in your editor (right-click, Delete), or in the terminal with `rm`, which removes a file for good:

```sh check=delete
rm content/en/52-Scratch.md
```

Start Senu again:

```sh check=deleted server
cargo run --quiet -- site
```

```text check=deleted
senu: listening on http://127.0.0.1:3000
```

The scratch page is gone. `-w "\n"` makes curl print a line break after the answer, so that the prompt starts on a new line:

```sh check=deleted-page
curl -s -w "\n" http://127.0.0.1:3000/scratch
```

```text check=deleted-page
Not Found
```

It has left the chapter list too. When `grep` finds nothing, it prints nothing, and its exit status is 1:

```sh check=deleted-list exit=1
curl -s http://127.0.0.1:3000/ | grep scratch
```

```text check=deleted-list
```

## Exercise

Stop Senu, create `content/en/51-tasks.md` containing the line `# Tasks`, and start Senu again. Which rule does the new file break, and how do you fix it?

**Solution.** The file `content/en/51-tasks.md` is:

```markdown file=content/en/51-tasks.md
# Tasks
```

```sh check=duplicate exit=2
cargo run --quiet -- site
```

```text check=duplicate
senu: content/en/51-tasks.md: the number 51 is already used by content/en/51-my-first-note.md
```

Two notes in the same language cannot share a number, because the number decides the order. Give the new note a free number, such as `53-tasks.md`, or delete it, here with `rm`:

```sh check=duplicate-deleted
rm content/en/51-tasks.md
```

## If something goes wrong

Every message names the file. The most common ones are:

- "the first block must be a level-1 heading": the first line must be `# ` followed by a title.
- "the title is empty" or "the title is longer than 80 characters": fill in or shorten the title.
- "the file name must be…": check the two digits, the hyphens and that every letter is lowercase.
- "is already used by": two notes share a number or a slug; rename one of them.
- "the link … is neither…": use `/` and a slug, or a full address that starts with `https://`.
- "only the title can be a level-1 heading": use `## ` for sections.
- "an image is not supported": Senu has no images; describe the picture in words or link to it with `https://`.

If Senu seems to ignore a change even after a restart, run `cargo clean -p senu`, which deletes Senu's own build results, and start it again.

Next: [your first Rust function](/first-function).
