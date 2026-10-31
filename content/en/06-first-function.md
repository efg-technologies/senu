# Your first Rust function

Every article page shows a reading time under its title, such as "About 1 min read". In this chapter you rebuild the Rust function that computes it, test first, and then see the result on your note's page. All the Rust you need is explained before you type it.

## Where the function lives

Rust code lives in the `src` folder. Open `src/reading.rs` in your editor and read it before you change anything. Each `.rs` file is a **module**, Rust's name for a file of code that other files can use. `src/lib.rs` lists Senu's modules with lines such as `pub mod reading;`, where `pub` (public) works much like `export` in JavaScript. Another module brings the function in with `use crate::reading::reading_minutes;`, much like `import { readingMinutes } from "./reading.js"`. Such a **path** names its parts from the outside in, separated by `::` much as `/` separates folders: `crate` is the top of Senu's own code, `reading` the module and `reading_minutes` the function.

When Senu builds a page, it counts the words of the note outside code boxes and asks `reading_minutes` how many minutes they take. The estimate is deliberately simple: 200 words per minute, rounded up, so that a short note shows 1 minute. Japanese has no spaces between words, so for Japanese notes Senu counts two characters as one word; that is rougher still.

## The Rust in this chapter

- `fn reading_minutes(words: usize) -> usize { ... }` is like `function readingMinutes(words) { ... }`. Rust writes the **type** of each parameter after a colon, and the type of the result after `->`.
- `usize` is the type of whole numbers that cannot be negative: 0, 1, 2 and so on, up to a very large maximum called `usize::MAX`. Rust refuses to build code that passes text or a negative number where a `usize` belongs.
- `const WORDS_PER_MINUTE: usize = 200;` names a fixed value for the whole file.
- `let full_minutes = words / WORDS_PER_MINUTE;` is like `const fullMinutes = ...` in JavaScript: a Rust `let` cannot be changed later.
- `/` divides whole numbers and drops the remainder, so `201 / 200` is `1`, and `%` gives that remainder, so `201 % 200` is `1` too.
- `if leftover_words > 0 { a } else { b }` works like JavaScript's `if`, but it also produces a value, `a` or `b`. The last line of a function, written without a semicolon, is the value the function returns, as if it had `return` in front.

A **test** is a small function that calls your code and checks the answer. The line `#[test]` above a function marks it as a test, and `assert_eq!(left, right)` checks that two values are equal and stops the test with a message when they are not. The `!` marks a **macro**, code that Rust expands for you; here you can read it as an ordinary function call.

Tests usually sit at the end of the file, in a small module of their own: `#[cfg(test)]` above `mod tests { ... }` means that this module is built only when tests run, and inside it `use super::reading_minutes;` imports the function from `super`, the module around it, which here is the rest of `src/reading.rs`.

## Red: a first attempt

Replace everything in `src/reading.rs` with this first attempt. It has one test with four examples, and a function that only divides:

```rust file=src/reading.rs
// Turns a word count into the minutes shown under an article's title.

const WORDS_PER_MINUTE: usize = 200;

pub fn reading_minutes(words: usize) -> usize {
    words / WORDS_PER_MINUTE
}

#[cfg(test)]
mod tests {
    use super::reading_minutes;

    #[test]
    fn rounds_up_to_whole_minutes() {
        assert_eq!(reading_minutes(0), 0);
        assert_eq!(reading_minutes(1), 1);
        assert_eq!(reading_minutes(200), 1);
        assert_eq!(reading_minutes(201), 2);
    }
}
```

Run the tests of this module. `--lib` means the library part of Senu, and `reading::` selects the tests whose full names start with it:

```sh check=red exit=101
cargo test --quiet --lib reading::
```

```text check=red

running 1 test
reading::tests::rounds_up_to_whole_minutes --- FAILED

failures:

---- reading::tests::rounds_up_to_whole_minutes stdout ----

thread 'reading::tests::rounds_up_to_whole_minutes' (ID) panicked at src/reading.rs:16:9:
assertion `left == right` failed
  left: 0
 right: 1
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    reading::tests::rounds_up_to_whole_minutes

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 131 filtered out; finished in 0.00s

error: test failed, to rerun pass `--lib`
```

The test failed, and that is what you wanted: it shows that the test can catch a mistake. A failing test is called **red**. The message points at `assert_eq!(reading_minutes(1), 1)` in `src/reading.rs`: the function returned 0 (`left`) where the test expected 1 (`right`), because `1 / 200` is 0. One word still needs a minute. The number in brackets after the test name, written `(ID)` here, differs on every run.

## Green: round up

Change the function so that leftover words add one more minute. Replace `src/reading.rs` again:

```rust file=src/reading.rs
// Turns a word count into the minutes shown under an article's title.

const WORDS_PER_MINUTE: usize = 200;

pub fn reading_minutes(words: usize) -> usize {
    let full_minutes = words / WORDS_PER_MINUTE;
    let leftover_words = words % WORDS_PER_MINUTE;
    if leftover_words > 0 {
        full_minutes + 1
    } else {
        full_minutes
    }
}

#[cfg(test)]
mod tests {
    use super::reading_minutes;

    #[test]
    fn rounds_up_to_whole_minutes() {
        assert_eq!(reading_minutes(0), 0);
        assert_eq!(reading_minutes(1), 1);
        assert_eq!(reading_minutes(200), 1);
        assert_eq!(reading_minutes(201), 2);
    }
}
```

```sh check=green
cargo test --quiet --lib reading::
```

```text check=green

running 1 test
.
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 131 filtered out; finished in 0.00s

```

All four examples pass. A passing test is called **green**.

