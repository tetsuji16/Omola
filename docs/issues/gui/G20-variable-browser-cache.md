# G20: Variable Browser の木を毎フレーム作り直さない

- 難易度: 中 / 依存: なし / 触るファイル: `crates/omola-gui/src/ui/browsers.rs`, `src/app.rs`, `src/var_tree.rs`
- 先に `docs/issues/gui/README.md` を読むこと。親 issue: #60。

## 背景(現在のコード)

`ui/browsers.rs` の Variable Browser(`variable_order(self, run)` の直後、640 行付近)は、**毎フレーム**・**結果ごとに**:
1. 全変数を走査して表示対象 `names: Vec<(usize, &str)>` を作り、フィルタ時は全変数名に `to_lowercase()` をかけ、
2. `crate::var_tree::build_variable_tree(&names)` で木を一から作り直しています。

MSL の例題でも変数は数百〜数千あり、結果を複数保持すると比例して重くなります。

## 実装手順

1. `app.rs` にキャッシュ型を追加:
   ```rust
   /// Variable tree of one result for one set of browser filters.
   pub struct VarTreeCache {
       pub key: VarTreeKey,
       pub tree: crate::var_tree::VarNode,
       /// Matching variable indices when a text filter is active (flat list).
       pub flat: Vec<usize>,
   }
   #[derive(Debug, Clone, PartialEq, Eq)]
   pub struct VarTreeKey {
       pub run_id: u64,
       pub filter: String,          // 小文字化済み
       pub show_parameters: bool,
       pub show_time_varying: bool,
   }
   ```
   `OmolaApp` に `pub var_tree_cache: BTreeMap<u64, VarTreeCache>`(キーは run id)を追加。
2. `fn var_tree_for(&mut self, run_id: u64) -> &VarTreeCache` を `app.rs` に作る。現在のキーと一致すればそれを返し、違えば今 `browsers.rs` で行っている計算(`variable_order` → フィルタ → `build_variable_tree`)をそのまま移して作り直す。
   - `build_variable_tree` は `&[(usize, &str)]` を受け取る。キャッシュの `VarNode` は `String` を持つので借用問題は無い。
3. `browsers.rs` の描画側は `var_tree_for` の結果を使うだけにする。
   - 借用の都合で、描画ループの前に `let ids: Vec<u64> = self.runs.iter().map(|r| r.id).collect(); for id in &ids { self.var_tree_for(*id); }` のように先に全部計算し、描画中は `self.var_tree_cache.get(&id)` を不変参照で読む。
4. フィルタ中のフラットリストは `egui::ScrollArea` の `show_rows` で見えている行だけ描く(既存のスクロール領域の中に入れ子にできない場合は、`flat.len() > 500` のときに先頭 500 件だけ描いて「... N more」を出す方式でよい)。
5. 無効化: `add_run` / 結果の削除(`runs` から消す処理すべて)で、該当 run id のキャッシュを `remove` する。
6. 展開状態(`variable_expanded`)はキャッシュに含めない(描画時に参照するだけ)ので、展開/折りたたみでは作り直さない。

## テスト(`app.rs` の tests)

- 同じ状態で `var_tree_for` を 2 回呼ぶと作り直さない(`VarTreeCache` に `pub(crate) generation: u64` を持たせ、作り直すたびに増やして確認してよい)。
- フィルタ文字列・Parameters/Time-varying トグルを変えると作り直す。
- run を削除するとキャッシュも消える。
- フィルタ `"w"` のとき flat に `der(w)` や `inertia1.w` が含まれる(既存の `var_tree.rs` テストと同じ変数名で)。

## 受け入れ条件

- [ ] 表示・チェックボックスでのプロット・Value 欄の編集・`Time:` 指定が変更前と同じ。
- [ ] G16 の操作 3 の avg/max を変更前後で PR に示す。
- [ ] テストが通る。README 共通コマンド 5 つが成功。
