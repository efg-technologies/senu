# Start here: what you will build

This course takes you from "I have never written Rust" to changing a real Rust program and proving the change with a test. You will not build a toy in a separate folder. The pages you are reading come from a small program called Senu. You will run that same program on your own computer, add your own notes to it, and then rebuild one of its functions.

## What you need

You need a computer running Linux, macOS, or Windows with WSL (a Linux system inside Windows, set up in the next chapter), permission to install software on it, and an internet connection for the downloads. You do not need to know any Rust. It helps if you have seen a little JavaScript, because the course compares Rust with these JavaScript words when they first appear: `let`, `const`, `function`, `if` and `else`, `import` and `export`.

Plan for about two hours. The first build alone can take several minutes.

## Three windows

You will switch between three programs, and every step says which one to use.

- The **terminal** is a window where you type commands and press Enter. On macOS it is the Terminal app; on Linux it is usually called Terminal or Console; with WSL it is the Ubuntu window.
- The **editor** is where you open, change and save files. Any plain-text editor works, and Visual Studio Code is a common choice. A word processor such as Word does not work, because it adds formatting to the text.
- The **browser**, such as Firefox, Chrome, Edge or Safari, shows the pages that Senu serves.

## Your first terminal commands

When the terminal is ready for a command, it shows a **prompt**: a short text that often ends in `$` or `%`, such as `ada@laptop:~$`. The terminal prints the prompt; you never type it. In this course, a grey box with a command contains exactly what you type after the prompt, and the box after it shows what should come back.

Open a terminal now. Type this command and press Enter; it prints the folder the terminal is working in, called the **working directory**:

```text
pwd
```

The answer is one line like the one below. Do not type it: it is output, and yours will differ, because it shows your own computer:

```text
/home/ada
```

A new terminal starts in your **home folder**, your own folder on the computer. Its short name is `~` (a tilde), which is why many prompts show `~`, as in `ada@laptop:~$` above. Now type this command and press Enter to list what is in the working directory:

```text
ls
```

It answers with names such as these:

```text
Desktop  Documents  Downloads
```

`cd` changes the working directory: `cd Documents` moves into the Documents folder, `cd ..` moves one level up, and `cd ~` goes back to your home folder from anywhere. `~/senu` means the `senu` folder inside your home folder, so `cd ~/senu` works from any folder too. Most commands in this course must run inside the `senu` folder that you download in the next chapter, so `pwd` is the first thing to try whenever a command says it cannot find something.

## Exercise

Use `cd` to move into a folder that `ls` showed you, run `pwd` to confirm where you are, and come back with `cd ..`.

**Solution.** If `ls` showed `Documents`, type `cd Documents`, then `pwd`, whose answer now ends in `/Documents`, then `cd ..`. If `cd` answers "No such file or directory", check the spelling: on Linux, and usually on macOS, `documents` and `Documents` are different names.

## If something goes wrong

- **"command not found"** usually means a typing mistake. Check the spelling, and check that you did not type the `$` of the prompt.
- **The terminal seems stuck** and shows no prompt: a command is still running. Press Ctrl+C (hold the Control key and press C) to stop it and get the prompt back. You will use Ctrl+C often in this course.

Next: [install Rust and get the code](/install).
