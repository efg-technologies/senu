# 作ったものと、この先の学び方

あなたは Rust のプログラムを起動し、そのページをブラウザーで読み、Markdown でノートを書き、メッセージを通して Senu の決まりを知り、関数の一つをテストから先に作り直しました。どの手順も同じ小さなプログラムを使っていました。

## 覚えておくこと

- `cargo run --quiet -- site` でサイトをビルドして起動し、Ctrl+C で止めます。何かを変えたら再起動します。
- ノートは `content/ja/` (英語なら `content/en/`) に置き、`51-my-first-note.md` のような名前を付けます。
- Senu が起動を断るときは、メッセージがファイル、たいていは行、そして決まりを教えてくれます。
- 直す前に期待どおりの理由で失敗し、直したあとに通るテストは、直したことが正しいと示してくれます。

## 正直な限界

Senu は実験的な試作で、製品ではありません。サイトは自分のコンピューターの中でしか動かず、ノートはビルドするときにしか読みません。アカウントもデータベースもなく、ブラウザーからノートを書き換える方法もありません。動作は Linux でしか確かめていません。

## この先の学び方

- 公式の本 [The Rust Programming Language](https://doc.rust-lang.org/book/) (英語。有志による日本語訳もあります) は、この講座で触れたことすべてを、自分のペースで詳しく説明しています。
- [Rust by Example](https://doc.rust-lang.org/rust-by-example/) は、実行できる短いプログラムで教えてくれます。
- [CommonMark のリファレンス](https://commonmark.org/help/)には、ノートで使える Markdown が載っています。Senu は表を足し、画像は扱いません。
- 二つの付録で Senu の残りの部分を見られます。[HTML と JSON で配信するプロジェクト一覧](/ja/appendix-catalogue)ではテストから先に名前を変え、[同じ一覧を MCP で使う](/ja/appendix-mcp)付録が続きます。どちらもこの講座を終えた人向けで、読まなくてもかまいません。

## 演習

ここで学んだことを、自分の言葉でもう一つノートに書いてください。番号は 53 にして、最初のノートからリンクします。

**解答.** `content/ja/53-what-i-learned.md` のような名前で、1 行目がタイトルのファイルなら何でもかまいません。`content/ja/51-my-first-note.md` に `[学んだこと](/ja/what-i-learned)を読む。` のような行を書いてリンクし、Senu を再起動してリンクをたどってください。

## 発展: 見た目を変える

Senu のページの見た目は、[Tailwind CSS](https://tailwindcss.com/) から来ています。`templates/*.html` に書かれた `bg-white` (白い背景) のような小さなクラスです。Tailwind はそこで見つけたクラスの CSS の規則だけを `assets/ui.generated.css` に書き出し、Cargo はコミット済みのそのファイルを Senu に組み込みます。だからここまではほかの道具が要りませんでした。クラスを変えるには、[Node.js](https://nodejs.org/) 24 とそれに付く npm が一度だけ必要です。公式サイトからインストールし、確かめます。

```sh manual
node --version
```

Ctrl+C で Senu を止めます。次のパッチは、上の帯の背景を薄いローズ色にします。`rose` はまだどのテンプレートにも使われていません。

```diff patch
--- a/templates/site_base.html
+++ b/templates/site_base.html
@@ -11,3 +11,3 @@
 <a href="#main" class="absolute -top-16 left-2 z-10 border-2 border-blue-700 bg-white px-3 py-2 text-blue-700 underline focus:top-2"><%= frame.labels.skip %></a>
-<header class="border-b border-gray-200 bg-white">
+<header class="border-b border-gray-200 bg-rose-100">
 <div class="mx-auto flex max-w-6xl flex-wrap items-baseline justify-between gap-x-6 gap-y-2 px-5 py-3">
```

スタイルシートには新しいクラスの規則がまだないので、一致した行を数える `grep -c` は 0 を表示し、終了ステータス 1 で終わります。

```sh check=no-rule exit=1
grep -c bg-rose-100 assets/ui.generated.css
```

```text check=no-rule
0
```

`npm ci` は `package-lock.json` に書かれたとおりの道具を一度だけインストールし、`--silent` はその報告を隠します。`npm run css` は、クラスを変えるたびにスタイルシートを作り直します。

```sh check=tools
npm ci --silent
```

```sh check=css
npm run css
```

```text check=css

> css
> tailwindcss -i assets/ui.css -o assets/ui.generated.css --minify

≈ tailwindcss v4.3.3

Done in 0ms
```

`grep -o` は一致した部分だけ、ここでは新しい規則を表示します。

```sh check=new-rule
grep -o '\.bg-rose-100{[^}]*}' assets/ui.generated.css
```

```text check=new-rule
.bg-rose-100{background-color:var(--color-rose-100)}
```

Senu をもう一度起動し、http://127.0.0.1:3000/ を再読み込みすると、上の帯がローズ色になっています。

```sh check=rose server
cargo run --quiet -- site
```

```text check=rose
senu: listening on http://127.0.0.1:3000
```

テンプレートと `assets/ui.generated.css` は一緒にコミットします。そうすれば Node のない読者にも同じ見た目が届きます。すべてのページのフォントや色を変えたいときは、`assets/ui.css` の `@theme` ブロックを編集し、もう一度 `npm run css` を実行します。
