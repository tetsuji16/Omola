# G07: Icon 層の描画(グラフィックス注釈の読み取り専用レンダリング)

- 難易度: 中〜難 / 依存: G06(`docs/omc-model-instance-json.md` とフィクスチャ) / 触るファイル: 新規 `src/graphics.rs`, `src/ui/views.rs`, `src/backend.rs`, `src/app.rs`, `crates/omola-gui/tests/fixtures/`
- 先に `docs/issues/gui/README.md` と `docs/omc-model-instance-json.md`(G06 で作成)を読むこと。

## 目的

Graphics タブ > Icon で、選択クラスの `annotation(Icon(graphics={...}))` を描画します(編集はしない)。
今のプレースホルダ(`placeholder_canvas`)を、実際の図形描画に置き換えます。これは G12(Diagram 層)で部品アイコンを描くのにも使います。

## 背景: Modelica のグラフィックス注釈(Modelica 言語仕様 3.6 第 18.6 節)

- 座標系: `coordinateSystem(extent={{-100,-100},{100,100}})` が既定。**y 軸は上向き**(画面は下向きなので反転が必要)。
- 図形(この issue で対応するもの): `Line(points, color, thickness, pattern)`, `Rectangle(extent, radius, lineColor, fillColor, fillPattern)`, `Ellipse(extent, startAngle, endAngle, ...)`, `Polygon(points, ...)`, `Text(extent, textString, textColor, ...)`。
- 共通属性: `visible`, `origin={x,y}`, `rotation`(度、反時計回り)。
- `fillPattern` は `FillPattern.None` / `Solid` 以外(ハッチ等)は**Solid として塗る**(簡略化でよい。コメントに明記)。
- `Text` の `textString` 中の `%name` はクラス名(またはインスタンス名)に、`%パラメータ名` は今回は置換せずそのまま表示でよい。
- `Bitmap` は対象外(描かない)。

## 実装手順

1. **フィクスチャ**: G06 と同じ方法で、Icon 注釈を持つ MSL クラスのインスタンス JSON を取得する。
   - `Modelica.Blocks.Continuous.FirstOrder`、`Modelica.Electrical.Analog.Basic.Resistor`、`Modelica.Mechanics.Rotational.Components.Inertia`
   - 注釈だけなら `print(getModelInstanceAnnotation(CLASS, {"Icon"}))` も使える(G06 の観察結果に従う)。どちらを使ったか `docs/omc-model-instance-json.md` に追記する。
   - **継承した Icon**(`extends Modelica.Blocks.Interfaces.SISO` 等の親の Icon)も描く必要がある。JSON 内で親の注釈がどこにあるかを観察して文書に書く。
2. 新規 `src/graphics.rs`:
   ```rust
   pub struct Extent { pub x1: f64, pub y1: f64, pub x2: f64, pub y2: f64 }
   pub struct Style { pub line: [u8; 3], pub fill: Option<[u8; 3]>, pub thickness: f64 }
   pub enum Shape {
       Line { points: Vec<[f64; 2]>, style: Style },
       Polygon { points: Vec<[f64; 2]>, style: Style },
       Rectangle { extent: Extent, radius: f64, style: Style },
       Ellipse { extent: Extent, start_angle: f64, end_angle: f64, style: Style },
       Text { extent: Extent, text: String, color: [u8; 3] },
   }
   pub struct Graphic { pub shape: Shape, pub origin: [f64; 2], pub rotation_deg: f64 }
   pub struct Icon { pub coordinate_extent: Extent, pub graphics: Vec<Graphic> }

   pub fn parse_icon(json: &serde_json::Value) -> Result<Icon, String>
   /// Modelica 座標 → 画面座標(rect は描画先、y 反転、アスペクト比維持で中央寄せ)
   pub fn to_screen(p: [f64; 2], icon_extent: &Extent, rect: egui::Rect) -> egui::Pos2
   pub fn paint_icon(painter: &egui::Painter, rect: egui::Rect, icon: &Icon, name: &str)
   ```
   - 図形の点は「origin を足す → rotation で回す → to_screen」の順で変換する。回転は純粋関数 `fn transform(p, origin, rotation_deg) -> [f64; 2]` にしてテストする。
   - 描画: Line/Polygon は `painter.add(egui::Shape::line(..))` / `egui::Shape::convex_polygon(..)`(凹多角形は `PathShape` の closed line で外形だけでもよい。コメントに明記)。Rectangle は 4 点の Polygon として描くと回転にも対応できる。Ellipse は 48 分割の多角形近似。Text は extent の中央に `painter.text(..)`、フォントサイズは extent の高さに合わせて 8〜48 px にクランプ。
   - 色が無い場合の既定: lineColor 黒 `[0,0,0]`、fill なし。
3. `backend.rs`: `Job::LoadIcon { source_text, class, libraries }` → `Event::Icon { class, icon_json: String }`(パースは UI 側でなく **worker 側で行い**、`Event::Icon { class, icon: Icon }` を送ってもよい。どちらかに統一)。
4. `ui/views.rs`: Graphics タブで `GraphicsLayer::Icon` のとき、icon があれば `paint_icon`、無ければ今のプレースホルダ。背景のグリッドと -100..100 の枠は残す。
5. テスト(`graphics.rs`):
   - `transform([10,0], [0,0], 90)` → `[0,10]`(誤差 1e-9)。
   - `to_screen` で extent の左上 `(-100,100)` が rect の左上付近、`(100,-100)` が右下付近になる(正方形 rect で)。
   - 3 つのフィクスチャそれぞれで `parse_icon` が成功し、図形数が 1 以上。Resistor には Rectangle と Line がある(フィクスチャを見て、実在する図形の種類を assert する)。
6. 目視確認用に、`examples/` には何も足さず、PR にスクリーンショット 3 枚(各クラスの Icon 層)を貼る。**Dymola や OMEdit の画面と並べた比較画像はコミットしない。**

## 受け入れ条件

- [ ] 3 つの MSL クラスの Icon が、図形の位置・色が概ね正しく描かれる(スクリーンショット)。
- [ ] 回転・origin 付きの図形が正しい位置に出る(テスト)。
- [ ] パースできない図形があってもパニックせず、その図形だけ飛ばして Messages に Warning を 1 回出す。
- [ ] README 共通コマンド 5 つが成功。

## やってはいけないこと

- MSL のアイコン画像(PNG/SVG)をリポジトリにコミットしない。描画は常に OMC から得た注釈から行う。
- 編集機能(図形の追加・移動)は範囲外。
