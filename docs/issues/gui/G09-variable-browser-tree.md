# G09: Variable Browser の木構造化・パラメータ編集から再シミュレーション

- 難易度: 中 / 依存: なし / 触るファイル: `src/ui/browsers.rs`, `src/app.rs`, 新規 `src/series.rs`(G10 が先に作っていれば再利用)
- 先に `docs/issues/gui/README.md` と `docs/reference/dymola-gui.md` の R7〜R11 を読むこと。

## 目的

いまの Variable Browser は変数を平坦に並べるだけです。次を実装します。

1. 変数名のドット区切りで**木構造**にする(`a.b.c` → `a` > `b` > `c`)。
2. フィルタのトグル: 「Parameters」「Time-varying」(両方 ON が既定)。
3. **時刻を指定して値を表示**(既定は最終時刻)。
4. 最新結果の**パラメータ値をその場で編集**し、次の Simulate で `-override` として渡す(Dymola で Translate 後に初期値を変える操作に相当、R10)。

## 実装手順

1. 新規 `src/series.rs`(既にあれば追加のみ):
   ```rust
   /// 時刻 t の値。time は非減少(イベントで同じ時刻が 2 回出る)。
   /// t が範囲外なら端の値。ちょうど同じ時刻が複数あれば「最後」のサンプルの値(イベント後の値)。
   /// その間は線形補間。
   pub fn value_at(time: &[f64], values: &[f64], t: f64) -> Option<f64>
   ```
   空配列なら None。二分探索(`partition_point`)で実装。
2. 木の構築: `src/ui/browsers.rs` ではなく新規関数 `fn build_variable_tree(names: &[(usize, &str)]) -> VarNode`(`app.rs` か新規 `src/var_tree.rs`)。
   - `der(a.b.x)` は、`a.b` の下に表示名 `der(x)` として置く。
   - 配列添字 `x[1]` は `x` の下に `[1]` として置く(`a.b[2].c` は `a` > `b[2]` > `c` でよい)。
   - 各葉は変数インデックスを持つ。
3. 表示: 結果ごとの `CollapsingHeader` の中に木を描く。中間ノードは `CollapsingHeader`(既定は閉、ただし子が 20 個以下で深さ 1 なら開)。葉は既存と同じ「チェックボックス・値・説明」の 3 列。
   - フィルタ文字列があるときは木ではなく一致する葉の平坦リストにする(今の挙動)。
4. 時刻指定: Variable Browser 上部に `Time:` の `DragValue`(範囲は最新結果の time の最小〜最大)と「End」ボタン。値列は `value_at(time, series, t)`。
5. パラメータ編集:
   - **最新の結果**(`runs.last()`)のパラメータ行だけ、値列を `egui::TextEdit::singleline`(幅 80)にする。
   - 編集された値は `OmolaApp.pending_overrides: BTreeMap<String, String>` に入れる(キーは変数のフル名)。数値として `str::parse::<f64>` できない値は赤文字にして保存しない。
   - 変更済みの行は太字 + 右に `x` ボタン(取り消し)。
   - Simulate 時: `setup.parameters` に `pending_overrides` をマージして Job を送る(同名は pending を優先)。送信後も `pending_overrides` は残す(次の実行でも同じ値)。「Reset overrides」ボタンで全消去。
   - **重要**: OMC の `-override` に渡せるのはパラメータ名のみ。名前は `omola_core::validate_parameter_overrides` で検証し、エラーなら Messages に出して Simulate しない。
6. テスト:
   - `value_at`: 線形補間、範囲外、重複時刻(`[0,1,1,2]` / `[0,10,20,30]` で t=1 → 20)、空配列。
   - `build_variable_tree`: `a.b`, `a.c`, `der(a.b)`, `x[1]`, `x[2]` が期待どおりの親子になる。
   - `pending_overrides` のマージ関数(純粋関数にする)で同名上書き。

## 受け入れ条件

- [ ] MSL モデル(G04 済みなら)または変数名にドットを含む自作モデルで木が表示される(スクリーンショット)。
- [ ] BouncingBall で `restitution` を 0.9 に編集 → Simulate → 新しい結果でバウンドが高くなる(実 OMC。fake-omc では結果は変わらないので、その場合は Job に override が入っていることをログで示す)。
- [ ] 1 万変数の結果でも操作が重くならない(demo_mat を改造した一時的な大きいファイルで確認し、PR に書く。改造コードはコミットしない)。
- [ ] テストが通る。README 共通コマンド 5 つが成功。