## See it on the page

The site calls the same function, so restart Senu to build it with your version: Ctrl+C in its terminal, then the usual command.

```sh check=reading server
cargo run --quiet -- site
```

```text check=reading
senu: listening on http://127.0.0.1:3000
```

Open http://127.0.0.1:3000/my-first-note. Your note is much shorter than 200 words, so it shows 1 minute. From the second terminal, `grep -A1` prints each matching line and the one after it, here the title and the reading time below it:

```sh check=reading-time
curl -s http://127.0.0.1:3000/my-first-note | grep -A1 '<h1>'
```

```text check=reading-time
<h1>My first note</h1>
<p class="text-sm text-gray-600">About 1 min read</p>
```

## Exercise: the largest number

What happens for the largest possible word count, `usize::MAX`? Add a second test for it below the first one, so that `src/reading.rs` reads:

```rust file=src/reading.rs
// Turns a word count into the minutes shown under an article's title.

const WORDS_PER_MINUTE: usize = 200;

pub fn reading_minutes(words: usize) -> usize {
    let full_minutes = words / WORDS_PER_MINUTE;
    let leftover_words = words % WORDS_PER_MINUTE;
    if leftover_words > 0 {
        full_minutes + 1
    } else {
        full_minutes
    }
}

#[cfg(test)]
mod tests {
    use super::reading_minutes;

    #[test]
    fn rounds_up_to_whole_minutes() {
        assert_eq!(reading_minutes(0), 0);
        assert_eq!(reading_minutes(1), 1);
        assert_eq!(reading_minutes(200), 1);
        assert_eq!(reading_minutes(201), 2);
    }

    #[test]
    fn the_largest_count_does_not_overflow() {
        assert_eq!(reading_minutes(usize::MAX), usize::MAX / 200 + 1);
    }
}
```

```sh check=both
cargo test --quiet --lib reading::
```

```text check=both

running 2 tests
..
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 131 filtered out; finished in 0.00s

```

A shorter formula looks tempting, because `(words + 199) / WORDS_PER_MINUTE` also rounds up. Try it: replace the body of the function so that `src/reading.rs` reads:

```rust file=src/reading.rs
// Turns a word count into the minutes shown under an article's title.

const WORDS_PER_MINUTE: usize = 200;

pub fn reading_minutes(words: usize) -> usize {
    (words + 199) / WORDS_PER_MINUTE
}

#[cfg(test)]
mod tests {
    use super::reading_minutes;

    #[test]
    fn rounds_up_to_whole_minutes() {
        assert_eq!(reading_minutes(0), 0);
        assert_eq!(reading_minutes(1), 1);
        assert_eq!(reading_minutes(200), 1);
        assert_eq!(reading_minutes(201), 2);
    }

    #[test]
    fn the_largest_count_does_not_overflow() {
        assert_eq!(reading_minutes(usize::MAX), usize::MAX / 200 + 1);
    }
}
```

Run only the new test; a filter that matches a single test name keeps the output short:

```sh check=overflow exit=101
cargo test --quiet --lib overflow
```

```text check=overflow

running 1 test
reading::tests::the_largest_count_does_not_overflow --- FAILED

failures:

---- reading::tests::the_largest_count_does_not_overflow stdout ----

thread 'reading::tests::the_largest_count_does_not_overflow' (ID) panicked at src/reading.rs:6:5:
attempt to add with overflow
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    reading::tests::the_largest_count_does_not_overflow

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 132 filtered out; finished in 0.00s

error: test failed, to rerun pass `--lib`
```

What went wrong, and why does the first version not have this problem?

**Solution.** When `words` is `usize::MAX`, `words + 199` is larger than the largest `usize`, so the addition **overflows**: the result does not fit. In a test build, Rust stops with "attempt to add with overflow" instead of carrying on with a wrong number. Dividing first never makes a number larger, and `usize::MAX / 200 + 1` fits easily, so the quotient-and-remainder version works for every input. Put it back, so that `src/reading.rs` reads:

```rust file=src/reading.rs
// Turns a word count into the minutes shown under an article's title.

const WORDS_PER_MINUTE: usize = 200;

pub fn reading_minutes(words: usize) -> usize {
    let full_minutes = words / WORDS_PER_MINUTE;
    let leftover_words = words % WORDS_PER_MINUTE;
    if leftover_words > 0 {
        full_minutes + 1
    } else {
        full_minutes
    }
}

#[cfg(test)]
mod tests {
    use super::reading_minutes;

    #[test]
    fn rounds_up_to_whole_minutes() {
        assert_eq!(reading_minutes(0), 0);
        assert_eq!(reading_minutes(1), 1);
        assert_eq!(reading_minutes(200), 1);
        assert_eq!(reading_minutes(201), 2);
    }

    #[test]
    fn the_largest_count_does_not_overflow() {
        assert_eq!(reading_minutes(usize::MAX), usize::MAX / 200 + 1);
    }
}
```

```sh check=final
cargo test --quiet --lib reading::
```

```text check=final

running 2 tests
..
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 131 filtered out; finished in 0.00s

```

Your `src/reading.rs` is now exactly the file that Senu ships with: you rebuilt a real part of the program.

## If something goes wrong

- **Lines that start with `error[E` and end with "could not compile"**: that is a compiler error, not a failing test. The message names a file, a line and a column; look there for a missing `;` or `}` or a misspelt name, and compare with the box above.
- **"running 0 tests"**: the filter matched nothing; check that you typed `reading::` with two colons.
- **The page shows the old reading time**: restart Senu after changing Rust code, as after changing a note.

Next: [what you built, and where to go next](/next-steps).
