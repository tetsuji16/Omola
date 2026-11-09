# G36: 【不具合】Diagram 層で部品のアイコンが描かれず空の四角になる

- 難易度: 中 / 依存: なし / 触るファイル: `crates/omola-gui/src/backend.rs`, `src/library.rs`, `src/diagram_view.rs`, `src/diagram.rs`
- 先に `docs/issues/gui/README.md` と `docs/issues/gui/G12-diagram-layer-readonly.md` を読むこと。

## 不具合の内容(2026-11-09 メンテナ報告、Windows・実 OMC 1.27.1)

`Modelica.Blocks.Examples.PID_Controller` の Diagram 層で、**すべての部品が白い四角(placement の枠)だけ**で描かれる。G12 の受け入れ条件「部品の型の Icon で描く」を満たしていない。
あわせて次も見える:

- 部品名がアイコンの下に別に描かれ、隣の部品と重なる(例: `kinematicPTP` と `integrator` がつながって読める)。
- 部品のコネクタ(flange・入出力の三角など)が描かれない。

## コードから分かっていること

`backend.rs` の `Job::LoadDiagram`(`// Load icons for all component types` 以降):

1. 部品の型ごとに `library::icon_script()` を作り、**型ごとに OMC プロセスを 1 回ずつ起動**している(毎回 MSL を `loadModel` し直す)。PID_Controller なら 9 型 = 9 回。G12 の「1 回のスクリプトにまとめる」に反する。
2. `run_query_script` の失敗、`OMOLA_BEGIN icon` 区間が無い、JSON 解析失敗、`parse_icon` 失敗の**どれでも黙って捨てている**。そのため原因がログに出ない。
3. `diagram_view.rs` で `paint_graphics(..., &comp.type_name)` としており、アイコンの `Text(textString="%name")` が**部品名ではなく型名**に置き換わる(アイコンが出た後で必ず問題になる)。
4. アイコンの有無にかかわらず、部品名を下に別途描いている。MSL のアイコンは `%name` の Text を自分で持つので二重になる。

## 実装手順

1. **まず原因を特定する。** 2 の各失敗を `Event::Log(LogTab::Simulation, LogLevel::Warning, "icon of <型>: <理由>")` で出すようにし、実 OMC で PID_Controller を開いてログを確認する。PR に実際のログを貼る。
   - 確認ポイント: Windows のパス(`loadFile` に渡す `\`)のエスケープ、`getModelInstanceAnnotation` の戻り値が文字列(引用符付き)で返っていないか、出力が OMC の警告と混ざっていないか。
   - 同じ型を Graphics > Icon 層で開いたときは描けるか(描けるなら Diagram 側の取得経路だけの問題)。
2. **1 スクリプトにまとめる**: `library::icons_script(types: &[String], libraries, user_file)` を新設し、ライブラリ読み込みは 1 回、型ごとに `OMOLA_BEGIN icon:<型>` 〜 `OMOLA_END icon:<型>` の区間を print する。`parse_sections` で型ごとに取り出す。
3. **キャッシュ**: 取得したアイコンは `OmolaApp` の既存の Icon キャッシュ(G07 の `icons`、`app.rs` の「Icon graphics per class」)と共有し、2 回目以降の Diagram 表示では OMC を呼ばない型を除外する。
4. **%name**: `paint_graphics` に渡す名前を `comp.name` に直す。`%class` など他の置換があれば型名を使う(Modelica 仕様 18.6.5.5 の置換規則を doc コメントに書く)。
5. **部品名の別描画**: アイコンがある部品では下の名前ラベルを描かない。アイコンが無い(取得失敗)部品だけ、枠の中央に名前を描く(下ではなく中に。重なり防止)。
6. **コネクタ**: 部品の型のインスタンス情報(`getModelInstance` の `elements` のうち connector 型のもの)から深さ 2 のコネクタの Placement とアイコンを取り、部品の変換と合成して描く。量が多ければ PR を分けてよい(その場合は PR に「コネクタは未対応」と書き、別 issue を提案)。

## テスト

- `icons_script` の出力に `loadModel` が 1 回だけ含まれ、各型の区間が出ること。
- 複数区間の出力文字列(fixture を手で作る)から型ごとのアイコンを取り出す関数のテスト。1 型の区間が壊れていても他の型は取れること。
- `%name` 置換のテスト(`Text("%name")` → 部品名)。

## 受け入れ条件

- [ ] **実 OMC** で PID_Controller と `Modelica.Electrical.Analog.Examples.CauerLowPassAnalog` の Diagram 層のスクリーンショット。部品が MSL のアイコンで描かれ、名前が重ならない。
- [ ] Diagram 表示までの時間を変更前後で計測して PR に書く(OMC 起動回数が減っていること)。
- [ ] アイコン取得に失敗した型があってもパニックせず、ログに型名と理由が出る。
- [ ] テストが通る。README 共通コマンド 5 つが成功。
