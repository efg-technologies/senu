# Senu

[English](README.md) | [日本語](README.ja.md)

**Rust の関数を一度宣言すれば、Web ページ・JSON API・MCP ツールとして提供できます。**

[![CI](https://github.com/efg-technologies/senu/actions/workflows/ci.yml/badge.svg)](https://github.com/efg-technologies/senu/actions/workflows/ci.yml)
[ガイドと講座](https://senu.efg-technologies.com/ja/) ·
[API リファレンス](https://docs.rs/senu) ·
[変更履歴](CHANGELOG.md) ·
[セキュリティ](SECURITY.md)

Senu はオープンソースの Rust ライブラリです。読み取り専用の関数を、権限とパラメータと
一緒に一度だけ宣言します。Senu はそれを、[Askama](https://askama.rs/) で描画する HTML
ページ、[Axum](https://docs.rs/axum) の JSON API、公式 SDK
[`rmcp`](https://rust.sdk.modelcontextprotocol.io/) で作った AI アシスタント向けの
[MCP](https://modelcontextprotocol.io/) ツールとして提供します。3 つとも同じ手順を同じ
順序で通ります。サイズの上限、メソッド、認証、権限、パラメータの枠、あなたの関数、
固定のエラー対応の順です。これを証明するテストはキットとして同梱していて、自分のルートに
対して走らせられます。

```sh
cargo add senu --no-default-features
```

crates.io の代わりに、Git のタグからビルドすることもできます。

```sh
cargo add senu --git https://github.com/efg-technologies/senu --tag v0.2.0 --no-default-features
```

## 完全なアプリケーション

[`examples/hello.rs`](examples/hello.rs) の、import より後の全体です。操作が 1 つあり、
Bearer トークンで守られ、MCP ツールとしても一覧に載っています。

```rust
// 1. Your state. Here it is one string; in a real service it holds a
//    database pool.
struct Greeter {
    greeting: &'static str,
}

// 2. A permission, and a principal that holds it.
const GREET: Permission = Permission::new("greet");
const GUEST: Principal = Principal::new("guest", TenantId::from_static("main"), &[GREET]);

// 3. The function: state, caller and arguments in, a result or a safe
//    error out. It knows nothing about HTTP or MCP.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GreetingArgs {
    name: String,
}

#[derive(Serialize)]
struct Greeting {
    message: String,
}

fn greeting(greeter: &Greeter, _caller: &Principal, args: &Args) -> Result<Greeting, OpError> {
    let GreetingArgs { name } = decode(args)?;
    if name.is_empty() || name.chars().count() > 40 || name.chars().any(char::is_control) {
        return Err(OpError::Invalid {
            code: "invalid_name",
            phrase: "Invalid name",
        });
    }
    Ok(Greeting {
        message: format!("{}, {name}!", greeter.greeting),
    })
}

// 4. The declaration. It exposes nothing by itself.
const GREETING: Operation<Greeter, Greeting> = Operation {
    name: "greeting",
    description: "Greets one person by name.",
    permission: GREET,
    params: &[Param::required("name", "Who to greet: 1 to 40 characters.")],
    run: Run::Sync(greeting),
};

// 5. Exposure: one JSON route and one MCP tool, both listed by hand.
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let token = Secret::new(std::env::var("HELLO_TOKEN")?)?;
    let greeter = Greeter { greeting: "Hello" };

    if std::env::args().nth(1).as_deref() == Some("mcp") {
        // MCP over stdio: every call runs as GUEST.
        let server = McpServer::with_tools(greeter, GUEST, vec![mcp::tool(&GREETING)])?;
        return Ok(server.with_info("hello", "1.0.0").serve_stdio().await?);
    }

    // HTTP: `router` is an ordinary `axum::Router`.
    let state = AppState::builder(greeter)
        .api_auth(Bearer::new(token, GUEST))
        .build();
    let router = Endpoints::new()
        .api("/api/greeting", &GREETING)
        .into_router(state)?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    axum::serve(listener, router).await?;
    Ok(())
}
```

```text
$ HELLO_TOKEN=choose-a-long-random-value cargo run --example hello &
$ curl -H 'Authorization: Bearer choose-a-long-random-value' 'http://127.0.0.1:3000/api/greeting?name=Ada'
{"message":"Hello, Ada!"}
```

[`examples/inventory.rs`](examples/inventory.rs) はもう一歩進んだ例です。非同期の操作、
`X-Api-Key` を読む自作の認証器、2 つのテナント、HTML ページ、スタイルシート、MCP を
含みます。[`tests/example_inventory.rs`](tests/example_inventory.rs) が、この例に
Senu の契約テストを走らせます。

## 得られるもの

- **宣言は 1 つ、経路は 3 つ。** `Operation` が名前、説明、権限、文字列パラメータ、関数を
  持ちます。`Endpoints` にルートを、`mcp::tool` でツールを並べます。宣言しただけでは
  何も公開されず、並べたものだけが到達可能になります。
- **検査の順序は 1 つ。** HTML でも JSON でも MCP でも、認証が権限より先、権限が検証より
  先です。権限のない呼び出し元は、どの入力が有効かを何も知ることができません。
- **認証は持ち込めます。** `auth::Authenticate` が、リクエストのヘッダーをサーバー側の
  `Principal` に変えます。Bearer と Basic は組み込みです。セッション、データベースの
  API キー、プロキシが付けるヘッダーは、このトレイトを実装して扱います。認証器のない
  ルートは起動時に拒否されます。
- **同期でも非同期でも。** メモリ上の処理は `Run::Sync`、待つ処理は `Run::Async` です。
- **安全なエラー。** エラーはどれも固定のコードと文言なので、入力や秘密を繰り返しません。
  `OpError::Internal` は詳細を運びません。
- **契約をテストキットとして。** `senu::contract::check` (`contract` 機能) は、あなたの
  ルートとツールに実際のリクエストを送り、CT-1 から CT-7 の 7 つの規則を確かめます。
- **ふつうの `axum::Router`。** 好きな場所にバインドし、ログ、タイムアウト、レート制限の
  tower レイヤーを足せます。

[ガイド](https://senu.efg-technologies.com/ja/overview.html)が各部分を説明します。
[操作](https://senu.efg-technologies.com/ja/operations.html)、
[認証](https://senu.efg-technologies.com/ja/authentication.html)、
[MCP ツール](https://senu.efg-technologies.com/ja/mcp-tools.html)、
[デプロイ](https://senu.efg-technologies.com/ja/deployment.html)、
[テスト](https://senu.efg-technologies.com/ja/testing.html)、
[質問と比較](https://senu.efg-technologies.com/ja/faq.html)があります。
AI アシスタントは [llms.txt](https://senu.efg-technologies.com/llms.txt) (英語) を
読めます。

## 状態と制限

バージョン 0.2 は、ライブラリとして使うための最初のリリースです。API には文書があり、
契約はテストされています。できないことも、はっきり書いておきます。

- **読み取り専用。** 操作は `GET` で提供し、MCP ツールにはすべて読み取り専用の印が
  付きます。書き込み操作、フォーム、CSRF トークンはありません。
- **文字列と完全一致のパス。** パラメータは文字列で、解釈は操作が行います。
  パスパラメータはありません。
- **MCP は stdio だけ。** stdio サーバーは、自分を起動したローカルのプロセスを
  信頼します。HTTP 上の MCP はありません。
- **セッション、ログイン画面、OAuth はありません。** あなたの `Authenticate` の実装と、
  あなた自身のルートに置きます。
- **TLS、ログ、タイムアウト、レート制限はありません。** ルーターの手前か、tower の
  レイヤーに置きます。
- **未監査で、1.0 より前です。** 独立したセキュリティ監査は受けておらず、性能についての
  主張もしません。0.x の間は、マイナーバージョンで API が変わることがあります。変更は
  すべて [CHANGELOG.md](CHANGELOG.md) に書きます。

このリリースは[仕様 0006](docs/specs/0006-library-release-and-public-site.md) (英語) が
定めています。

## 機能 (feature)

| 機能 | 既定 | 足されるもの |
|---|---|---|
| `demo` | 有効 | カタログのデモ、`senu` バイナリ、講座を含む記事サイト。依存する側は `default-features = false` で外します。 |
| `contract` | 無効 | テストキット `senu::contract`。`[dev-dependencies]` で有効にします。 |

この README の残りは、デモ、講座、このリポジトリでの開発の説明です。

## 目指すこと (MCS): 最小・一貫・仕様駆動

- **最小 (Minimal):** アプリケーションの開発者が持つコード、設定、重複をできるだけ少なくします。
  操作の追加は宣言 1 つとルートやツールごとに 1 行、記事の追加は Markdown のファイル 1 つ、
  見た目の変更はテンプレートのクラスと `npm run css` です。
- **一貫 (Consistent):** 人と AI の道具が同じ決まりで書けるようにし、`rustfmt`、
  `clippy -D warnings`、契約テスト、スタイルシートについては CI のずれの検査で保ちます。
  これらが確かめるのは形とアダプターで、業務の規則が正しいことまでは証明しません。
- **仕様駆動 (Spec-driven):** [docs/specs/](docs/specs/) の仕様が先、次にテスト、最後にコードです。
  [開発ログ](docs/development.md) は、実際に走らせた検査を記録しています。

Rust を土台にしたのは、メモリーと型の安全性、そして速さのためですが、言語だけで
プログラムが安全で速くなるわけではありません。

## Rust がはじめての方は講座から

初心者向けの講座はサイトの一部で、
https://senu.efg-technologies.com/ja/start.html (日本語) と
https://senu.efg-technologies.com/start.html (英語) で読めます。講座では、
同じサイトを自分のコンピューターで動かします。講座の
[インストールの章](content/ja/02-install.md)のとおりに Rust と Git を入れてから、
次を実行してください。

```text
cd ~
git clone https://github.com/efg-technologies/senu.git
cd ~/senu
cargo run --quiet -- site
```

最初のビルドには数分かかります。そのあと http://127.0.0.1:3000/ja/ (日本語) か
http://127.0.0.1:3000/ (英語) を開いてください。講座に認証情報は要りません。詳しくは
[サイトと `senu site`](#サイトと-senu-site) を見てください。

## デモ: `senu serve` と `senu mcp`

`senu` バイナリはライブラリのローカルデモで、製品ではありません。

- **固定のデモデータ。** カタログは 2 つのテナントに属する 5 つのプロジェクトで、
  バイナリに組み込まれメモリ上にだけあります。保存はせず、変更操作もありません。
- **`senu serve`** はループバックアドレスだけで待ち受けます。`/projects` は HTTP
  Basic 認証、`/api/projects` は Bearer トークンを使います。TLS はないため、認証情報は
  ループバック上を平文で流れ、同じマシンの他のユーザーから届く可能性があります。
- **`senu mcp --enable-mcp`** は stdio で MCP を話し、このフラグがない限り無効です。
  起動前に `SENU_MCP_TOKEN` の設定が必要です。MCP の認可仕様では stdio サーバーは
  環境変数から認証情報を受け取るので、これは Senu を起動したローカルのプロセスを
  信頼する仕組みです。リモートの相手を認証するものではありません。

## クイックスタート

`rust-toolchain.toml` で固定した Rust ツールチェーン (rustup などで導入)、
`bash`、`curl`、`python3` が必要です。clone したディレクトリで `scripts/quickstart.sh` を
実行します。スクリプトの全文は次のとおりです。

```sh
#!/usr/bin/env bash
# Senu quickstart: build, call the JSON API and the HTML page, then call the
# MCP tool over stdio and check every reply. Run it from a clone of the
# repository; it needs cargo, curl and python3. The credentials are demo values
# for a loopback-only server; choose your own for anything else.
set -euo pipefail

command -v python3 >/dev/null || { echo "quickstart: python3 is required" >&2; exit 1; }
cargo build --locked --quiet
senu=./target/debug/senu
log=$(mktemp)

SENU_BIND=127.0.0.1:0 SENU_WEB_USER=demo SENU_WEB_PASSWORD=demo-password \
  SENU_API_TOKEN=demo-api-token "$senu" serve 2>"$log" &
server=$!
trap 'kill "$server" 2>/dev/null || true; wait "$server" 2>/dev/null || true; rm -f "$log"' EXIT

url=
for _ in $(seq 100); do
  url=$(sed -n 's/^senu: listening on //p' "$log")
  [ -n "$url" ] && break
  sleep 0.1
done
[ -n "$url" ] || { cat "$log" >&2; exit 1; }

echo "== JSON API (Bearer): $url/api/projects?q=launch"
curl -fsS -H 'Authorization: Bearer demo-api-token' "$url/api/projects?q=launch"
echo
echo "== HTML page (Basic): $url/projects?q=launch"
curl -fsS -u demo:demo-password "$url/projects?q=launch" | grep 'data-project-id'

echo "== MCP over stdio: initialize, then tools/call list_projects {\"q\":\"launch\"}"
SENU_MCP_TOKEN=demo-mcp-token python3 scripts/check_mcp.py "$senu" mcp --enable-mcp
```

MCP の手順では、Python の標準ライブラリだけを使う小さなクライアント
`scripts/check_mcp.py` を実行します。`initialize` の応答を待ってからツールを呼び出し、
各応答の id が要求と一致し、JSON-RPC エラーも `isError` もなく、プロジェクト acme-001 と
acme-003 が返った場合にだけ成功します。そうでなければサーバーを終了させて失敗し、
いずれかの手順が失敗すると quickstart は 0 以外の終了コードで終わります。

HTML の出力では Askama のエスケープが働き、`<script>alert('x')</script> & "Launch" Notes`
という名前のデモプロジェクトが文字列として表示されます。

## サイトと `senu site`

`senu site` は、ライブラリガイドと、Rust を知らない人向けの講座を配信します。どちらも
`content/en` と `content/ja` の Markdown の記事で、Senu 自身がループバックアドレスで
配信します。

```text
cargo run --quiet -- site
```

起動したら http://127.0.0.1:3000/ja/ (日本語) か http://127.0.0.1:3000/ (英語) を
開きます。講座はまずブラウザーにページを表示し、学習者がノートを書き、最後に読む時間を
計算する関数をテストから先に作り直します。任意の付録二つで、プロジェクト一覧、API、MCP を
扱います。

`content/ja/51-my-note.md` のような名前 (2 桁の数字、小文字のスラッグ、`.md`) の
ファイルは、次に `cargo run` か `cargo build` を実行したときに `/ja/my-note` のページに
なります。`build.rs` がファイルを埋め込むので、バイナリーは実行中に何も読まず、変更のたびに
ビルドし直す必要があります。1 行目は `# タイトル` でなければなりません。生の HTML は文字として
表示し、画像は断り、リンクは既存のページの正確なアドレスか `https://` で始まるものに限ります。
決まりを破ると、ファイル、行、理由を表示して起動を止めます。内容はリポジトリーの作者が書く
信頼できるものを前提とし、信頼できない入力を扱う仕組みではありません。`senu site` は
`SENU_BIND` だけを受け付け、認証情報の変数はすべて拒みます。`/projects`、`/api/*`、
MCP のルートはありません。記事、サイト、講座は
[仕様 0003](docs/specs/0003-articles-and-beginner-tutorial.md) (英語) にまとめていて、
仕様 0002 の mdBook のチュートリアルを置き換えます。

公開サイトは、同じページの静的なスナップショットです。
`python3 scripts/snapshot_site.py --base-url https://senu.efg-technologies.com/ target/site`
は `SENU_SITE_URL` を付けて `senu site` を起動します。すると各ページが正規のアドレス、
翻訳、構造化データを持ちます。さらに `sitemap.xml`、`robots.txt`、`llms.txt`、
`llms-full.txt`、各記事の Markdown、`404.html` を書き出します。`pages` ワークフローが、
`main` からこのディレクトリを GitHub Pages にデプロイします。記事の番号は、10〜49 が
ガイド、00〜09 が講座、50〜89 がノート、90〜99 が付録です。

## コマンド

| コマンド | 動作 |
|---|---|
| `senu serve` | `SENU_BIND` (既定は `127.0.0.1:3000`、ループバックの IP リテラルのみ、ポート 0 可) で HTML と JSON を提供 |
| `senu site` | Markdown の記事を `SENU_BIND` で HTML として提供 (認証情報なし)。`SENU_SITE_URL` (`/` で終わる `https://` のアドレス) を付けると、正規のアドレスと構造化データが加わります |
| `senu mcp --enable-mcp` | stdin が閉じるまで stdio で MCP を提供し、EOF で終了コード 0 |
| `senu --help` | 使い方を表示 |

`serve` には `SENU_WEB_USER` (表示可能な ASCII 1〜64 文字、`:` なし)、
`SENU_WEB_PASSWORD` と `SENU_API_TOKEN` (表示可能な ASCII 1〜256 文字) が必要です。
`mcp` には `SENU_MCP_TOKEN` が必要です。認証情報はすべて互いに異なる値にし、未知の
`SENU_*` 変数は拒否されます。設定エラーは終了コード 2 で、値を表示せずに変数名を示します。
待ち受けの失敗は終了コード 1 です。テナントやロールは設定できません。

## ルートと MCP ツール

- `GET /projects?q=` (Basic): 検索フォーム付きの HTML 一覧。
- `GET /api/projects?q=` (Bearer): JSON `{"projects": [{"id", "name"}]}`。
- `GET /project?id=` (Basic): 一つのプロジェクトの HTML ページ。
- `GET /api/project?id=` (Bearer): JSON `{"id", "name"}`。
- `GET /_senu/ui.css` (認証なし、`senu site` にもあります): スタイルシート。
  `text/css` でメモリーから返します。`/_senu/` は Senu の予約で、その下のほかのパスは 404 です。
- MCP ツール `list_projects`: `readOnlyHint: true`、任意の文字列 `q` のみを受け付けます。
  結果は `structuredContent` と JSON テキストの両方で返り、処理のエラーは
  `{"error": code}` を持つ `isError` の結果になります。未知のツールは JSON-RPC
  エラー `-32602`、rmcp が認識しないメソッドは `-32601` になり、どちらも
  要求された名前を繰り返さない固定のメッセージを返します。
- MCP ツール `get_project`: `readOnlyHint: true`、必須の文字列 `id` のみを受け付け、
  結果とエラーの形は `list_projects` と同じです。

`id` は必須で、`a-z`、`0-9`、`-` の 1 から 32 文字をそのまま照合します。それ以外の値は
`invalid_id` (400) です。存在しない id と、ほかのテナントの id は、どの経路でも同じ `not_found` (404) になります。
`q` は前後の空白を除いてから、プロジェクト名と大文字小文字を区別せずに照合します。
空白を除いた後の長さは Unicode スカラー値で 80 個までで、制御文字は使えません。未知のフィールドは拒否されます。
それ以外のパスは 404、それ以外のメソッドは 405 を返します。リクエストボディ (413) と
2048 バイトを超えるリクエストターゲット (414) は拒否し、クエリ文字列に入れた認証情報は読みません。
応答には `Cache-Control: no-store` と `X-Content-Type-Options: nosniff` が付き、
エラーが入力や秘密の値を繰り返すことはありません。

## 処理を追加する

自分のクレートでは、[`examples/inventory.rs`](examples/inventory.rs) と
[操作のガイド](https://senu.efg-technologies.com/ja/operations.html)に従ってください。
このリポジトリでは、アプリケーションは `src/demo/` のデモで、Senu のアダプターは
`src/http.rs` と `src/mcp.rs` にあります。

| アプリケーションが持つもの | Senu が持つもの |
|---|---|
| 状態、テナント、プリンシパル。認証器に渡す資格情報、または自作の `Authenticate`。`Permission` の定数。各処理の関数、引数の構造体、結果、エラーコード、`Operation` の宣言。ビューとテンプレート。ルート表とツールの一覧。業務のテスト | Basic と Bearer の認証器、サイズの制限、メソッドの確認、パラメーターの枠より先の権限確認、エラーの対応付け、CSP とヘッダー、パラメーターから作る JSON Schema、起動時のルートとツールの検証、契約テストのキット |

1. `src/demo/<name>.rs` を作り、`operation::decode` で `#[serde(deny_unknown_fields)]`
   の構造体に読み込む非公開の関数 `fn(&S, &Principal, &Args) -> Result<T, OpError>` と、
   名前・説明・権限・文字列のパラメーター・`run: Run::Sync(関数)` を持つ
   `pub const OPERATION: Operation<S, T>` を書きます。処理固有のエラーは
   `OpError::Invalid { code, phrase }` (400) か `OpError::NotFound` です。
2. `src/demo/mod.rs` に並べます。`endpoints()` に `.html(path, &OPERATION, view)` や
   `.api("/api/…", &OPERATION)` を、`tools()` に `mcp::tool(&OPERATION)` を書きます。
   宣言しても並べていない処理は公開されません。
3. 業務のテストを処理の隣に書き、`tests/contract.rs` の `demo_subject` に成功する例を
   1 つ足します。すると CT-1 から CT-7 がその処理にも走ります。ルートとツールを
   両方の README に書くと、`tests/docs_sync.rs` が確かめます。

制限: パラメーターは文字列で、パスは完全一致です。それ以外は `.custom(path, handler)`
を使います。サイズの制限と応答ヘッダーは付きますが、認証、メソッドの確認、CSP は付かないので、
ハンドラーが自分で行います。契約テストもこれを対象にしません。`Operation::call` は宣言された
権限をパラメーターの枠より先に、枠を関数より先に確かめます。関数の中でさらに行う方針の確認は
枠の後になります。ルート表やツールの一覧が正しくないと、`senu serve` や `senu mcp` は
待ち受けや stdin の読み込みの前に状態 2 で終了します。

0.1 からの移行 (仕様 0006、§9): デモのルート、本文、ヘッダー、スキーマ、エラーコードは
変わりません。`run: f` は `run: Run::Sync(f)` になります。`Operation::call` と
`McpServer::call` は `async` です。`Principal::tenant()` は `&TenantId` を返します。
`TenantId` は `operation` に移り、`Copy` ではなくなりました。`Project` も `Copy` では
ありません。`OpError::InvalidSearch`、`OpError::InvalidId` と、その `HttpError` 版は、
`Invalid { code, phrase }` に等しい関連定数になりました。`demo` 機能で使え、式でも
パターンでもこれまでどおり書けます。エラーの enum は `#[non_exhaustive]` になり、
`Internal` が加わったので、`match` には `_` の腕が要ります。`AppState` と `McpServer` の
状態の型に `Catalogue` の既定はなくなったので、`McpServer<Catalogue>` と書きます。
`AppState::new`、`with_principals`、`http::router`、`McpServer::new`、`with_principal`、
`mcp::tools`、`TOOL_NAME`、`GET_PROJECT_TOOL`、`operation::list_projects`、
`operation::get_project`、`Principal::demo_*` は、`demo` の下で同じパスのままです。
`Permission::ListProjects` と `Permission::ViewProject` は、仕様 0005 のときと同じく
関連定数として残っています。

## テンプレート

テンプレートは HTML エスケープを有効にした Askama の `.html` ファイルです。
`askama.toml` で ERB に似た構文を設定しています。

```text
<%# comments %>
<%! for p in projects %>
<li data-project-id="<%= p.id %>"><%= p.name %></li>
<%! endfor %>
```

ブロックは `<%! %>`、式は `<%= %>`、コメントは `<%# %>` です。ブロックに `<%` を
そのまま使えないのは、Askama がほかの区切り文字の接頭辞になる区切り文字を拒否するためです。
テンプレートや設定が `safe` フィルターを使ったりエスケープを無効にしたりすると、
テストが失敗します。唯一の例外は `TrustedHtml` で、作れるのは `src/markdown.rs` だけです。
Markdown のポリシーが作る記事の本文と、`<`、`>`、`&` を `\u` のエスケープにした JSON-LD の
要素がこれに当たります。同じテストが、`src/` の中の `HtmlSafe` の実装がその型に対する
一つだけであることを確かめます。
`senu serve` のページは、head、スタイルシートへのリンク、デモの注意書きを持つ
`serve_base.html` を拡張します。

## 見た目: Tailwind CSS と Typography

ページは `templates/*.html` の [Tailwind CSS](https://tailwindcss.com/) のユーティリティクラスと、
記事の本文に使う公式 Typography プラグインの `prose` で見た目を決めています。
見た目を変えるには、テンプレートのクラスを編集するか、すべてのページに効くテーマ変数を
ただ一つの CSS のソースである `assets/ui.css` で変えます。

```css
@theme {
  --font-sans: "Trebuchet MS", "Noto Sans JP", sans-serif;
}
```

Tailwind はテンプレートで見つけたクラスの規則だけを `assets/ui.generated.css` に書き出します。
このファイルはコミットされていてバイナリーに組み込まれるので、Senu のビルドと実行には
Cargo だけで足ります。Node が要るのは見た目を変えるときだけです。Node 24 と付属の npm で:

```sh
npm ci            # 一度だけ: package-lock.json のとおりの版を入れる
npm run css       # クラスやテーマを変えたあと
cargo run -- site # または serve。これまでどおり
```

ソースと `assets/ui.generated.css` は一緒にコミットします。CI は `npm ci`、`npm run css`、
`git diff --exit-code -- assets/ui.generated.css` を実行し、テンプレートのクラスに規則がなければ
`cargo test` が失敗します。Typography は自分の色を持つので、`@theme` では記事の中のリンクの色は
変わりません。`templates/site_base.html` の `<main>` に `prose-a:` のクラスを付ければ変わります。
両パッケージのライセンスは [THIRD-PARTY-NOTICES.txt](THIRD-PARTY-NOTICES.txt) にあり、
スタイルシートにもそれぞれの表示が残っています。

## 開発

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo clippy --locked --no-default-features --lib -- -D warnings
cargo test --locked -- --test-threads=2
RUSTDOCFLAGS="-D warnings" cargo doc --locked --no-deps --all-features
scripts/quickstart.sh
python3 scripts/check_tutorial.py
python3 scripts/snapshot_site.py target/site
```

`tests/contract.rs` は、デモと、テスト専用の二つ目のアプリケーションが並べたすべてのルートと
ツールに契約のキットを実行します。`tests/library.rs` はライブラリの API を確かめ、
`tests/example_inventory.rs` は `examples/inventory.rs` にキットを実行します。
テストにはほかに、quickstart の MCP チェッカーを偽のサーバーに対して動かす
`tests/quickstart_check.rs` と、チュートリアル確認スクリプトとサイトのスナップショットの
それぞれのテストを実行する `tests/tutorial_checker.rs` が含まれ、どちらも `python3` が必要です。
`check_tutorial.py` は講座を英語、日本語の順に、それぞれ最後のコミットの一時的なコピーで
実行し、作業ツリーを変えません。追跡しているファイルが HEAD と違う間は実行を拒むので、
先に変更をコミットしてください。講座の最後の発展の節は `npm ci` を実行するので、
`check_tutorial.py` には Node 24、npm、npm レジストリーへの接続も必要です。
`snapshot_site.py` には新しいか空のディレクトリーを渡します。
`senu site` を起動し、すべてのページを相対リンクの静的ファイルとして、スタイルシートと
ライセンス表示とともに保存して確かめます。
CI は GitHub がホストする Ubuntu で同じコマンドを実行します。`pages` ワークフローは
公開用のスナップショットを作り、`main` ではそれをデプロイします。詳細は
[仕様](docs/specs/)、[開発ログ](docs/development.md)、
[調査メモ](docs/research.md) を参照してください (いずれも英語)。

## ライセンス

Senu は [Apache-2.0](LICENSE) で提供します。
