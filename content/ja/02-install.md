# Rust をインストールしてコードを手に入れる

この章では Rust と Git をインストールし、Senu をダウンロードして、道具が動くことを確かめます。作業はすべてターミナルで行い、インストーラーはこの講座ではなく公式サイトから入手します。

## Rust

Rust は **rustup** でインストールします。rustup は、Rust のコンパイラーと、Rust のプログラムをビルドして実行する道具である **Cargo** をダウンロードする小さなプログラムです。自分のシステム向けの公式の手順に従い、終わったらこのページに戻ってきてください。

- **Linux と macOS:** [Rust Book の「インストール」](https://doc.rust-lang.org/book/ch01-01-installation.html)に従ってください (日本語訳もありますが、手順は英語版が最新です)。同じページで説明されているとおり、Rust には**リンカー**も必要です。リンカーは、コンパイルされた部品を一つのプログラムにまとめる道具です。macOS では `xcode-select --install` を実行すると入ります。Linux では C コンパイラーのパッケージに含まれていて、Ubuntu や Debian では `build-essential` という名前です。
- **Windows:** この講座では WSL を使います。[Microsoft の WSL のインストール手順](https://learn.microsoft.com/ja-jp/windows/wsl/install)に従い、作られた Ubuntu のターミナルを開いて、その中で Linux 向けの手順に従ってください。それ以降はすべて Linux と同じように進みます。WSL を使わない Windows は扱いません。

インストーラーが終わったら、ターミナルを閉じて新しく開き直してください。新しいコマンドが見つかるようになります。

## Git

**Git** は Senu のようなプロジェクトをダウンロードし、その変更を記録する道具です。`git --version` と入力して Enter キーを押してください。「command not found」と返ってきたら、[Git のダウンロードページ](https://git-scm.com/downloads)からインストールしてください。Ubuntu では `sudo apt install git` で入ります。

## Senu をダウンロードする

Git でプロジェクトをダウンロードすることを**クローン**と呼びます。ターミナルで次の三つのコマンドを実行してください。一つ目は、ターミナルがどこにいてもホームフォルダー `~` に移動します。二つ目がそこに `senu` フォルダーを作り、三つ目がそのフォルダーを作業ディレクトリーにします。

```sh manual
cd ~
git clone https://github.com/efg-technologies/senu.git
cd ~/senu
```

ここから先、この講座のコマンドはすべてこの `senu` フォルダーの中で実行します。あとで新しいターミナルを開いたときは、最初に `cd ~/senu` と入力してください。どのフォルダーからでも使えます。

## 道具を確かめる

Senu は `rust-toolchain.toml` というファイルで、使う Rust の版を一つに決めています。`senu` フォルダーの中で初めて Cargo を実行すると、rustup がその版をダウンロードし、答えの前に進み具合を何行か表示することがあります。これは想定どおりです。次を実行してください。

```sh check=versions
cargo --version
rustc --version
```

どちらも 1.99.1 で始まっていれば大丈夫です。括弧の中は正確なビルドを表します。

```text check=versions
cargo 1.99.1 (5f94df478 2026-08-27)
rustc 1.99.1 (58cf8f9c3 2026-10-13)
```

## 演習

`senu` フォルダーの中で `ls` を実行してください。いま読んでいるページは、どのフォルダーに入っていると思いますか。

**解答.** `content` フォルダーです。中に英語用の `en` と日本語用の `ja` があり、1 ページにつき 1 つの Markdown ファイルが入っています。二つ先の章で、ここに自分のファイルを足します。

## うまくいかないとき

- **「cargo: command not found」**: 新しいターミナルを開いてください。それでも直らなければ、インストーラーをもう一度実行し、最後に表示される行を読んでください。PATH (ターミナルがコマンドを探すフォルダーの一覧) の更新方法が書いてあります。
- **「toolchain '1.99.1' is not installed」**: `senu` フォルダーの中で `rustup toolchain install 1.99.1` を実行してください。
- 次の章の最初のビルドで**「linker `cc` not found」**と出た場合: C コンパイラーのパッケージが入っていません。上の Linux と macOS の説明を見てください。
- **「destination path 'senu' already exists」**: 以前にクローン済みです。`cd ~/senu` と入力して先に進んでください。

この講座は Linux (GitHub のサーバー上の Ubuntu と、Linux のワークステーション 1 台) でしか試していません。macOS と WSL は公式の手順に沿っていますが、この講座として動作は確かめていません。

次へ: [ブラウザーで最初のページを見る](/ja/first-page)
