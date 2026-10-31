# Senu

[English](README.md) | [日本語](README.ja.md)

**ひとつの Rust 処理を、Web ページ・JSON API・MCP ツールとして提供します。**

Senu は実験的な Rust のシード (たたき台) です。純粋関数 `list_projects` ひとつを、
[Askama](https://askama.rs/) で描画する HTML ページ、[Axum](https://docs.rs/axum)
の JSON API、公式 SDK [`rmcp`](https://rust.sdk.modelcontextprotocol.io/) で作った
stdio 上の [MCP](https://modelcontextprotocol.io/) ツール (明示的に有効化したときだけ)
の三つの経路で共有します。各経路はそれぞれの認証情報を確かめ、サーバー側で
プリンシパル (呼び出し主体) を選んでから同じ関数を呼ぶため、三つとも同じプロジェクトを
同じ順序で返します。これはテストで確認しています。Senu は Markdown の記事による
小さなサイトも配信し、そこに初心者向けの講座があります。

## 目指すこと: 最小・一貫・仕様駆動

- **最小 (Minimal):** アプリケーションの開発者が持つコード、設定、重複をできるだけ少なくします。
  いまは、記事の追加は Markdown のファイル一つ、見た目の変更はテンプレートのクラスと
  `npm run css` です。
- **一貫 (Consistent):** 人と AI の道具が同じ決まりで書けるようにし、`rustfmt`、
  `clippy -D warnings`、スタイルシートについては CI のずれの検査で保ちます。
  これらが確かめるのは形で、業務の規則が正しいことまでは証明しません。
- **仕様駆動 (Spec-driven):** [docs/specs/](docs/specs/) の仕様が先、次にテスト、最後にコードです。
  [開発ログ](docs/development.md) は仕様 0004 の受け入れ基準とテストの対応を記録しています。

Rust を土台にしたのは、メモリーと型の安全性、そして速さのためですが、言語だけで
プログラムが安全で速くなるわけではありません。Tailwind CSS のような新しい道具は、
これらの目的に役立つときに採り入れます。実装済み: 三つの経路で共有する処理、記事のサイト、
Tailwind の UI (仕様 0004)。計画中で未実装: すべての経路に一度で宣言する処理
([仕様 0005](docs/specs/0005-app-developer-flow.md))。

## Rust がはじめての方は講座から

初心者向けの講座は、Senu が自分のコンピューターで配信する小さなサイトです。Web 上には
公開していません。講座の[インストールの章](content/ja/02-install.md)のとおりに
Rust と Git を入れてから、次を実行してください。

```text
cd ~
git clone https://github.com/efg-technologies/senu.git
cd ~/senu
cargo run --quiet -- site
```

最初のビルドには数分かかります。そのあと http://127.0.0.1:3000/ja/ (日本語) か
http://127.0.0.1:3000/ (英語) を開いてください。講座に認証情報は要りません。詳しくは
[初心者向け講座と `senu site`](#初心者向け講座と-senu-site) を見てください。この README の
残りは、三つの経路のデモの説明です。

## 状態: 実験的なローカルデモ

Senu は動かせるシードであって製品ではなく、監査済みのセキュリティソフトウェアでも
ありません。性能や安全性についての主張はしません。

- **固定のデモデータ。** カタログは二つのテナントに属する五つのプロジェクトで、
  バイナリに組み込まれメモリ上にだけあります。保存や永続化はせず、変更操作もありません。
- **`senu serve`** はループバックアドレスだけで待ち受けます。`/projects` は HTTP
  Basic 認証、`/api/projects` は Bearer トークンを使います。TLS はないため、認証情報は
  ループバック上を平文で流れ、同じマシンの他のユーザーから届く可能性があります。
  ここでの Basic 認証は本番用のログインではありません。セッション、ログイン画面、
  OAuth、ユーザーやロール、レート制限、リクエストログ、監査記録はありません。
- **`senu mcp --enable-mcp`** は stdio で MCP を話し、このフラグがない限り無効です。
  起動前に `SENU_MCP_TOKEN` の設定が必要です。MCP の認可仕様では stdio サーバーは
  環境変数から認証情報を受け取るので、これは Senu を起動したローカルのプロセスを
  信頼する仕組みです。リモートの相手を認証するものではありません。HTTP 上の MCP や
  OAuth はありません。
- **変更操作には追加の作業が要ります。** Basic 認証はブラウザが自動で送る認証情報なので、
  将来の書き込み操作には CSRF 対策と新しい仕様が必要です。
- ジェネレーターやスキャフォールドはなく、公開済みの crate もありません。
  このリポジトリを clone して実行してください。

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

## 初心者向け講座と `senu site`

`senu site` は、Rust を知らない人向けの講座を配信します。講座は `content/en` と
`content/ja` の Markdown の記事で、Senu 自身がループバックアドレスで配信します。

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
仕様 0002 の mdBook のチュートリアルを置き換えます。サイトはどこにも公開していません。
CI は静的なスナップショットを成果物として保存します。

## コマンド

| コマンド | 動作 |
|---|---|
| `senu serve` | `SENU_BIND` (既定は `127.0.0.1:3000`、ループバックの IP リテラルのみ、ポート 0 可) で HTML と JSON を提供 |
| `senu site` | Markdown の記事を `SENU_BIND` で HTML として提供 (認証情報なし) |
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
- `GET /_senu/ui.css` (認証なし、`senu site` にもあります): スタイルシート。
  `text/css` でメモリーから返します。`/_senu/` は Senu の予約で、その下のほかのパスは 404 です。
- MCP ツール `list_projects`: `readOnlyHint: true`、任意の文字列 `q` のみを受け付けます。
  結果は `structuredContent` と JSON テキストの両方で返り、処理のエラーは
  `{"error": code}` を持つ `isError` の結果になります。未知のツールは JSON-RPC
  エラー `-32602`、rmcp が認識しないメソッドは `-32601` になり、どちらも
  要求された名前を繰り返さない固定のメッセージを返します。

`q` は前後の空白を除いてから、プロジェクト名と大文字小文字を区別せずに照合します。
空白を除いた後の長さは Unicode スカラー値で 80 個までで、制御文字は使えません。未知のフィールドは拒否されます。
それ以外のパスは 404、それ以外のメソッドは 405 を返します。リクエストボディ (413) と
2048 バイトを超えるリクエストターゲット (414) は拒否し、クエリ文字列に入れた認証情報は読みません。
応答には `Cache-Control: no-store` と `X-Content-Type-Options: nosniff` が付き、
エラーが入力や秘密の値を繰り返すことはありません。

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
テストが失敗します。唯一の例外は記事の本文で、Markdown のポリシーが作る `TrustedHtml` です。
同じテストが、`src/` の中の `HtmlSafe` の実装がその型に対する一つだけであることを確かめます。

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
cargo test --locked -- --test-threads=2
scripts/quickstart.sh
python3 scripts/check_tutorial.py
python3 scripts/snapshot_site.py target/site
```

テストには、quickstart の MCP チェッカーを偽のサーバーに対して動かす
`tests/quickstart_check.rs` と、チュートリアル確認スクリプトとサイトのスナップショットの
それぞれのテストを実行する `tests/tutorial_checker.rs` が含まれ、どちらも `python3` が必要です。
`check_tutorial.py` は講座を英語、日本語の順に、それぞれ最後のコミットの一時的なコピーで
実行し、作業ツリーを変えません。追跡しているファイルが HEAD と違う間は実行を拒むので、
先に変更をコミットしてください。講座の最後の発展の節は `npm ci` を実行するので、
`check_tutorial.py` には Node 24、npm、npm レジストリーへの接続も必要です。
`snapshot_site.py` には新しいか空のディレクトリーを渡します。
`senu site` を起動し、すべてのページを相対リンクの静的ファイルとして、スタイルシートと
ライセンス表示とともに保存して確かめます。
CI は GitHub がホストする Ubuntu で同じコマンドを実行し、スナップショットを成果物として
保存します。どこにもデプロイはしません。詳細は
[仕様](docs/specs/)、[開発ログ](docs/development.md)、
[調査メモ](docs/research.md) を参照してください (いずれも英語)。

## ライセンス

Senu は [Apache-2.0](LICENSE) で提供します。
