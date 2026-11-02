# G04: MSL などのライブラリ読み込みと Package Browser の木表示

- 難易度: 中 / 依存: G01 / 触るファイル: `src/backend.rs`, `src/app.rs`, `src/ui/browsers.rs`, `src/ui/views.rs`, 新規 `src/library.rs`
- 先に `docs/issues/gui/README.md` を読むこと。

## 目的

Package Browser に、開いている .mo ファイルのクラスに加えて **OpenModelica にインストール済みのライブラリ**(まず Modelica Standard Library 4.1.0)を木で表示します。
クラスを選ぶと、そのクラスの Modelica テキストを**読み取り専用**で編集ウィンドウに表示します。
読み込んだライブラリはチェック/シミュレーション時にも OMC に読み込ませ、ユーザーのモデルから `Modelica.Blocks...` を使えるようにします。

## 背景

- OMC は Job ごとに**別プロセス**で起動されます(常駐セッションはありません)。したがって「ライブラリを読み込む」とは、毎回のスクリプトの先頭で `loadModel(...)` することです。
- `omola-core` には既にライブラリのピン留め読み込みがあります: `Omc::check_model_with_libraries(source, class, &[(name, version)])` と、`SimulationConfig { libraries: &[(name, version)], .. }`。これを使います。
- MSL のインストール: 開発コンテナで `omc tests/compat/install-msl.mos`。
- 使う OMC 関数(OpenModelica 1.27.1 scripting API で確認済み):
  - `loadModel(Modelica, {"4.1.0"})` → Boolean
  - `getClassNames(class_, recursive=true, qualified=true, sort=true)` → クラス名の配列
  - `getClassRestriction(cl)` → `"model"`, `"package"` などの文字列
  - `typeNameString(name)` → 型名を文字列に
  - `list(class_)` → クラスの Modelica テキスト(String)

## 実装手順

1. 新規 `src/library.rs`(`main.rs` に `mod library;` を追加):
   ```rust
   #[derive(Debug, Clone, PartialEq, Eq)]
   pub struct LibraryClass { pub qualified_name: String, pub restriction: String }

   /// OMC 出力の各行 "Qualified.Name\trestriction" を読む。タブを含まない行(cd の出力など)は無視。
   pub fn parse_class_listing(output: &str) -> Vec<LibraryClass>

   /// OMC に渡すスクリプト本体を作る(name と version は validate 済みであること)。
   pub fn listing_script(name: &str, version: &str) -> String
   ```
   `listing_script` が返す文字列(この形にする):
   ```
   if not loadModel(NAME, {"VERSION"}) then print(getErrorString()); end if;
   for c in getClassNames(NAME, recursive=true, qualified=true, sort=true) loop
     print(typeNameString(c) + "\t" + getClassRestriction(c) + "\n");
   end for;
   ```
   最上位の `NAME` 自身は getClassNames に含まれないので、`parse_class_listing` の結果の先頭に `LibraryClass { qualified_name: NAME, restriction: "package" }` を自分で追加する。
2. 入力検証: ライブラリ名・バージョンは `omola_core::validate_library_pins(&[(name, version)])` を通してからスクリプトに埋め込む(スクリプト注入防止)。
3. `backend.rs` に追加:
   - `Job::LoadLibrary { name: String, version: String }` → `omc.run_script(&library::listing_script(..))` → `Event::LibraryLoaded { name, version, classes: Vec<LibraryClass> }`。クラスが 0 件なら `Event::Log(Error, "ライブラリを読み込めません: ... install-msl.mos を実行してください")`。
   - `Job::ShowClassText { libraries: Vec<(String, String)>, class: String }` → スクリプト `load_libraries` 相当 + `print(list(CLASS));` → `Event::ClassText { class, text }`。クラス名は `validate_class` 相当の検証(英数字と `_` と `.` のみ)をしてから埋め込む。`omola_core` の `validate_class` は非公開なので、同等の関数を `library.rs` に書いてテストする。
   - `Job::Check` と `Job::Simulate` に `libraries: Vec<(String, String)>` を追加し、`check_model_with_libraries` / `SimulationConfig.libraries` に渡す。
4. `app.rs`:
   - `loaded_libraries: Vec<(String, String, Vec<LibraryClass>)>` を追加。
   - 起動時は何も読み込まない(OMC が無い環境で遅くならないように)。Tools タブに「Load Modelica 4.1.0」ボタンを置く。
   - `Event::LibraryLoaded` で追加(同名は置き換え)。
   - check/simulate の Job に `loaded_libraries` の (name, version) を渡す。
5. `ui/browsers.rs` の Package Browser:
   - 開いているファイルの下に、各ライブラリを `egui::CollapsingHeader` の木で表示する。`LibraryClass` の配列(qualified 名でソート済み)から木を作る関数 `fn build_tree(classes: &[LibraryClass]) -> TreeNode` を `library.rs` に書き、テストする。
   - **数千ノードを毎フレーム全部描画しない**: CollapsingHeader は閉じているノードの中身を描画しないので、既定は閉じた状態にする。
   - 上部に検索欄を置き、入力があるときは木ではなく一致するクラスの平坦リスト(最大 200 件)を出す。
   - クラスをクリック → `Job::ShowClassText` を送り、`Event::ClassText` で `library_view: Option<(String, String)>` に保存。
6. `ui/views.rs`: `library_view` が `Some` のとき、Text 層は読み取り専用(`TextEdit::multiline(&mut text.as_str())` のように `&str` を渡すと編集不可になる)で表示し、上部に「Read-only library class: NAME」と「Back to my model」ボタンを出す。
7. テスト(`library.rs`):
   - `parse_class_listing` に、タブ区切り行・空行・`"/tmp/xxx"` のような cd 出力行が混ざった文字列を与え、正しく 2 件だけ取れる。
   - `build_tree` が `Modelica`, `Modelica.Blocks`, `Modelica.Blocks.Continuous` を親子にする。
   - クラス名検証が `Modelica.Blocks` を許可し、`A"); system("rm` を拒否する。

## 受け入れ条件

- [ ] 開発コンテナ(MSL インストール済み)で Load → 木に `Modelica > Blocks > Continuous > FirstOrder` が出て、クリックでテキストが表示される(スクリーンショット)。
- [ ] `tests/compat/fixtures/MSLFirstOrder.mo` を開き、MSL を読み込んだ状態で Simulate が成功する。
- [ ] MSL 未インストール環境では Error メッセージが出て、GUI は動き続ける。
- [ ] 上記テストが通る。README 共通コマンド 5 つが成功。

## やってはいけないこと

- MSL のファイルをリポジトリにコピー・同梱しない。
- ライブラリのテキストをユーザーが編集・保存できるようにしない(読み取り専用)。
