# 書き込みの操作を宣言する

書き込みは、状態を変える操作です。読み取りと同じように宣言し、`effect: Effect::CREATE`・`UPDATE`・`DELETE` のどれかを付けます。Senu はそれを、JSON の本文つきの `POST` で、別サイトからの要求を断る検査の後ろで提供します。MCP ツールとしては、読み取り専用の印を付けずに出します。

## 書き込みはどう宣言しますか

このページの例は、Cookie のセッションを使う小さな管理アプリ `examples/items.rs` から取っています。次の書き込みは項目を 1 つ足します。

```rust excerpt=examples/items.rs
pub const CREATE_ITEM: Operation<Store, Created> = Operation {
    name: "create_item",
    description: "Adds an item to the caller's project and returns its id.",
    permission: EDIT_ITEMS,
    params: &[
        Param::required("title", "The title: 1 to 120 characters."),
        Param::required("lo", "The lower bound of the expected effect.").number(),
        Param::required("hi", "The upper bound; not below the lower bound.").number(),
    ],
    effect: Effect::CREATE,
    run: Run::Sync(create_item),
};
```

- `effect` は、操作が状態に何をするかを表します。`Effect::Read` は何も変えません。`Effect::CREATE` は足すだけです。`Effect::UPDATE` と `Effect::DELETE` は上書きや削除をし、繰り返しても害がありません。それ以外は `Effect::Write { destructive, idempotent }` を自分で書きます。
- 書き込みのパラメータには、文字列のほかに数値と真偽値を使えます。`.number()`・`.integer()`・`.boolean()` がパラメータの種類を変えます。読み取りのパラメータは文字列のままです。クエリ文字列はほかの型を運べないからです。それ以外の宣言は、起動時に Senu が拒否します。
- 権限は、たいてい読み取りとは別のものにします。見てよいが変えてはいけないプリンシパルを作れます。

関数は自分の構造体に読み込みます。フィールドに型が付きます。

```rust excerpt=examples/items.rs
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateArgs {
    title: String,
    lo: f64,
    hi: f64,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct Created {
    pub id: String,
}

fn create_item(store: &Store, caller: &Principal, args: &Args) -> Result<Created, OpError> {
    let CreateArgs { title, lo, hi } = decode(args)?;
    let title = checked_title(&title)?.to_owned();
    if !(lo.is_finite() && hi.is_finite() && lo <= hi) {
        return Err(INVALID_RANGE);
    }
    let mut inner = store.lock();
    let id = format!("P-{}", inner.next_id);
    inner.next_id += 1;
    // The tenant comes from the caller, never from the arguments.
    inner
        .items
        .entry(caller.tenant().clone())
        .or_default()
        .push(Item {
            id: id.clone(),
            title,
            lo,
            hi,
            status: "idea".to_owned(),
        });
    Ok(Created { id })
}
```

`title` が文字列で `lo` と `hi` が数値であること、ほかのキーが無いこと、null が無いことは、Senu がすでに確かめています。題名の長さのように、アプリケーションだけが知っている規則は関数の中に残し、独自のエラーコードを返します。

## 書き込みはどう提供されますか

読み取りと同じく `.api` で一覧に載せます。パスは何も決めません。決めるのは宣言です。

```rust excerpt=examples/items.rs
pub fn endpoints() -> Endpoints<Store> {
    Endpoints::new()
        .api("/api/items", &LIST_ITEMS)
        .api("/api/items/create", &CREATE_ITEM)
        .api("/api/items/rename", &RENAME_ITEM)
        .api("/api/items/delete", &DELETE_ITEM)
        .custom("/login", get(login_page))
        .custom_with_body("/logout", 1024, post(logout))
}
```

書き込みのルートは `POST` だけに答え、本文として JSON のオブジェクトを 1 つ受け取ります。端末からは次のとおりです。

```sh manual
curl -b 'sid=choose-a-long-random-value' -H 'Content-Type: application/json' -d '{"title":"CSV import","lo":0,"hi":0.05}' http://127.0.0.1:3000/api/items/create
```

```text
{"id":"P-3"}
```

成功はいつも `200` で、関数の結果が JSON で返ります。書き込みを `.html` で一覧に載せることはできません。何かを変える画面は、自分のスクリプトから JSON のルートに POST するか、`custom` ルートを使います。

## 書き込みはどの順序で検査されますか

いつも次の順序です。各段は、次の段が走る前に答えを返します。

1. 上限: 長すぎるアドレスは 414 です。本文の長さの申告がルートの上限を超えていれば 413 です。
2. メソッド: `POST` 以外は 405 で、`Allow: POST` が付きます。書き込みが `GET` で走ることはありません。
3. 下で説明する別サイトの検査: 403 `cross_origin` です。
4. 認証: 401 です。
5. 権限: 403 です。
6. クエリ文字列は空で、`Content-Type` は `application/json` でなければなりません。違えば `invalid_request` か 415 です。
7. 本文を上限まで読みます。JSON のオブジェクトでなければなりません。違えば 413 か `invalid_request` です。
8. パラメータの枠: 未知のキーと、種類の違う値は `invalid_request` です。
9. あなたの関数。

