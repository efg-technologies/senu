# Senu とは何か、いつ使うか

Senu は、一度宣言した関数を HTML ページ・JSON API・MCP ツールとして提供する Rust ライブラリです。3 つとも同じ検査を同じ順序で通ります。

## Senu は何を解決しますか

人とプログラムと AI アシスタントが同じサービスを使うとき、入口は 3 つ要ります。人にはページ、プログラムには JSON、アシスタントには MCP ツールです。これを手で書くと、認証・認可・検証・関数の呼び出し・エラーの変換を入口ごとに繰り返すことになります。写しは少しずつずれ、検査を 1 つ忘れた入口が抜け道になります。

Senu では関数を一度だけ書き、その関数に必要なものを宣言します。あとは Senu のアダプターが、すべてのルートとツールで同じ手順を決まった順序で実行します。

1. サイズの上限とメソッド: 長すぎるアドレスは 414、ボディがあれば 413、`GET` 以外は 405 です。
2. 認証: あなたの認証器がリクエストのヘッダーからサーバー側のプリンシパルを決めます。決まらなければ 401 です。
3. 権限: 操作の権限を持たないプリンシパルには、何を送ってきても 403 を返します。
4. パラメータの枠: 未知・重複・文字列以外のパラメータは `invalid_request` です。
5. あなたの関数。
6. 結果を JSON・ページ・ツールの結果として返します。失敗したときは固定のエラーを返し、入力も秘密も繰り返しません。

操作を宣言しただけでは何も公開されません。ルートやツールは、あなたが一覧に書いた所にだけ存在します。認証器のないルートを含む表は、起動時に Senu が拒否します。

## 完全なアプリケーションはどんな形ですか

リポジトリの `examples/hello.rs` の全体です。操作が 1 つあり、Bearer トークンで守られ、MCP ツールとしても一覧に載っています。

```rust excerpt=examples/hello.rs
use senu::auth::{Bearer, Secret};
use senu::http::{AppState, Endpoints};
use senu::mcp::{self, McpServer};
use senu::operation::{
    Args, OpError, Operation, Param, Permission, Principal, Run, TenantId, decode,
};
use serde::{Deserialize, Serialize};

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

起動して呼び出します。

```sh manual
HELLO_TOKEN=choose-a-long-random-value cargo run --example hello
```

```sh manual
curl -H 'Authorization: Bearer choose-a-long-random-value' 'http://127.0.0.1:3000/api/greeting?name=Ada'
```

```text
{"message":"Hello, Ada!"}
```

トークンがなければ `401` と `{"error":"unauthorized"}` が返ります。名前が空なら `400` と `{"error":"invalid_name"}`、パラメータ名を間違えると `400` と `{"error":"invalid_request"}` です。

## プロジェクトに追加するには

```sh manual
cargo add senu --no-default-features
cargo add axum tokio serde --features tokio/macros,tokio/rt,tokio/net,serde/derive
```

`--no-default-features` は `demo` 機能を外します。`demo` にはサンプルのカタログ、`senu` バイナリ、このサイトが入っています。ページを描画するなら `askama` を足します。[契約テスト](/ja/testing)を使うなら、`[dev-dependencies]` で Senu の `contract` 機能を有効にします。

## いつ使い、いつ使わないか

同じ読み取り専用のデータを人・プログラム・アシスタントに届け、認可を 1 か所にまとめたいときに使います。社内ダッシュボード、カタログ、状況や集計を返すサービス、アシスタントに調べ物だけをさせるツールが向いています。

Senu 0.2 を Web フレームワークの全体として使うのは向きません。書き込み操作、フォーム、セッション、データベース層がありません。Senu のルーターはふつうの `axum::Router` なので、残りを受け持つ Axum アプリケーションに合流させるのが普通の使い方です。

## バージョン 0.2 の制限は何ですか

- 操作は読み取り専用で、`GET` で提供します。
- パラメータは文字列です。数値や日付は操作が自分で解釈します。
- パスは完全一致です。パスパラメータはありません。
- MCP は stdio で動きます。MCP over HTTP は含みません。
- 組み込みの認証器は固定の秘密を持ちます。セッション、OAuth、鍵の更新は、1 つのトレイトの裏であなたが書きます。
- Senu は監査済みのセキュリティソフトウェアではありません。0.x の間は API が変わることがあります。

## 次に読むページ

- [操作を宣言する](/ja/operations): パラメータ、エラー、非同期処理、ページ。
- [呼び出し元を認証する](/ja/authentication): 組み込みの認証器と自作の認証器。
- [操作を MCP ツールとして公開する](/ja/mcp-tools)。
- [デプロイと運用](/ja/deployment): Senu がすることと、あなたに残ること。
- [契約でテストする](/ja/testing)。
- [質問と比較](/ja/faq)。
- Rust が初めてなら、[講座を始めてください](/ja/start)。
