# G23: 【不具合・最優先】Text 層でユーザーモデルが編集できない(G11 の回帰)を直す

- 難易度: 中 / 依存: なし / 触るファイル: `crates/omola-gui/src/text_editor.rs`, `src/ui/views.rs`, `src/model.rs`
- 先に `docs/issues/gui/README.md` を読むこと。親 issue: #60(重さの原因の 1 つでもある)。

## 不具合の内容

PR #59(G11 相当のテキストエディタ)以降:
- `text_editor.rs` の `draw_text_editor()` は `Painter` で文字を**描くだけ**で、キー入力を受け付けません。最後は常に `false` を返します(コメント `// For now, we don't detect text changes here`)。
  → **ユーザーが開いた .mo / Unnamed モデルの Modelica Text が編集できない**。Save・Check・Simulate はできても中身を直せない。
- 描画領域の高さが `available_height` 固定なので、画面より長いファイルは下までスクロールできない。
- 毎フレーム全行を syntect でハイライトしている(重い)。
- 配色が暗色テーマ `base16-ocean.dark` のまま白背景に描いているため、文字が読みにくい。

## 方針

egui 標準の `egui::TextEdit::multiline` を使い、色分けは **`layouter`**(`LayoutJob` を返す関数)で行います。カーソル・選択・コピー&ペースト・IME・Undo は TextEdit が提供します。ハイライト結果はテキストが変わったときだけ作り直してキャッシュします。

## 実装手順

1. `text_editor.rs` の `draw_text_editor` のシグネチャを変える:
   ```rust
   pub fn draw_text_editor(ui: &mut Ui, editor_state: &mut EditorState, text: &mut String, path: Option<&PathBuf>, error_lines: &[usize]) -> bool
   ```
   (`&mut str` → `&mut String`。`views.rs` の呼び出し側は `&mut doc.text` のままで通る)
2. ハイライトのキャッシュ:
   - `EditorState` に `cache_text: String`, `cache_job: Option<egui::text::LayoutJob>` を追加。
   - 関数 `fn highlight_job(text: &str, font: &FontId) -> LayoutJob` を作る。中身は既存の `Highlighter`(syntect)を使い、**`HighlightLines` を 1 つだけ作って全行を順に処理**する(行ごとに作ると複数行コメントの状態が切れる)。各トークンを `job.append(token, 0.0, TextFormat { font_id: font.clone(), color, ..Default::default() })` で追加し、行末の `"\n"` も append する。
   - テーマは `InspiredGitHub`(syntect 同梱の明るいテーマ)を優先し、無ければ `base16-ocean.light`、それも無ければ最初のテーマ。
3. TextEdit を出す:
   ```rust
   let mut layouter = |ui: &egui::Ui, buf: &dyn egui::TextBuffer, wrap_width: f32| {
       let s = buf.as_str();
       if editor_state.cache_job.is_none() || editor_state.cache_text != s {
           editor_state.cache_text = s.to_owned();
           editor_state.cache_job = Some(highlight_job(s, &font_id));
       }
       let mut job = editor_state.cache_job.clone().unwrap();
       job.wrap.max_width = f32::INFINITY; // 折り返さない
       ui.fonts_mut(|f| f.layout_job(job))
   };
   ```
   - **egui 0.37 の layouter の正確な型は `~/.cargo/registry/src/*/egui-0.37.0/src/widgets/text_edit/builder.rs` の `pub fn layouter` を grep して確認すること。** 上の引数型は目安です(`&dyn TextBuffer` か `&str` かはバージョンで違う)。
   - `TextEdit::multiline(text).code_editor().font(font_id.clone()).desired_width(f32::INFINITY).desired_rows(..).frame(egui::Frame::NONE).layouter(&mut layouter)` を `ScrollArea::both().auto_shrink([false,false])` の中に置く。
   - `let response = ui.add(...)`。戻り値は `response.changed()`。
   - キャッシュ更新で `editor_state` をクロージャ内で可変借用するため、TextEdit に `editor_state` 以外を渡す形にする(借用エラーが出たら、キャッシュを `editor_state` から一時的に `std::mem::take` してクロージャに move し、後で戻す)。
4. 行番号: ScrollArea の中で `ui.horizontal_top` を使い、左に行番号の列(幅 = 桁数+2 文字)、右に TextEdit を置く。行番号は `Painter` で描いてよいが、**見えている範囲だけ**描く(`ui.clip_rect()` と行の高さから開始・終了行を計算)。行の高さは `ui.fonts_mut(|f| f.row_height(&font_id))`。
5. エラー行と F7/F8 のジャンプ:
   - エラー行の背景(薄い赤)は、TextEdit の `response.rect` を基準に `line_index * row_height` で矩形を出して `ui.painter()` で塗る(**TextEdit より先に描く**ため、`ui.painter().add(Shape::Noop)` で場所を確保し後で `set` する egui の定番手法を使う。分からなければ行番号の列だけ赤くするのでもよい)。
   - F8/F7 で `highlight_line` が変わったら、その行が見えるように `ui.scroll_to_rect(rect, Some(egui::Align::Center))` を呼ぶ。カーソルも移す: `TextEditState::load(ui.ctx(), id)` → `state.cursor.set_char_range(Some(CCursorRange::one(CCursor::new(行頭の文字位置))))` → `state.store(...)`。API 名は egui 0.37 のソースで確認。
6. `views.rs` の `text_layer`: 戻り値 `changed` が true なら既存どおり `self.user_text_changed(i)` を呼ぶ(既にあるコード)。
7. 不要になった `Painter` 描画のコード(全行ループ)は削除する。

## テスト

GUI のキー入力はユニットテストしにくいので、次を純粋関数テストにする(`text_editor.rs`):
- `highlight_job("model A\n  // c\nend A;\n", &font)` の `job.text` が元の文字列と完全に一致する(色分けで文字が欠けない・増えない)。
- 複数行コメント `/* a\n b */ x` で 2 行目の ` b */` がコメント色になる(sections の color を比較)。
- 行頭の文字位置を返す `fn line_start_char(text: &str, line: usize) -> usize`(1 始まり)を作り、テストする(日本語コメントを含む行で、バイト位置でなく文字位置を返すこと)。

## 受け入れ条件

- [ ] `examples/BouncingBall.mo` を開き、Text 層で文字を入力・削除・コピー&ペースト・Ctrl+Z ができる。タイトルに未保存マーク(既存の dirty 表示)が付き、Ctrl+S で保存される(手順と結果を PR に書く。スクリーンショット 1 枚)。
- [ ] 編集後に Check すると、編集後のテキストが OMC に渡る(Messages の内容で確認)。
- [ ] 1000 行以上のファイルが最後までスクロールできる。
- [ ] 日本語のコメントを入力できる(Windows の IME で確認できればなお良い。できなければ未確認と明記)。
- [ ] G16 の操作 4 の avg/max を PR に示す。
- [ ] テストが通る。README 共通コマンド 5 つが成功。

## やってはいけないこと

- 新しいエディタ用クレート(`egui_code_editor` 等)を追加しない。syntect は既存の依存なので使ってよい。
- ライブラリクラス(読み取り専用)の表示部分(`texts` を `TextEdit::multiline(&mut text_as_str)` で出している部分)は、この issue では変えない。
