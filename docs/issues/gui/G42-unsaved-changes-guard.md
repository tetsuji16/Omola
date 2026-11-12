# G42: 【データ保護】未保存の変更の確認を完成させる(保存して終了・ドキュメントを閉じる)

- 難易度: 中 / 依存: なし / 触るファイル: `crates/omola-gui/src/app.rs`, `src/ui/dialogs.rs`, `src/ui/views.rs`(`model_tabs_strip`), `src/ui/chrome.rs`(File メニュー)
- 先に `docs/issues/gui/README.md` を読むこと。

## 背景(2026-11-12 のコード全体見直しで判明)

編集中の `.mo` を保存せずにウィンドウを閉じる・`File > Exit` を押すと、**確認なしで変更が失われていた**。また、一度開いたドキュメントは `self.documents` に残り続け、閉じる方法がない(タブの × は `model_tabs` から外すだけ)。

## すでに入っている骨格(このIssueでは作り直さない)

- `OmolaApp::unsaved_documents()`: 未保存ドキュメントのタイトル一覧。
- `OmolaApp::must_confirm_exit()`: 閉じる要求を止めるべきか。
- `eframe::App::ui` の冒頭: `close_requested()` かつ `must_confirm_exit()` のとき `ViewportCommand::CancelClose` を送り `confirm_exit = true`。
- `dialogs.rs` の `exit_dialog_window`: 「Unsaved changes」ウィンドウ(`Discard and exit` / `Cancel` の 2 ボタン)。
- テスト `unsaved_documents_lists_only_dirty_documents`。

## 実装手順

1. **Save all and exit**: `exit_dialog_window` に `Save all and exit` ボタンを足す(左端、既定の操作)。
   - `OmolaApp::save_all(&mut self) -> Result<(), Vec<String>>` を `app.rs` に追加する。`path` がある dirty なドキュメントを順に `Document::save()` し、失敗したタイトルを集めて返す。成功したものは `push_recent` する。
   - `path` が `None`(未命名)の dirty ドキュメントが 1 つでもあるときは、ボタンを無効にし `on_disabled_hover_text("Save unnamed documents with Save As first")` を付ける(終了フローの中で Save As ダイアログを連鎖させない。単純さを優先)。
   - 全て成功したら `exit_confirmed = true` にして `ViewportCommand::Close` を送る。1 つでも失敗したらダイアログは閉じず、`self.error(LogTab::Syntax, ...)` に失敗したタイトルを出す。
2. **ドキュメントを閉じる**: `OmolaApp::close_document(&mut self, doc: usize)` を追加する。
   - そのドキュメントに属するクラス(`user_classes` と `user_class_doc` から求める)を `model_tabs`・`history`・`model_diagrams`・`icons`・`docs`・`texts` から取り除き、`documents.remove(doc)` の後に `refresh_outline()` を呼ぶ。`active` がそのクラスだった場合は `model_tabs.last()` に移す。
   - Package Browser のユーザークラスの最上位の行(ファイル単位)を右クリック → `Close`、および `File > Close` から呼ぶ。
3. **dirty なドキュメントを閉じるときの確認**: `pub pending_close: Option<usize>` を `OmolaApp` に足し、dirty なら `close_document` の代わりに `pending_close = Some(doc)` にする。`dialogs.rs` に `close_dialog_window` を作り、`Save` / `Discard` / `Cancel` を出す(`Save` は `path` が無ければ無効)。
4. タブの × (`close_tab`) の挙動は変えない(タブを閉じてもドキュメントは残るので、データは失われない)。ツールチップを `Close tab (the document stays open)` にする。
5. `README.md` の GUI 節に「未保存の変更があると終了時に確認する」旨を 1 行追記する。

## テスト(`app.rs` の `tests`)

- `save_all_writes_every_dirty_document_with_a_path`: 一時フォルダに 2 ファイルを開き、両方を dirty にして `save_all()` → 両方 `dirty == false`、ファイル内容が更新されている。
- `save_all_reports_documents_that_cannot_be_written`: 存在しないフォルダを `path` に持つドキュメント → `Err` にそのタイトルが入り、`dirty` は `true` のまま。
- `close_document_removes_its_classes_and_tabs`: 2 つのドキュメントを開き、片方を閉じる → `documents.len() == 1`、閉じた側のクラスが `model_tabs` と `user_classes` に無い、`active` が残った側を指す。
- `closing_a_dirty_document_asks_first`: dirty なドキュメントで閉じる要求 → `pending_close == Some(i)` で `documents.len()` は変わらない。

## 受け入れ条件

- [ ] 未保存の変更がある状態でウィンドウの × / `File > Exit` を押すと「Unsaved changes」が出て、`Save all and exit` / `Discard and exit` / `Cancel` がそれぞれ期待どおりに動く(手動確認の手順とスクリーンショットを PR に書く)。
- [ ] 未保存の変更が無いときは、確認なしで終了する。
- [ ] ドキュメントを閉じられる。dirty なら確認が出る。
- [ ] 上のテスト 4 つが通る。README 共通コマンド 5 つが成功。
