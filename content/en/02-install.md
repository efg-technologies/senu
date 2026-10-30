# Install Rust and get the code

In this chapter you install Rust and Git, download Senu, and check that the tools work. Everything happens in the terminal, and the installers come from their official sites, not from this course.

## Rust

Rust is installed with **rustup**, a small program that downloads the Rust compiler and **Cargo**, the tool that builds and runs Rust programs. Follow the official instructions for your system and come back when they are done.

- **Linux and macOS:** follow [Installation in the Rust Book](https://doc.rust-lang.org/book/ch01-01-installation.html). The same page explains that Rust also needs a **linker**, the tool that joins compiled pieces into one program. On macOS you get it by running `xcode-select --install`. On Linux it comes with the C compiler package; on Ubuntu or Debian that package is called `build-essential`.
- **Windows:** this course uses WSL. Follow [Microsoft's guide to installing WSL](https://learn.microsoft.com/en-us/windows/wsl/install), open the Ubuntu terminal it creates, and follow the Linux instructions inside that terminal. From then on everything works as on Linux. Windows without WSL is not covered.

When the installer has finished, close the terminal and open a new one, so that it finds the new commands.

## Git

**Git** downloads projects like Senu and keeps track of their changes. Type `git --version` and press Enter. If the answer is "command not found", install Git from [the Git downloads page](https://git-scm.com/downloads); on Ubuntu, `sudo apt install git` does it.

## Download Senu

Downloading a project with Git is called **cloning**. In the terminal, from your home folder, run these two commands. The first creates a folder called `senu`; the second makes it the working directory:

```sh manual
git clone https://github.com/efg-technologies/senu.git
cd senu
```

From now on, every command in this course runs inside that `senu` folder. When you open a new terminal later, type `cd senu` first.

## Check the tools

Senu asks for one exact version of Rust in a file called `rust-toolchain.toml`. The first time you run Cargo inside the `senu` folder, rustup may download that version and print a few progress lines before the answer; that is expected. Run:

```sh check=versions
cargo --version
rustc --version
```

Both versions must start with 1.99.1. The text in brackets identifies the exact build:

```text check=versions
cargo 1.99.1 (5f94df478 2026-08-27)
rustc 1.99.1 (58cf8f9c3 2026-10-13)
```

## Exercise

Run `ls` inside the `senu` folder. Which folder do you think holds the pages you are reading?

**Solution.** The folder `content`. It holds `en` for English and `ja` for Japanese, with one Markdown file per page. You will add your own files there in two chapters.

## If something goes wrong

- **"cargo: command not found"**: open a new terminal. If that does not help, run the installer again and read its last lines, which explain how to update your PATH, the list of folders where the terminal looks for commands.
- **"toolchain '1.99.1' is not installed"**: run `rustup toolchain install 1.99.1` inside the `senu` folder.
- **"linker `cc` not found"** during the first build in the next chapter: the C compiler package is missing; see the Linux and macOS notes above.
- **"destination path 'senu' already exists"**: you cloned before. Type `cd senu` and continue.

This course is tested only on Linux (Ubuntu on GitHub's servers and one Linux workstation). macOS and WSL follow the official instructions, but nobody has checked them for this course.

Next: [your first page in the browser](/first-page).
