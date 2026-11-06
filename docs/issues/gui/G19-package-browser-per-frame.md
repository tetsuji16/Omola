# G19: Package Browser の毎フレームの無駄をなくす(文字列複製・検索・全行描画)

- 難易度: 中 / 依存: G18 / 触るファイル: `crates/omola-gui/src/ui/browsers.rs`, `src/app.rs`
- 先に `docs/issues/gui/README.md` を読むこと。親 issue: #60。

## 背景(現在のコード)

`ui/browsers.rs`:
- `tree_node()` は引数に `(String, String, String)`(名前・restriction・comment)を**値で**受け取り、`app.rs` の `children_of()` が子ごとに 3 つの `String` を `clone()` した `Vec` を毎フレーム作っています。
- 展開されていないノードについても、展開可能かどうかを調べるために `children_of(&name)`(子の Vec の生成)を呼んでいます。
- `package_search()` は検索文字列がある間、**毎フレーム全クラスに `to_lowercase()`** をかけて一致を探し、結果を全件 `ui` に描きます。

## 実装手順

### A. 木表示(`tree_node`)

1. `tree_node` の引数を参照に変える:
   ```rust
   fn tree_node(&self, ui: &mut egui::Ui, depth: usize, name: &str, restriction: &str, comment: &str, actions: &mut Vec<(String, RowAction)>)
   ```
2. 展開可能かどうかは `has_children` 相当で判定する。`app.rs` に追加:
   ```rust
   pub fn has_children_of(&self, class: &str) -> bool  // user_classes は従来どおり走査(件数が少ない)、ライブラリは G18 の has_children
   ```
3. **展開されているときだけ**子を列挙する。ライブラリのクラスは G18 の `children_iter` を使い、`String` を作らずに `&str` で再帰する。
   - `self` の借用の都合で再帰中に `self` を可変借用できない点に注意(現在も `actions` に積んで後で `apply_tree_actions` しているので、その方式のまま)。
4. `tooltip` の `format!` は、行に**ホバーしたときだけ**作るようにする(`tree_row` 内で `response.on_hover_text_at_pointer(...)` に渡している箇所を `response.on_hover_ui(|ui| { ui.label(...) })` 形式にすれば、ホバー時だけクロージャが実行される)。`tree_row` のシグネチャ変更が必要なら `tooltip: &dyn Fn() -> String` を渡す形でよい。

### B. 検索(`package_search`)

1. `OmolaApp` にキャッシュを追加:
   ```rust
   /// (query, matching qualified names) of the last package search.
   pub package_search_cache: Option<(String, Vec<String>)>,
   ```
2. `package_filter.trim().to_lowercase()` がキャッシュの query と違うときだけ再計算する。再計算は最大 500 件で打ち切り、500 件を超えたら最後に `"... more results, refine the search"` の灰色ラベルを出す。
3. 結果の描画は `egui::ScrollArea::vertical().show_rows(ui, row_height, results.len(), |ui, range| { ... })` を使い、見えている行だけ描く。`row_height` は既存の行の高さ定数(`browsers.rs` の tree_row で使っている高さ)に合わせる。
4. ライブラリが読み込み終わった(`Event` で `LibraryState::Loaded` になった)ときと、ユーザーファイルの outline が変わったとき(`refresh_outline`)に `package_search_cache = None` にする。

## テスト(`app.rs` の tests)

- `has_children_of` がユーザークラスとライブラリクラスの両方で正しい。
- 検索キャッシュ: 同じ query で 2 回呼んでも再計算されない(再計算関数を `fn search_classes(&self, q: &str) -> Vec<String>` として切り出し、呼び出し回数をテスト用カウンタで数えるか、キャッシュの `Vec` のポインタが変わらないことを確認)。
- 500 件で打ち切られる。

## 受け入れ条件

- [ ] 表示と操作(展開・ダブルクリックで開く・中クリック/新しいタブ・戻る/進む)が変更前と同じ(スクリーンショット 1 枚)。
- [ ] G16 の操作 2 の avg/max を変更前後で PR に表で示す(G18 の後の値と比較)。
- [ ] 検索欄に `pid` と入れたときの avg/max も示す。
- [ ] テストが通る。README 共通コマンド 5 つが成功。

## やってはいけないこと

- 行の見た目(高さ・アイコン・インデント幅)を変えない。
