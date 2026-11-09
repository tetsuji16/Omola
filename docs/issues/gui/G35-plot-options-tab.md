# G35: 【不具合】リボンの「Plot Options」タブがクリックできない → プロット設定のリボンを実装する

- 難易度: 中 / 依存: なし(G21 マージ済みの `views.rs` を前提) / 触るファイル: `crates/omola-gui/src/app.rs`, `src/ui/chrome.rs`, `src/ui/views.rs`, `src/plot.rs`
- 先に `docs/issues/gui/README.md` を読むこと。

## 不具合の内容(2026-11-09 メンテナ報告)

Simulation タブを開くとリボンの右に「Plot Options」タブが出るが、**クリックしても何も起きない**。

原因: `src/ui/chrome.rs` の `ribbon_tabs()` で、

```rust
if self.tab == RibbonTab::Simulation {
    let r = ribbon_tab(ui, "Plot Options", false);
    ...
    r.on_hover_text("Plot options: use the Plot group of the Simulation tab");
}
```

と見た目だけ描いており、`clicked()` を見ていない。対応するリボンも存在しない。

## 目的

「Plot Options」を**プロット窓用の文脈タブ**として動かす。

- Simulation タブ表示中だけ出る(今の動き)。クリックするとリボン本体がプロット設定のリボンに切り替わり、タブは選択状態の見た目になる。
- 他のタブ(Graphics 等)を押せば普通にそのリボンへ。Simulation タブを押せば Simulation リボンへ戻る。
- タイトル行の「Plot」見出し(`plot_tab_x`)は今のまま。

## 実装手順

1. 状態: `RibbonTab` に `PlotOptions` を足す方法と、`OmolaApp` に `plot_options: bool` を足す方法がある。`RibbonTab::ALL` に入れるとタブが常時出てしまうので、**`ALL` には入れない**こと。どちらを選んだか PR に書く。
   - Simulation 以外の層に関係する処理(`select_tab` の Text 層切り替えなど)が `PlotOptions` で壊れないこと。`self.tab == RibbonTab::Simulation` で判定している箇所(プロット窓を中央に出す、下段のモデルタブ、ウィンドウタイトルの `Plot [1*]` 等)を grep して、Plot Options 中も Simulation と同じ扱いになるようにする(例: `fn simulation_mode(&self) -> bool`)。
2. `PlotDiagram`(`src/plot.rs`)に表示設定を足す。**ダイアグラムごと**の設定とし、「選択中のダイアグラム」(`active_diagram`)に対して操作する:
   ```rust
   pub struct PlotStyle {
       pub grid: bool,            // 既定 true
       pub legend: LegendPos,     // Top(既定) / Bottom / Hidden
       pub line_width: f32,       // 既定 1.0。1.0 / 1.5 / 2.0 / 3.0
       pub x_view: Option<(f64, f64)>, // 時間軸の手動範囲(None = 全範囲)
   }
   ```
3. リボン(`chrome.rs` に `plot_options_ribbon()`)。既存の `fixed_group` / `place` / `big_button` / `small_button` を使う。グループと項目:
   - **View**: 「Rescale」(選択ダイアグラムの y_view / x_view を None に)、「Rescale All」(全ダイアグラム)
   - **Display**: 「Grid」トグル、「Legend」ドロップダウン(Top / Bottom / Hidden)、「Line width」ドロップダウン
   - **Diagram**: 「New diagram」「Remove diagram」「Erase curves」(Simulation リボンの Plot グループと同じ処理を呼ぶ。処理を関数に切り出して共用)
   - **Range**: 「Time from / to」の数値入力 2 つ(x_view を設定。空なら None)。y も同様に「Y min / max」。
   - 未実装の項目は置かない(無効ボタンを並べない)。
4. `views.rs` の `plot_window()` で `PlotStyle` を反映する(グリッドの描画有無、凡例の位置・非表示、曲線の太さ、x 範囲)。
5. 設定は保存しなくてよい(G02 の永続化は範囲外)。

## テスト

- `plot.rs`: `PlotStyle::default()` の値、Rescale で `y_view`/`x_view` が `None` になる関数のテスト。
- 時間範囲入力の解析(`"0.5"` → Some、空 → None、`"a"` → エラー表示で値は変えない)を関数にしてテスト。

## 受け入れ条件

- [ ] `Modelica.Blocks.Examples.PID_Controller` をシミュレーションし、Plot Options タブをクリック → リボンが切り替わる(スクリーンショット)。
- [ ] Grid オフ・Legend Bottom・Line width 2 の状態のプロットのスクリーンショット。
- [ ] Time 0.4〜0.8 を入力するとその範囲だけが表示され、Rescale で戻る。
- [ ] Graphics → Simulation → Plot Options → Text と切り替えても表示が崩れない。
- [ ] テストが通る。README 共通コマンド 5 つが成功。
