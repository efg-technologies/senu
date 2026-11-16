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
| CT-6 | 一覧に載せたツールだけが存在します。どれも閉じたスキーマがパラメータと一致し、読み取りには読み取り専用の印が付きます。 |
| CT-7 | ページとエラーには `Cache-Control: no-store` と `X-Content-Type-Options: nosniff` が付き、ページには CSP も付きます。 |
| CT-8 | 書き込みは `POST` だけに答えます。ほかのメソッドは 405 で、`Allow: POST` が付きます。 |
| CT-9 | ブラウザが別サイトから送った書き込みは、資格情報が正しくても 403 `cross_origin` です。 |
| CT-10 | 資格情報のない書き込みは 401、権限のない書き込みは 403 です。どちらも JSON でない本文で確かめるので、本文を読んでいないことが分かります。 |
| CT-11 | 書き込みでは、`Content-Type` が違えば 415、本文が上限を超えれば 413 です。オブジェクトでない本文、クエリ文字列、未知のキー、型の違う値は `invalid_request` です。 |
| CT-12 | 書き込みのツールには読み取り専用の印が付かず、印は宣言と一致します。サンプルは JSON でもツール呼び出しでも成功します。 |

## 自分のアプリケーションではどう走らせますか

この機能は、テストのときだけ有効にします。

```toml
[dev-dependencies]
senu = { version = "0.3", default-features = false, features = ["contract"] }
tokio = { version = "1", features = ["macros", "rt"] }
```

次に、状態、ルートの表、ツールの一覧、すべての権限を持つプリンシパル、操作ごとに成功するサンプルを 1 つずつ、キットに渡します。サンプルは、読み取りならクエリ文字列、書き込みなら JSON のオブジェクトです。`auth: None` のとき、キットは自前の Basic と Bearer の資格情報を使います。

```rust excerpt=tests/example_inventory.rs
#[tokio::test]
async fn the_example_meets_senus_contract() {
    contract::check(Subject {
        state: Inventory::sample,
        endpoints: inventory::endpoints,
        tools: inventory::tools,
        principal: NORTH_CLERK,
        samples: &[("get_stock", "sku=BOLT-8"), ("list_low_stock", "below=10")],
        auth: None,
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

## 自分の認証器を通して走らせるには

`auth` に、与えられたプリンシパルのための状態を作る関数と、認証に使うリクエストヘッダーを渡します。`examples/items.rs` は Cookie のセッションを使うので、そのテストは、キットが渡すプリンシパルのセッションを作り、Cookie を指定します。

```rust excerpt=tests/example_items.rs
#[tokio::test]
async fn the_example_meets_the_contract_through_its_own_authenticator() {
    contract::check(Subject {
        state: Store::sample,
        endpoints: items::endpoints,
        tools: items::tools,
        principal: EDITOR,
        samples: &[
            ("list_items", ""),
            ("create_item", r#"{"title":"CSV import","lo":0,"hi":0.05}"#),
            ("rename_item", r#"{"id":"P-1","title":"Renamed"}"#),
            ("delete_item", r#"{"id":"P-1"}"#),
        ],
        auth: Some(OwnAuth {
            app: |store, principal| {
                store.sign_in("contract-session-5T", principal.clone());
                items::state(store)
            },
            html: &[("cookie", "sid=contract-session-5T")],
            api: &[("cookie", "sid=contract-session-5T")],
        }),
    })
    .await;
}
```

キットはこの関数を 2 回呼びます。あなたのプリンシパルと、権限を 1 つも持たないプリンシパルです。書き込みのサンプルは、ルートの表の順に、1 つの状態の上で走ります。上の例の削除は、直前に名前を変えた項目を消します。

## 契約が確かめないことは何ですか

- **あなたの認証器に固有の規則。** `Subject::auth` を使えば、次の節のように、契約はあなたの認証器を通って走ります。ただしキットが知っているのは、渡されたヘッダーだけです。期限切れ、サインアウト、鍵の更新は自分でテストしてください。
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

一度、わざと壊してみます。Senu 自身の開発では、契約の各検査が、対応する変更で実際に失敗することを確かめました。認証器が無いときに拒否する検査を外す、どんな Bearer トークンでも受け付ける、権限より先に検証する、ページの CSP を落とす。書き込みでは、別サイトの検査を飛ばす、本文のあとで権限を調べる、どの Content-Type でも受け付ける、どのメソッドでも受け付ける、書き込みに読み取り専用の印を付ける、です。一度も失敗したことのないテストは、まだ何も証明していません。

次のページ: [質問と比較](/ja/faq)。
