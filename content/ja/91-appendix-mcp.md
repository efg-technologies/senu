# 付録: MCP で Senu を使う (任意)

MCP (Model Context Protocol) は、AI の道具がほかのプログラムの関数を呼ぶための決まりです。Senu はプロジェクト一覧を、`list_projects` という一つの MCP の道具として提供できます。やり取りは **stdio** で行います。つまり AI の道具が Senu を自分で起動し、ネットワークのポートではなく Senu の入力と出力を通して話します。この動き方は、頼まない限り無効です。

動いている Senu があれば Ctrl+C で止めてください。追加の指定がないと、Senu は断ります。

```sh check=off exit=2
cargo run --quiet -- mcp
```

```text check=off
senu: MCP is off by default; start it with `senu mcp --enable-mcp`
```

MCP は既定で無効なので `senu mcp --enable-mcp` で起動するように、という意味です。スクリプト `scripts/check_mcp.py` は AI の道具の役をします。Senu を MCP モードで起動し、道具の一覧を尋ね、`list_projects` を呼んで答えを表示します。Python 3 が必要です。MCP モードは起動の前にトークンも必要とし、ここではデモ用の値を使います。

```sh check=mcp
SENU_MCP_TOKEN=demo-mcp-token python3 scripts/check_mcp.py ./target/debug/senu mcp --enable-mcp
```

```text check=mcp
{"jsonrpc":"2.0","id":2,"result":{"content":[{"type":"text","text":"{\"projects\":[{\"id\":\"acme-001\",\"name\":\"Artemis Launch\"},{\"id\":\"acme-003\",\"name\":\"<script>alert('x')</script> & \\\"Launch\\\" Notes\"}]}"}],"structuredContent":{"projects":[{"id":"acme-001","name":"Artemis Launch"},{"id":"acme-003","name":"<script>alert('x')</script> & \"Launch\" Notes"}]},"isError":false}}
```

`./target/debug/senu` は `cargo run` がビルドしたプログラムです。前の付録でプロジェクトの名前を変えていれば、答えには Artemis Launch と出ます。Web ページも JSON も MCP も、同じ関数を通して同じ一覧を読んでいるからです。

## 演習

同じコマンドを `SENU_MCP_TOKEN=demo-mcp-token` なしで実行してください。何が起きるでしょう。それはなぜ役に立つのでしょう。

**解答.** Senu は入力を読む前に止まり、`SENU_MCP_TOKEN` が設定されていないと伝えます。トークンを渡されていない道具が、うっかり Senu を起動することはありません。確認用のスクリプトは、Senu が終了したと報告します。
