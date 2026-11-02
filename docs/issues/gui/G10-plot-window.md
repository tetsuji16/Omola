# G10: プロット強化(複数ダイアグラム・カーソル・CSV 出力)

- 難易度: 中 / 依存: なし / 触るファイル: `src/ui/views.rs`, `src/app.rs`, `src/ui/browsers.rs`, `src/ui/chrome.rs`, 新規 `src/series.rs`(G09 が先に作っていれば再利用)
- 先に `docs/issues/gui/README.md` と `docs/reference/dymola-gui.md` の R12〜R18 を読むこと。

## 目的

1. **New Diagram**: 1 つのプロットウィンドウに縦に複数のプロット(ダイアグラム)を並べる。X 軸(時間)は連動。
2. **測定カーソル**: マウス位置の時刻に縦線を出し、各曲線の値を表示。
3. **CSV 出力**: アクティブなダイアグラムの曲線を CSV に保存。

## 実装手順

1. 状態の変更(`app.rs`):
   ```rust
   pub struct PlotDiagram { pub signals: Vec<PlotSignal> }
   pub diagrams: Vec<PlotDiagram>,   // 最低 1 個
   pub active_diagram: usize,
   pub cursor_enabled: bool,
   ```
   既存の `plotted: Vec<PlotSignal>` を `diagrams[active_diagram].signals` に置き換える。`add_run` の「同じクラスの最新結果へ曲線を付け替える」処理は**全ダイアグラム**に対して行う。既存テスト `first_run_autoplots_and_old_runs_are_dropped` を新構造に合わせて修正し、意味(自動プロット・付け替え・古い結果の削除)は変えない。
2. Variable Browser のチェックボックスは**アクティブなダイアグラム**の曲線を ON/OFF する。
3. Simulation リボン: 「New diagram」「Remove diagram」(1 個のときは無効)「Cursor」(トグル)「Export CSV…」を追加。既存の「Erase curves」はアクティブなダイアグラムだけを消す。
4. 描画(`views.rs` `plot_window`): プロットは egui_plot ではなく自前描画です。`plot_window()` の枠計算(余白 左53・上31・右24・下29)と目盛り関数(`nice_step` / `minor_step` / `nice_range` / `ticks`)を関数に切り出し、ダイアグラムごとに縦に等分した矩形へ同じ描画を行う。x 範囲(`plot_view` の x0/x1)は全ダイアグラムで共有し、ドラッグズームの x はすべてのダイアグラムに反映する。
5. カーソル: `cursor_enabled` のとき、ホバー位置の x を時刻 t として各枠に縦線を描き、枠の右上に各曲線の `series::value_at(time, values, t)` を `name = value` で列挙する。
6. CSV 出力:
   - G03 が済んでいればファイルダイアログ、未了なら既存の `PathDialog` に `ExportCsv(String)` を追加して使う。
   - 出力対象はアクティブなダイアグラムの曲線のうち**同じ結果(run)**のもの。複数 run が混ざっている場合は最新 run の曲線だけを出し、Messages に Warning。
   - 形式は既存 CLI の Omola CSV と同じ: 1 行目 `time,<name1>,<name2>`、値は `format!("{v}")`(Rust の最短往復表現)。名前にカンマや `"` を含む場合は `"` で囲み `"` を `""` にエスケープ。
   - 時刻の重複行(イベント)について: 既存 CLI の `crates/omola-cli/src/results.rs` の `validate_csv_records` が重複時刻を許すか確認し、許さないなら**同時刻の最後の行だけ**を残す。どちらにしたか PR に書く。
   - CSV 生成は純粋関数 `fn plotted_csv(time: &[f64], columns: &[(&str, &[f64])]) -> String` にしてテスト。
7. テスト:
   - `plotted_csv` のヘッダ・エスケープ・値の往復(`"0.1".parse::<f64>()` と一致)。
   - ダイアグラム追加/削除で `active_diagram` が範囲外にならない(削除時に詰める)。
   - `series::value_at`(G09 と同じ仕様。G09 が先ならテストは追加不要)。

## 受け入れ条件

- [ ] 2 ダイアグラムで height と velocity を別々に表示、ズームすると両方の X 範囲が一緒に動く(スクリーンショット)。
- [ ] カーソル ON で縦線と値が出る。
- [ ] 出力した CSV が `cargo run -p omola -- results inspect FILE.csv` で検証を通る。
- [ ] テストが通る。README 共通コマンド 5 つが成功。
