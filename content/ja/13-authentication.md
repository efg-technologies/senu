# 呼び出し元を認証する

Senu の認証は 1 つのトレイトです。リクエストのヘッダーを、サーバー側のプリンシパルに変えるか、何も返さないかを決めます。HTML ページと JSON API には、それぞれ別の認証器を付けます。

## プリンシパルとは何ですか

`Principal` は、サーバーから見た「誰が呼んでいるか」です。ログ用の名前、テナント、権限の一覧を持ちます。リクエストがテナントや権限を運ぶことはできません。それらはプリンシパルの中にしかなく、プリンシパルを作るのはあなたの認証器だけです。

```rust excerpt=examples/inventory.rs
pub const VIEW_STOCK: Permission = Permission::new("view_stock");

pub const NORTH_CLERK: Principal =
    Principal::new("north-clerk", TenantId::from_static("north"), &[VIEW_STOCK]);
```

`Principal::new` は定数を作ります。サービスアカウント向けです。`Principal::owned(name, tenant, permissions)` は実行時に作ります。読み込んだばかりのユーザー向けです。保存してあるロールは、あなたのコードで権限の定数に対応づけます。Senu がリクエストからロール名を読むことはありません。

## どの認証器が組み込まれていますか

- `auth::Bearer::new(token, principal)` は `Authorization: Bearer <token>` を受け付けます。`.and(token, principal)` でトークンを足せます。トークンごとにプリンシパルを決めます。
- `auth::Basic::new(user, password, principal)` は HTTP Basic を受け付けます。Basic はリクエストのたびにパスワードを送るので、TLS の上でだけ使ってください。人が使う画面には、自作のセッションを勧めます。

どちらも `Authorization` ヘッダーがちょうど 1 つのときだけ受け付けます。2 つあれば、無いものとして扱います。秘密は、長さが同じなら定数時間で比較します。資格情報をログに書いたり、返したりしません。`Secret` は 1〜256 文字の可視 ASCII を持ち、表示すると `Secret(<redacted>)` になります。

認証器は、状態を組み立てるときに付けます。

```rust excerpt=examples/inventory.rs
    let state = AppState::builder(Inventory::sample())
        .html_auth(Basic::new(
            Secret::new("clerk").map_err(|err| err.to_string())?,
            secret("INVENTORY_WEB_PASSWORD")?,
            NORTH_CLERK,
        ))
        .api_auth(ApiKeys::new(vec![(
            secret("INVENTORY_API_KEY")?,
            NORTH_CLERK,
        )]))
        .build();
```

## 自分の認証器はどう書きますか

`Authenticate` を実装します。次の例は、独自のヘッダーから API キーを読みます。

```rust excerpt=examples/inventory.rs
impl Authenticate for ApiKeys {
    fn authenticate<'a>(&'a self, headers: &'a HeaderMap) -> AuthFuture<'a> {
        Box::pin(async move {
            // Exactly one header, as for `Authorization`.
            let mut values = headers.get_all("x-api-key").iter();
            let (Some(presented), None) = (values.next(), values.next()) else {
                return None;
            };
            // Compare with every key, so the time does not show which one
            // matched.
            let mut found = None;
            for (key, principal) in &self.0 {
                if key.matches(presented.as_bytes()) && found.is_none() {
                    found = Some(principal.clone());
                }
            }
            found
        })
    }

    fn challenge(&self) -> Option<HeaderValue> {
        Some(HeaderValue::from_static("ApiKey realm=\"inventory\""))
    }
}
```

`authenticate` は非同期なので、データベースやキャッシュに問い合わせられます。渡されるのはヘッダーだけです。パス、クエリ文字列、ボディは渡されないので、URL に入った資格情報を誤って読むことがありません。`challenge` は、401 と一緒に送る `WWW-Authenticate` の値です。省略でき、セッションの認証器では書きません。

本番でよくある形は、同じトレイトで書けます。

