# `getModelInstance` の JSON(OMC 1.27.1 で実測)

Omola の Component Browser(G06)と、その上に作る Icon/Diagram 層(G07/G12)は
`getModelInstance` が返す JSON を読みます。この文書は **実際の出力を観察して** 分かった
構造だけを書いています。推測は書いていません。

## 取得方法

```
$ omc --version
OpenModelica v1.27.1 (64-bit)
```

```modelica
loadModel(Modelica, {"4.1.0"});
loadFile("tests/compat/fixtures/MSLFirstOrder.mo");
print(getModelInstance(MSLFirstOrder, prettyPrint=true));
```

`print` の前後に OMC 自身の `true` などの行が出るので、JSON 部分だけを切り出して保存します
(`instance::extract_json` と同じく、最初の `{` から最後の `}` まで)。

- `crates/omola-gui/tests/fixtures/instance_MSLFirstOrder.json`(75 KB)
- `crates/omola-gui/tests/fixtures/instance_BouncingBall.json`(1.5 KB)

## トップレベル

| キー | 型 | 内容 |
|---|---|---|
| `name` | string | クラス名(修飾なし) |
| `restriction` | string | `model` / `block` / `package` など |
| `elements` | array | 要素の配列。無いこともある |
| `annotation` | object | クラスの注釈(`uses` など)。無いこともある |
| `connections` | array | `connect(...)`。`lhs`/`rhs` は `{"$kind":"cref","parts":[{"name":"a"},{"name":"b"}]}` |
| `source` | object | `filename` / `lineStart` / `columnStart` / `lineEnd` / `columnEnd` |

## 要素(`elements[]`)の種類

各要素は `$kind` で種類を表します。フィクスチャに現れたのはこの 2 種類です。

| `$kind` | 主なキー | 意味 |
|---|---|---|
| `component` | `name`, `type`, `comment`, `prefixes`, `modifiers`, `value`, `annotation` | 宣言された部品・変数 |
| `extends` | `baseClass`, `modifiers` | 継承。`baseClass` に基底クラスの定義が入れ子で入る |

### `component`

| キー | 型 | 内容 |
|---|---|---|
| `name` | string | インスタンス名(`plant`) |
| `type` | **string または object** | 組み込み型なら `"Real"` のような文字列。クラスなら後述のオブジェクト |
| `comment` | string | 宣言の説明文字列。無いこともある |
| `prefixes.variability` | string | `parameter` / `constant`。無ければ変数 |
| `modifiers` | string | 修飾子の式(`"1"`、`"T=0.25, k=1"` など) |
| `value.binding` | number など | 束縛値 |
| `annotation` | object | `Dialog.groupImage` など |

**注意**: 列挙型のリテラル(`NoInit`, `SteadyState` など)も `$kind: "component"` で出ますが、
`type` キーを持ちません。部品ではないので読み飛ばします。

### `extends`

`baseClass` は `type` と同じ形(文字列またはオブジェクト)です。オブジェクトのときは
`elements` に基底クラスの部品が入っているので、**親の階層に平坦化**して扱います。
フィクスチャでは `Step` → `SignalSource` → `SO` → `Blocks.Icons.Block` の 3 段の
`extends` が並び、そのあとに `y` などの継承部品が続きます。

派生クラス自身の部品が先に、継承した部品が後に並ぶため、同じ名前が 2 回出たときは
**最初のもの(= 派生側)** を採用します。

## クラス定義オブジェクト(`type` / `baseClass` が object のとき)

| キー | 型 | 内容 |
|---|---|---|
| `name` | string | 完全修飾名(`Modelica.Blocks.Continuous.FirstOrder`) |
| `restriction` | string | `block` など |
| `comment` | string | クラスの説明 |
| `prefixes` | object | `{"partial": true}` など |
| `elements` | array | そのクラスの要素(さらに `component` / `extends`) |
| `annotation` | object | `Icon` / `Documentation` / `Dialog` など |
| `imports` | array | `{"path": "Modelica.Blocks.Interfaces"}` |
| `source` | object | 定義位置 |

