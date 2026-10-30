<nav class="senu-lang" aria-label="言語"><a href="../web.html" hreflang="en" lang="en">English</a></nav>

# HTML と JSON API

この章の二つのルートは、前の章で起動した `senu serve` のプロセスが提供するので、
動かしたままにしておいてください。二つのルートは
[`src/http.rs`](https://github.com/efg-technologies/senu/blob/main/src/http.rs)
の一つの Axum ルーターを共有しています。どの要求もサイズの上限、ルーティング、認証の順に
確かめられ、そのあとで初めて `list_projects` に届きます。

## HTML ページはマークアップをエスケープする

ページは Askama テンプレート
[`templates/projects.html`](https://github.com/efg-technologies/senu/blob/main/templates/projects.html)
から作られます。[`askama.toml`](https://github.com/efg-technologies/senu/blob/main/askama.toml)
で ERB に似た構文を設定しており、ブロックは `<%! %>`、式は `<%= %>`、コメントは
`<%# %>` です。各プロジェクトは一つのリスト項目になります。

```html
<li data-project-id="<%= p.id %>"><%= p.name %></li>
```

Askama は `.html` テンプレートの `<%= %>` の式をすべて HTML エスケープします。Basic 認証の
情報を付けてページを取得し、リスト項目だけを取り出してみます。

```sh
{{#include ../../commands/web.sh:html}}
```

```text
{{#include ../../commands/web.out:html}}
```

プロジェクト名には `<script>` 要素が含まれていますが、ページには `&#60;` (`<`) や
`&#34;` (`"`) のような文字参照しか入っていないので、ブラウザーは名前を文字として表示します。
検索語が検索欄の `value` 属性としてページに戻るときも、同じようにエスケープされます。

```sh
{{#include ../../commands/web.sh:echoed-search}}
```

```text
{{#include ../../commands/web.out:echoed-search}}
```

[`tests/adapter_http.rs`](https://github.com/efg-technologies/senu/blob/main/tests/adapter_http.rs)
の回帰テストは、テンプレートが `safe` や `escape` フィルターを使ったり、`askama.toml` や
`#[template]` 属性がエスケープの方式や `.html` という拡張子を変えたりすると失敗します。
ソースの文字列からこうした書き方を探すテストなので、よくある誤りは見つけられますが、
エスケープを無効にする手段が一切ないことを証明するものではありません。HTML の応答には、スクリプトを一切許さない制限の強い
`Content-Security-Policy` も付きます。

## JSON API はデータを返す

同じ検索を API で行うと、名前は JSON の文字列として返ります。JSON はマークアップではなく
データなので、ここでは HTML エスケープをしません。名前を画面に表示する側が、HTML ページと
同じようにエスケープする責任を持ちます。

```sh
{{#include ../../commands/web.sh:json}}
```

```text
{{#include ../../commands/web.out:json}}
```

## 認証情報は経路ごとに分かれている

HTML ページは HTTP Basic 認証だけを、API は専用の Bearer トークンだけを受け付けます。
API のトークンを HTML ページに送ると 401 で失敗し、`WWW-Authenticate` ヘッダーが
ページの求める方式を示します。

```sh
{{#include ../../commands/web.sh:wrong-credential}}
```

```text
{{#include ../../commands/web.out:wrong-credential}}
```

クエリ文字列に入れた認証情報は読まれず、どの応答にも `Cache-Control: no-store` と
`X-Content-Type-Options: nosniff` が付きます。ここでサーバーを Ctrl-C で止めてかまいません。

[`tests/adapter_parity.rs`](https://github.com/efg-technologies/senu/blob/main/tests/adapter_parity.rs)
は、いくつかの検索について、このページとこの API を `list_projects` の直接呼び出しと
比べます。HTML の項目は先に文字参照を元に戻してから比べるので、エスケープが変えるのは
名前の書き方だけで、名前そのものではないことも確かめています。
