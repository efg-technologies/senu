# 操作を MCP ツールとして公開する

操作は、`mcp::tool` で一覧に載せると MCP ツールになります。AI アシスタントは、JSON API やページと同じ関数を、同じ権限検査の後ろで呼び出します。

## MCP とは何で、なぜ同じ関数から提供するのですか

MCP (Model Context Protocol) は、Claude のような AI アシスタントが他のプログラムのツールを呼ぶための取り決めです。ツールは名前、説明、引数の JSON Schema を持ちます。ツールを API とは別の実装として書くと、2 つはずれていきます。片方だけ検証が違ったり、テナントの検査を忘れたりします。Senu ではツールと API ルートが同じ `Operation` なので、食い違うことがありません。[契約テスト](/ja/testing)が 2 つの答えを比べます。

## ツールはどう一覧にしますか

```rust excerpt=examples/inventory.rs
pub fn tools() -> Vec<McpTool<Inventory>> {
    vec![mcp::tool(&GET_STOCK), mcp::tool(&LIST_LOW_STOCK)]
}
```

各ツールの名前と説明は操作から取ります。入力スキーマは宣言したパラメータから作ります。プロパティにはそれぞれ型と説明が付き、`required` には必須のものが並び、`additionalProperties` は `false` です。読み取りには `readOnlyHint: true` が付きます。[書き込み](/ja/writes)には `readOnlyHint: false` と、宣言どおりの `destructiveHint`・`idempotentHint` が付きます。自動で公開されるものはなく、マクロもリフレクションも使いません。

## サーバーはどう動かしますか

```rust excerpt=examples/inventory.rs
async fn serve_mcp() -> Result<(), String> {
    // Stdio trusts the local process that starts the server, so every call
    // is bound to one principal chosen here.
    McpServer::with_tools(Inventory::sample(), NORTH_CLERK, tools())
        .map_err(|err| err.to_string())?
        .with_info("inventory", "1.0.0")
        .serve_stdio()
        .await
        .map_err(|err| err.to_string())
}
```

`serve_stdio` は、クライアントが stdin を閉じるまで、プロセスの stdin と stdout で MCP を話します。その間、stdout には JSON-RPC だけが流れるので、ログは stderr に書きます。`with_info` は、`initialize` が報告する名前とバージョンを決めます。

すべての呼び出しは、あなたが渡した 1 つのプリンシパルとして走ります。MCP の仕様では、stdio サーバーは資格情報を環境から受け取ります。サーバーは自分を起動したローカルのプロセスを信頼し、遠隔の相手を認証しません。このプリンシパルは慎重に選び、アシスタントに持たせてよい権限だけを与えてください。書き込みの権限を与えれば、アシスタントは書き込めます。

## アシスタントはどうつなぎますか

多くの MCP クライアントは、起動するコマンドを書いた JSON ファイルを読みます。上の例なら次のとおりです。

```json
{
  "mcpServers": {
    "inventory": {
      "command": "/absolute/path/to/inventory",
      "args": ["mcp"]
    }
  }
}
```

クライアントはプログラムを起動し、`initialize` を送り、ツールを一覧し、呼び出します。呼び出しとその答えは、通信上では次のようになります。

```json
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"list_low_stock","arguments":{"below":"4"}}}
```

```json
{"jsonrpc":"2.0","id":2,"result":{"content":[{"type":"text","text":"{\"below\":4,\"items\":[{\"name\":\"Nut M8\",\"sku\":\"NUT-8\",\"units\":3},{\"name\":\"Washer M8\",\"sku\":\"WASHER-8\",\"units\":0}]}"}],"structuredContent":{"below":4,"items":[{"name":"Nut M8","sku":"NUT-8","units":3},{"name":"Washer M8","sku":"WASHER-8","units":0}]},"isError":false}}
```

結果は 2 通りで届きます。JSON を読むクライアントには `structuredContent`、読まないクライアントには同じ JSON を入れたテキストです。

## エラーはどう見えますか

操作のエラーは、`isError: true` と `{"error": "<code>"}` を持つ通常のツール結果です。コードは JSON API と同じで、`forbidden`、`invalid_request`、`not_found`、`internal`、または操作独自のものです。書き込みの 409 のコードもここに入ります。アシスタントはこれを読んで、呼び出しを直せます。未知のツール名は JSON-RPC のエラー `-32602` になります。メッセージは固定で、名前を繰り返しません。

## MCP over HTTP は使えますか

バージョン 0.3 では使えません。ツールは stdio で動きます。ローカルのプログラムを起動するアシスタントに向いた形です。遠隔のアシスタントには JSON API を使わせるか、あとのバージョンの HTTP トランスポートを待ってください。遠隔の呼び出し元はリクエストごとに認証する必要があるので、専用の仕様が要ります。

## アシスタントにとって良いツールとは

- `description` は、質問を返せない読み手に向けて書きます。何について何を返すのか、制限は何かを書きます。
- 各パラメータの形式を `description` に書きます。たとえば «A-Z、0-9、'-' の 1〜16 文字» です。
- 結果は小さく平らにし、フィールド名を変えません。
- アシスタントが直せる間違いには、`invalid_sku` のような独自のエラーコードを使います。

次のページ: [デプロイと運用](/ja/deployment)。
