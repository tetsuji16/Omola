# G54 【不具合】Diagram 層が空のまま/表示が遅い(継承した部品・結線を読まない、OMC を 2 回呼ぶ)

**GitHub:** issue #145 / PR #144

**報告(2026-11-17):** ユーザーモデルで Diagram 層を開くと、ローディングに時間がかかったあと
`Fit + - Zoom: 100%` のツールバーだけが出て図が描かれない。描画まで 20 秒かかる。

## 原因(実測)

1. **継承した部品・結線を読んでいなかった。** `getModelInstance` の `elements` / `connections`
   には *そのクラス自身の宣言* しか入らない。`model M extends Base; end M;` の形のモデルでは
   `elements` が `extends` 1 件だけ、`connections` はトップレベルに存在しない
   (`docs/omc-model-instance-json.md`)。Component Browser は `extends` を平坦化していたので
   部品は並ぶが、Diagram 層は素直に読んでいたため 0 部品 = 真っ白になっていた。
2. **OMC を 2 回呼んでいた。** Diagram 層が `getModelInstance` と
   `getModelInstanceAnnotation`(全部品型のアイコン)を別プロセスで実行し、さらに
   Component Browser も同じ `getModelInstance` を別ジョブで実行していた。`loadModel(Modelica)`
   は 1 プロセスあたり約 5.9 秒なので、同じ JSON を 2 回取るだけで 6 秒損していた。
3. 何も描くものがないクラス(Placement 注釈の無い部品だけのモデル)では、
   真っ白なキャンバスだけが残り、不具合に見えた。

## 受け入れ条件

- [x] `extends` を深さ優先で平坦化し、継承した部品と結線も描く
      (`diagram::placed_elements`, `diagram::connections_of_class`、どちらも単体テスト付き)
- [x] Diagram 層は `getModelInstance` 1 回で完結する(アイコンは応答の `type.annotation.Icon`
      から読む。`getModelInstanceAnnotation` の別クエリと `icons_script` を削除)
- [x] Component Browser と Diagram 層は 1 つの応答を共有する(`instance::InstanceData`、
      `OmolaApp::instances`)。クラスを切り替えて戻ったときは OMC を呼ばない
- [x] 描くものが無いときは理由を出す
      (「Nothing to draw in the Diagram layer. No component of this class has a Placement
      annotation, and the class has no Diagram graphics.」)
- [x] ローディング中は待ち時間の理由を出す(OpenModelica が Modelica ライブラリを読み直す)
- [x] 実 OMC で確認: 継承モデル(`extends Placed`)で r1 / r2 / g と結線が描ける。
      描画までの時間 20 秒 → 10 秒(Windows / OMC 1.27.1 / MSL 4.1.0、実測)

## 残り(未対応)

10 秒のうち約 6.5 秒は `loadModel(Modelica)` で、**OMC プロセスを起動するたびに払う**。
これ以上速くするには OMC を常駐させる(`omc --interactive=zmq` + ZMQ クライアント)しかない。
依存クレートが増えるため別 issue とする。

## 検証

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git diff --check
```

実 OMC の結合テスト(要 `OMOLA_OMC`):

```sh
OMOLA_OMC="C:/Program Files/OpenModelica1.27.1-64bit/bin/omc.exe" \
  cargo test -p omola-gui -- --ignored one_instance_query_supplies_components_and_diagram_with_real_omc
```
