# G44: Simulation Setup の仕上げ(保持する結果数・入力エラーの表示・Interval length)

- 難易度: 易 / 依存: なし / 触るファイル: `crates/omola-gui/src/ui/dialogs.rs`(`setup_dialog`), `src/backend.rs`(`SimulationSetup`), `src/app.rs`
- 先に `docs/issues/gui/README.md` を読むこと。

## 背景(2026-11-12 のコード全体見直しで判明)

- `OmolaApp::keep_results`(保持する結果数、既定 2)は保存・復元されるが、**変更する画面がどこにも無い**。
- Start time ≥ Stop time などの不正な設定は、Simulate を押してログを見るまで分からない。
- 出力間隔は「Number of intervals」でしか指定できない(間隔の長さで指定できない)。

## すでに入っている骨格

- `SimulationSetup::validate() -> Result<(), String>`(`backend.rs`): 開始/終了時刻、間隔数、許容誤差、アルゴリズム名を検査する。`OmolaApp::simulate()` は実行前にこれを呼び、エラーを Translation ログに出す。テスト `setup_validation_names_the_offending_field`。
- リボンのアルゴリズム選択は `omola_core::SIMULATION_METHODS` を使う(Setup ダイアログと同じ一覧)。
- リボンの Stop time / アルゴリズムを変えると `setup_edited_by_user = true` になる。

## 実装手順

1. **入力エラーの表示**: `setup_dialog` の `Simulate` / `OK` ボタンの直前で `let problem = self.setup.validate().err();` を求め、`Some(message)` なら赤字(`egui::Color32::RED`)で表示し、`Simulate` ボタンを無効にする(`OK` は押せてよい)。
2. **保持する結果数**: `General` グリッドの下に `Output` 見出しを作り、`Number of results to keep` を `egui::DragValue::new(&mut self.keep_results).range(1..=20)` で出す。値を減らしたときは、ダイアログの後で `self.evict_results()` を呼ぶ(`before`/`after` の比較と同じ場所で、変更があったときだけ)。この値は `setup_edited_by_user` には影響させない。
3. **Interval length**: `Number of intervals` の行の下に `Interval length` を足す。保存する値は増やさず、表示用に `(stop - start) / intervals` を計算して `DragValue` に渡し、編集されたら `intervals = ((stop - start) / length).round().clamp(1.0, 1e7) as u32` に書き戻す。この換算は純関数 `pub fn intervals_for_length(start: f64, stop: f64, length: f64) -> u32` として `backend.rs` に置く(`length <= 0` や非有限のときは 1 を返す)。
4. `docs/reference/dymola-gui.md` §3.2 の Simulation Setup の行を更新する。

## テスト

- `backend.rs`: `intervals_for_length_rounds_and_clamps`(`(0, 10, 0.02) → 500`、`(0, 1, 0.0) → 1`、`(0, 1, f64::NAN) → 1`、`(0, 1, 1e-12) → 10_000_000`)。
- `app.rs`: `lowering_keep_results_evicts_old_results`(結果を 3 つ追加 → `keep_results = 1` → `evict_results()` → 1 つだけ残る。Keep した結果は残る)。

## 受け入れ条件

- [ ] Stop time を Start time 以下にすると、ダイアログに赤字の理由が出て `Simulate` が押せない。
- [ ] `Number of results to keep` を変えると、次回起動後も値が残り、古い結果がその数まで減る。
- [ ] `Interval length` を変えると `Number of intervals` が連動する(逆も)。
- [ ] 上のテストが通る。README 共通コマンド 5 つが成功。ダイアログのスクリーンショットを PR に付ける。
