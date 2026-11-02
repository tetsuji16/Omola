# G06: Component Browser(getModelInstance の JSON を使う)

- 難易度: 中 / 依存: G01(G04 があればライブラリ付きで動く) / 触るファイル: `crates/omola-gui/Cargo.toml`, `src/backend.rs`, `src/app.rs`, `src/ui/browsers.rs`, 新規 `src/instance.rs`, 新規 `crates/omola-gui/tests/fixtures/`
- 先に `docs/issues/gui/README.md` を読むこと。

## 目的

左下の Component Browser に、選択中クラスの**部品(コンポーネント)の階層**を表示します。
例: `MSLFirstOrder` なら `firstOrder : Modelica.Blocks.Continuous.FirstOrder`、`step : Modelica.Blocks.Sources.Step` など。
この issue で作る「インスタンス JSON の読み取り」は G07(Icon)・G12(Diagram)の土台になります。

## 背景

- OMC 1.27.1 の `getModelInstance(className, prettyPrint=false)` はモデルのインスタンスを **JSON 文字列**で返します(OMEdit が使う API)。スクリプトでは `print(getModelInstance(CLASS));` で JSON が標準出力に出ます。
- JSON の正確な構造は公式ドキュメントに載っていません。**推測でパースせず、まず実物を取得してフィクスチャにします。**

## 実装手順

1. **フィクスチャ取得(最初に必ずやる)**: 開発コンテナで MSL 4.1.0 をインストールし(`omc tests/compat/install-msl.mos`)、次のスクリプトを実行して出力を保存する。
   ```
   loadModel(Modelica, {"4.1.0"});
   loadFile("tests/compat/fixtures/MSLFirstOrder.mo");
   print(getModelInstance(MSLFirstOrder, prettyPrint=true));
   ```
   保存先: `crates/omola-gui/tests/fixtures/instance_MSLFirstOrder.json`(先頭の `cd` 出力などは除き、JSON 部分だけ)。同様に `examples/BouncingBall.mo` の `BouncingBall` を `instance_BouncingBall.json` に。
   - PR 本文に、取得に使った omc のバージョン(`omc --version`)を書く。
   - `docs/omc-model-instance-json.md` を新規作成し、**フィクスチャで実際に観察できたキー**(要素の種類を表すキー名、名前・型・コメント・修飾子・注釈がどのキーにあるか)を表にまとめる。G07/G12 の実装者がこれを読みます。
2. 依存: `serde_json = "1"` を `crates/omola-gui/Cargo.toml` に追加(既にワークスペースの lock にあるので inventory 変更は不要なはず。`licenses check` で確認)。
3. 新規 `src/instance.rs`:
   ```rust
   #[derive(Debug, Clone, PartialEq)]
   pub struct ComponentNode {
       pub name: String,        // 例: "firstOrder"
       pub type_name: String,   // 例: "Modelica.Blocks.Continuous.FirstOrder" / "Real"
       pub comment: String,
       pub is_parameter: bool,  // prefixes の variability が parameter/constant
       pub children: Vec<ComponentNode>,
   }
   pub fn parse_components(json: &str, max_depth: usize) -> Result<Vec<ComponentNode>, String>
   ```
   - `serde_json::Value` を辿る方式で書く(厳密な struct にしない。未知のキーは無視)。
   - 手順 1 で観察した構造に合わせる。要素のうち **component だけ**を拾い、extends の中の component は親の直下に平坦化して含める。
   - 子要素は、型が組み込み型(Real, Integer, Boolean, String, 列挙)でなければ、その型の JSON に含まれる要素から再帰的に作る(`max_depth` で打ち切り。GUI では 4)。
4. `backend.rs`: `Job::LoadInstance { source_text, class, libraries }` → スクリプト(ライブラリ読み込み + `loadFile` + `print(getModelInstance(CLASS));`)→ JSON 部分を取り出す関数 `extract_json(output: &str) -> Option<&str>`(最初の `{` から最後の `}` まで)→ `Event::Instance { class, components: Vec<ComponentNode> }` または Error ログ。
5. `app.rs`: `components: Option<(String, Vec<ComponentNode>)>` を持つ。Check 成功時・クラス選択変更時に `Job::LoadInstance` を送る(テキスト編集のたびには送らない)。
6. `ui/browsers.rs` の Component Browser: 木で表示。各行 `name : TypeName`、parameter には `(p)`、ホバーで comment。子を持つものは `CollapsingHeader`(既定は閉)。データ未取得なら「Press Check to load components」と表示。
7. テスト(`instance.rs`、フィクスチャを `include_str!("../tests/fixtures/instance_MSLFirstOrder.json")` で読む):
   - MSLFirstOrder の最上位に、フィクスチャに実在する部品名がすべて出る(名前はフィクスチャを見て書く)。
   - BouncingBall では `gravity`, `restitution`, `initialHeight` が `is_parameter == true`、`height`, `velocity` が false。
   - `extract_json` が前後のゴミ行を除去できる。
   - 壊れた JSON で Err(パニックしない)。

## 受け入れ条件

- [ ] フィクスチャ 2 個と `docs/omc-model-instance-json.md` がコミットされている。
- [ ] 実 OMC + MSL で MSLFirstOrder を開き Check すると Component Browser に部品の木が出る(スクリーンショット)。
- [ ] 上記テストが通る。README 共通コマンド 5 つが成功。

## やってはいけないこと

- JSON 構造をドキュメントや記憶から推測してパーサを書かない。必ずフィクスチャの実物に合わせる。
- 毎フレーム・毎キー入力で OMC を呼ばない。
