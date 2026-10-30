# 最初のノートを書く

このサイトのページは、どれも `content` フォルダーにある **Markdown** のファイルです。Markdown は、いくつかの記号が書式になるプレーンテキストです。この章ではノートを一つ書き換え、もう一つ書き足して、Senu がそれをどう取り込むかを見ます。

## 1 分でわかる Markdown

- `# ` で始まる行はタイトルになります。どのノートにも、1 行目にちょうど一つあります。
- `## ` で始まる行は節の見出しになります。
- 空行は段落を区切ります。
- `- ` で始まる行は箇条書きになります。
- `cargo run` のようにバッククォートで囲んだ文字は、コードとして表示されます。
- `[文字](/ja/start)` はこのサイトのページへのリンクに、`[文字](https://www.rust-lang.org/)` はインターネット上のサイトへのリンクになります。

## サンプルのノートを書き換える

サイトにはサンプルのノートが付いています。ブラウザーで http://127.0.0.1:3000/ja/sample-note を開いて見てください。ファイルは `content/ja/50-sample-note.md` です。

エディターで `content/ja/50-sample-note.md` を開いてください (Visual Studio Code なら、ファイル、フォルダーを開く、で `senu` フォルダーを選び、左の一覧で `content`、`ja`、そのファイルの順に開きます)。ファイルの中身をすべて次の文章に置き換えて、保存してください。

```markdown file=content/ja/50-sample-note.md
# 書き換えたサンプルのノート

このノートはただの Markdown ファイル、`content/ja/50-sample-note.md` です。
タイトルとこの段落を書き換えました。

## 気づいたこと

- `#` で始まる行がページのタイトルになった。
- `-` で始まる行がこの箇条書きの項目になった。
- `cargo run` のようにバッククォートで囲んだ言葉はコードのように見える。

[最初の章](/ja/start)に戻る。
```

ブラウザーでページを再読み込みしても、何も変わりません。Senu はノートを動いているあいだではなく**ビルドするとき**に読むので、動いている Senu はビルドしたときのページを持ち続けます。Senu が動いているターミナルで Ctrl+C を押し、同じコマンドをもう一度実行してください。`cargo run` はファイルの変更に気づき、起動の前にビルドし直します。

```sh check=edited server
cargo run --quiet -- site
```

```text check=edited
senu: listening on http://127.0.0.1:3000
```

ブラウザーでページを再読み込みするか、二つ目のターミナルで確かめます。

```sh check=edited-title
curl -s http://127.0.0.1:3000/ja/sample-note | grep "<h1>"
```

```text check=edited-title
<h1>書き換えたサンプルのノート</h1>
```

## 自分のノートを足す

ノートのファイル名は二つのことを決めます。名前は `51-my-first-note.md` のように、2 桁の数字、ハイフン、小文字の英字・数字・単独のハイフンでできた**スラッグ**、そして `.md` の順に並べます。数字は章の一覧での位置を決め、スラッグはアドレスになります。このファイルなら http://127.0.0.1:3000/ja/my-first-note です。日本語のノートでも、ファイル名には英字を使います。

エディターで `content/ja/51-my-first-note.md` という名前の新しいファイルを作ってください (Visual Studio Code なら、`ja` フォルダーを右クリックして「新しいファイル」を選び、名前を入力します)。次の文章を入れて保存します。

```markdown file=content/ja/51-my-first-note.md
# 最初のノート

今日は自分のコンピューターで Rust のプログラムを動かし、そのページをブラウザーで読んだ。

- プログラムの名前は Senu。
- ターミナルで Ctrl+C を押すまで動き続ける。
```

さっきと同じように Senu を再起動してください。Senu のターミナルで Ctrl+C を押し、同じコマンドを実行します。

```sh check=added server
cargo run --quiet -- site
```

```text check=added
senu: listening on http://127.0.0.1:3000
```

http://127.0.0.1:3000/ja/my-first-note を開いてください。タイトルの下に、読むのにかかる時間の目安が表示されています。この目安を計算する関数は、あとの章で作り直します。ノートは章の一覧にも、サンプルのノートのすぐ後ろに並んでいます。51 は 50 の次だからです。二つ目のターミナルで確かめます。

```sh check=added-title
curl -s http://127.0.0.1:3000/ja/my-first-note | grep "<h1>"
```

```text check=added-title
<h1>最初のノート</h1>
```

```sh check=added-list
curl -s http://127.0.0.1:3000/ja/ | grep my-first-note
```

最初のページには、章の一覧と目次の 2 か所にノートが載ります。

```text check=added-list
<li><a href="/ja/my-first-note">最初のノート</a></li>
<li><a href="/ja/my-first-note">最初のノート</a></li>
```

ノートをどこかに登録する必要はありませんでした。Senu はビルドのときに `content/ja` にある `.md` ファイルをすべて見つけるので、ファイルを足して再起動するだけで十分です。

## 演習

ノートに Rust の公式サイト https://www.rust-lang.org/ へのリンクを足し、Senu を再起動して、ブラウザーでリンクをたどってください。

**解答.** 下の最後の行のようなリンクの行を `content/ja/51-my-first-note.md` に足して、ファイルを次のようにします。

```markdown file=content/ja/51-my-first-note.md
# 最初のノート

今日は自分のコンピューターで Rust のプログラムを動かし、そのページをブラウザーで読んだ。

- プログラムの名前は Senu。
- ターミナルで Ctrl+C を押すまで動き続ける。

次は [Rust の公式サイト](https://www.rust-lang.org/)でもっと読んでみたい。
```

```sh check=linked server
cargo run --quiet -- site
```

```text check=linked
senu: listening on http://127.0.0.1:3000
```

```sh check=linked-href
curl -s http://127.0.0.1:3000/ja/my-first-note | grep rust-lang.org
```

```text check=linked-href
<p>次は <a href="https://www.rust-lang.org/">Rust の公式サイト</a>でもっと読んでみたい。</p>
```

## うまくいかないとき

- **ページが古い文章のまま**: Senu を再起動していません。Senu のターミナルで Ctrl+C を押し、`cargo run --quiet -- site` をもう一度実行してください。
- **ノートの代わりに「Not Found」と出る**: アドレスとファイル名を見比べてください。`51-my-first-note.md` は、数字と `.md` を除いた `/ja/my-first-note` で配信されます。
- **Senu が起動せずにメッセージを出して止まる**: 次の章でそのメッセージを説明します。

次へ: [Senu が「だめ」と言うとき](/ja/when-senu-says-no)
