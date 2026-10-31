# はじめての Rust の関数

記事のページには、タイトルの下に「約 1 分で読めます」のような読む時間の目安が出ています。この章では、それを計算している Rust の関数をテストから先に作り直し、ノートのページで結果を確かめます。必要な Rust は、入力する前にすべて説明します。

## 関数のある場所

Rust のコードは `src` フォルダーにあります。何かを変える前に、エディターで `src/reading.rs` を開いて読んでみてください。`.rs` のファイルはそれぞれ**モジュール**、つまりほかのファイルから使えるコードのまとまりです。`src/lib.rs` は `pub mod reading;` のような行で Senu のモジュールを並べています。`pub` (public) は JavaScript の `export` によく似ています。別のモジュールは `use crate::reading::reading_minutes;` でこの関数を取り込みます。これは `import { readingMinutes } from "./reading.js"` によく似ています。このような**パス**は、外側から順に部分を並べた名前で、フォルダーを `/` で区切るように `::` で区切ります。`crate` は Senu 自身のコードのいちばん上、`reading` はモジュール、`reading_minutes` は関数です。

Senu はページを作るとき、ノートのコードの枠の外にある語を数え、それが何分かかるかを `reading_minutes` に尋ねます。目安はわざと単純にしてあり、1 分に 200 語として切り上げるので、短いノートは 1 分になります。日本語は語と語のあいだに空白がないので、日本語のノートでは 2 文字を 1 語として数えます。その分、英語よりさらに大まかな目安です。

## この章で使う Rust

- `fn reading_minutes(words: usize) -> usize { ... }` は `function readingMinutes(words) { ... }` に当たります。Rust では、引数ごとにコロンの後ろに**型**を書き、結果の型を `->` の後ろに書きます。
- `usize` は負にならない整数の型です。0、1、2 と続き、`usize::MAX` というとても大きな最大値まであります。`usize` が来るべきところに文字列や負の数を渡すコードを、Rust はビルドしません。
- `const WORDS_PER_MINUTE: usize = 200;` は、ファイル全体で使う決まった値に名前を付けます。
- `let full_minutes = words / WORDS_PER_MINUTE;` は JavaScript の `const fullMinutes = ...` に当たります。Rust の `let` は、あとから値を変えられません。
- `/` は整数どうしで割り算をして余りを捨てるので、`201 / 200` は `1` です。`%` はその余りを返すので、`201 % 200` も `1` です。
- `if leftover_words > 0 { a } else { b }` は JavaScript の `if` と同じように働きますが、`a` か `b` の値にもなります。関数の最後の行をセミコロンなしで書くと、その値が関数の戻り値になります。前に `return` が付いているのと同じです。

**テスト**は、自分のコードを呼んで答えを確かめる小さな関数です。関数の上に `#[test]` と書くとテストの印になり、`assert_eq!(left, right)` は二つの値が等しいかを確かめて、等しくなければメッセージを出してテストを止めます。`!` は**マクロ**の印で、Rust が代わりに展開してくれるコードです。ここでは普通の関数呼び出しとして読んでかまいません。

テストはふつうファイルの最後に、専用の小さなモジュールとして置きます。`mod tests { ... }` の上の `#[cfg(test)]` は、このモジュールがテストを実行するときだけビルドされるという意味です。その中の `use super::reading_minutes;` は、`super`、つまりこのモジュールを囲むモジュール (ここでは `src/reading.rs` の残りの部分) から関数を取り込みます。

## Red: 最初の試み

`src/reading.rs` の中身をすべて、次の最初の試みに置き換えてください。四つの例を確かめるテストが一つと、割り算をするだけの関数があります。

```rust file=src/reading.rs
// Turns a word count into the minutes shown under an article's title.

const WORDS_PER_MINUTE: usize = 200;

pub fn reading_minutes(words: usize) -> usize {
    words / WORDS_PER_MINUTE
}

#[cfg(test)]
mod tests {
    use super::reading_minutes;

    #[test]
    fn rounds_up_to_whole_minutes() {
        assert_eq!(reading_minutes(0), 0);
        assert_eq!(reading_minutes(1), 1);
        assert_eq!(reading_minutes(200), 1);
        assert_eq!(reading_minutes(201), 2);
    }
}
```

このモジュールのテストを実行します。`--lib` は Senu のライブラリー部分を指し、`reading::` は完全な名前がそれで始まるテストを選びます。

