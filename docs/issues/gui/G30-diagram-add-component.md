# G30: Package Browser から Diagram 層へドラッグして部品を追加する

- 難易度: 中 / 依存: G28 / 触るファイル: `crates/omola-gui/src/ui/browsers.rs`, `src/ui/views.rs`, `src/app.rs`
- 先に `docs/issues/gui/README.md` を読むこと。

## 目的

Package Browser のクラス(model / block / connector など、`package` と `function` 以外)を Diagram 層へドラッグ&ドロップすると、ドロップ位置にその部品が追加される。

## 実装手順

1. egui 0.37 のドラッグ&ドロップ API を使う。`~/.cargo/registry/src/*/egui-0.37.0/src/` で `dnd_drag_source` と `dnd_drop_zone`、`DragAndDrop::set_payload` / `Response::dnd_release_payload` を grep して使い方を確認すること(推測で書かない)。
2. ドラッグ元: `browsers.rs` の `tree_row` の行の `Response` で、ドラッグが始まったら `egui::DragAndDrop::set_payload(ui.ctx(), ClassPayload { qualified: name.to_string(), restriction })`。`package`, `function`, `type`, `record`(※record は部品として置けるが Dymola でも Diagram には描かれない)は payload を設定しない。
   - ドラッグ中はマウス位置に半透明でクラス名を表示(`ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, ..))`)。
3. ドロップ先: `views.rs` の `diagram_layer` のキャンバスの `Response` で `response.dnd_release_payload::<ClassPayload>()` を受ける。
   - ユーザークラスでなければ「Library classes are read-only」をログに出して何もしない。
   - 名前を決める: `fn unique_component_name(type_short: &str, existing: &[String]) -> String` — 型の短い名前の先頭を小文字にし(`Constant` → `constant`)、既存名と重なれば末尾に `1`, `2`, ...(`constant1`)。既存の部品名は `self.components`(G06 の木)か `model_diagrams` の部品一覧から取る。
   - 位置: ドロップ点を G29 の `to_diagram` で変換しスナップ、`extent = [[-10,-10],[10,10]]`, `origin = ドロップ点`。
   - `apply_edit(class, vec![EditOp::AddComponent { name, type_name: qualified, extent, origin }])`。
4. 追加後、新しい部品を選択状態にする(`selected_component = Some(name)`)。

## テスト

- `unique_component_name`: `Constant` + `[]` → `constant`、`[constant]` → `constant1`、`[constant, constant1]` → `constant2`、`PID` → `pID`(Dymola と違ってもよい。大文字が続く場合の規則を PR に書く)、`'quoted'` 名は対象外。
- payload を設定しない restriction の判定関数のテスト。

## 受け入れ条件

- [ ] `Unnamed` モデルに `Modelica.Blocks.Sources.Sine` と `Modelica.Blocks.Continuous.FirstOrder` をドラッグで追加 → Text 層に 2 つの宣言と Placement がある → Check が通る(**実 OMC**。スクリーンショット)。
- [ ] テストが通る。README 共通コマンド 5 つが成功。
