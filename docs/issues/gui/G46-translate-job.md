# G46: Translate を Check から分離する(翻訳だけ行うジョブ)

- 難易度: 中 / 依存: なし / 触るファイル: `crates/omola-gui/src/backend.rs`, `src/library.rs`, `src/app.rs`, `src/ui/chrome.rs`
- 先に `docs/issues/gui/README.md` を読むこと。

## 背景(2026-11-12 のコード全体見直しで判明)

Simulation リボンの `Translate` ボタンは `self.check()` を呼んでいるだけで、ツールチップにも「a separate translation step is issue G05」とあるが、G05 では実装されなかった。`checkModel` は式の数を数えるだけで、翻訳(コード生成とコンパイル)で出るエラーは Simulate するまで分からない。

## 実装手順

1. `library.rs` に `pub fn translate_script(class: &str, libraries: &[LibrarySpec], file: Option<&Path>, work_dir: &Path) -> Result<String, BackendError>` を追加する。`check_script` をコピーして、`checkModel({class})` の部分を次に変える:
   - `cd("<work_dir>");`(パスは `omola_core::escape_modelica_string` を通す)
   - `omolaBuild := buildModel({class});`
   - `print("OMOLA_BEGIN build\n" + omolaBuild[1] + "\nOMOLA_END build\n");`(成功時は実行ファイル名、失敗時は空文字列)
   - `print("OMOLA_BEGIN errors\n" + getErrorString() + "\nOMOLA_END errors\n");`
   - クラス名は既存の `check_script` と同じ検証関数を通す(スクリプトへの文字列の埋め込みで新しい穴を作らない)。
2. `backend.rs`:
   - `Job::Translate { class: String, context: ClassContext }` を追加し、`fail_job` の何もしない腕に足す。
   - `run_job` の `Job::Check` の処理を手本に、`with_user_file` → `translate_script` → `omc.run_query_script`。`library::parse_sections` で `build` と `errors` を取り出し、`build` が空でなければ成功。
   - 成功: `Event::Log(LogTab::Translation, LogLevel::Info, "Translation of <class> succeeded.\n<errors があれば警告として続ける>")`。失敗: `LogLevel::Error` で `errors` を出す。新しい `Event` は不要(ログだけ)。
   - 取り消し(Stop)は既存の `with_cancel_flag` がそのまま効く。`BackendError::Cancelled` のときは `"Translation stopped."` を Warning で出す。
3. `app.rs` に `pub fn translate(&mut self)` を追加(`check()` を手本に。package は `simulate()` と同じ文面で断る。`log_tab = LogTab::Translation`、`start_error_scan()` を呼ぶ)。
4. `chrome.rs` の `Translate` ボタンを `self.translate()` に変え、ツールチップを `"Translate (build the model without simulating)"` にする。
5. `scripts/fake-omc.sh` が `buildModel` を含むスクリプトにも応答するようにする(既存の simulate 用の分岐を見て、`OMOLA_BEGIN build` を含む出力を返す分岐を足す)。

## テスト

- `library.rs`: `translate_script_builds_in_the_work_directory`(生成されたスクリプトに `buildModel(M)` と `cd(` が含まれ、ユーザーファイルがあるとき `loadFile` が含まれる。不正なクラス名は `Err`)。
- `backend.rs`(`#[cfg(unix)]` の fake-omc テストの並び): `translate_job_reports_success_through_fake_omc`。
- `backend.rs`(`#[ignore]`、実 OMC): `translate_bouncing_ball_with_real_omc`。CI の `omc-integration` ジョブで実行されるよう、`.github/workflows/rust.yml` の `cargo test -p omola-gui -- --ignored ...` の行にテスト名を足す。

## 受け入れ条件

- [ ] `Translate` を押すと Translation タブに成功/失敗が出る。結果(Variable Browser)は増えない。
- [ ] 翻訳エラーのあるモデルで、エラー行へのジャンプ(G25)が Check と同じように効く。
- [ ] Stop で翻訳を中断できる。
- [ ] 上のテストが通る(実 OMC のテストは CI の omc-integration で通る)。README 共通コマンド 5 つが成功。
