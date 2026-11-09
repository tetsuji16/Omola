# G40: ライブラリのクラスの Text 層を、行番号・色分け付きの読み取り専用エディタで表示する

- 難易度: 易〜中 / 依存: G24(Modelica トークナイザ。先にマージされていれば使う) / 触るファイル: `crates/omola-gui/src/ui/views.rs`, `src/text_editor.rs`
- 先に `docs/issues/gui/README.md` を読むこと。

## 現状(2026-11-09 メンテナ報告)

MSL の `Modelica.Blocks.Examples.PID_Controller` を Text 層で開くと、

- **行番号が無い**、**色分けが無い**(すべて黒)
- 長い行が**折り返され**、`annotation(` の続きが次の行頭に来て読みにくい

ユーザーモデル(`.mo` ファイル)は `text_editor.rs` の `draw_text_editor`(行番号・色分けあり)で描いているが、ライブラリのクラス(読み取り専用)は `views.rs` の `text_layer()` の `Some(Loadable::Ready(text))` 分岐で素の `egui::TextEdit::multiline(...).code_editor()` を使っているため。

## 実装手順

1. `draw_text_editor` に**読み取り専用モード**を足す(引数か `EditorState` のフラグ)。読み取り専用では:
   - テキストは変更できない(`&mut &str` を渡す、または `interactive(false)` ではなく選択・コピーはできる形にする)
   - 行番号・色分け・検索(Find)・エラー行ハイライトは通常どおり
2. `text_layer()` のライブラリ分岐を `draw_text_editor` の読み取り専用モードに置き換える。
3. 折り返しはしない(横スクロール)。ユーザーモデルの表示と同じにする。
4. 大きなクラス(`Modelica.Media` の一部は数千行)でも重くならないこと: 色分けは G24 の仕組み(キャッシュ)をそのまま使う。G16 の計測でフレーム時間を PR に書く。

## 受け入れ条件

- [ ] PID_Controller の Text 層のスクリーンショット: 行番号・色分けあり、折り返しなし。
- [ ] 文字を入力しても変わらない。選択と Ctrl+C はできる。
- [ ] テストが通る。README 共通コマンド 5 つが成功。
