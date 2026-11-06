# G28: モデル編集の基盤(OMC の編集 API を通してテキストを書き換える Job)

- 難易度: 中 / 依存: G23(テキストが編集可能になっていること) / 触るファイル: `crates/omola-gui/src/backend.rs`, `src/app.rs`, 新規 `crates/omola-gui/src/edit.rs`, `src/main.rs`(`mod edit;`)
- 先に `docs/issues/gui/README.md` を読むこと。
- **この issue は G29〜G34(Diagram 層の編集)すべての前提です。UI は作りません。**

## 方針

Omola は Modelica のパーサを持ちません(README の Architecture)。Diagram 層での編集は、OMEdit と同じく **OMC のスクリプト API でクラスを書き換え、`list()` で得たテキストをドキュメントに戻す**方式にします。

```
ユーザー操作 → Job::EditModel { doc_text, file_name, class, ops } → worker:
  loadString(doc_text, file_name) → 各 op の API 呼び出し → list(class) を取得
→ Event::ModelEdited { class, result: Ok(new_class_text) | Err(msg) }
→ app: ドキュメント内の該当クラスのテキストを new_class_text で置き換え → dirty, outline 更新, Diagram 再読込
```

## 実装手順

1. 新規 `src/edit.rs` に編集操作の型と、OMC スクリプト生成の**純粋関数**を書く:
   ```rust
   #[derive(Debug, Clone, PartialEq)]
   pub enum EditOp {
       /// Move/resize/rotate an existing component: rewrites its Placement annotation.
       SetPlacement { component: String, extent: [[f64; 2]; 2], origin: [f64; 2], rotation: f64 },
       AddComponent { name: String, type_name: String, extent: [[f64; 2]; 2], origin: [f64; 2] },
       DeleteComponent { name: String },
       RenameComponent { old: String, new: String },
       AddConnection { from: String, to: String, points: Vec<[f64; 2]> },
       DeleteConnection { from: String, to: String },
       /// `component.parameter = value` modifier; `value = None` removes the modifier.
       SetModifier { path: String, value: Option<String> },
   }
   pub fn edit_script(class: &str, ops: &[EditOp]) -> Result<String, String>
   ```
   - 生成する API(OpenModelica 1.27 の scripting API。**実装前に下の「API の確認」を必ず行う**):
     - SetPlacement → `updateComponent(name, <型名>, Class, annotate=Placement(visible=true, transformation=transformation(origin={x,y}, extent={{x1,y1},{x2,y2}}, rotation=r)))`
       - `<型名>` は既存の型が必要。worker 側で `getComponents(Class)` から引くか、`EditOp` に `type_name` を持たせる(どちらにしたか PR に書く)。
     - AddComponent → `addComponent(name, TypeName, Class, annotate=Placement(...))`
     - DeleteComponent → `deleteComponent(name, Class)`
     - RenameComponent → `renameComponentInClass(Class, old, new)`
     - AddConnection → `addConnection(from, to, Class, annotate=Line(points={{..},{..}}))`
     - DeleteConnection → `deleteConnection(from, to, Class)`
     - SetModifier → `setElementModifierValue(Class, path, $Code(=value))`、削除は `setElementModifierValue(Class, path, $Code())`
   - 各呼び出しの後に `print(String(<戻り値>) + "\n")` ではなく、戻り値を変数に受けて `if not ok then print("OMOLA_EDIT_FAIL " + getErrorString()); end if;` のようにして失敗を検出する(OMC スクリプトの `if` 文の書き方を確認すること)。
   - 最後に `print("OMOLA_EDIT_BEGIN\n" + list(Class) + "\nOMOLA_EDIT_END\n");`
   - 名前・型名・パスは**識別子として検証**し(`[A-Za-z_][A-Za-z0-9_]*` をドットでつないだもの、配列添字 `[1]` を許す)、不正なら `Err`。**文字列をスクリプトにそのまま埋め込まない**(インジェクション防止)。`value` は Modelica の式なので検証できないが、`;` と改行と `"` を含む場合は `Err` にする(文字列リテラルの値を入れたい場合は別 issue)。
   - 数値は `format!("{}", x)` ではなく、`f64` を Modelica の数値として正しく出す関数(`1e-05` などの指数表記、NaN/inf は Err)を作る。