```sh check=red exit=101
cargo test --quiet --lib reading::
```

```text check=red

running 1 test
reading::tests::rounds_up_to_whole_minutes --- FAILED

failures:

---- reading::tests::rounds_up_to_whole_minutes stdout ----

thread 'reading::tests::rounds_up_to_whole_minutes' (ID) panicked at src/reading.rs:16:9:
assertion `left == right` failed
  left: 0
 right: 1
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    reading::tests::rounds_up_to_whole_minutes

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 125 filtered out; finished in 0.00s

error: test failed, to rerun pass `--lib`
```

テストが失敗しました。これは狙いどおりで、テストが間違いを見つけられることを示しています。失敗しているテストを **red** と呼びます。メッセージは `src/reading.rs` の `assert_eq!(reading_minutes(1), 1)` を指しています。`1 / 200` は 0 なので、テストが 1 (`right`) を期待したところで関数は 0 (`left`) を返しました。1 語でも読むのに 1 分はかかります。テスト名の後ろの括弧の中の番号 (ここでは `(ID)` と書いています) は、実行するたびに変わります。

## Green: 切り上げる

余った語があれば 1 分足すように関数を変えます。もう一度 `src/reading.rs` を置き換えてください。

```rust file=src/reading.rs
// Turns a word count into the minutes shown under an article's title.

const WORDS_PER_MINUTE: usize = 200;

pub fn reading_minutes(words: usize) -> usize {
    let full_minutes = words / WORDS_PER_MINUTE;
    let leftover_words = words % WORDS_PER_MINUTE;
    if leftover_words > 0 {
        full_minutes + 1
    } else {
        full_minutes
    }
}

#[cfg(test)]
mod tests {
    use super::reading_minutes;

    #[test]
    fn rounds_up_to_whole_minutes() {
        assert_eq!(reading_minutes(0), 0);
        assert_eq!(reading_minutes(1), 1);
        assert_eq!(reading_minutes(200), 1);
        assert_eq!(reading_minutes(201), 2);
    }
}
```

```sh check=green
cargo test --quiet --lib reading::
```

```text check=green

running 1 test
.
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 125 filtered out; finished in 0.00s

```

四つの例がすべて通りました。通っているテストを **green** と呼びます。

## ページで確かめる

サイトも同じ関数を呼んでいるので、Senu を再起動して自分の版でビルドします。Senu のターミナルで Ctrl+C を押し、いつものコマンドを実行してください。

```sh check=reading server
cargo run --quiet -- site
```

```text check=reading
senu: listening on http://127.0.0.1:3000
```

http://127.0.0.1:3000/ja/my-first-note を開いてください。ノートは 200 語よりずっと短いので、1 分と表示されます。二つ目のターミナルでも確かめます。`grep -A1` は、一致した行とその次の行を表示します。ここでは題名と、その下の読了時間です。

```sh check=reading-time
curl -s http://127.0.0.1:3000/ja/my-first-note | grep -A1 '<h1>'
```

```text check=reading-time
<h1>最初のノート</h1>
<p class="text-sm text-gray-600">約 1 分で読めます</p>
```

## 演習: いちばん大きな数

語の数がいちばん大きな値、`usize::MAX` のときはどうなるでしょう。最初のテストの下に二つ目のテストを足して、`src/reading.rs` を次のようにしてください。

```rust file=src/reading.rs
// Turns a word count into the minutes shown under an article's title.

const WORDS_PER_MINUTE: usize = 200;

pub fn reading_minutes(words: usize) -> usize {
    let full_minutes = words / WORDS_PER_MINUTE;
    let leftover_words = words % WORDS_PER_MINUTE;
    if leftover_words > 0 {
        full_minutes + 1
    } else {
        full_minutes
    }
}

#[cfg(test)]
mod tests {
    use super::reading_minutes;

    #[test]
    fn rounds_up_to_whole_minutes() {
        assert_eq!(reading_minutes(0), 0);
        assert_eq!(reading_minutes(1), 1);
        assert_eq!(reading_minutes(200), 1);
        assert_eq!(reading_minutes(201), 2);
    }

    #[test]
    fn the_largest_count_does_not_overflow() {
        assert_eq!(reading_minutes(usize::MAX), usize::MAX / 200 + 1);
    }
}
```

