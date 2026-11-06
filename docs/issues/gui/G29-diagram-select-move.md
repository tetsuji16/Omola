# G29: Diagram 層で部品を選択・移動・回転する

- 難易度: 中 / 依存: G28, G26 / 触るファイル: `crates/omola-gui/src/ui/views.rs`(`diagram_layer`), `src/diagram_view.rs`, `src/diagram.rs`, `src/app.rs`
- 先に `docs/issues/gui/README.md` を読むこと。

## 目的

ユーザーモデル(ライブラリのクラスではない)の Diagram 層で:
1. 部品をクリックで選択(選択枠を表示)。空白クリックで解除。Component Browser の選択と同期(既存の `selected_component` を使う)。
2. 選択した部品を左ドラッグで移動。マウスを離したら G28 の `EditOp::SetPlacement` を送る。
3. 移動はグリッド(Modelica の既定 2 単位。ダイアグラム座標)にスナップ。Shift を押している間はスナップしない。
4. キーボード: 矢印キーで 2 単位移動、`Ctrl+R` で 90 度回転(反時計回り)。
5. ライブラリのクラスでは選択はできるが移動できない(ドラッグしても何も起きず、ステータスに「read-only」と出す)。

## 実装手順

1. 画面座標 ↔ ダイアグラム座標の変換を `diagram_view.rs` に関数として切り出す(既存の `to_screen` の逆関数 `to_diagram(pos: Pos2, extent, rect, zoom, pan) -> [f64; 2]` を追加)。
2. 当たり判定: G26 の `component_bounds(&Placement) -> Rect`(ダイアグラム座標)を使い、**後に描いた部品が優先**(配列の後ろから探す)。
   ```rust
   pub fn hit_component(diagram: &Diagram, p: [f64; 2]) -> Option<usize>
   ```
3. ドラッグ中の状態を `OmolaApp` に持つ:
   ```rust
   pub diagram_drag: Option<DiagramDrag>,
   pub struct DiagramDrag { pub component: String, pub start: [f64; 2], pub offset: [f64; 2] }
   ```
   - ドラッグ中は**ローカルに**位置をずらして描く(OMC を呼ばない)。`drag_stopped()` で offset をスナップして `apply_edit(class, vec![EditOp::SetPlacement { .. }])`。
   - 既存の「右ドラッグ/中ドラッグでパン」はそのまま。左ドラッグで部品の上ならその部品を移動、空白ならパンしない(将来の範囲選択用に何もしない)。
4. 移動量 0(クリックだけ)のときは編集を送らない。
5. 回転: `rotation = (rotation + 90) % 360`。origin を中心に回すので extent は変えない。
6. 選択枠: 部品の外接矩形を青(theme::ACCENT)の 1px 破線、四隅に 6px の四角。
7. 編集結果が返る(G28 の `Event::ModelEdited`)まで、ドラッグ後の位置を仮表示し続ける(`pending_placement: Option<(String, Placement)>`)。失敗したら仮表示を消す。

## テスト

- `to_diagram(to_screen(p)) ≈ p`(zoom・pan を変えた 3 ケース)。
- `hit_component`: 重なった 2 部品で後の方が選ばれる、範囲外は None、回転した部品。
- スナップ関数 `snap(v: f64, grid: f64) -> f64`(負の値・ちょうど中間の値)。
- ドラッグ完了で `EditOp::SetPlacement` が 1 回だけ作られる(`apply_edit` の代わりに Op を返す関数に分けてテスト)。

## 受け入れ条件

- [ ] 自作モデル(Constant と Gain を connect したもの)を開き、部品を移動 → Text 層を見ると Placement の数値が変わっている → 保存して開き直しても位置が保たれる(**実 OMC**。GIF かスクリーンショット 2 枚)。
- [ ] 結線は部品の移動に追従しない(結線の更新は G32)ことを PR に明記。
- [ ] テストが通る。README 共通コマンド 5 つが成功。
