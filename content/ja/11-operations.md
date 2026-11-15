# 操作を宣言する

操作は、名前・説明・権限・文字列パラメータを持つ読み取り専用の関数です。定数として一度だけ宣言し、到達させたい場所の一覧に載せます。

## 操作は何でできていますか

このページの例は、小さな在庫サービス `examples/inventory.rs` から取っています。最初の操作は品目を 1 つ調べます。

```rust excerpt=examples/inventory.rs
pub const GET_STOCK: Operation<Inventory, Level> = Operation {
    name: "get_stock",
    description: "Units in stock for one SKU of the caller's warehouse.",
    permission: VIEW_STOCK,
    params: &[Param::required(
        "sku",
        "The SKU: 1 to 16 characters of A-Z, 0-9 and '-'.",
    )],
    run: Run::Async(|inventory, caller, args| Box::pin(get_stock(inventory, caller, args))),
};
```

- `name` は 64 バイト以内の snake_case です。MCP ツールの名前になります。
- `description` は人とアシスタントに向けた 1 文です。ツールの説明になるので、何を誰に返すかを書きます。
- `permission` はアプリケーションの定数で、`Permission::new("view_stock")` のように宣言します。ほかの何よりも先に検査されます。
- `params` は、呼び出し元が送ってよいキーのすべてです。どれも説明つきの文字列です。`Param::required` は必須で、`Param::optional` は省略できます。
- `run` が関数です。メモリ上の純粋な処理は `Run::Sync`、待つ処理は `Run::Async` にします。

Senu はこのメタデータを、ルーターやツールの一覧を組み立てるときに検査します。不正な名前、空の説明、二重に宣言したパラメータは起動時のエラーになり、本番で初めて気づくことがありません。

## 関数はどう書きますか

関数は状態・呼び出し元・引数を受け取り、`Serialize` を実装した結果か `OpError` を返します。

```rust excerpt=examples/inventory.rs
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StockArgs {
    sku: String,
}

async fn get_stock(
    inventory: &Inventory,
    caller: &Principal,
    args: &Args,
) -> Result<Level, OpError> {
    let StockArgs { sku } = decode(args)?;
    let well_formed = (1..=16).contains(&sku.len())
        && sku
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'-');
    if !well_formed {
        return Err(INVALID_SKU);
    }
    let levels = inventory.levels(caller).await?;
    levels
        .into_iter()
        .find(|level| level.sku == sku)
        .ok_or(OpError::NotFound)
}
```

安全に保つ習慣が 3 つあります。

1. `decode` で、`#[serde(deny_unknown_fields)]` を付けた構造体に読み込みます。欠けた項目や型の違う項目は `invalid_request` になります。
2. テナントは引数からではなく、呼び出し元から読みます。`caller.tenant()` は、サーバーがリクエストを認証したときに決めた値です。
3. 存在しないものにも、別のテナントのものにも、同じ `OpError::NotFound` を返します。他のテナントのデータを探る手がかりを与えません。

## 操作はどんなエラーを返せますか

| エラー | HTTP | JSON と MCP のコード | 意味 |
|---|---|---|---|
| `OpError::Forbidden` | 403 | `forbidden` | 呼び出し元に権限がありません。関数が走る前に Senu が返します。 |
| `OpError::InvalidRequest` | 400 | `invalid_request` | 引数が宣言したパラメータに合いません。 |
| `OpError::Invalid { code, phrase }` | 400 | あなたの `code` | `invalid_sku` のような、操作独自の検証エラーです。 |
| `OpError::NotFound` | 404 | `not_found` | この呼び出し元に見えるものの中に、該当がありません。 |
| `OpError::Internal` | 500 | `internal` | バックエンドが失敗しました。原因は自分でログに残します。呼び出し元には詳細を見せません。 |

コードも文言も固定の文字列なので、エラーが入力や秘密を繰り返すことはありません。`/api/` の下では本文が `{"error": "<code>"}` になります。HTML のルートは文言をプレーンテキストで返します。MCP ツールは `isError` つきの結果に同じ JSON を入れて返します。

## 非同期の操作はどう動きますか

`Run::Async` に、Box に入れた future を返す関数を渡します。つなぎのコードは、上の宣言にあるクロージャ `|inventory, caller, args| Box::pin(get_stock(inventory, caller, args))` だけです。関数の中ではデータベースのクライアントを await するか、ブロックする処理をエグゼキューターの外へ移します。

```rust excerpt=examples/inventory.rs
    async fn levels(&self, caller: &Principal) -> Result<Vec<Level>, OpError> {
        let (items, tenant) = (Arc::clone(&self.0), caller.tenant().clone());
        let read = move || {
            items
                .iter()
                .filter(|item| item.tenant == tenant)
                .map(|item| item.level.clone())
                .collect()
        };
        tokio::task::spawn_blocking(read)
            .await
            .map_err(|_| OpError::Internal)
    }
```

`Run::Sync` の関数は非同期エグゼキューターの上でそのまま走ります。メモリ上のデータを扱う処理に限ってください。ブロックしうるなら非同期にします。

## 操作はどう公開しますか

`Endpoints` の表とツールの一覧に載せます。載せたものだけが到達可能になります。

```rust excerpt=examples/inventory.rs
pub fn endpoints() -> Endpoints<Inventory> {
    Endpoints::new()
        .api("/api/stock", &GET_STOCK)
        .api("/api/low-stock", &LIST_LOW_STOCK)
        .html("/low-stock", &LIST_LOW_STOCK, low_stock_page)
        .asset(
            "/assets/inventory.css",
            "text/css; charset=utf-8",
            STYLESHEET,
        )
}

pub fn tools() -> Vec<McpTool<Inventory>> {
    vec![mcp::tool(&GET_STOCK), mcp::tool(&LIST_LOW_STOCK)]
}
```

- `.api(path, &OPERATION)` は JSON を返します。パスは `/api/` の下でなければなりません。
- `.html(path, &OPERATION, view)` はページを返します。`view` は、受理された引数と結果から HTML を作る関数です。Askama を使えば、すべての値が既定でエスケープされます。
- `.asset(path, content_type, bytes)` は、スタイルシートのような公開の静的ファイルを返します。
- `.custom(path, handler)` は、それ以外のための抜け道です。サイズの上限とレスポンスヘッダーは付きますが、認証は付きません。認証はそのハンドラーの責任です。
- `.html_csp(policy)` は、ページの Content-Security-Policy を置き換えます。既定では同一オリジンのスタイルシートとフォームだけを許し、スクリプト・画像・フレームは許しません。

`into_router(state)` は表の全体を検査し、`axum::Router` か、パスを名指しするエラーを返します。パスは完全一致で、重複せず、`/_senu/` の外になければなりません。`/_senu/` は Senu の組み込みスタイルシートのために予約されています。

## 検査はどの順序で走りますか

いつも同じです。上限、メソッド、認証、権限、パラメータの枠、そしてあなたの関数の順です。権限が検証より先にあることには意味があります。権限のない呼び出し元は、どの入力が有効だったかを何も知ることができません。[契約テスト](/ja/testing)は、この順序をあなたのルートで確かめます。

次のページ: [呼び出し元を認証する](/ja/authentication)。
