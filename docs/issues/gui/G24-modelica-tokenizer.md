# G24: Modelica 専用のトークナイザで色分けする(Ada 文法の代用をやめる)

- 難易度: 中 / 依存: G23 / 触るファイル: 新規 `crates/omola-gui/src/highlight.rs`, `src/text_editor.rs`, `src/main.rs`(`mod highlight;`), `src/ui/views.rs`(ライブラリの読み取り専用テキスト)
- 先に `docs/issues/gui/README.md` を読むこと。

## 背景

`text_editor.rs` の `syntax_for_path()` は syntect に Modelica の文法が無いため **Ada の文法で代用**しています。Modelica のキーワード(`model`, `equation`, `der`, `connect`, `parameter` など)が正しく色分けされず、`annotation` 内の文字列やコメントも崩れます。

## 実装手順

1. 新規 `src/highlight.rs` に、正規表現を使わない 1 文字ずつ走査するトークナイザを書く:
   ```rust
   #[derive(Debug, Clone, Copy, PartialEq, Eq)]
   pub enum TokenKind { Keyword, Type, Builtin, Comment, String, Number, QuotedIdent, Operator, Plain }
   /// (byte range, kind) の列。text 全体を隙間なく・重なりなく覆うこと。
   pub fn tokenize(text: &str) -> Vec<(std::ops::Range<usize>, TokenKind)>
   ```
   - Keyword(Modelica 3.6 仕様の予約語。全部): `algorithm and annotation block break class connect connector constant constrainedby der discrete each else elseif elsewhen encapsulated end enumeration equation expandable extends external false final flow for function if import impure in initial inner input loop model not operator or outer output package parameter partial protected public pure record redeclare replaceable return stream then true type when while within`
   - Type: `Real Integer Boolean String`
   - Builtin: `abs sign sqrt sin cos tan asin acos atan atan2 sinh cosh tanh exp log log10 floor ceil integer min max sum product size ndims zeros ones fill identity linspace pre edge change reinit sample noEvent smooth homotopy assert terminate cardinality semiLinear delay`
   - Comment: `// ...` 行末まで、`/* ... */`(閉じが無ければ末尾まで)。
   - String: `"..."`(`\"` と `\\` のエスケープに対応、閉じが無ければ末尾まで、改行を含んでよい)。
   - QuotedIdent: `'...'`。
   - Number: `12`, `1.5`, `1e-3`, `2.5E+10`, `.5`。
   - 識別子は `[A-Za-z_][A-Za-z0-9_]*`。上の表に無ければ Plain。
   - 非 ASCII 文字(日本語コメント等)でパニックしないこと(`char_indices` で走査)。
2. 色(明るい背景用。`theme.rs` に定数として追加):
   - Keyword (0,0,255) 太字なし / Type (0,128,128) / Builtin (128,0,128) / Comment (0,128,0) / String (163,21,21) / Number (9,134,88) / QuotedIdent (0,0,0) / Operator (0,0,0) / Plain (0,0,0)
   - (Dymola の配色をコピーしたものではなく、一般的なエディタの配色です)
3. `text_editor.rs` の `highlight_job` を `highlight::tokenize` ベースに書き換える。**syntect を使うコードを削除**し、`crates/omola-gui/Cargo.toml` から `syntect` を外す。外したら `cargo run -p omola -- licenses check` を実行し、`licenses/inventory.toml` から不要になったクレートの行を削除する(check が「unused」等を出す場合はその指示に従う。出ない場合は PR に「inventory は変更不要だった」と書く)。
   - `Cargo.lock` の更新を含めること。
4. ライブラリクラスの読み取り専用テキスト(`views.rs` の `Some(Loadable::Ready(text))` 分岐)にも同じ `layouter` を使って色分けする(テキストが変わらないのでキャッシュはクラス名をキーに `BTreeMap<String, LayoutJob>` で持つ)。

## テスト(`highlight.rs`)

- 隙間・重なりが無い(全テストで `ranges` を連結すると `0..text.len()` になることを確認するヘルパを作る)。
- `model M "doc" Real x(start=1.5e-3); equation der(x) = -x; end M;` で Keyword: model/equation/end、Type: Real、Builtin: der、String: `"doc"`、Number: `1.5e-3`, `1`。
- `/* 日本語 */ x` で Comment と Plain。
- 閉じていない `"abc` と `/* abc` でパニックしない。
- `'quoted name'` が QuotedIdent。
- `Real2` は Type ではなく Plain(識別子の一部一致で判定しない)。

## 受け入れ条件

- [ ] `examples/Pendulum.mo` と MSL の `Modelica.Blocks.Continuous.PID` の Text 層のスクリーンショット(変更前後)。
- [ ] `syntect` が依存から消え、`cargo run -p omola -- licenses check` が `PASS`。
- [ ] テストが通る。README 共通コマンド 5 つが成功。
