# G26: 【不具合】Diagram 層: origin が省略された部品が原点に重なる

- 難易度: 易 / 依存: なし / 触るファイル: `crates/omola-gui/src/diagram.rs`
- 先に `docs/issues/gui/README.md` を読むこと。

## 不具合の内容

`diagram.rs` の `parse_placement()` は、`Placement(transformation(extent=..., origin=...))` の `origin` が無いと `origin = [0, 0]` のままにします。
OMC の `getModelInstance` は、`extent` を**ダイアグラムの絶対座標**で書いた部品(`origin` 省略)をよく返します。例:
- `tests/fixtures/instance_cauer.json` の部品 `G`: `extent=[[-10,-90],[10,-70]]`、origin なし
- `tests/fixtures/instance_pid.json` の部品 `PI`: `extent=[[-56,-20],[-36,0]]`、origin なし

現在、これらの部品は extent の位置ではなく原点付近に重なって描かれます(または描画側の計算と二重に足される)。

## 実装手順

1. まず `diagram_view.rs` で `placement.origin` と `placement.extent` をどう使って画面座標を出しているかを読み、**Modelica 仕様どおり** 「部品の点 p(部品の座標系)→ ダイアグラム座標 = origin + rotate(extent 内に写した p)」になっているか確認する。Modelica 仕様 18.6.1.1(Placement)では、`origin` 省略時は `{0,0}` で、`extent` はそのまま親の座標系の矩形です。
   - つまり**仕様上は origin=[0,0] が正しく、extent をそのまま使えば部品は正しい位置に来る**はずです。重なるのは描画側が extent を origin 中心の相対値として扱っている場合です。
2. どちらが原因かを確かめるため、先にテストを書く(下記)。
3. 直し方は次のどちらか(PR にどちらを選んだか書く):
   - (a) 描画側(`diagram_view.rs`)が extent を origin からの相対として扱っていて、origin 省略時に壊れているなら、描画側を仕様どおりに直す。
   - (b) 描画側が「extent の中心 = origin、extent は中心からの相対」という前提で書かれており直すのが大変なら、`parse_placement` で origin が無いとき `origin = extent の中心`、`extent = extent - 中心` に**正規化**する(extent も相対化すること。origin だけ変えると二重に移動する)。
4. 回転(`rotation`)がある部品でも位置がずれないことをテストで確認する。

## テスト(`diagram.rs` の tests)

- `instance_cauer.json` を読み、部品 `G` の画面上の外接矩形(ダイアグラム座標)が `x -10..10, y -90..-70` になる。
- `instance_pid.json` の `PI` が `x -56..-36, y -20..0`。
- origin あり・回転 90 度の部品(既存の fixture から 1 つ選ぶか、小さな JSON を手で書く)の外接矩形が Modelica 仕様どおり。
- 「ダイアグラム座標の外接矩形」を返す関数(例: `pub fn component_bounds(p: &Placement) -> Rect`)が無ければ作ってテストする。描画側もこの関数を使うようにする。

## 受け入れ条件

- [ ] `Modelica.Electrical.Analog.Examples.CauerLowPassAnalog` と `Modelica.Blocks.Examples.PID_Controller` の Diagram 層のスクリーンショット(変更前後)。部品が重ならず結線の端と部品のコネクタが一致している。
- [ ] テストが通る。README 共通コマンド 5 つが成功。

## 補足

メンテナのローカル環境に、この不具合の暫定修正(origin を extent 中心にするだけ)が未コミットで存在します。extent を相対化していないため、上記 3(b) の注意どおり二重移動の可能性があります。この issue の実装で置き換えてください。
