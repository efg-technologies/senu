<nav class="senu-lang" aria-label="言語"><a href="../mcp.html" hreflang="en" lang="en">English</a></nav>

# 明示的に有効化する stdio の MCP

[Model Context Protocol](https://modelcontextprotocol.io/) (MCP) は、AI アシスタントなどの
クライアントがツールを見つけて呼び出すための仕組みです。Senu は公式 SDK の
[`rmcp`](https://rust.sdk.modelcontextprotocol.io/) を使い、
[`src/mcp.rs`](https://github.com/efg-technologies/senu/blob/main/src/mcp.rs)
でツール `list_projects` を一つだけ宣言しています。クライアントは `senu` を子プロセスとして
起動し、その標準入出力で JSON-RPC のメッセージを 1 行に一つずつやり取りします。
ネットワークのポートは使わず、`senu serve` を動かしておく必要もありません。

## 有効化しない限り動かない

フラグを付けずに `senu mcp` を実行すると、起動を拒否します。

```sh
{{#include ../../commands/mcp.sh:off}}
```

```text
{{#include ../../commands/mcp.out:off}}
```

`--enable-mcp` を付けた場合も、HTTP の認証情報とは別のトークン `SENU_MCP_TOKEN` が
必要です。MCP の認可の取り決めでは stdio サーバーは環境変数から認証情報を受け取るので、
これは Senu を起動したローカルのプロセスを信頼する仕組みです。リモートの相手を認証する
ものではなく、HTTP 上の MCP や OAuth はありません。

## ツールを呼び出して応答を確かめる

quickstart のチェッカーは、Python の標準ライブラリだけを使う小さなクライアントです。
これを単独で実行します。

```sh
{{#include ../../commands/mcp.sh:check}}
```

```text
{{#include ../../commands/mcp.out:check}}
```

[`scripts/check_mcp.py`](https://github.com/efg-technologies/senu/blob/main/scripts/check_mcp.py)
は `initialize` を送ってその応答を待ち、そのあとで初めて `notifications/initialized` と、
`{"q": "launch"}` を指定した `tools/call` を送ります。各応答は要求と同じ id を持ち、
エラーがなく、呼び出しの結果は `isError` ではなく、`acme-001` と `acme-003` だけを並べて
いなければなりません。最後に stdin を閉じ、Senu が終了コード 0 で終わることを確かめます。
どこかで失敗すると、チェッカーは Senu を止めて終了コード 1 で終わります。

応答には同じ `ProjectList` が二度入っています。プログラム向けの `structuredContent` と、
文字しか表示しないクライアント向けの `content` 内の JSON テキストです。並んでいる
プロジェクトとその順序は、HTML ページや API と同じです。

## ツールが宣言している内容

`tools/list` はツールを一つだけ返し、そのツールは `readOnlyHint: true` を持ち、
入力スキーマは任意の文字列 `q` だけを許します。無効な検索語や未知のフィールドは
`{"error": code}` を持つ `isError` の結果として返り、未知のツール名には JSON-RPC
エラー `-32602` が返ります。これらの場合は
[`tests/adapter_mcp.rs`](https://github.com/efg-technologies/senu/blob/main/tests/adapter_mcp.rs)
のアダプターテストと
[`tests/e2e_mcp.rs`](https://github.com/efg-technologies/senu/blob/main/tests/e2e_mcp.rs)
の stdio テストで確かめています。

stdio サーバーを起動できるほかの MCP クライアントでも、同じコマンドと同じ環境変数で
動かせるはずですが、このチュートリアルではどのクライアントも試していません。
