# G25: 【不具合】OMC のエラー行の抽出を直し、Messages からエラー行へジャンプする

- 難易度: 中 / 依存: G23 / 触るファイル: `crates/omola-gui/src/text_editor.rs`, `src/ui/views.rs`, `src/ui/bottom.rs`, `src/app.rs`
- 先に `docs/issues/gui/README.md` を読むこと。

## 不具合の内容(現在のコード)

1. `views.rs` の `text_layer` は `self.logs.iter().find(|l| l.tab == LogTab::Simulation)` で **Simulation タブの最初の 1 件だけ**からエラー行を探しています。Check の結果は `LogTab::Syntax`、翻訳エラーは `LogTab::Translation` に出るので、Check で見つかったエラーが行番号に反映されません。
2. `extract_error_lines` は `file.mo:123:4: error:` 形式を想定していますが、OMC 1.27 の実際の形式は
   ```
   [/home/u/BouncingBall.mo:12:3-12:20:writable] Error: ...
   [C:\Users\u\BouncingBall.mo:12:3-12:20:writable] Error: ...
   ```
   です。Windows のパスはドライブレター `C:` の `:` で最初に区切られてしまい、正しく取れません。
3. G11 の目的にあった「Messages のエラーをクリックすると該当行へ移動」が未実装です。

## 実装手順

1. `text_editor.rs` に新しい純粋関数を作る(既存の `extract_error_lines` は削除して置き換え、テストも書き換える):
   ```rust
   #[derive(Debug, Clone, PartialEq, Eq)]
   pub struct SourceLocation { pub file: String, pub line: usize, pub column: usize }
   /// Find every `[<file>:<line>:<col>-<line>:<col>:<writable|readonly>]` in `text`.
   pub fn parse_omc_locations(text: &str) -> Vec<SourceLocation>
   ```
   - 解析方法: `[` を探し、対応する `]` までを取り出す。中身を**右から** `:` で分割する(`rsplitn(4, ':')` を使うと `[writable, "12:20" ではなく…]` となるので、次の手順で行う):
     1. 末尾の `:writable` / `:readonly` を取り除く(無ければその括弧は無視)。
     2. 残りを `-` で**最後の 1 回だけ**分ける(`rsplit_once('-')`)。左側 `"<file>:<line>:<col>"` を `rsplitn(3, ':')` で `[col, line, file]` に分ける。
     3. line/col が数値でなければ無視。
   - こうすると `C:\Users\...` の `:` は file 側に残る。
   - 旧形式 `file.mo:123:4: error:` は対応しなくてよい(OMC はこの形式を出さない)。
2. 対象ファイルとの照合関数:
   ```rust
   pub fn lines_for_file(locs: &[SourceLocation], path: Option<&Path>) -> Vec<usize>
   ```
   - `path` が `None`(Unnamed など未保存)のときは、file が `<interactive>` または `.mo` で終わらないものを対象にする(OMC は loadString したテキストを `<interactive>` と報告する。**実際の表記を `omc` で確認して PR に貼る**こと: `echo 'loadString("model A Real x; equation x = ; end A;"); getErrorString();' > t.mos && omc t.mos`)。
   - `path` があるときは、両方を `std::path::Path::new` にして `file_name()` が一致するもの。大文字小文字は Windows でも区別してよい。
   - 結果は昇順・重複なし。
3. `app.rs`: キャッシュを追加する(毎フレーム全ログを解析しないため):
   ```rust
   pub error_locations: Vec<SourceLocation>,   // 全ログからの抽出結果
   error_locations_log_len: usize,             // 解析済みの logs.len()
   ```
   `fn refresh_error_locations(&mut self)` で、`logs.len()` が変わったときだけ `Syntax` / `Translation` / `Simulation` タブの**全エントリ**を解析し直す。毎フレーム `ui()` の先頭(`handle_events` の後)で呼ぶ。ログを Clear したら(G22)`error_locations_log_len = usize::MAX` にして次フレームで作り直す。
   - 新しい Check / Simulate を開始したときは古い結果を対象外にしたいので、`check()` と `simulate()` の開始時に `self.error_from_log_index = self.logs.len()` を記録し、それ以降のエントリだけを解析する。
4. `views.rs` の `text_layer` は `lines_for_file(&self.error_locations, doc.path.as_deref())` の結果を `draw_text_editor` に渡す。
5. `bottom.rs` のログ表示: エントリのテキストに `SourceLocation` が含まれる場合、その行を `ui.add(egui::Label::new(...).sense(egui::Sense::click()))` にしてクリック可能にし、ホバー時にカーソルを手の形にする(`on_hover_cursor(egui::CursorIcon::PointingHand)`)。クリックで:
   - `file` に対応する開いているドキュメントを探す(`documents[i].path` の `file_name` 一致。`<interactive>` なら Unnamed)。
   - 見つかれば、そのドキュメントの最初のクラスを `open_class` し、Text 層に切り替え(`select_tab(RibbonTab::Text)`)、`documents[i].editor.highlight_line = Some(line)`。G23 の実装でカーソル移動・スクロールが行われる。
   - 見つからなければ Messages に `Info` で `"<file> is not open"`。

## テスト(`text_editor.rs`)

- `[/home/u/A.mo:12:3-12:20:writable] Error: x` → file `/home/u/A.mo`, line 12, col 3。
- `[C:\Users\u\A.mo:7:1-7:5:writable] Error` → file `C:\Users\u\A.mo`, line 7。
- 1 つのテキストに 3 件 → 3 件。`[` だけ・`]` の無い文字列・数字でない行番号でパニックしない。
- `[x:1:1-1:2:readonly]` も拾う。`[not a location]` は拾わない。
- `lines_for_file`: ファイル名一致・不一致、`None` + `<interactive>`。

## 受け入れ条件

- [ ] `examples/BouncingBall.mo` に構文エラーを入れて Check → Text 層の該当行が赤くなり、Messages の行をクリックするとその行に移動する(**実 OMC で確認**。fake-omc では確認できないので、無理なら単体テストのみと明記)。
- [ ] Windows のパスでも正しく動く(単体テストで可)。
- [ ] テストが通る。README 共通コマンド 5 つが成功。
