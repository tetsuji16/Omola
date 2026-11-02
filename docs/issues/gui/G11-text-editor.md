# G11: テキストエディタ(シンタックスハイライト・行番号・エラー行ジャンプ)

- 難易度: 中 / 依存: なし / 触るファイル: 新規 `src/highlight.rs`, 新規 `src/diagnostics.rs`, `src/ui/views.rs`, `src/ui/chrome.rs`, `src/app.rs`
- 先に `docs/issues/gui/README.md` を読むこと。

## 目的

Text 層の編集体験を改善します。

1. Modelica のシンタックスハイライト(キーワード・コメント・文字列・数値)。
2. 行番号の表示。
3. Messages に出た OMC のエラーをクリックすると、該当行にカーソルが移動する。

## 実装手順

1. 新規 `src/highlight.rs`:
   ```rust
   #[derive(Debug, Clone, Copy, PartialEq, Eq)]
   pub enum TokenKind { Keyword, Type, Comment, String, Number, Operator, Plain }
   /// バイト範囲とトークン種別の列。text 全体を隙間なく覆うこと。
   pub fn tokenize(text: &str) -> Vec<(std::ops::Range<usize>, TokenKind)>
   pub fn layout_job(text: &str, font: egui::FontId, dark: bool) -> egui::text::LayoutJob
   ```
   - キーワード(Modelica 3.6 仕様 2.3.3 の予約語): `algorithm and annotation block break class connect connector constant constrainedby der discrete each else elseif elsewhen encapsulated end enumeration equation expandable extends external false final flow for function if import impure in initial inner input loop model not operator or outer output package parameter partial protected public pure record redeclare replaceable return stream then true type when while within`。
   - Type: `Real Integer Boolean String`。
   - コメント `//` 〜行末、`/* */`(閉じなしなら末尾まで)。文字列 `"..."`(`\"` エスケープ対応、閉じなしなら末尾まで)。数値 `1`, `1.5`, `1e-3`, `2.0E+5`。
   - **マルチバイト文字(日本語コメント等)で範囲がずれないよう**、`char_indices()` で走査しバイト境界を守る。
   - 色: dark/light で 2 セット。既存の `model.rs` の `tokenize` とは別物(あちらはクラス一覧用)。
2. `views.rs` の TextEdit にレイアウタを付ける。egui 0.37 での書き方は **`~/.cargo/registry/src/*/egui-0.37.0/src/widgets/text_edit/builder.rs` の `layouter` の doc コメントの例をそのまま真似る**(シグネチャが版ごとに変わるため推測しない)。
   - 毎フレーム全文をトークナイズすると重いので、`ui.ctx().memory_mut(|m| m.caches.cache::<...>())` の `ComputerMut` キャッシュを使うか、`(text のハッシュ, dark)` をキーに `LayoutJob` を `OmolaApp` に保持する。
3. 行番号: TextEdit の左に、同じ monospace フォント・同じ行高さで `1..=行数` を並べた `ui.label` を置く(`ui.horizontal_top` で並べ、両方を同じ `ScrollArea` の中に入れて一緒にスクロールさせる)。折り返しは無効(`desired_width(f32::INFINITY)` 済み)なので行の高さは一致する。
4. 新規 `src/diagnostics.rs`:
   ```rust
   pub struct Diagnostic { pub file_name: String, pub line: usize, pub column: usize, pub message: String }
   /// OMC の "[/path/File.mo:12:3-12:20:writable] Error: ..." 形式を拾う
   pub fn parse_omc_diagnostics(output: &str) -> Vec<Diagnostic>
   ```
   - `[` の後、最後の `.mo:` の前をパス、その後ろの `行:列` を取る。パスからはファイル名だけ残す(GUI は run ディレクトリのコピーを OMC に渡しているので、パスは一致しない。`<クラス名を _ に置換>.mo` になっている)。
   - **実際の OMC エラー出力を開発コンテナで 1 つ取得し(`tests/compat/fixtures/InvalidSyntax.mo` を check)、テストのフィクスチャ文字列に使う。**
5. `app.rs`: ログを `Vec<(LogLevel, String)>` から `Vec<LogEntry { level, text, diagnostics: Vec<Diagnostic> }>` に変える(Error ログ追加時に `parse_omc_diagnostics` を走らせる)。
6. `chrome.rs` の Messages: diagnostics があるエントリでは、各 diagnostic を `ui.link(format!("line {}:{}", ..))` で表示。クリックで `OmolaApp.goto_line = Some((line, column))` をセットし、Text タブに切り替える。
7. `views.rs`: `goto_line` があれば TextEdit の `TextEditState` を読み出してカーソル位置(文字インデックス)を設定し、`goto_line = None`。カーソル設定の API は `egui-0.37.0/src/widgets/text_edit/state.rs` を grep して確認(`set_ccursor_range` 等)。行・列 → 文字インデックスの変換は純粋関数にしてテスト。
8. テスト:
   - `tokenize`: キーワード・コメント・文字列内のキーワード(色付けしない)・日本語コメント・数値の各ケース。範囲が text 全体を隙間なく覆うこと。
   - `parse_omc_diagnostics`: 実フィクスチャで行・列が正しい。
   - 行・列 → 文字インデックス(1 始まり、日本語を含む行)。

## 受け入れ条件

- [ ] BouncingBall がキーワード・コメント・文字列で色分けされる(dark/light 両方のスクリーンショット)。
- [ ] わざと構文エラーを入れて Check → Messages の `line N:M` をクリック → その行にカーソルが移動。
- [ ] 1 万行のファイルでも入力がもたつかない(手動確認、PR に書く)。
- [ ] テストが通る。README 共通コマンド 5 つが成功。
