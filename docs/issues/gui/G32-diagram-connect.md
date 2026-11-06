# G32: Diagram 層でコネクタ間をドラッグして結線する / 結線を削除する

- 難易度: 中〜難 / 依存: G28, G29 / 触るファイル: `crates/omola-gui/src/diagram.rs`, `src/diagram_view.rs`, `src/ui/views.rs`, `src/app.rs`
- 先に `docs/issues/gui/README.md` を読むこと。

## 目的

1. 部品のコネクタ(ポート)にマウスを乗せると強調表示。そこから左ドラッグすると線が伸び、別のコネクタの上で離すと `connect(a.p, b.n)` が追加される。
2. 線の経路は Dymola 風の直交(水平→垂直→水平)の 3 線分で自動生成する。途中の折れ点を手で決める機能は作らない。
3. 結線をクリックで選択(太く表示)、`Delete` キーで削除。
4. 部品を移動したとき(G29)、その部品につながる結線の端点を移動後のコネクタ位置に付け替える。

## 実装手順

1. コネクタの位置: G12 の `Diagram` が部品ごとのコネクタ(部品型の中の connector 型の要素と、その Icon 層での Placement)を持っているか確認する。
   - 持っていなければ、`getModelInstance` の JSON(`docs/omc-model-instance-json.md` を参照)で、部品の型の `elements` のうち `$kind == "component"` かつ型の restriction が `connector`(または `expandable connector`)のものの `annotation.Placement` を取り出して `Diagram` の部品に `connectors: Vec<ConnectorPin { name: String, extent: Extent(部品座標) }>` として持たせる。
   - ピンのダイアグラム座標は「部品の Placement で変換した、ピンの extent の中心」。G26 の変換関数を再利用すること。
   - `tests/fixtures/instance_pid.json` など既存の fixture で、少なくとも 1 つの部品のピン座標をテストする。
2. 当たり判定: `fn hit_pin(diagram: &Diagram, p: [f64; 2], tolerance: f64) -> Option<(usize /*component*/, usize /*pin*/)>`。tolerance は画面上 6px をダイアグラム座標に換算した値。**部品本体の当たり判定(G29)より優先**する。
3. 経路生成: `fn orthogonal_route(a: [f64; 2], b: [f64; 2]) -> Vec<[f64; 2]>` — `[a, (mx, a.y), (mx, b.y), b]`(mx は中点の x をグリッドにスナップ)。a と b の y が同じなら `[a, b]`。
4. ドラッグ中は仮の線を描く(OMC は呼ばない)。ピンの上で離したら `EditOp::AddConnection { from: "comp.pin", to: "comp2.pin2", points }`。
   - 同じピン同士・同じ部品内のピン同士は無視。
   - 既に同じ結線がある場合は無視。
   - 型の互換性チェックはしない(Check で OMC がエラーを出す)。
5. 結線の当たり判定: 線分との距離が 4px 以内。`fn hit_connection(diagram, p, tol) -> Option<usize>`(点と線分の距離の関数もテスト)。
6. 結線の削除: 選択中の結線で `Delete` → `EditOp::DeleteConnection`。
7. 部品の移動への追従(G29 の SetPlacement と**同じ** `apply_edit` に、つながる結線ごとに `DeleteConnection` + 新しい `points` の `AddConnection` を加える)。

## テスト

- ピン座標(fixture)、`hit_pin`、`orthogonal_route`(同じ y、逆向き、グリッドスナップ)、点と線分の距離、`hit_connection`。
- 部品移動時に作られる `Vec<EditOp>` の内容。

## 受け入れ条件

- [ ] `Unnamed` に Sine と FirstOrder を置き(G30)、`sine.y` → `firstOrder.u` を結線 → Check と Simulate が通り、`firstOrder.y` がプロットできる(**実 OMC**。スクリーンショットか GIF)。
- [ ] 結線を削除できる。部品を動かすと線が付いてくる。
- [ ] テストが通る。README 共通コマンド 5 つが成功。
