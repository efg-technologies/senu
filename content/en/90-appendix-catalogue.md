# Appendix: the project catalogue (optional)

Besides articles, Senu has a second mode, `senu serve`. It shows a fixed list of sample projects as a web page and as JSON, a text format that programs read, each behind its own demo credential. This appendix runs it and then renames a project test first. It assumes that you have finished the course; the demo credentials below work only on your own computer.

## Start the catalogue

Stop the site with Ctrl+C first, because both modes use port 3000. `senu serve` needs three credentials. Writing `NAME=value` in front of a command gives that command a setting called an **environment variable**:

```sh check=serve server
SENU_WEB_USER=demo SENU_WEB_PASSWORD=demo-password SENU_API_TOKEN=demo-api-token cargo run --quiet -- serve
```

```text check=serve
senu: listening on http://127.0.0.1:3000
```

## Look at the list

The web page is at http://127.0.0.1:3000/projects; the browser asks for the user name `demo` and the password `demo-password`. The command below searches for projects whose name contains "notes", and grep keeps only the list items. The project's name contains a `<script>` tag on purpose, and it is shown as text, as in your notes:

```sh check=html
curl -s -u demo:demo-password 'http://127.0.0.1:3000/projects?q=notes' | grep 'data-project-id'
```

```text check=html
<li data-project-id="acme-003">&#60;script&#62;alert(&#39;x&#39;)&#60;/script&#62; &#38; &#34;Launch&#34; Notes</li>
```

The same search as JSON, with the API token instead of the password:

```sh check=json
curl -s -w '\n' -H 'Authorization: Bearer demo-api-token' 'http://127.0.0.1:3000/api/projects?q=notes'
```

```text check=json
{"projects":[{"id":"acme-003","name":"<script>alert('x')</script> & \"Launch\" Notes"}]}
```

Each credential opens only its own door. Sending the API token to the web page gets the status code 401, Unauthorized:

```sh check=wrong-credential
curl -s -o /dev/null -w '%{http_code} %header{www-authenticate}\n' -H 'Authorization: Bearer demo-api-token' 'http://127.0.0.1:3000/projects'
```

```text check=wrong-credential
401 Basic realm="senu", charset="UTF-8"
```

## Exercise: rename a project, test first

Rename the first project from "Apollo Launch" to "Artemis Launch". As in the function chapter, change the test first. Each box below is a **diff**: lines that start with `-` are removed and lines that start with `+` are added, and the other lines show where. Make each change in your editor.

First the expectation. In `src/catalogue.rs`, the test `demo_catalogue_matches_the_spec_table` lists every project:

```diff patch
--- a/src/catalogue.rs
+++ b/src/catalogue.rs
@@ -93,7 +93,7 @@
         assert_eq!(
             all,
             vec![
-                ("acme-001", "acme", "Apollo Launch"),
+                ("acme-001", "acme", "Artemis Launch"),
                 ("acme-002", "acme", "Café Roadmap"),
                 (
                     "acme-003",
```

```sh check=red exit=101
cargo test --quiet --lib demo_catalogue_matches_the_spec_table
```

```text check=red

running 1 test
catalogue::tests::demo_catalogue_matches_the_spec_table --- FAILED

failures:

---- catalogue::tests::demo_catalogue_matches_the_spec_table stdout ----

thread 'catalogue::tests::demo_catalogue_matches_the_spec_table' (ID) panicked at src/catalogue.rs:93:9:
assertion `left == right` failed
  left: [("acme-001", "acme", "Apollo Launch"), ("acme-002", "acme", "Café Roadmap"), ("acme-003", "acme", "<script>alert('x')</script> & \"Launch\" Notes"), ("globex-001", "globex", "Globex Launch"), ("globex-002", "globex", "Zephyr Migration")]
 right: [("acme-001", "acme", "Artemis Launch"), ("acme-002", "acme", "Café Roadmap"), ("acme-003", "acme", "<script>alert('x')</script> & \"Launch\" Notes"), ("globex-001", "globex", "Globex Launch"), ("globex-002", "globex", "Zephyr Migration")]
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    catalogue::tests::demo_catalogue_matches_the_spec_table

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 126 filtered out; finished in 0.00s

error: test failed, to rerun pass `--lib`
```

Red, for the right reason: the data still says Apollo. Now change the data, a few lines higher in the same file:

```diff patch
--- a/src/catalogue.rs
+++ b/src/catalogue.rs
@@ -37,7 +37,7 @@
     Project {
         id: "acme-001",
         tenant: TenantId::ACME,
-        name: "Apollo Launch",
+        name: "Artemis Launch",
     },
     Project {
         id: "acme-002",
```

```sh check=green
cargo test --quiet --lib demo_catalogue_matches_the_spec_table
```

```text check=green

running 1 test
.
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 126 filtered out; finished in 0.00s

```

The tests in the `tests` folder call Senu from outside, through HTTP, and two of them still expect the old name. Update them in `tests/adapter_http.rs`:

```diff patch
--- a/tests/adapter_http.rs
+++ b/tests/adapter_http.rs
@@ -129,7 +129,7 @@
     assert_eq!(
         reply.json(),
         json!({"projects": [
-            {"id": "acme-001", "name": "Apollo Launch"},
+            {"id": "acme-001", "name": "Artemis Launch"},
             {"id": "acme-003", "name": "<script>alert('x')</script> & \"Launch\" Notes"},
         ]})
     );
@@ -298,7 +298,7 @@
     let app = app();
     let reply = send(&app, api_get("/api/projects?q=CAF%C3%89+")).await;
     assert_eq!(ids(&reply.json()), ["acme-002"]);
-    let reply = send(&app, api_get("/api/projects?q=apollo+launch")).await;
+    let reply = send(&app, api_get("/api/projects?q=artemis+launch")).await;
     assert_eq!(ids(&reply.json()), ["acme-001"]);
     let reply = send(&app, api_get("/api/projects?q")).await;
     assert_eq!(ids(&reply.json()), ["acme-001", "acme-002", "acme-003"]);
```

```sh check=adapters
cargo test --quiet --test adapter_http --test adapter_parity
```

```text check=adapters

running 19 tests
...................
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s


running 1 test
.
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

```

**Solution.** The three diffs above are the solution: one name in the expected table, one in the data, and the two places in the HTTP tests. Searching the project for the old name, for example with `grep -rn Apollo src tests`, is a quick way to find every place.

## If something goes wrong

- **"senu: SENU_WEB_USER is not set"**: the three settings must be on the same line as the command.
- **"address in use"**: the site from the course is still running; press Ctrl+C in its terminal.
- **A patch line does not match your file**: the line numbers in a diff are a hint; search for the text of the `-` line instead.

Next: [Senu over MCP](/appendix-mcp).
