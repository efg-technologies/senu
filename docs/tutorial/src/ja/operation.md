<nav class="senu-lang" aria-label="言語"><a href="../operation.html" hreflang="en" lang="en">English</a></nav>

# 共有される一つの処理

## `list_projects` を読む

[`src/operation.rs`](https://github.com/efg-technologies/senu/blob/main/src/operation.rs)
を開いてください。`list_projects` はカタログ、プリンシパル、解釈済みの要求引数を受け取り、
`ProjectList` か `OpError` を返します。入出力を行わず、時計やグローバルな状態も読まないので、
単体テストから直接呼び出せます。関数内の番号付きコメントは次の手順に対応しています。

1. **最初に認可する。** `ListProjects` 権限のないプリンシパルは、入力を読む前に
   `Forbidden` になります。そのため権限のない呼び出し元は、どの入力なら有効だったかを
   知ることができません。
2. **厳密に解釈する。** 受け付ける引数は任意の文字列 `q` だけです。`tenant` のような
   それ以外のフィールドは `InvalidRequest` になります。
3. **検索語を正規化し、上限を確かめる。** `q` の前後の空白を除きます。空なら検索なしとして
   扱い、制御文字を含むか、空白を除いた後に 80 文字を超える場合は `InvalidSearch` になります。
4. **テナントの中だけを見る。** 検索は `catalogue.for_tenant(principal.tenant())`
   だけを読みます。
5. **id 順に並べる。** これでどのアダプターも同じ順序で返します。

プリンシパルが要求から作られることはありません。アダプターが使う三つのプリンシパル
`demo_web`、`demo_api`、`demo_mcp` は同じファイルの定数で、どれもテナント `acme` に属し、
唯一の権限を持ちます。カタログ自体は
[`src/catalogue.rs`](https://github.com/efg-technologies/senu/blob/main/src/catalogue.rs)
の定数配列です。

## サーバーを起動する

clone したディレクトリで二つ目のターミナルを開き、デモ用の認証情報を使って
`senu serve` をポート 3000 で起動します。これはループバック専用のサーバーのための固定の
デモ値なので、ほかの用途では自分で値を決めてください。Senu は `127.0.0.1` のような
ループバックアドレス以外での待ち受けを拒否します。

```sh
{{#include ../../commands/serve.sh:serve}}
```

ビルドが終わると、サーバーは次の 1 行を表示して動き続けます。

```text
{{#include ../../commands/serve.out:serve}}
```

この章と次の章のあいだは動かしたままにし、終わったら Ctrl-C で止めます。

## 外から呼び出す

一つ目のターミナルに戻り、JSON API で `launch` を検索します。`-w` オプションで、
本文のあとに HTTP のステータスを表示します。

```sh
{{#include ../../commands/operation.sh:launch}}
```

```text
{{#include ../../commands/operation.out:launch}}
```

**テナント分離。** "Globex Launch" も `launch` に一致しますが、これはテナント `globex`
のプロジェクトです。その名前で直接検索しても、API のプリンシパルは `acme` に属するので
何も見つかりません。

```sh
{{#include ../../commands/operation.sh:other-tenant}}
```

```text
{{#include ../../commands/operation.out:other-tenant}}
```

要求の側からテナントを選ぶこともできません。`tenant` は `list_projects` の引数ではないので、
手順 2 で拒否されます。

```sh
{{#include ../../commands/operation.sh:pick-tenant}}
```

```text
{{#include ../../commands/operation.out:pick-tenant}}
```

**入力検証。** `%07` は制御文字の BEL で、手順 3 で拒否されます。エラーは固定のコードで、
入力した値を繰り返しません。

```sh
{{#include ../../commands/operation.sh:bad-search}}
```

```text
{{#include ../../commands/operation.out:bad-search}}
```

**認証。** トークンがなければ、アダプターは `list_projects` を呼び出しません。

```sh
{{#include ../../commands/operation.sh:no-token}}
```

```text
{{#include ../../commands/operation.out:no-token}}
```

デモ用のプリンシパルはどれも権限を持っているので、外から `Forbidden` を起こすことは
できません。`src/operation.rs` の末尾にある単体テストは権限のないプリンシパルを作り、
HTTP と MCP のアダプターテストはそれを差し込んで、各経路がこのエラーを正しく返すことを
確かめています。