- **セッション Cookie:** `auth::cookie(headers, "sid")` は、その Cookie がちょうど 1 つあるときに値を返します。セッションを引き、`Principal::owned` でプリンシパルを作ります。次の節に例があります。
- **データベースの API キー:** 提示されたキーをハッシュし、そのハッシュを引き、保存してあるスコープを権限に対応づけます。
- **信頼できるプロキシ:** ゲートウェイが JWT を検証したあとに付けるヘッダーを読みます。そのゲートウェイのネットワークからのリクエストだけを受け付けます。

## Cookie のセッションはどう扱いますか

`examples/items.rs` は、セッションの Cookie でサインインします。認証器は数行で済みます。Cookie の解析は Senu が行い、セッションはアプリケーションが持つからです。

```rust excerpt=examples/items.rs
pub struct Sessions(pub Store);

impl Authenticate for Sessions {
    fn authenticate<'a>(&'a self, headers: &'a HeaderMap) -> AuthFuture<'a> {
        Box::pin(async move {
            let session = auth::cookie(headers, "sid")?;
            self.0.lock().sessions.get(session).cloned()
        })
    }
}
```

セッションのないページの要求は、401 を返すのではなく、ログイン画面に移したいものです。`html_login` は、固定の 1 つのパスへ `303 See Other` で移します。パスが固定なので、任意の場所へのリダイレクトには使えません。JSON API はこれまでどおり 401 を返します。

```rust excerpt=examples/items.rs
pub fn state(store: Store) -> AppState<Store> {
    AppState::builder(store.clone())
        .html_auth(Sessions(store.clone()))
        .api_auth(Sessions(store))
        .html_login("/login")
        .build()
}
```

ログイン画面そのもの、ログアウト、OAuth の戻り先は、あなた自身のルートです。`.custom` で一覧に載せます。フォームを受け取るなら `.custom_with_body` を使います。どちらにも Senu の上限と応答ヘッダーが付きます。

```rust excerpt=examples/items.rs
async fn logout(State(app): State<Shared<Store>>, headers: HeaderMap) -> impl IntoResponse {
    if !auth::same_origin(&headers, &[]) {
        return (StatusCode::FORBIDDEN, "Cross-origin request refused\n");
    }
    if let Some(session) = auth::cookie(&headers, "sid") {
        app.state().sign_out(session);
    }
    (StatusCode::OK, "Signed out\n")
}
```

セッションの Cookie には `HttpOnly`・`Secure`・`SameSite=Lax` を付けます。セッション ID は長くランダムにし、保存するのはそのハッシュだけにします。

## 認証器がないとどうなりますか

表が拒否されます。`html_auth` のない HTML ルートや、`api_auth` のない API ルートがあると、`into_router` は `RouteError::NoAuthenticator` を返します。1 行書き忘れても、ルートが開いたままになることはありません。アセットと `custom` ルートは認証しません。認証が要る `custom` ハンドラーは、自分で認証します。

## Senu は CSRF から守りますか

はい。宣言した書き込みのすべてを守ります。ブラウザは、別のサイトが引き起こした要求にも Cookie や Basic の資格情報を付けます。そこで Senu は、認証の前に、各書き込みを `Sec-Fetch-Site` で、または `Origin` と `Host` の比較で検査し、別サイトからの要求を 403 `cross_origin` で断ります。規則は[書き込みのページ](/ja/writes)にあります。何かを変える `custom` ルートは、上のログアウトのように、自分で `auth::same_origin` を呼ぶ必要があります。Senu は CORS ヘッダーを送らないので、別オリジンのスクリプトは応答を読めません。すべての応答には `Cache-Control: no-store` が付きます。

## 公開前に何を確かめますか

1. 秘密は環境変数かシークレットストアから読みます。長くランダムな値を、クライアントごとに変えます。
2. TLS はルーターの手前で終端します。平文の HTTP はリダイレクトするか閉じます。
3. 認証器は、ユーザー名が違うときもパスワードが違うときも同じ経路を通ります。資格情報をログに書きません。
4. 各クライアントのプリンシパルには、必要な権限だけを持たせます。
5. [契約テスト](/ja/testing)が、あなたの表で通ります。

次のページ: [操作を MCP ツールとして公開する](/ja/mcp-tools)。