2. `backend.rs`:
   - `Job::EditModel { doc_text: String, file_name: String, class: String, ops: Vec<EditOp>, libraries: Vec<...> }`(ライブラリ読み込みは既存の `Check` と同じ前置きを再利用する)。
   - worker で `edit_script` → 既存の `run_script` 相当で実行 → `OMOLA_EDIT_FAIL` があれば Err、無ければ BEGIN/END の間を取り出す(既存の `between` 相当の関数を使う。無ければ作る)。
   - `Event::ModelEdited { class: String, result: Result<String, String> }` を追加。
3. `app.rs`:
   - `pub fn apply_edit(&mut self, class: &str, ops: Vec<EditOp>)`: ユーザークラスでなければ `error(...)` で「Library classes are read-only」。ドキュメントのテキストを渡して Job を送る。編集中は `busy` にはしないが、同じクラスへの二重送信を防ぐため `pending_edit: Option<String>` を持つ。
   - `Event::ModelEdited` の処理: ドキュメント内の **そのクラスの定義範囲**を新テキストで置き換える。範囲は `model.rs` の `scan_classes` が返す情報で求める(開始行・終了行を返していなければ、`ClassEntry` に `byte_range: Range<usize>` を追加する。`end <Name>;` までを含むこと)。
     - ネストしたクラス(`package P model M ... end M; end P;` の `P.M`)にも対応すること。`list(P.M)` はインデントなしのテキストを返すので、元のインデントを各行に付け直す。
     - 置き換え後: `dirty = true`、`user_text_changed(i)`、そのクラスの `model_diagrams` / `icons` / `components` のキャッシュを消して再読込させる。
   - 失敗時は `LogTab::Syntax` に Error を出し、テキストは変えない。
4. `within` 文、ファイル先頭のコメント、他のクラスのテキストは**一切変えない**こと(置き換えるのは対象クラスの範囲だけ)。

## API の確認(必須。結果を PR に貼る)

実 OMC(開発コンテナの 1.27.1)で次の `.mos` を実行し、各 API の戻り値と `list(A)` の出力を確認する:
```
loadString("model A Real x(start=1); equation der(x) = -x; end A;", "A.mo");
loadModel(Modelica);
addComponent(k, Modelica.Blocks.Sources.Constant, A, annotate=Placement(visible=true, transformation=transformation(origin={0,0}, extent={{-10,-10},{10,10}}, rotation=0)));
getErrorString();
setElementModifierValue(A, k.k, $Code(=2.5));
getErrorString();
updateComponent(k, Modelica.Blocks.Sources.Constant, A, annotate=Placement(visible=true, transformation=transformation(origin={20,0}, extent={{-10,-10},{10,10}}, rotation=90)));
getErrorString();
list(A);
```
API 名や引数の形が上と違った場合は、**動いた形に合わせて** `edit_script` を実装し、PR に違いを書くこと。

## テスト(`edit.rs`, `app.rs`)

- `edit_script` の出力の文字列比較(各 EditOp につき 1 ケース)。
- 不正な名前(`a;b`, `x y`, 空, `"q"`)が `Err`。
- `SetModifier` の value に `;` や改行があると `Err`。
- 数値の書式: `0.1`, `-10`, `1e-5`, `NaN`(Err)。
- `app.rs`: `Event::ModelEdited` で、2 クラスを含むドキュメントの片方だけが置き換わり、もう片方と先頭コメントがバイト単位で変わらない。ネストクラスのインデントが付く。
- 実 OMC のテスト: `crates/omola-gui/src/backend.rs` の tests にある既存の実 OMC 結合テスト(`#[ignore = "needs OpenModelica ..."]` が付いたもの。CI の omc-integration ジョブで `--ignored` 実行される)と同じ方式で、`AddComponent` → `list` に `k` が現れるテストを 1 つ追加。

## 受け入れ条件

- [ ] 上記「API の確認」の出力が PR にある。
- [ ] テストが通る(実 OMC テストは CI の omc-integration ジョブで)。README 共通コマンド 5 つが成功。

## やってはいけないこと

- Rust で Modelica のパーサを書かない(クラス範囲の検出は既存の `scan_classes` の拡張に留める)。
- ライブラリ(MSL など)のファイルを書き換えない。