`elements` は **型の定義をそのまま展開したもの** なので、再帰すると MSL 全体に広がります。
GUI は深さ 4 で打ち切っています(`backend::INSTANCE_DEPTH`)。

## Omola での読み取り方

`crates/omola-gui/src/instance.rs`:

- `parse_components(json, max_depth)` — `elements` を走査し、`component` を
  `ComponentNode { name, type_name, comment, is_parameter, children }` に変換する。
  `extends` は平坦化。`Real` / `Integer` / `Boolean` / `String` / `enumeration` の
  中には入らない。
- `comment` は要素の `comment`、無ければ型の `comment` を使う。
- `extract_json(output)` — OMC の前後の出力から JSON 部分を取り出す。

## `getModelInstanceAnnotation`(G07 で追加)

Icon 層(G07)は `getModelInstance` ではなく、注釈だけを返す
`getModelInstanceAnnotation` を使います。応答が数 KB で済み、継承した注釈も
`extends` の入れ子の中に入っているためです。

```modelica
loadModel(Modelica, {"4.1.0"});
print(getModelInstanceAnnotation(Modelica.Electrical.Analog.Basic.Resistor, {"Icon"}));
```

得られる JSON は `getModelInstance` と同じ形(`name` / `restriction` / `elements` /
`annotation`)ですが、**注釈以外は入っていません**。`annotation.Icon` を持たない
クラスでは、OMC は `print` に対して **何も出力しません**(空文字列)。Omola はそれを
「Icon 注釈なし」として扱います。

### `Icon` 注釈

```json
{"Icon": {
  "coordinateSystem": {"preserveAspectRatio": true, "extent": [[-100,-100],[100,100]]},
  "graphics": [ {"$kind":"record", "name":"Rectangle", "elements":[ ... ]}, ... ]
}}
```

- `coordinateSystem.extent` が無ければ `{{-100,-100},{100,100}}`。
- `graphics[]` は **位置引数のレコード** です。キー名は無く、`elements` の並びが
  Modelica 言語仕様 3.6 第 18.6 節のフィールド順そのものです。たとえば `Rectangle`
  は `visible, origin, rotation, lineColor, fillColor, linePattern, fillPattern,
  lineThickness, borderPattern, extent, radius` の 11 個です。
- 列挙値は `{"$kind":"enum","name":"FillPattern.Solid","index":2}`。
- `visible` は `true` / `false` / **`cref`** のいずれかです。`cref` は
  `{"$kind":"cref","parts":[{"name":"useHeatPort"}]}` のような条件付き表示で、
  `Resistor` の放熱ポート線がこれに当たります。

### 継承した Icon

`annotation.Icon` は **そのクラス自身のもの** だけです。親の Icon は
`elements[].$kind == "extends"` → `baseClass.annotation.Icon` に入れ子で現れます。
例:`InheritsBlockIcon`(`extends Modelica.Blocks.Interfaces.SISO`)の Icon は 2 段下の
`Modelica.Blocks.Icons.Block` にあります。

Modelica では派生クラスが自分の `Icon` を宣言すると親のものを置き換えるので、
Omola は **自分自身 → `extends` を深さ優先** の順で最初に見つかった `Icon` を使います。
`Modelica.Blocks.Continuous.FirstOrder` は自身の Icon(ステップ応答のプロット)を持つため、
`Modelica.Blocks.Icons.Block` の `%name` テキストは描かれません。

### フィクスチャ

G07 で取得した実データ(`crates/omola-gui/tests/fixtures/`):

| ファイル | クラス | 図形 |
|---|---|---|
| `icon_FirstOrder.json` | `Modelica.Blocks.Continuous.FirstOrder` | Line ×3, Polygon ×2, Text ×2 |
| `icon_Resistor.json` | `Modelica.Electrical.Analog.Basic.Resistor` | Rectangle, Line ×2, Text ×2(条件付き Line は除外) |
| `icon_Inertia.json` | `Modelica.Mechanics.Rotational.Components.Inertia` | Rectangle ×4, Line ×9, Text ×2 |
| `icon_InheritedBlock.json` | `InheritsBlockIcon`(`icon_InheritedBlock.mo`) | Rectangle, Text(親から継承) |
