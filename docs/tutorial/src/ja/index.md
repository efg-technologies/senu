<nav class="senu-lang" aria-label="言語"><a href="../index.html" hreflang="en" lang="en">English</a></nav>

# 完成したアプリ

このチュートリアルでは、実験的な小さな Rust プログラム
[Senu](https://github.com/efg-technologies/senu) を、clone するところから、
先に失敗するテストを書いて変更するところまで順にたどります。実際に動かし、
共有されている一つの処理を読み、同じ結果を HTML ページ・JSON API・MCP ツールの
三つで確かめたあと、プロジェクト名を一つ変えてテストを赤から緑へ進めます。
英語版が正本で、この日本語版は同じ章立てと同じコマンドを使います。

## ここで扱うもの

Senu には純粋関数 `list_projects` が一つだけあります。この関数は、二つのテナント
(`acme` と `globex`) に属する五つのプロジェクトからなる固定のデモカタログを一覧します。
カタログはバイナリに組み込まれています。この関数を三つのアダプター (各経路の入口) が
呼び出します。各アダプターはそれぞれの認証情報を確かめ、サーバー側でプリンシパル
(呼び出し主体) を選び、同じ関数を呼んで、結果をそれぞれの形式で返します。

<figure class="senu-flow" aria-labelledby="senu-flow-caption">
<div class="senu-flow-core">
<p class="senu-flow-title"><code>src/operation.rs</code> の <code>list_projects</code></p>
<p>権限を確かめ、任意の検索語 <code>q</code> を解釈し、固定カタログのうちプリンシパルのテナントだけを検索して、id 順に並べます。</p>
</div>
<ul class="senu-flow-adapters">
<li><p class="senu-flow-title">HTML ページ</p><p><code>GET /projects</code>、HTTP Basic 認証、エスケープする Askama テンプレート</p></li>
<li><p class="senu-flow-title">JSON API</p><p><code>GET /api/projects</code>、Bearer トークン、<code>{"projects": [...]}</code></p></li>
<li class="senu-flow-optin"><p class="senu-flow-title">MCP ツール (明示的に有効化)</p><p>stdio 上の <code>senu mcp --enable-mcp</code>、トークンは環境変数から、有効化しない限り動きません</p></li>
</ul>
<figcaption id="senu-flow-caption">一つの処理を三つのアダプターが使います。<code>senu serve</code> は HTML ページと JSON API をループバックアドレスで提供します。MCP は別プロセスの <code>senu mcp --enable-mcp</code> としてだけ動き、標準入出力でやり取りします。</figcaption>
</figure>

[`tests/adapter_parity.rs`](https://github.com/efg-technologies/senu/blob/main/tests/adapter_parity.rs)
のテストが、三つのアダプターが `list_projects` の結果と完全に同じものを同じ順序で
返すことを確かめています。

## この先でやること

1. 必要なツールを用意し、リポジトリを clone して quickstart を実行します。
2. `list_projects` を読み、認可・入力検証・テナント分離を外から確かめます。
3. マークアップをエスケープする HTML ページと、データを返す JSON API を比べます。
4. 指定しない限り無効な MCP ツールを起動し、応答を確かめます。
5. プロジェクト名を一つ変えます。先にテストを変えて失敗を確かめ、カタログを変え、
   古い名前を固定しているほかのテストも追従させます。

このチュートリアルのコマンドと期待される出力は、すべて
[`docs/tutorial/commands`](https://github.com/efg-technologies/senu/tree/main/docs/tutorial/commands)
と [`docs/tutorial/exercise`](https://github.com/efg-technologies/senu/tree/main/docs/tutorial/exercise)
のファイルから取り込んでいます。英語版と日本語版は同じファイルを取り込み、
`scripts/check_tutorial.py` がそれらを実行して出力を照合するので、ここに書かれている
のは最後に確認されたとおりの内容です。

## Senu ではないもの

Senu は動かせるシード (たたき台) であって製品ではなく、監査済みのセキュリティ
ソフトウェアでもありません。カタログは固定の読み取り専用で、データベースも永続化も
データを変える手段もありません。ここでの HTTP Basic 認証はデモ用の認証情報であって
本番用のログインではなく、セッション、ユーザー、ロールもありません。サーバーには TLS
がなく、ループバックアドレスだけで待ち受けるので、認証情報は自分のマシンの中を
平文で流れます。このチュートリアルのサイトは静的ホスティングから HTTPS で配信されますが、
それで手元で動かすデモに TLS が加わるわけではありません。MCP はローカルの stdio
だけで動き、リモートの MCP や OAuth はありません。ジェネレーター、スキャフォールド、
ORM はなく、公開済みの crate もありません。

すべてのルートと設定は [README](https://github.com/efg-technologies/senu/blob/main/README.ja.md)
に一覧があります。コードは [仕様 0001](https://github.com/efg-technologies/senu/blob/main/docs/specs/0001-project-catalogue.md)
(英語) に沿って作られ、テストされています。
