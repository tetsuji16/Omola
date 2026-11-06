# G18: ライブラリのクラス索引(class / children / has_children を O(1) に)

- 難易度: 易〜中 / 依存: なし / 触るファイル: `crates/omola-gui/src/library.rs`, `src/cache.rs`(必要な場合のみ), `src/app.rs`
- 先に `docs/issues/gui/README.md` を読むこと。親 issue: #60。

## 背景(現在のコード)

`library.rs` の `LoadedLibrary` は `classes: Vec<ClassEntry>`(MSL 4.1.0 では数千件)を持ち、次のメソッドが**毎回全件を線形探索**しています:

```rust
pub fn children(&self, parent: &str) -> Vec<&ClassEntry> { self.classes.iter().filter(|c| parent_name(&c.qualified_name) == Some(parent)).collect() }
pub fn class(&self, qualified: &str) -> Option<&ClassEntry> { self.classes.iter().find(|c| c.qualified_name == qualified) }
pub fn has_children(&self, parent: &str) -> bool { let prefix = format!("{parent}."); self.classes.iter().any(|c| c.qualified_name.starts_with(&prefix)) }
```

Package Browser(`ui/browsers.rs` の `tree_node`)は**表示中の各行ごとに毎フレーム** `children_of` → `children` を呼ぶため、「表示行数 × 全クラス数」の文字列比較が毎フレーム走ります。これが #60 の主要因の 1 つです。

## 実装手順

1. `LoadedLibrary` に索引フィールドを追加する:
   ```rust
   pub struct LoadedLibrary {
       pub spec: LibrarySpec,
       pub version: String,
       pub source_file: PathBuf,
       pub classes: Vec<ClassEntry>,
       /// qualified_name -> index into `classes`.
       by_name: std::collections::HashMap<String, usize>,
       /// children[i] = indices of the direct children of classes[i], in declaration order.
       children: Vec<Vec<usize>>,
   }
   ```
2. `impl LoadedLibrary` に `pub fn new(spec, version, source_file, classes) -> Self` を作り、その中で索引を作る:
   - `by_name`: `classes` を 1 回走査して `insert(name.clone(), i)`。
   - `children`: `vec![Vec::new(); classes.len()]` を作り、各 `i` について `parent_name(&classes[i].qualified_name)` を `by_name` で引いて、見つかれば `children[p].push(i)`。
3. **`LoadedLibrary { ... }` を構造体リテラルで作っている箇所をすべて `LoadedLibrary::new(...)` に置き換える。** `grep -rn "LoadedLibrary {" crates/omola-gui/src` で探す(`library.rs`, `cache.rs`, `app.rs`, テスト)。フィールドが private になるので、置き換え漏れはコンパイルエラーで分かる。
   - `cache.rs` のディスク形式(`Serialize` する構造体)は**変えない**。読み込み後に `LoadedLibrary::new` を呼ぶだけにする。キャッシュの互換性を壊さないこと。
4. メソッドを索引版に書き換える(シグネチャは変えない):
   ```rust
   pub fn class(&self, q: &str) -> Option<&ClassEntry> { self.by_name.get(q).map(|&i| &self.classes[i]) }
   pub fn children(&self, parent: &str) -> Vec<&ClassEntry> {
       self.by_name.get(parent).map(|&p| self.children[p].iter().map(|&i| &self.classes[i]).collect()).unwrap_or_default()
   }
   pub fn has_children(&self, parent: &str) -> bool { self.by_name.get(parent).is_some_and(|&p| !self.children[p].is_empty()) }
   ```
   さらに、割り当てなしで回せる版を追加する(G19 で使う):
   ```rust
   pub fn children_iter<'a>(&'a self, parent: &str) -> impl Iterator<Item = &'a ClassEntry> + 'a
   ```
5. 注意: 現在の `has_children` は「`parent.` で始まる名前が 1 つでもあるか」です。中間のクラスが `classes` に無い場合(例: `A.B.C` はあるが `A.B` が無い)に結果が変わる可能性があります。`classes` は `getClassNames(recursive=true)` 由来で中間クラスは必ず含まれる前提ですが、**既存テスト(`library.rs` 末尾の `has_children` のテスト)が全部通ること**で確認してください。

## テスト(`library.rs` の tests に追加)

- 5 クラス程度の小さな `LoadedLibrary::new` を作り、`class`, `children`(順序が宣言順であること), `has_children`, `children_iter` を確認。
- 存在しない名前で `class` → `None`、`children` → 空。
- 親が `classes` に無い子は、どの `children` にも出ない(パニックしない)。

## 受け入れ条件

- [ ] 既存テストを含めて `cargo test --workspace` が通る。
- [ ] キャッシュ済みの状態で起動しても Package Browser が今までどおり表示される(スクリーンショット 1 枚)。
- [ ] G16 がマージ済みなら、G16 の操作 2 の avg/max を変更前後で PR に示す。
- [ ] README 共通コマンド 5 つが成功。

## やってはいけないこと

- `cache.rs` のファイル形式やキャッシュのバージョン番号を変えない。
- UI のコード(`ui/browsers.rs`)はこの issue では変えない(G19 で行う)。
