# G21: プロットで曲線データを毎フレーム複製しない + 画素幅で間引いて描く

- 難易度: 中 / 依存: なし / 触るファイル: `crates/omola-gui/src/ui/views.rs`, 新規 `crates/omola-gui/src/decimate.rs`, `src/main.rs`(`mod decimate;`)
- 先に `docs/issues/gui/README.md` を読むこと。親 issue: #60。

## 背景(現在のコード)

`ui/views.rs`:
- `plot_window()` は毎フレーム `self.curves(&diagram.signals)` を呼び、`curves()` は曲線ごとに `time: run.outcome.result.time.clone()` と `values: values.to_vec()` で **時間軸と値の Vec を丸ごと複製**しています。
- 描画(580 行付近)は全サンプルを画面座標に変換して `Shape::line` にしています。`numberOfIntervals` が大きい結果(数十万点)では、1 フレームに数十万点 × 曲線数の頂点を作ります。

## 実装手順

### A. 複製をやめる

1. `Curve` 構造体(`views.rs` 内)を借用型にする:
   ```rust
   struct Curve<'a> { name: String, color: Color32, time: &'a [f64], values: &'a [f64] }
   ```
2. `fn curves<'a>(&'a self, signals: &[PlotSignal]) -> Vec<Curve<'a>>` にする。
3. `plot_window(&mut self, ...)` の中で `self` を可変借用している箇所(ズーム範囲 `self.plot_x` の更新など)と衝突する場合は、**描画に必要な値を先にローカル変数へ取り出し**、曲線の借用が終わってから `self` を更新する。どうしても衝突する場合のみ `std::sync::Arc<[f64]>` 化を検討し、その場合は PR に理由を書く。
4. CSV 出力(`plot::plotted_csv` へ渡す `DiagramColumns`)は今までどおり複製してよい(ボタンを押したときだけなので)。

### B. 間引き(min/max decimation)

1. 新規 `src/decimate.rs` に純粋関数を書く:
   ```rust
   /// Reduce `(t, v)` to at most ~4 points per pixel column while keeping the visual shape:
   /// for each column keep the first, min, max and last sample (in time order).
   /// `t` must be non-decreasing. Samples outside [x0, x1] are skipped except the one just
   /// before x0 and just after x1 (so the line reaches the frame edge).
   pub fn min_max_indices(t: &[f64], v: &[f64], x0: f64, x1: f64, columns: usize) -> Vec<usize>
   ```
   - `columns == 0` や `t.len() < 2`、`x1 <= x0` のときは全インデックスを返す(パニックしない)。
   - `t.len() <= columns * 4` のときは間引かず範囲内の全インデックスを返す。
   - NaN は飛ばす。
   - 同じ時刻が連続する(イベント時の不連続)場合も順序を保つ。
2. 描画側: `columns = frame.width().ceil() as usize` として `min_max_indices` で得たインデックスだけを `map.to_screen` に通す。
3. ホバー値・測定カーソルの値は**間引き前のデータ**から求める(今の計算をそのまま使う)。

## テスト(`decimate.rs`)

- 10 点・columns=100 → 全点。
- 100_000 点の正弦波・columns=500 → 結果が 2000 点以下、かつ全体の最大値と最小値のインデックスを含む。
- 範囲外の点は、範囲の直前・直後の 1 点ずつ以外含まれない。
- 時刻が重複する列 `[0,1,1,2]` でパニックせず、昇順のインデックスを返す。
- 空配列・NaN 混入でパニックしない。

## 受け入れ条件

- [ ] `numberOfIntervals=200000` で BouncingBall をシミュレーション(fake-omc なら `demo_mat` を改造せず、テスト内で大きな配列を作る)してプロットしても、見た目が変更前と同じ(変更前後のスクリーンショットを並べる)。
- [ ] G16 の操作 3 の avg/max を変更前後で PR に示す。
- [ ] テストが通る。README 共通コマンド 5 つが成功。
