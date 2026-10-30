<nav class="senu-lang" aria-label="言語"><a href="../exercise.html" hreflang="en" lang="en">English</a></nav>

# 演習: プロジェクト名を変える

この演習では、プロジェクト `acme-001` の名前を "Apollo Launch" から "Artemis Launch" に
変えます。id は `acme-001` のままです。先にテストを変えて失敗を確かめ、次にカタログを変えて
成功を確かめ、古い名前を固定しているほかのテストも追従させて、テスト全体と quickstart が
通るところまで進めます。最後に元のカタログに戻します。

以下の変更はどれも差分で示します。ファイルを手で編集するか、差分の下にある `git apply` の
コマンドで同じ差分を当ててください。演習は専用のブランチで行います。

```sh
{{#include ../../exercise/steps.sh:branch}}
```

```text
{{#include ../../exercise/steps.out:branch}}
```

## 1. 赤: 新しい名前を期待する

[`src/catalogue.rs`](https://github.com/efg-technologies/senu/blob/main/src/catalogue.rs)
の末尾近くにあるテスト `demo_catalogue_matches_the_spec_table` は、すべてのプロジェクトを
`(id, tenant, name)` の組で並べています。このテストで、`acme-001` の期待する名前を変えます。

```diff
{{#include ../../exercise/1-expect-artemis.patch}}
```

```sh
{{#include ../../exercise/steps.sh:expect}}
```

このテストだけを実行します。

```sh
{{#include ../../exercise/steps.sh:red}}
```

コンパイルは通り、アサーションで失敗します。出力の中に次の行が見えるはずです
(`...` は長い行の残りを省略しています)。

```text
{{#include ../../exercise/steps.out:red}}
```

`left` はカタログが持っている値、`right` はテストがいま期待している値です。赤の手順は、
この理由で失敗するときにだけ意味があります。コンパイルエラーで失敗しても、カタログについては
何も分かりません。

## 2. 緑: プロジェクト名を変える

次に、同じファイルの先頭近くにある `DEMO_PROJECTS` で、カタログそのものを変えます。

```diff
{{#include ../../exercise/2-rename-catalogue.patch}}
```

```sh
{{#include ../../exercise/steps.sh:rename}}
```

同じテストをもう一度実行します。

```sh
{{#include ../../exercise/steps.sh:green}}
```

```text
{{#include ../../exercise/steps.out:green}}
```

## 3. もう一度赤: テスト全体

一つのテストが通っただけでは終わりではありません。すべてのテストを実行します。

```sh
{{#include ../../exercise/steps.sh:suite-red}}
```

ライブラリのテストは通りますが、`tests/adapter_http.rs` で止まります。

```text
{{#include ../../exercise/steps.out:suite-red}}
```

この二つのテストは HTTP の契約 (外から見た振る舞いの約束) の一部で、意図してカタログの
データを固定しています。一つは `launch` に対する JSON の本文を、`acme-001` の名前まで含めて
そのまま期待しています。もう一つは `apollo launch` で検索して `acme-001` を期待しており、
これはもう一致しません。カタログを意図して変えたときは、こうした保守対象の期待値も
その変更に追従させる必要があります。"Apollo Launch" はほかに `src/view.rs` と
`tests/quickstart_check.rs` のテストにも出てきますが、これらはカタログを読まずに独自の
サンプルデータを作っているので、そのまま通ります。手を加える必要はありません。

## 4. 緑: 期待値を追従させる

[`tests/adapter_http.rs`](https://github.com/efg-technologies/senu/blob/main/tests/adapter_http.rs)
の期待する JSON の本文と検索語を変えます。

```diff
{{#include ../../exercise/3-follow-fixtures.patch}}
```

```sh
{{#include ../../exercise/steps.sh:fixtures}}
```

```sh
{{#include ../../exercise/steps.sh:suite-green}}
```

すべてのテストバイナリが `ok` を報告します。その中に HTTP アダプターのテストの行も
見えるはずです。

```text
{{#include ../../exercise/steps.out:suite-green}}
```

## 5. 三つの経路を確かめる

パリティテストを単独で実行してから、quickstart を実行します。

```sh
{{#include ../../exercise/steps.sh:parity}}
```

```text
{{#include ../../exercise/steps.out:parity}}
```

JSON API、HTML ページ、MCP ツールのどれにも "Artemis Launch" が表示され、id は
`acme-001` と `acme-003` のままです。パリティテストは、三つのアダプターがいまも
`list_projects` と一致していることを確かめています。どのアダプターも同じ一つの処理を通して
同じ一つのカタログを読むので、アダプターには何も手を加える必要がありませんでした。

## 6. 元に戻す

名前を変えたカタログはこの演習のためだけのものです。二つのファイルを元に戻し、
前のブランチに戻って、演習用のブランチを削除します。

```sh
{{#include ../../exercise/steps.sh:restore}}
```

```text
{{#include ../../exercise/steps.out:restore}}
```

実際に名前を変えるときは、ブランチ上でコミットし、テストが読まない
[仕様 0001](https://github.com/efg-technologies/senu/blob/main/docs/specs/0001-project-catalogue.md)
のカタログの表も更新します。

## チュートリアルを自分で確かめる

[`scripts/check_tutorial.py`](https://github.com/efg-technologies/senu/blob/main/scripts/check_tutorial.py)
は、このチュートリアルが取り込んでいるのと同じファイルを実行します。`exercise` は
リポジトリを一時的な git リポジトリにコピーし、手元の `target/debug` のビルドキャッシュの
コピーを再利用して、上の手順をすべて順に実行します。各手順の終了コードを確かめ、赤の手順が
パッチやコンパイラーのエラーではなくテストのアサーションで失敗していることを確かめ、最後に
自分の作業ツリーが変わっていないことを確かめます。`commands` は第 3 章と同じように
サーバーを起動し、第 2 章から第 5 章のほかのコマンドの出力を期待される出力と比べます。

```sh
{{#include ../../exercise/check.sh:check}}
```

## 限界と次の一歩

Senu は意図してここで止めています。カタログは組み込みの読み取り専用なので、実際の
アプリケーションには保存先、マイグレーション、書き込み操作が必要で、それぞれに仕様が
要ります。Basic 認証はブラウザーが自動で送る認証情報なので、書き込み操作を加える前に
CSRF 対策も必要です。本番で使うには、本物のユーザー認証、セッション、サーバーの前段の
TLS、ログ、レート制限が必要ですが、Senu にはどれもありません。次の一歩としては、
[仕様 0001](https://github.com/efg-technologies/senu/blob/main/docs/specs/0001-project-catalogue.md)
と [開発ログ](https://github.com/efg-technologies/senu/blob/main/docs/development.md)
(どちらも英語) を読んで最初のスライスがどう作られテストされたかを確かめることと、
`tests/adapter_parity.rs` に検索の場合を一つ足すような小さな変更を、同じ赤から緑への
流れで試すことをおすすめします。
