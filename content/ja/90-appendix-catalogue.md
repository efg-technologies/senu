# 付録: プロジェクト一覧 (任意)

Senu には記事のほかに、`senu serve` というもう一つの動き方があります。決まったサンプルのプロジェクト一覧を、Web ページと JSON (プログラムが読むテキストの形式) で、それぞれ別のデモ用の認証情報の後ろに表示します。この付録ではそれを動かし、テストから先にプロジェクトの名前を変えます。講座を終えていることを前提にしています。下のデモ用の認証情報は、自分のコンピューターの中でしか使えません。

## 一覧を起動する

どちらの動き方もポート 3000 を使うので、先に Ctrl+C でサイトを止めてください。`senu serve` には認証情報が三つ必要です。コマンドの前に `NAME=value` と書くと、そのコマンドに**環境変数**という設定を渡せます。

```sh check=serve server
SENU_WEB_USER=demo SENU_WEB_PASSWORD=demo-password SENU_API_TOKEN=demo-api-token cargo run --quiet -- serve
```

```text check=serve
senu: listening on http://127.0.0.1:3000
```

## 一覧を見る

Web ページは http://127.0.0.1:3000/projects にあり、ブラウザーがユーザー名 `demo` とパスワード `demo-password` を尋ねます。次のコマンドは名前に「notes」を含むプロジェクトを検索し、grep が一覧の項目だけを残します。プロジェクトの名前にはわざと `<script>` タグが入っていますが、ノートと同じように文字として表示されます。

```sh check=html
curl -s -u demo:demo-password 'http://127.0.0.1:3000/projects?q=notes' | grep 'data-project-id'
```

```text check=html
<li data-project-id="acme-003">&#60;script&#62;alert(&#39;x&#39;)&#60;/script&#62; &#38; &#34;Launch&#34; Notes</li>
```

同じ検索を、パスワードの代わりに API トークンを使って JSON で行います。

```sh check=json
curl -s -w '\n' -H 'Authorization: Bearer demo-api-token' 'http://127.0.0.1:3000/api/projects?q=notes'
```

```text check=json
{"projects":[{"id":"acme-003","name":"<script>alert('x')</script> & \"Launch\" Notes"}]}
```

認証情報はそれぞれ自分の扉しか開けません。API トークンを Web ページに送ると、ステータスコード 401 (Unauthorized、認証されていない) が返ります。

```sh check=wrong-credential
curl -s -o /dev/null -w '%{http_code} %header{www-authenticate}\n' -H 'Authorization: Bearer demo-api-token' 'http://127.0.0.1:3000/projects'
```

```text check=wrong-credential
401 Basic realm="senu", charset="UTF-8"
```

## 演習: テストから先にプロジェクトの名前を変える

最初のプロジェクトの名前を「Apollo Launch」から「Artemis Launch」に変えます。関数の章と同じように、先にテストを変えます。下の枠はそれぞれ**差分** (diff) です。`-` で始まる行を消し、`+` で始まる行を足します。ほかの行は場所の目印です。エディターでそれぞれの変更をしてください。

まず期待する値です。`src/catalogue.rs` のテスト `demo_catalogue_matches_the_spec_table` は、すべてのプロジェクトを並べています。

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

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 132 filtered out; finished in 0.00s

error: test failed, to rerun pass `--lib`
```

正しい理由で red になりました。データがまだ Apollo のままだからです。次に、同じファイルの少し上にあるデータを変えます。

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
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 132 filtered out; finished in 0.00s

```

`tests` フォルダーのテストは Senu を外側から HTTP で呼んでいて、そのうち二つがまだ古い名前を期待しています。`tests/adapter_http.rs` を直します。

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

running 24 tests
........................
test result: ok. 24 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s


running 2 tests
..
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

```

**解答.** 上の三つの差分が解答です。期待する表の名前が一つ、データの名前が一つ、HTTP のテストの 2 か所です。`grep -rn Apollo src tests` のように古い名前でプロジェクトを検索すると、直すべき場所がすぐに見つかります。

## うまくいかないとき

- **「senu: SENU_WEB_USER is not set」**: 三つの設定はコマンドと同じ行に書きます。
- **「address in use」**: 講座のサイトがまだ動いています。そのターミナルで Ctrl+C を押してください。
- **差分の行がファイルと合わない**: 差分の行番号は目安です。代わりに `-` の行の文字で検索してください。

次へ: [MCP で Senu を使う](/ja/appendix-mcp)
