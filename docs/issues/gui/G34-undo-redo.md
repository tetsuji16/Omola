# G34: モデル編集の Undo / Redo(テキストのスナップショット方式)

- 難易度: 易〜中 / 依存: G28 / 触るファイル: `crates/omola-gui/src/model.rs`, `src/app.rs`, `src/ui/chrome.rs`
- 先に `docs/issues/gui/README.md` を読むこと。

## 目的

Diagram 層の編集(G29〜G33)を `Ctrl+Z` / `Ctrl+Y`(`Ctrl+Shift+Z` も)で元に戻す・やり直す。Graphics リボンの既存の無効ボタン `Undo` / `Redo`(`chrome.rs` の `small_button(ui, ICON_UNDO, DARK, "Undo", false, false)`)を有効にする。

Text 層の中での文字入力の Undo は egui の TextEdit が持っているので、この issue では**扱わない**(Text 層にフォーカスがあるときは Ctrl+Z を TextEdit に任せる)。

## 実装手順

1. `model.rs` の `Document` に履歴を追加:
   ```rust
   pub undo: Vec<String>,   // 編集前のテキスト全体
   pub redo: Vec<String>,
   ```
   上限 100 件(超えたら古いものから捨てる)。
2. `app.rs` の `Event::ModelEdited` で**テキストを置き換える直前に** `undo.push(old_text)`、`redo.clear()`。
3. `fn undo(&mut self)` / `fn redo(&mut self)`: アクティブクラスのドキュメントについて、現在のテキストを反対側に積み、取り出したテキストで置き換え、`user_text_changed(i)` と、Diagram/Icon/Component のキャッシュ破棄(G28 で作った関数を再利用)。
4. Text 層で手入力したあとに Diagram 層で Undo した場合の扱い: 手入力はスナップショットに含まれないため、Undo すると手入力も巻き戻る。これを避けるため、**Diagram 層の編集を送る直前**(`apply_edit`)にも現在テキストが undo の最後と違えば積む、という規則にする(PR に挙動を書く)。
5. ショートカット: `handle_shortcuts` に追加。Text 層かつ TextEdit にフォーカスがあるとき(`ui.ctx().memory(|m| m.focused())` が Some)は処理しない。
6. リボンの Undo/Redo ボタンは `!undo.is_empty()` / `!redo.is_empty()` で有効化。

## テスト(`app.rs`)

- 編集 2 回 → Undo 2 回で元のテキスト → Redo 1 回で 1 回目の編集後。
- Undo 後に新しい編集をすると redo が空になる。
- 101 回編集しても undo は 100 件。

## 受け入れ条件

- [ ] 部品を追加 → 移動 → Undo で移動前 → Undo で追加前 → Redo で追加後(**実 OMC** か、G28 の Event を直接入れるテストで確認。どちらか明記)。
- [ ] テストが通る。README 共通コマンド 5 つが成功。
