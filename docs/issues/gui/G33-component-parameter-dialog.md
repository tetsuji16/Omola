# G33: 部品のパラメータダイアログ(ダブルクリックで開き、modifier を書き換える)

- 難易度: 中 / 依存: G28 / 触るファイル: `crates/omola-gui/src/ui/dialogs.rs`, `src/ui/views.rs`, `src/backend.rs`, `src/app.rs`, `src/instance.rs`
- 先に `docs/issues/gui/README.md` を読むこと。

## 目的

Diagram 層で部品をダブルクリック(または Component Browser で部品をダブルクリック)すると、その部品のパラメータ一覧ダイアログが開く。

```
+-- Parameters: firstOrder (Modelica.Blocks.Continuous.FirstOrder) ----------+
| Name        | Value         | Unit | Description                          |
| k           | [ 1        ]  |      | Gain                                 |
| T           | [ 0.5      ]  | s    | Time Constant                        |
| initType    | [ Init.NoInit v ] |  | Type of initialization               |
| y_start     | [ 0        ]  |      | Initial or guess value of output     |
|                                                    [ OK ] [ Cancel ]      |
+---------------------------------------------------------------------------+
```

- 既定値は灰色で表示(入力欄は空で placeholder に既定値)。変更した値は黒。
- OK で、変更された行だけ `EditOp::SetModifier { path: "firstOrder.k", value: Some("2") }`、空に戻した行は `value: None`。
- ライブラリクラスの中の部品では読み取り専用(OK を無効)。
- タブ(`Dialog(tab="...", group="...")` 注釈)・Advanced 表示は**作らない**(別 issue)。一覧のみ。

## 実装手順

1. パラメータ情報の取得: `getModelInstance(<部品の型>)` の JSON から `elements` のうち `prefixes.variability == "parameter"`(または `"constant"` は除外)のものを、名前・型・既定値(`modifiers` / `value`)・単位(`modifiers.unit` か型の `unit`)・`comment` で取る。JSON の形は `docs/omc-model-instance-json.md` と `instance.rs` の既存の解析を参照し、**実際の JSON(fixture)で確認**すること。
   - 継承(`extends`)したパラメータも含めること(`instance.rs` に継承をたどる処理があれば再利用)。
   - 新しい Job: `Job::LoadParameters { component_type: String, class: String, component: String, context }` → `Event::Parameters { component, result: Result<Vec<ParamRow>, String> }`。
2. 現在の modifier: 開いているクラスの `getModelInstance(class)` の JSON の、その部品の `modifiers`(G06 で読んでいるもの)から取る。
3. enumeration 型(`Init` など)は `ComboBox` で列挙値を選ばせる。列挙値は JSON の型情報に含まれていなければ、テキスト入力でよい(PR に書く)。Boolean は `true`/`false` の ComboBox。
4. 値の検証はしない(OMC がエラーを返したら G28 の仕組みで Messages に出る)。ただし `;` と改行は入力させない。
5. ダイアログは既存の `Simulation Setup` ダイアログ(`dialogs.rs`)と同じ作り(`egui::Window` + `open(&mut bool)`)。

## テスト

- fixture(`instance_MSLFirstOrder.json` など)からパラメータ行を作る関数のテスト: `k`, `T` が含まれ、`T` の unit が `s`。
- 変更行だけが `EditOp` になる関数のテスト(変更なし → 空、空に戻す → `None`)。

## 受け入れ条件

- [ ] FirstOrder を置いたモデルで T を 2 に変更 → Text 層に `firstOrder(T=2)` → Simulate すると応答が遅くなる(**実 OMC**。スクリーンショット)。
- [ ] テストが通る。README 共通コマンド 5 つが成功。
