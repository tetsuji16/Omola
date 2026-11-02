# G02: 設定・最後に開いたファイルの保存と復元

- 難易度: 易 / 依存: なし / 触るファイル: `crates/omola-gui/Cargo.toml`, `src/app.rs`, `src/backend.rs`, `src/main.rs`, `licenses/inventory.toml`(必要なら)
- 先に `docs/issues/gui/README.md` を読むこと。

## 目的

GUI を閉じて開き直したとき、次が復元されるようにします(ウィンドウ位置/サイズは eframe が自動で保存します)。

- 最後に開いていたファイルのパス(ファイルが存在する場合のみ再オープン)
- 最後のリボンタブ
- Simulation Setup の内容(`SimulationSetup` 全体)
- Variable Browser の「Keep results」の値

## 実装手順

1. `crates/omola-gui/Cargo.toml` の eframe の `features` に `"persistence"` を追加。`serde = { version = "1", features = ["derive"] }` を `[dependencies]` に追加(serde は既にワークスペースで使用中なので inventory 追加は不要なはず。`licenses check` で確認)。
2. `app.rs` に保存用の構造体を作る(`OmolaApp` 全体を serialize しない。Worker などは保存できない):
   ```rust
   #[derive(serde::Serialize, serde::Deserialize, Default)]
   #[serde(default)]
   pub struct PersistedState {
       pub last_file: Option<std::path::PathBuf>,
       pub tab: Option<String>,          // RibbonTab::label() の文字列で保存
       pub setup: Option<SimulationSetup>,
       pub keep_results: Option<usize>,
   }
   ```
   - `SimulationSetup`(backend.rs)に `#[derive(serde::Serialize, serde::Deserialize)]` と `#[serde(default)]` を付ける。
   - `RibbonTab` に `fn from_label(s: &str) -> Option<RibbonTab>` を追加(`ALL` を走査して `label()` と比較)。
3. `impl eframe::App for OmolaApp` に `fn save(&mut self, storage: &mut dyn eframe::Storage)` を実装し、`eframe::set_value(storage, "omola_gui_state", &state)` で保存する。キー名は定数 `const STATE_KEY: &str = "omola_gui_state";` にする。
4. `OmolaApp::new` で `cc.storage` が `Some` なら `eframe::get_value::<PersistedState>(storage, STATE_KEY)` を読み、値を反映する。
   - 優先順位: **コマンドライン引数のファイル > 保存された last_file > 組み込みサンプル**。
   - `last_file` が存在しない(削除された)場合は黙って無視し、Messages に `Info` で「前回のファイルが見つかりません: パス」と出す。
5. 状態変換を純粋関数にしてテストする: `fn to_persisted(&self) -> PersistedState` と `fn apply_persisted(&mut self, state: PersistedState, cli_file: Option<PathBuf>)`。
6. テストを追加(`app.rs` の tests):
   - `to_persisted` → `serde_json::to_string` → `from_str` → `apply_persisted` で setup・keep_results・tab が戻ること(serde_json は dev-dependency に追加)。
   - 存在しない last_file でもパニックせず、サンプルのままになること。
   - CLI 引数がある場合はそちらが優先されること(一時ファイルを作って確認)。

## 受け入れ条件

- [ ] 起動 → Stop time を 7 に変更 → Simulation タブ → 終了 → 再起動で、Stop time 7・Simulation タブで開く(手動確認。PR に手順と結果を書く)。
- [ ] 上記ユニットテスト 3 つが通る。
- [ ] README 共通コマンド 5 つが成功。

## やってはいけないこと

- 結果データ(`runs`)やエディタの未保存テキストを保存しない(サイズが大きく、古い結果の誤用につながる)。
- 保存形式の読み込み失敗でパニックしない(`#[serde(default)]` で欠損を許容)。
