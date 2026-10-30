<nav class="senu-lang" aria-label="Language"><a href="ja/exercise.html" hreflang="ja" lang="ja">日本語</a></nav>

# Exercise: rename a project

In this exercise you rename one project, `acme-001`, from "Apollo Launch" to
"Artemis Launch". Its id stays `acme-001`. You change the test first and watch
it fail, then change the catalogue and watch it pass, then follow the other
tests that pin the old name until the whole suite and the quickstart pass.
At the end you put the original catalogue back.

Each change below is shown as a diff. Edit the file by hand, or apply the
same diff with the `git apply` command under it.

Do the exercise on its own branch, starting from a clean working tree: the
last step restores two files, which would throw away any uncommitted edits to
them. `git status --short` lists your changes, and `git diff --quiet HEAD`
succeeds only when no tracked file differs from your last commit, so the
branch is created only then. If a change is listed and no branch is created,
commit or stash your work first, or stop here. Untracked files, marked `??`,
are left alone by the exercise.

```sh
{{#include ../exercise/steps.sh:branch}}
```

```text
{{#include ../exercise/steps.out:branch}}
```

## 1. Red: expect the new name

The test `demo_catalogue_matches_the_spec_table`, near the end of
[`src/catalogue.rs`](https://github.com/efg-technologies/senu/blob/main/src/catalogue.rs),
lists every project as an `(id, tenant, name)` tuple. In the test, change the
expected name of `acme-001`:

```diff
{{#include ../exercise/1-expect-artemis.patch}}
```

```sh
{{#include ../exercise/steps.sh:expect}}
```

Run only that test:

```sh
{{#include ../exercise/steps.sh:red}}
```

It compiles and fails on the assertion. Among the output you should see
these lines (`...` stands for the rest of a long line):

```text
{{#include ../exercise/steps.out:red}}
```

`left` is what the catalogue holds and `right` is what the test now expects.
A red step is only useful if it fails for this reason; a compile error would
tell you nothing about the catalogue.

## 2. Green: rename the project

Now change the catalogue itself, in `DEMO_PROJECTS` near the top of the same
file:

```diff
{{#include ../exercise/2-rename-catalogue.patch}}
```

```sh
{{#include ../exercise/steps.sh:rename}}
```

Run the same test again:

```sh
{{#include ../exercise/steps.sh:green}}
```

```text
{{#include ../exercise/steps.out:green}}
```

## 3. Red again: the whole suite

One passing test is not the whole story. Run every test:

```sh
{{#include ../exercise/steps.sh:suite-red}}
```

The library tests pass, but the run stops at `tests/adapter_http.rs`:

```text
{{#include ../exercise/steps.out:suite-red}}
```

These two tests are part of the HTTP contract and pin catalogue data on
purpose. One expects the exact JSON body for `launch`, including the name of
`acme-001`. The other searches for `apollo launch` and expects `acme-001`,
which no longer matches. When you change the catalogue on purpose, these
maintained fixtures have to follow the change. Other tests that mention
"Apollo Launch", in `src/view.rs` and `tests/quickstart_check.rs`, build their
own sample data instead of reading the catalogue, so they keep passing and
you leave them alone.

## 4. Green: follow the fixtures

Update the expected JSON body and the search in
[`tests/adapter_http.rs`](https://github.com/efg-technologies/senu/blob/main/tests/adapter_http.rs):

```diff
{{#include ../exercise/3-follow-fixtures.patch}}
```

```sh
{{#include ../exercise/steps.sh:fixtures}}
```

```sh
{{#include ../exercise/steps.sh:suite-green}}
```

Every test binary now reports `ok`. Among them you should see the HTTP
adapter tests:

```text
{{#include ../exercise/steps.out:suite-green}}
```

## 5. Check all three transports

Run the parity test on its own, then the quickstart:

```sh
{{#include ../exercise/steps.sh:parity}}
```

```text
{{#include ../exercise/steps.out:parity}}
```

The JSON API, the HTML page and the MCP tool all show "Artemis Launch", the
ids are still `acme-001` and `acme-003`, and the parity test confirms that the
three adapters still agree with `list_projects`. None of the adapters needed
a change, because they all read the one catalogue through the one operation.

## 6. Put it back

The renamed catalogue belongs to this exercise only. Restore the two files,
return to your previous branch and delete the exercise branch:

```sh
{{#include ../exercise/steps.sh:restore}}
```

```text
{{#include ../exercise/steps.out:restore}}
```

In a real rename you would commit on the branch instead, and also update the
catalogue table in
[spec 0001](https://github.com/efg-technologies/senu/blob/main/docs/specs/0001-project-catalogue.md),
which no test reads.

## Check the tutorial yourself

[`scripts/check_tutorial.py`](https://github.com/efg-technologies/senu/blob/main/scripts/check_tutorial.py)
runs the same files this tutorial includes. `exercise` copies your last
commit, and nothing else, into a temporary git repository, so commit your
changes first: it refuses to run while tracked files differ from that commit.
It reuses a copy of your `target/debug` build cache and runs every step above
in order. It checks each exit status, checks
that the red steps fail on test assertions rather than on patch or compiler
errors, and finally checks that your own working tree did not change.
`commands` starts a server like chapter 3 and compares every other command in
chapters 2 to 5 with its expected output.

```sh
{{#include ../exercise/check.sh:check}}
```

## Limits and next steps

Senu stops here on purpose. The catalogue is compiled in and read-only, so a
real application would need storage, migrations and write operations, each
with its own specification. Because Basic authentication is a credential that
browsers send automatically, any write operation would also need CSRF
protection first. Production use would need real user authentication,
sessions, TLS in front of the server, logging and rate limits; Senu has none
of these. Good next steps are to read
[spec 0001](https://github.com/efg-technologies/senu/blob/main/docs/specs/0001-project-catalogue.md)
and the [development log](https://github.com/efg-technologies/senu/blob/main/docs/development.md),
which record how the first slice was built and tested, and to try
another small change, such as a new search case in
`tests/adapter_parity.rs`, with the same red-then-green rhythm.
