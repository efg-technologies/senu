# 契約でテストする

Senu は、自分のセキュリティテストをキットとして同梱しています。関数を 1 回呼ぶと、あなたのルートとツールに実際のリクエストを送り、Senu が約束する規則があなたのアプリケーションで成り立たなければ失敗します。

## 契約は何を確かめますか

| 検査 | 成り立つべきこと |
|---|---|
| CT-1 | 資格情報が無い・違う・重複しているときは、チャレンジつきの 401 です。権限の検査より前で、入力は読みません。 |
| CT-2 | ほかのメソッドは 405、ボディは 413、長すぎるアドレスは 414 です。どれも資格情報より前です。 |
| CT-3 | 権限のないプリンシパルには、入力が何であっても 403 です。 |
| CT-4 | 未知・重複・文字列以外のパラメータは `invalid_request` です。 |
| CT-5 | 同じ引数は、JSON でもページでもツールの結果でも同じ結末になります。各サンプルは成功します。 |
| CT-6 | 一覧に載せたツールだけが存在します。どれも読み取り専用で、閉じたスキーマがルートのパラメータと一致します。 |
| CT-7 | ページとエラーには `Cache-Control: no-store` と `X-Content-Type-Options: nosniff` が付き、ページには CSP も付きます。 |

## 自分のアプリケーションではどう走らせますか

この機能は、テストのときだけ有効にします。

```toml
[dev-dependencies]
senu = { version = "0.2", default-features = false, features = ["contract"] }
tokio = { version = "1", features = ["macros", "rt"] }
```

次に、状態、ルートの表、ツールの一覧、すべての権限を持つプリンシパル、操作ごとに成功するクエリを 1 つずつ、キットに渡します。

```rust excerpt=tests/example_inventory.rs
#[tokio::test]
async fn the_example_meets_senus_contract() {
    contract::check(Subject {
        state: Inventory::sample,
        endpoints: inventory::endpoints,
        tools: inventory::tools,
        principal: NORTH_CLERK,
        samples: &[("get_stock", "sku=BOLT-8"), ("list_low_stock", "below=10")],
    })
    .await;
}
```

このリポジトリでは、同じテストが `examples/inventory.rs` に対して走ります。

```sh check=contract
cargo test --quiet --test example_inventory
```

```text check=contract

running 6 tests
......
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

```

違反があると、検査の名前とルートを入れたメッセージでパニックします。たとえば `CT-3 /api/stock?zz_unknown=1` です。

## 契約が確かめないことは何ですか

- **あなたの認証器。** キットは自前の Basic と Bearer の資格情報を使います。確かめる対象が、アダプターとあなたの宣言だからです。認証器は直接テストしてください。正しい資格情報、違う資格情報、無い場合、重複した場合を確かめます。
- **業務のルール。** `get_stock` が正しい数を返すかどうかは、関数のそばであなたがテストします。
- **テナントの分離。** あるテナントの資格情報で別のテナントのデータを求め、`not_found` が返ることを確かめるテストを書いてください。
- **`custom` ルートとアセット。** これらはあなたのハンドラーです。

## 操作を直接テストするには

操作はただの関数なので、たいていのテストに HTTP は要りません。状態、プリンシパル、引数を渡して関数を呼びます。Senu の権限検査とパラメータの枠も含めたいときは、宣言のほうを呼びます。

```rust
let args = senu::http::query_arguments(Some("sku=BOLT-8"));
let level = GET_STOCK.call(&Inventory::sample(), &NORTH_CLERK, &args).await?;
assert_eq!(level.units, 240);
```

## テストが失敗できることは、どう確かめますか

一度、わざと壊してみます。Senu 自身の開発では、契約の各検査が、対応する変更で実際に失敗することを確かめました。認証器が無いときに拒否する検査を外す、どんな Bearer トークンでも受け付ける、権限より先に検証する、ページの CSP を落とす、の 4 つです。一度も失敗したことのないテストは、まだ何も証明していません。

次のページ: [質問と比較](/ja/faq)。
