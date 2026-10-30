# Your first page in the browser

Before you read any Rust, you will start Senu and open its pages in your browser. It is the same site you are reading now, served by your own computer.

## Start Senu

The command is `cargo run --quiet -- site`. `cargo run` first **builds** the program, which means turning the Rust source files into a program your computer can run, and then **runs** it. `--quiet` hides Cargo's progress messages. Everything after `--` is passed to Senu itself, and `site` tells Senu to serve these articles. In the terminal, inside the `senu` folder, type it and press Enter:

```sh check=site server
cargo run --quiet -- site
```

The first build compiles Senu and everything it uses, so the terminal may show nothing at all for several minutes. Let it work. When Senu is ready, it prints one line:

```text check=site
senu: listening on http://127.0.0.1:3000
```

The prompt does not come back, because Senu is now a **server**: a program that keeps running and answers requests. Leave this terminal open.

## Open the page

Switch to the browser, type **http://127.0.0.1:3000/** into the address bar and press Enter. You should see the start page of this course with its list of chapters.

The address has three parts. `http://` is how the browser talks to the server. `127.0.0.1` means "this computer": it is a **local address**, so nobody else on the internet can open it. `3000` is the **port**, a number that tells your computer which running program gets the request. While Senu runs, these pages exist only for you.

## Stop and start again

Go back to the terminal and press Ctrl+C. Senu stops and the prompt returns. Reload the page in the browser: it now says it cannot connect, because nothing answers at that address.

Start Senu again with the same command. This time it starts within seconds, because nothing has changed since the last build:

```sh check=site-again server
cargo run --quiet -- site
```

```text check=site-again
senu: listening on http://127.0.0.1:3000
```

Reload the page in the browser, and it is back.

## Ask from a second terminal

A browser is not the only program that can ask a server for a page. Open a **second terminal** (a new window, or a new tab in the same window), so that Senu keeps running in the first one, and type `cd ~/senu` in it. The next command uses two programs. `curl` fetches the page and prints its HTML, the text from which the browser draws the page; `-s` hides curl's progress meter. The `|` sign passes that output to `grep`, which prints only the lines that contain `<h1>`, the tag around the page's main heading. Run:

```sh check=title
curl -s http://127.0.0.1:3000/ | grep "<h1>"
```

It prints the heading of the start page:

```text check=title
<h1>Build a small notes site with Rust</h1>
```

The course uses this second terminal to show the effect of each step in text. You can always look at the same page in the browser instead.

## Exercise

Find the link at the top of the page that switches to Japanese and follow it. What is the address of the Japanese start page, and how do you get back?

**Solution.** The link reads 日本語 and leads to http://127.0.0.1:3000/ja/. In the same place on the Japanese page, a link reading English brings you back. Every article that exists in both languages links to its translation there.

## If something goes wrong

- **"could not find `Cargo.toml`"**: the terminal is not in the `senu` folder. Run `pwd`, then `cd ~/senu`.
- **"senu: cannot listen on 127.0.0.1:3000: address in use"**: Senu or another program already uses port 3000. Find the terminal where Senu runs and press Ctrl+C there. If another program holds the port, start Senu on another one. Writing `NAME=value` in front of a command sets the setting `NAME` for just that command, with no spaces around `=`; `SENU_BIND` tells Senu which address and port to use. Run `SENU_BIND=127.0.0.1:3001 cargo run --quiet -- site` and open http://127.0.0.1:3001/ instead.
- **The browser cannot connect**: Senu is not running. Look at its terminal; if the prompt is back, start Senu again.
- **"linker `cc` not found"** or another error during the first build: go back to the installation chapter.

Next: [write your first note](/first-note).
