# G12: Diagram 層の読み取り専用表示(部品配置と結線)

- 難易度: 難 / 依存: G06(インスタンス JSON)、G07(`graphics.rs` のアイコン描画) / 触るファイル: 新規 `src/diagram.rs`, `src/ui/views.rs`, `src/app.rs`, `src/backend.rs`, `crates/omola-gui/tests/fixtures/`
- 先に `docs/issues/gui/README.md`、`docs/omc-model-instance-json.md`、`docs/reference/dymola-gui.md` を読むこと。

## 目的

Graphics タブ > Diagram で、選択クラスの

- 部品(component)を、`annotation(Placement(transformation(extent=..., origin=..., rotation=...)))` の位置に、その部品の型の **Icon**(G07)で描く
- `connect(a.p, b.n) annotation(Line(points=...))` の結線を線で描く
- クラス自身の `annotation(Diagram(graphics=...))` の図形も描く

を行います。**編集(移動・結線追加)はしません。** 表示とズーム/パンだけです。

## 背景

- Placement の `extent` は部品のアイコン座標系(既定 -100..100)を親の Diagram 座標のどの矩形に写すかを表します。`origin` で平行移動、`rotation`(度)で回転、extent の x1>x2 なら左右反転、y1>y2 なら上下反転(Modelica 仕様 18.6.1)。
- 結線の `points` は親の Diagram 座標系。
- コネクタ(`connector` 型の部品)はアイコンが小さい四角や三角であることが多い。部品の中のコネクタの位置はその部品の型のインスタンス情報にある Placement から決まります。G12 では**部品レベル(深さ 1)とその部品のコネクタ(深さ 2)だけ**描けばよい。

## 実装手順

1. フィクスチャ: MSL の `Modelica.Electrical.Analog.Examples.CauerLowPassAnalog` と `Modelica.Blocks.Examples.PID_Controller` のインスタンス JSON を G06 と同じ方法で取得しコミット。観察結果(Placement と connections がどこにあるか)を `docs/omc-model-instance-json.md` に追記。
2. 新規 `src/diagram.rs`:
   ```rust
   pub struct PlacedComponent {
       pub name: String,
       pub type_name: String,
       pub transform: Placement,          // extent, origin, rotation
       pub icon: Option<crate::graphics::Icon>,
   }
   pub struct Placement { pub extent: crate::graphics::Extent, pub origin: [f64; 2], pub rotation_deg: f64 }
   pub struct Connection { pub from: String, pub to: String, pub points: Vec<[f64; 2]>, pub color: [u8; 3] }
   pub struct Diagram {
       pub coordinate_extent: crate::graphics::Extent,
       pub own_graphics: Vec<crate::graphics::Graphic>,
       pub components: Vec<PlacedComponent>,
       pub connections: Vec<Connection>,
   }
   pub fn parse_diagram(json: &serde_json::Value) -> Result<Diagram, String>
   /// 部品アイコン座標 → 親 Diagram 座標
   pub fn placement_transform(p: [f64; 2], icon_extent: &Extent, placement: &Placement) -> [f64; 2]
   ```
   - `placement_transform`: アイコン座標を extent の矩形へ線形写像(反転含む)→ rotation で origin 周りに回転 → origin を加算。**仕様 18.6.1 の式を読んで実装し、doc コメントに式を書く。**
3. 描画(`views.rs`):
   - 自前のビューポート: `zoom: f32` と `pan: egui::Vec2` を `OmolaApp` に持つ。ホイールでズーム(カーソル位置中心)、中ボタン or 右ドラッグでパン、「Fit」ボタンで全体表示。
   - 描画順: Diagram 自身の図形 → 結線 → 部品アイコン → 部品名(アイコン下に `name`)。
   - 部品にホバーしたら tooltip で `name : type_name`。クリックで Component Browser の同じ行を選択状態にする(`selected_component: Option<String>` を共有)。
4. 取得の流れ: `Job::LoadDiagram { source_text, class, libraries }`。インスタンス JSON 1 回の取得で足りなければ、部品の型ごとに `getModelInstanceAnnotation(TYPE, {"Icon"})` を追加で呼ぶ(同じ型はキャッシュ)。**1 回のスクリプトにまとめて**、OMC プロセスの起動回数を減らすこと。
5. テスト(`diagram.rs`):
   - `placement_transform`: extent `{{-10,-10},{10,10}}`, origin `[0,0]`, rotation 0 で `(100,100)` → `(10,10)`。rotation 90 で `(100,0)` → `(0,10)`。x 反転 extent `{{10,-10},{-10,10}}` で `(100,0)` → `(-10,0)`。
   - 2 つのフィクスチャで部品数・結線数がフィクスチャ内の実数と一致(数はフィクスチャから数えて書く)。
6. スクリーンショット 2 枚(上記 MSL 例)を PR に貼る。

## 受け入れ条件

- [ ] 2 つの MSL 例で、部品が正しい位置・向きに並び、結線がコネクタ付近で接続して見える(スクリーンショット)。
- [ ] ズーム・パン・Fit が動く。部品ホバーで tooltip。
- [ ] 未対応の注釈(Bitmap 等)があってもパニックしない。
- [ ] テストが通る。README 共通コマンド 5 つが成功。

## 範囲外(別 issue にする)

- 部品のドラッグ移動、結線の追加・削除(OMC の `addConnection` / `deleteConnection` と `list()` による保存が必要)、ライブラリからのドラッグ&ドロップ。
