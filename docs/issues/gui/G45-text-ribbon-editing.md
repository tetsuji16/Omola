# G45: Text リボンの編集ボタン(Cut / Copy / Paste・Go To Line・Replace)を動かす

- 難易度: 中 / 依存: なし(`text_editor.rs` を触る他の Issue と同時に進めない) / 触るファイル: `crates/omola-gui/src/ui/chrome.rs`(`text_ribbon`, `clipboard_group`), `src/text_editor.rs`, `src/app.rs`, `src/ui/views.rs`(`text_layer`)
- 先に `docs/issues/gui/README.md` を読むこと。

## 背景(2026-11-12 のコード全体見直しで判明)

Text リボンの Clipboard グループ、`Replace`、`Go To` は常に無効の飾りになっている。キーボード(Ctrl+C/V/X、Ctrl+F)は動くが、リボンからは何もできない。(Undo / Redo は今回の骨格で `self.undo()` / `self.redo()` に接続済み。)

## 実装手順

### A. Go To Line(最も簡単。先にこれだけで PR を出してもよい)

1. `app.rs` に `pub goto_line_dialog: Option<String>`(入力中の文字列)を足す。
2. `text_ribbon` の `Go To` ボタンを `editable || ライブラリの Text 層が表示中` のとき有効にし、押したら `goto_line_dialog = Some(String::new())`。ショートカット `Ctrl+G` も `handle_shortcuts` に足す(Text 層のときだけ)。
3. `dialogs.rs` に `goto_line_dialog_window`: 1 行の入力と `OK` / `Cancel`。数値でない・0・行数より大きいときは赤字で `Line must be between 1 and N` を出し `OK` を無効にする。
4. `OK` で、既存の「エラー行へジャンプ」(G25)と同じ仕組みで移動する。`text_editor.rs` の `line_start_char(text, line)` でカーソル位置を求め、`EditorState` にある「次のフレームでこの行へスクロールする」ための項目(G25 が足したもの。`grep -n "scroll\|jump" crates/omola-gui/src/text_editor.rs` で確認)に行番号を入れる。
5. 行番号の検証は純関数 `pub fn parse_goto_line(input: &str, line_count: usize) -> Result<usize, String>` として `text_editor.rs` に置く。

### B. Cut / Copy / Paste

1. egui の `TextEdit` はフォーカスがあるとき `egui::Event::Copy` / `Cut` / `Paste(String)` を処理する。リボンのボタンは、**次のフレームでエディタにそのイベントを渡す**方式にする: `app.rs` に `pub pending_editor_event: Option<EditorCommand>`(`enum EditorCommand { Cut, Copy, Paste }`)を足す。
2. `views.rs` の `text_layer` でエディタを描く直前に `pending_editor_event.take()` を見て、エディタにフォーカスを要求(`ui.memory_mut(|m| m.request_focus(editor_id))`)した上で `ui.ctx().input_mut(|i| i.events.push(egui::Event::Copy))` のようにイベントを積む。
3. `Paste` はクリップボードの内容が必要。`ViewportCommand::RequestPaste` を送ると、OS から `Event::Paste` が届く(egui 0.37 で使えることを `cargo doc -p egui --open` か egui のソースで確認し、使えない場合は Paste ボタンだけ無効のまま残してその旨を PR に書く)。
4. `clipboard_group(ui, delete_enabled)` を、押されたボタンを返す形(`Option<EditorCommand>` と delete の `bool`)に変える。Graphics リボンからの既存の呼び出しは、Cut/Copy/Paste を無効のままにする引数を足して挙動を変えない。
5. ライブラリのクラス(読み取り専用)では `Copy` だけ有効にする。

### C. Replace

1. 既存の検索(Ctrl+F、`grep -n "find" crates/omola-gui/src/app.rs crates/omola-gui/src/ui/views.rs` で場所を確認)に、置換文字列の入力欄と `Replace` / `Replace All` を足す。ユーザークラスのときだけ表示する。
2. 置換は純関数 `pub fn replace_all(text: &str, needle: &str, replacement: &str) -> (String, usize)`(大文字小文字は検索の設定に合わせる。`needle` が空なら何もしない)として `text_editor.rs` に置き、適用前に `record_undo_snapshot(doc, old_text)`、適用後に `user_text_changed(doc)` を呼ぶ(Undo で戻せること)。

## テスト

- `text_editor.rs`: `parse_goto_line_accepts_only_existing_lines`、`replace_all_counts_replacements_and_ignores_an_empty_needle`。
- `app.rs`: `replace_all_can_be_undone`(置換 → `undo()` で元のテキストに戻る)。

## 受け入れ条件

- [ ] `Go To`(と Ctrl+G)で指定した行にカーソルが移り、画面内に入る。
- [ ] リボンの Copy で選択範囲がクリップボードに入る。Cut / Paste はユーザークラスで動く(Paste が egui の制約で不可の場合は PR に理由を書き、ボタンは無効のままにする)。
- [ ] Replace / Replace All が動き、Undo で戻せる。ライブラリのクラスでは置換できない。
- [ ] 上のテストが通る。README 共通コマンド 5 つが成功。
