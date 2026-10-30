<nav class="senu-lang" aria-label="言語"><a href="../setup.html" hreflang="en" lang="en">English</a></nav>

# 準備と実行

## 必要なもの

`git`、`bash`、`curl`、`python3` と、Rust のツールチェーン管理ツール
[rustup](https://rustup.rs/) が必要です。リポジトリは
[`rust-toolchain.toml`](https://github.com/efg-technologies/senu/blob/main/rust-toolchain.toml)
で Rust 1.99.1 を固定しており、rustup は clone したディレクトリの中でその版を使います。
未導入だと `cargo` が報告した場合は `rustup toolchain install 1.99.1` を実行してください。
CI は `git clone` を除くこのチュートリアルのコマンドを GitHub がホストする Ubuntu で
実行しています。ほかの環境では試していません。

## clone して quickstart を実行する

リポジトリを clone して、そのディレクトリに移動します。以降のコマンドはすべて
このディレクトリで実行します。

```sh
{{#include ../../commands/setup.sh:clone}}
```

続けて quickstart を実行します。

```sh
{{#include ../../commands/setup.sh:quickstart}}
```

[`scripts/quickstart.sh`](https://github.com/efg-technologies/senu/blob/main/scripts/quickstart.sh)
は Senu をビルドし、デモ用の認証情報で `senu serve` を空いているループバックのポートで
起動して、JSON API と HTML ページを呼び出します。続けて `senu mcp --enable-mcp` を起動し、
そのツールを呼び出します。最初のビルドでは依存クレートのダウンロードとコンパイルに
時間がかかりますが、2 回目以降はすぐに終わります。次のように表示されます。`PORT` の
部分には実行のたびに選ばれたポート番号が入ります。

```text
{{#include ../../commands/setup.out:quickstart}}
```

すべての手順が成功したときだけ、スクリプトは終了コード 0 で終わります。
スクリプトの全文は次のとおりです。

```sh
{{#include ../../../../scripts/quickstart.sh}}
```

## 注目するところ

三つの呼び出しはどれも `launch` で検索し、どれも同じ二つのプロジェクト `acme-001` と
`acme-003` を同じ順序で返します。カタログには "Globex Launch" というプロジェクトも
ありますが、もう一方のテナントに属しているため、どのデモ用の認証情報からも見えません。

`acme-003` の名前は `<script>alert('x')</script> & "Launch" Notes` です。JSON API と
MCP はこれをそのまま文字列として返します。HTML ページは `<`、`>`、`&`、`'`、`"` を
`&#60;` のような文字参照に置き換えるので、ブラウザーはスクリプトを実行せずに文字として
表示します。第 4 章でこの点を詳しく見ます。

最後の手順は MCP の応答を表示するだけではありません。
[`scripts/check_mcp.py`](https://github.com/efg-technologies/senu/blob/main/scripts/check_mcp.py)
は各応答を待ち、正しい要求への応答でエラーがないことと、ツールが `acme-001` と
`acme-003` を返したことを確かめます。
