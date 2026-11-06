# G14: 結果の読み込み・保持/破棄・エクスポート

> **2026-11-06 再オープン:** issue #42 は閉じられていましたが、`ResultRun.kept` / `Job::LoadResult` / 結果ノードの右クリックメニューはいずれも main に存在しません(未実装)。本文どおりに実装してください。Tools リボンの `Load Result: issue G14` アイコン(`ui/chrome.rs`)もこの issue で有効にします。G22 と同時に進める場合は `app.rs` の衝突に注意。

- 難易度: 易 / 依存: なし(G03 があればファイルダイアログを使う) / 触るファイル: `src/backend.rs`, `src/app.rs`, `src/ui/browsers.rs`, `src/ui/chrome.rs`, `src/ui/dialogs.rs`
- 先に `docs/issues/gui/README.md` と `docs/reference/dymola-gui.md` の R8 を読むこと。

## 目的

Dymola の Variable Browser の結果操作に相当するものを実装します。

1. **Simulation > Load Result…**: 既存の OpenModelica `.mat` を開いて Variable Browser に追加。
2. 結果ノードの**右クリックメニュー**: Keep result(自動削除しない)/ Close result / Save result as…(.mat をコピー)/ Copy result path。
3. Keep された結果は「Keep results」の上限で消されない。

## 実装手順

1. `ResultRun` に `pub kept: bool` を追加(既定 false)。
2. `add_run` の古い結果の削除を「`kept == false` のもののうち古い順」に変更。上限 `keep_results` は**kept でない結果の数**に対して適用する。
3. `backend.rs`: `Job::LoadResult { path: PathBuf }` → worker で `omola_core::mat::read_trajectory_file` → 成功時 `Event::SimulationFinished(SimulationOutcome { class: ファイル名から _res.mat / .mat を除いた名前, result_file: path, result, elapsed_seconds: 0.0 })`、失敗時 Error ログ。
   - `.dsres` 等、OpenModelica 形式でないファイルは `mat` リーダのエラーをそのまま Messages に出す(Dymola 形式対応はしない)。
4. メニュー: `chrome.rs` の Simulation メニューに `Load Result…` を追加。G03 済みなら `rfd` で `.mat` フィルタ、未了なら `PathDialog::LoadResult(String)` を追加して使う。
5. 右クリックメニュー(`browsers.rs`): 結果の `CollapsingHeader` の header の `Response` に `.context_menu(|ui| { ... })` を付ける(egui 0.37 の API 名は `egui-0.37.0/src/response.rs` を grep で確認)。
   - Keep result: チェック付きメニュー項目(`ui.checkbox(&mut run.kept, "Keep result")`)。kept の結果はヘッダ名の後ろに `(kept)`。
   - Close result: その run と、その run を参照する曲線を削除。
   - Save result as…: `std::fs::copy(result_file, dest)`。**上書き禁止**: dest が存在したらエラーを出してコピーしない。
   - Copy result path: `ui.ctx().copy_text(path)`。
6. テスト(`app.rs` tests):
   - kept の run は上限を超えても残る。kept でない run だけが古い順に消える。
   - Close で曲線も消える。
   - `LoadResult` のクラス名変換(`BouncingBall_res.mat` → `BouncingBall`、`foo.mat` → `foo`)を純粋関数にしてテスト。

## 受け入れ条件

- [ ] `cargo run -p omola -- simulate ...` で作った `_res.mat` を GUI で Load Result → 変数が出てプロットできる(実 OMC。無ければ `demo_mat` で生成したファイル)。
- [ ] Keep した結果が 3 回シミュレーションしても残る(スクリーンショット)。
- [ ] テストが通る。README 共通コマンド 5 つが成功。
