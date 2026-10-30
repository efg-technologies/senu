# Senu が「だめ」と言うとき

Senu は配信を始める前に、すべてのノートを確かめます。ノートが決まりを破っていると、何がどこでおかしいかを表示して止まるので、壊れたページがブラウザーに届くことはありません。この章では練習用のノートを作り、わざといくつかの方法で壊して、それぞれのメッセージを読み、最後に削除します。

まず Senu を止めてください。Senu が動いているターミナルで Ctrl+C を押します。二つ目のターミナルは開いたままにしておきます。

## タイトルのないノート

エディターで `content/ja/52-scratch.md` というファイルを作り、タイトルの行がない次の文章を入れてください。

```markdown file=content/ja/52-scratch.md
この練習用ノートはタイトルを書き忘れた。
```

一つ目のターミナルで Senu を起動します。

```sh check=no-title exit=2
cargo run --quiet -- site
```

待ち受けの行の代わりに、Senu は次のように表示します。

```text check=no-title
senu: content/ja/52-scratch.md:1: the first block must be a level-1 heading, such as `# My note`
```

左から順に読みます。文句を言っているプログラム (`senu`)、ファイル、そのファイルの何行目か (`1`)、そして何がおかしいか、です。Senu のメッセージは英語で、ここでは「最初のブロックは `# My note` のようなレベル 1 の見出しでなければならない」と言っています。Senu が止まったので、プロンプトはすぐに戻ります。プログラムは、成功したか失敗したかを**終了ステータス**という数でも伝えます。0 が成功、それ以外は失敗です。Senu は入力を直せば解決する問題に 2 を使います。コマンドの直後に `echo $?` と入力すると、そのコマンドの終了ステータスが表示されます。

## 許されないリンク

ノートにタイトルを付け、ページを開く代わりにコードを実行してしまうリンクを足します。`content/ja/52-scratch.md` の中身を次のように置き換えてください。

```markdown file=content/ja/52-scratch.md
# 練習

[ここを押す](javascript:alert(1))
```

```sh check=bad-link exit=2
cargo run --quiet -- site
```

```text check=bad-link
senu: content/ja/52-scratch.md:3: the link "javascript:alert(1)" is neither a page of this site, such as "/", nor an https:// address
```

Senu が受け付けるリンクは 2 種類だけです。`/ja/start` や `/start` のような、このサイトのページの正確なアドレスと、`https://` で始まるアドレスです。それ以外はすべて断ります。リンクは、ページに思いがけないものを紛れ込ませるいちばん簡単な道だからです。

## HTML は実行されず、表示される

Markdown のファイルには HTML のタグを書けますが、Senu はそれを HTML としてブラウザーに渡しません。`content/ja/52-scratch.md` の中身を次のように置き換えてください。

```markdown file=content/ja/52-scratch.md
# 練習

<script>alert("hi")</script>
```

今度は Senu が起動します。

```sh check=html server
cargo run --quiet -- site
```

```text check=html
senu: listening on http://127.0.0.1:3000
```

http://127.0.0.1:3000/ja/scratch を開いてください。タグはコードの枠の中に文字として表示され、実行されません。二つ目のターミナルで仕組みがわかります。山括弧が `&lt;` と `&gt;` に変わっています。ブラウザーはこれを `<` と `>` として表示しますが、実行はしません。

```sh check=html-shown
curl -s http://127.0.0.1:3000/ja/scratch | grep script
```

```text check=html-shown
<pre><code>&lt;script&gt;alert("hi")&lt;/script&gt;
```

## 決まりに合わない名前

Ctrl+C で Senu を止めてください。ファイル名を、S を大文字にした `52-Scratch.md` に変えます。エディターでファイルを右クリックし、「名前の変更」を選びます。ターミナルなら、ファイルを移動する (つまり名前も変えられる) `mv` で同じことができます。

```sh check=rename
mv content/ja/52-scratch.md content/ja/52-Scratch.md
```

```sh check=bad-name exit=2
cargo run --quiet -- site
```

```text check=bad-name
senu: content/ja/52-Scratch.md: the file name must be two digits, a hyphen, a slug of lowercase letters and digits joined by single hyphens, then .md, such as 51-my-note.md
```

ファイル名は「2 桁の数字、ハイフン、小文字の英字と数字を単独のハイフンでつないだスラッグ、そして .md」でなければならない、という意味です。

## 練習用ノートを削除する

エディターで `content/ja/52-Scratch.md` を削除するか (右クリックして「削除」)、ターミナルでファイルを完全に消す `rm` を使います。

```sh check=delete
rm content/ja/52-Scratch.md
```

Senu をもう一度起動します。

```sh check=deleted server
cargo run --quiet -- site
```

```text check=deleted
senu: listening on http://127.0.0.1:3000
```

練習用のページはなくなりました。`-w "\n"` は答えの後に改行を出力させるので、プロンプトが新しい行から始まります。

```sh check=deleted-page
curl -s -w "\n" http://127.0.0.1:3000/ja/scratch
```

```text check=deleted-page
Not Found
```

章の一覧からも消えています。`grep` は何も見つからないと何も表示せず、終了ステータスは 1 になります。

```sh check=deleted-list exit=1
curl -s http://127.0.0.1:3000/ja/ | grep scratch
```

```text check=deleted-list
```

## 演習

Senu を止め、`# タスク` という 1 行だけの `content/ja/51-tasks.md` を作って、Senu をもう一度起動してください。新しいファイルはどの決まりを破っていて、どう直せばよいでしょう。

**解答.** ファイル `content/ja/51-tasks.md` は次のとおりです。

```markdown file=content/ja/51-tasks.md
# タスク
```

```sh check=duplicate exit=2
cargo run --quiet -- site
```

```text check=duplicate
senu: content/ja/51-tasks.md: the number 51 is already used by content/ja/51-my-first-note.md
```

番号は順番を決めるので、同じ言語の二つのノートが同じ番号を使うことはできません。新しいノートに `53-tasks.md` のような空いている番号を付けるか、削除します。ここでは `rm` で削除します。

```sh check=duplicate-deleted
rm content/ja/51-tasks.md
```

## うまくいかないとき

メッセージはいつもファイルを名指しします。よく出るものは次のとおりです。

- 「the first block must be a level-1 heading」: 1 行目は `# ` とタイトルで始めます。
- 「the title is empty」「the title is longer than 80 characters」: タイトルが空、または 80 文字より長いので、書き足すか短くします。
- 「the file name must be…」: 2 桁の数字、ハイフン、すべて小文字かどうかを確かめます。
- 「is already used by」: 二つのノートが同じ番号かスラッグを使っています。どちらかの名前を変えます。
- 「the link … is neither…」: `/ja/` とスラッグ、または `https://` で始まる完全なアドレスを使います。
- 「only the title can be a level-1 heading」: 節には `## ` を使います。
- 「an image is not supported」: Senu は画像を扱いません。言葉で説明するか、`https://` で画像にリンクします。

再起動しても Senu が変更を無視しているように見えるときは、Senu 自身のビルド結果を消す `cargo clean -p senu` を実行してから、もう一度起動してください。

次へ: [はじめての Rust の関数](/ja/first-function)