7 の前に本文を読むことはありません。資格情報や権限のない呼び出し元のために解析の手間はかからず、どの入力が有効だったかも伝わりません。

## Senu は CSRF をどう防ぎますか

ブラウザは、別のサイトが引き起こした要求にも Cookie や Basic の資格情報を付けます。そこで Senu は、認証器が何であっても、すべての書き込みを検査します。使うのは、ブラウザが付け、ページからは偽造できないヘッダーです。

1. `Sec-Fetch-Site` があれば、`same-origin` と `none` だけを通します。
2. 無くて `Origin` があれば、その Origin のホストとポートが要求の `Host` と同じでなければなりません。
3. どちらも無ければ、ブラウザからの要求ではないので通します。

これは Go の `CrossOriginProtection` と同じ規則です。画面が API とは別のオリジンにあるなら、`Endpoints::trusted_origin("https://app.example.com")` でそのオリジンを指定します。Senu は CORS ヘッダーを送らないので、ほかのオリジンのページが応答を読むこともできません。

この検査が守るのは、サインインしている人のブラウザです。正しい資格情報を持つプログラムには何もしません。プログラムは偽造をしなくても要求を送れるからです。

## 本文はどこまで大きくできますか

既定は 64 KiB です。`Endpoints::max_body_bytes(n)` で、その表の書き込みの上限を 16 MiB まで変えられます。上限は、申告された長さにも、実際に届いた量にも効きます。読み取りとアセットは、これまでどおり本文をすべて断ります。

## 書き込みは «いまは出来ない» をどう伝えますか

`OpError::Conflict` を使います。409 になります。要求の形は正しいが、状態がそれを許さない、という意味です。

```rust excerpt=examples/items.rs
const ITEM_DEPLOYED: OpError = OpError::Conflict {
    code: "item_deployed",
    phrase: "A deployed item cannot be deleted",
};
```

```rust excerpt=examples/items.rs
fn delete_item(store: &Store, caller: &Principal, args: &Args) -> Result<Deleted, OpError> {
    let DeleteArgs { id } = decode(args)?;
    let id = checked_id(&id)?;
    let mut inner = store.lock();
    let Some(items) = inner.items.get_mut(caller.tenant()) else {
        return Ok(Deleted { deleted: false });
    };
    let Some(at) = items.iter().position(|item| item.id == id) else {
        // Deleting what is not there is not an error, so a retry is safe.
        return Ok(Deleted { deleted: false });
    };
    if items[at].status == "deployed" {
        // Well-formed, but not possible now: 409 with its own code.
        return Err(ITEM_DEPLOYED);
    }
    items.remove(at);
    Ok(Deleted { deleted: true })
}
```

| 結果 | HTTP | 本文 |
|---|---|---|
| 完了 | 200 | 関数の結果の JSON |
| 引数の形が違う | 400 | `{"error":"invalid_request"}` |
| 独自の検証エラー | 400 | あなたのコード。たとえば `{"error":"invalid_title"}` |
| この呼び出し元に見える中に該当がない | 404 | `{"error":"not_found"}` |
| いまは出来ない | 409 | あなたのコード。たとえば `{"error":"item_deployed"}` |
| バックエンドの失敗 | 500 | `{"error":"internal"}` |

削除は、例のように、同じ呼び出しを 2 回しても 2 回目は何も変えずに成功する形にします。そうしておけば、応答を受け取り損ねたクライアントやアシスタントがやり直しても害がありません。

## 書き込みは MCP ツールとしてどう見えますか

読み取りと同じく、`mcp::tool(&CREATE_ITEM)` で一覧に載せます。印は宣言に従います。`readOnlyHint` は `false` になり、`destructiveHint` と `idempotentHint` は effect のとおりになります。クライアントは、何かを変えたり消したりするツールを呼ぶ前に、利用者に確認できます。スキーマは `"lo": {"type": "number"}` のように、各パラメータの型を示します。

stdio では、すべての呼び出しが、起動時に決めた 1 つのプリンシパルとして走ります。そのプリンシパルに書き込みの権限を与えるのは、書き込ませてよいプロセスのときだけにしてください。

## Senu が書き込みのためにしないことは何ですか

- 関数をアトミックにも冪等にもしません。保存先にトランザクションがあるなら使ってください。
- 冪等キーも楽観ロックもありません。
- 引数は平らなスカラーです。リストや入れ子の構造が要る操作は、JSON の文字列を受け取って自分で解釈します。
- HTML フォームの送信とファイルのアップロードは、宣言した操作では扱いません。`custom` ルートを使ってください。

次のページ: [呼び出し元を認証する](/ja/authentication)。
