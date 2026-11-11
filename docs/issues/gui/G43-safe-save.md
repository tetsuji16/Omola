# G43: 【データ保護】保存の安全性(上書き確認・ディスク上で変更されたファイルの検出)

- 難易度: 易〜中 / 依存: なし(G42 と同じ `app.rs` を触るので同時に進めない) / 触るファイル: `crates/omola-gui/src/model.rs`, `src/app.rs`, `src/ui/dialogs.rs`
- 先に `docs/issues/gui/README.md` を読むこと。

## 背景(2026-11-12 のコード全体見直しで判明)

1. テキスト入力の代替ダイアログ(`OMOLA_GUI_TEXT_DIALOGS=1`)の `Save as` / `Export plot as CSV` は、**既存ファイルを確認なしで上書きする**(ネイティブダイアログは OS が確認する)。
2. Omola で開いている `.mo` を別のエディタや `git checkout` が書き換えても気付かず、次の `Save` で**相手の変更を黙って消す**。

## すでに入っている骨格

- `model::write_atomic(path, text)`: 一時ファイルに書いてから rename する保存。`Document::save()` はこれを使う(途中で失敗しても元のファイルは壊れない)。テスト `write_atomic_replaces_the_file_and_leaves_no_temporary_file`。

## 実装手順

### A. 上書き確認(テキスト入力の代替ダイアログのみ)

1. `app.rs` に `pub overwrite_confirm: Option<PathDialog>` を足す(確認待ちの要求をそのまま持つ)。
2. `dialogs.rs` の `path_dialog_window` で、`PathDialog::SaveAs` / `PathDialog::ExportCsv` が送信され、`path.exists()` のときは、すぐ実行せず `overwrite_confirm = Some(dialog)` にして `path_dialog = None` にする。
3. `overwrite_dialog_window` を追加: `"<path> already exists. Replace it?"` と `Replace` / `Cancel`。`Replace` で元の処理(`documents[i].path = Some(path); self.save()` または `export_csv`)を実行する。
4. 実行部分は `fn apply_path_dialog(&mut self, dialog: PathDialog, path: PathBuf)` に切り出し、2 か所から呼ぶ。

### B. ディスク上の変更の検出

1. `model.rs` の `Document` に `pub disk_stamp: Option<(std::time::SystemTime, u64)>`(更新時刻とサイズ)を足す。`Document::open`(または `from_file` 相当の関数)と `save()` の成功時に、`std::fs::metadata(path)` から取り直す。取得できないときは `None`。
2. `Document::changed_on_disk(&self) -> bool` を追加: `path` があり `disk_stamp` が `Some` で、現在の `metadata` の (modified, len) が違うとき `true`。ファイルが無くなった場合も `true`。
3. `OmolaApp::save()`: 保存前に `changed_on_disk()` を見て、`true` なら保存せず `pub save_conflict: Option<usize>` にドキュメント番号を入れる。
4. `dialogs.rs` に `save_conflict_dialog_window`: `"<file> was changed on disk by another program."` と 3 ボタン。
   - `Overwrite`: `disk_stamp = None` にして `save()` をやり直す。
   - `Reload from disk`: ファイルを読み直して `text` を置き換え、`dirty = false`、`undo`/`redo` を空にし、`user_text_changed(doc)` 相当の再読み込み(`refresh_outline()` とキャッシュの破棄)を行う。
   - `Cancel`: 何もしない。
5. フレーム毎に `metadata` を呼ばないこと(保存時のみ調べる)。

## テスト

- `model.rs`: `changed_on_disk_detects_an_external_write`(開く → `std::fs::write` で長さの違う内容に書き換える → `true`、`save()` 後は `false`)。更新時刻の分解能に依存しないよう、**サイズが変わる内容**で書き換えること。
- `app.rs`: `save_stops_when_the_file_changed_on_disk`(`save()` 後に `save_conflict == Some(i)`、ファイル内容は外部で書いたまま)。
- `app.rs`: `text_dialog_save_as_asks_before_replacing_an_existing_file`(`apply` 前に `overwrite_confirm` が入り、ファイルは変わらない)。

## 受け入れ条件

- [ ] `OMOLA_GUI_TEXT_DIALOGS=1` で既存ファイルを Save as / Export CSV の保存先にすると確認が出る。`Cancel` でファイルは変わらない。
- [ ] 外部で書き換えたファイルを Omola から保存しようとすると確認が出て、3 つのボタンが期待どおりに動く。
- [ ] 上のテスト 3 つが通る。README 共通コマンド 5 つが成功。