```sh check=both
cargo test --quiet --lib reading::
```

```text check=both

running 2 tests
..
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 125 filtered out; finished in 0.00s

```

`(words + 199) / WORDS_PER_MINUTE` でも切り上げになるので、短い式のほうが魅力的に見えます。試してみましょう。関数の本体を置き換えて、`src/reading.rs` を次のようにしてください。

```rust file=src/reading.rs
// Turns a word count into the minutes shown under an article's title.

const WORDS_PER_MINUTE: usize = 200;

pub fn reading_minutes(words: usize) -> usize {
    (words + 199) / WORDS_PER_MINUTE
}

#[cfg(test)]
mod tests {
    use super::reading_minutes;

    #[test]
    fn rounds_up_to_whole_minutes() {
        assert_eq!(reading_minutes(0), 0);
        assert_eq!(reading_minutes(1), 1);
        assert_eq!(reading_minutes(200), 1);
        assert_eq!(reading_minutes(201), 2);
    }

    #[test]
    fn the_largest_count_does_not_overflow() {
        assert_eq!(reading_minutes(usize::MAX), usize::MAX / 200 + 1);
    }
}
```

新しいテストだけを実行します。一つのテスト名にだけ合う絞り込みを使うと、出力が短くなります。

```sh check=overflow exit=101
cargo test --quiet --lib overflow
```

```text check=overflow

running 1 test
reading::tests::the_largest_count_does_not_overflow --- FAILED

failures:

---- reading::tests::the_largest_count_does_not_overflow stdout ----

thread 'reading::tests::the_largest_count_does_not_overflow' (ID) panicked at src/reading.rs:6:5:
attempt to add with overflow
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    reading::tests::the_largest_count_does_not_overflow

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 126 filtered out; finished in 0.00s

error: test failed, to rerun pass `--lib`
```

何が起きたのでしょう。最初の版にこの問題がないのはなぜでしょう。

**解答.** `words` が `usize::MAX` のとき、`words + 199` は `usize` の最大値より大きくなるので、足し算が**オーバーフロー**します。結果が収まらないのです。テスト用のビルドでは、Rust は間違った数のまま進まずに「attempt to add with overflow」(足し算でオーバーフローしようとした) と言って止まります。先に割り算をすれば数が大きくなることはなく、`usize::MAX / 200 + 1` は余裕で収まるので、商と余りを使う版はどんな入力でも動きます。元に戻して、`src/reading.rs` を次のようにしてください。

```rust file=src/reading.rs
// Turns a word count into the minutes shown under an article's title.

const WORDS_PER_MINUTE: usize = 200;

pub fn reading_minutes(words: usize) -> usize {
    let full_minutes = words / WORDS_PER_MINUTE;
    let leftover_words = words % WORDS_PER_MINUTE;
    if leftover_words > 0 {
        full_minutes + 1
    } else {
        full_minutes
    }
}

#[cfg(test)]
mod tests {
    use super::reading_minutes;

    #[test]
    fn rounds_up_to_whole_minutes() {
        assert_eq!(reading_minutes(0), 0);
        assert_eq!(reading_minutes(1), 1);
        assert_eq!(reading_minutes(200), 1);
        assert_eq!(reading_minutes(201), 2);
    }

    #[test]
    fn the_largest_count_does_not_overflow() {
        assert_eq!(reading_minutes(usize::MAX), usize::MAX / 200 + 1);
    }
}
```

```sh check=final
cargo test --quiet --lib reading::
```

```text check=final

running 2 tests
..
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 125 filtered out; finished in 0.00s

```

これで `src/reading.rs` は、Senu に最初から入っているファイルとまったく同じになりました。プログラムの本物の一部を作り直したことになります。

## うまくいかないとき

- **`error[E` で始まる行と「could not compile」**: テストの失敗ではなく、コンパイルのエラーです。メッセージがファイル、行、桁を示しているので、そこで `;` や `}` の抜けやつづりの間違いを探し、上の枠と見比べてください。
- **「running 0 tests」**: 絞り込みに何も合いませんでした。`reading::` のコロンが二つあるか確かめてください。
- **ページの読む時間が古いまま**: ノートを変えたときと同じく、Rust のコードを変えたあとも Senu を再起動します。

次へ: [作ったものと、この先の学び方](/ja/next-steps)
