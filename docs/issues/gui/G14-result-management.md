# G14: 結果の読み込み・保持/破棄・エクスポート

> **2026-11-12 更新(残作業はこの枠内だけ):** 見直しの PR(`gui-audit-skeleton`)で、下の実装手順 1〜3、4 の Tools リボン分、5 の Keep / Close / Copy result path、6 のテストは**実装済み**です(`ResultRun.kept`、`evict_results`、`close_result`、`set_result_kept`、`Job::LoadResult`、`result_class_from_path`、`request_load_result_dialog`、`ui/browsers.rs` の結果ヘッダの右クリックメニュー)。作り直さないでください。
>
> **残り:**
> 1. **Save result as…**(結果の右クリックメニュー): `app.rs` に `pub fn save_result_as(&mut self, run_id: u64, dest: PathBuf)` を足す。`dest` が存在したら**コピーせず** `"<dest> already exists; choose another name."` を Error ログに出す。存在しなければ `std::fs::copy(result_file, dest)`。保存先はネイティブダイアログ(`rfd`、`.mat` フィルタ)、`OMOLA_GUI_TEXT_DIALOGS=1` のときは `PathDialog::SaveResult(u64, String)` を足す(`PathDialog::LoadResult` の実装を手本に)。存在判定とコピーは純関数 `pub fn copy_result_file(src: &Path, dest: &Path) -> Result<(), String>` に分け、テスト `copy_result_file_refuses_to_overwrite` を書く。
> 2. **メニューの `Simulation > Load Result…`**: メニューバーにも `request_load_result_dialog()` を呼ぶ項目を足す(今は Tools リボンのアイコンだけ)。
> 3. **実 OMC での確認**: `omola simulate` で作った `_res.mat` を Load Result で開き、プロットできることを確認してスクリーンショットを PR に付ける(見直しの PR では OMC の無い環境だったため未確認)。
> 4. kept の結果のヘッダ名の後ろに `(kept)` を付ける(今はアイコンの色とツールチップだけ)。
>
> 以下は元の本文です(経緯として残す)。

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
