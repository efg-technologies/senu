# Senu

**ひとつの Rust 処理を、Web ページ・JSON API・MCP ツールとして提供します。**

Senu は実験的な Rust のシード (たたき台) です。純粋関数 `list_projects` ひとつを、
[Askama](https://askama.rs/) で描画する HTML ページ、[Axum](https://docs.rs/axum)
の JSON API、公式 SDK [`rmcp`](https://rust.sdk.modelcontextprotocol.io/) で作った
stdio 上の [MCP](https://modelcontextprotocol.io/) ツール (明示的に有効化したときだけ)
の三つの経路で共有します。各経路はそれぞれの認証情報を確かめ、サーバー側で
プリンシパル (呼び出し主体) を選んでから同じ関数を呼ぶため、三つとも同じプロジェクトを
同じ順序で返します。これはテストで確認しています。

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
- マクロ、ジェネレーター、スキャフォールドはなく、公開済みの crate もありません。
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

## コマンド

| コマンド | 動作 |
|---|---|
| `senu serve` | `SENU_BIND` (既定は `127.0.0.1:3000`、ループバックの IP リテラルのみ、ポート 0 可) で HTML と JSON を提供 |
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
テストが失敗します。

## 開発

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked -- --test-threads=2
scripts/quickstart.sh
```

テストには、quickstart の MCP チェッカーを偽のサーバーに対して動かす
`tests/quickstart_check.rs` が含まれ、`python3` が必要です。CI は GitHub が
ホストする Ubuntu で同じコマンドを実行します。詳細は
[仕様](docs/specs/0001-project-catalogue.md)、[開発ログ](docs/development.md)、
[調査メモ](docs/research.md) を参照してください (いずれも英語)。

## ライセンス

Apache-2.0 は暫定のライセンスです (`LICENSE` を参照)。仕様の決定事項 D1 にあるとおり、
マージ前にオーナーがライセンスを確定します。
