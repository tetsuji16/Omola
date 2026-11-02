# G05: Simulation Setup の全項目を OMC に渡す + experiment 注釈の読み込み

- 難易度: 中 / 依存: G01 / 触るファイル: `crates/omola-core/src/lib.rs`, `crates/omola-cli/src/*.rs`(コンパイルを通すための最小修正のみ), `crates/omola-gui/src/backend.rs`, `src/app.rs`, `src/ui/dialogs.rs`
- 先に `docs/issues/gui/README.md` を読むこと。

## 目的

Simulation Setup ダイアログの Start time / Number of intervals / Tolerance / Algorithm は**いま OMC に渡っていません**(`backend.rs` が Warning を出しているだけ)。これを実際に渡します。
また、モデルの `annotation(experiment(StopTime=..., Tolerance=...))` を読み取り、Setup の初期値にします(Dymola と同じ挙動)。

## 実装手順

### A. omola-core

1. `SimulationConfig` に次のフィールドを追加:
   ```rust
   pub start_time: Option<f64>,
   pub number_of_intervals: Option<u32>,
   pub tolerance: Option<f64>,
   pub method: Option<&'a str>,
   ```
   `None` のときは今まで通り OMC に渡さない(既定値は OMC 任せ)= 既存の CLI 挙動は変わらない。
2. `grep -rn "SimulationConfig {" crates/` で全ての構築箇所を探し、新フィールドを `None` で追加してコンパイルを通す(CLI の挙動は変えない)。
3. `simulate_project_model_with_parameters` 内の `simulate(...)` 呼び出し文字列を作る部分を、**純粋関数**に切り出す:
   ```rust
   fn simulate_call(class: &str, output_prefix: &str, config: &SimulationConfig<'_>, simflags: &str) -> Result<String, BackendError>
   ```
   既存の文字列(`simulationResult := simulate(CLASS, stopTime=..., fileNamePrefix="...", simflags="...");`)と**同じ順番**で、`Some` のものだけ `startTime=`, `numberOfIntervals=`, `tolerance=`, `method="..."` を `stopTime=` の後ろに追加する。
4. 検証(`simulate_call` 内でエラーを返す):
   - start_time は有限、かつ `start_time <= stop_time`
   - number_of_intervals は 1 以上
   - tolerance は有限かつ `0 < tol < 1`
   - method は許可リスト `["dassl", "ida", "cvode", "euler", "heun", "rungekutta", "impeuler", "trapezoid", "imprungekutta", "gbode", "irksco", "symSolver", "qss"]` のいずれか(これ以外はスクリプト注入防止のため拒否)。OpenModelica 1.27 のユーザーガイドの「Integration Methods」で一覧を確認し、存在しないものはリストから外す。
5. テスト(lib.rs の tests): `simulate_call` が
   - 全部 None → 既存と同一の文字列
   - 全部 Some → 期待する文字列(完全一致で比較)
   - method `"dassl\"); system(\"x"` → Err
   - start_time > stop_time → Err

### B. experiment 注釈の読み込み

6. omola-core に `pub fn simulation_options(&self, source: &Path, class: &str) -> Result<SimulationOptions, BackendError>` を追加:
   - スクリプト: `loadFile("...")` の後に `getSimulationOptions(CLASS);`(式文。OMC は結果をタプル `(0.0, 3.0, 1e-06, 500, 0.006)` として標準出力に出す)。
   - 出力から「`(` で始まり `)` で終わり、カンマ区切りで 5 個の数値」の**最後の行**を探してパースする純粋関数 `parse_simulation_options(output: &str) -> Option<SimulationOptions>` を書く。
   - `pub struct SimulationOptions { pub start_time: f64, pub stop_time: f64, pub tolerance: f64, pub number_of_intervals: u32, pub interval: f64 }`
   - テスト: `"\"/tmp/x\"\ntrue\n(0.0, 3.0, 1e-06, 500, 0.006)\n"` を正しくパース、壊れた入力で None。
   - **実際の出力形式を開発コンテナの omc で必ず確認**し、違っていたらパーサを合わせる。確認した実出力をテストのフィクスチャ文字列に使う。
7. GUI: `Job::ReadExperiment { source_text, class }` → `Event::Experiment(SimulationOptions)`。
   - モデルを開いたとき・Model コンボで別クラスを選んだときに 1 回送る。
   - 受け取ったら `setup` の start/stop/tolerance/intervals を上書きし、Messages に `Info` で「Experiment annotation: StopTime=3」等を出す。
   - ユーザーが Setup を手で変えた後は上書きしない(`setup_edited_by_user: bool` フラグ)。

### C. GUI 配線

8. `backend.rs` の Warning 出力ロジックを削除し、`SimulationSetup` の値を `SimulationConfig` に `Some(...)` で渡す。
9. `dialogs.rs` の「Only Stop time ... reach OMC today」の注記を削除。method のコンボの選択肢を A-4 の許可リストと同じ定数から作る(定数は omola-core に `pub const SIMULATION_METHODS: &[&str]` として置き、GUI から参照)。

## 受け入れ条件

- [ ] 開発コンテナの実 OMC で、Number of intervals = 10 にしてシミュレーションすると、結果の時刻点数が大幅に減る(BouncingBall はイベントがあるので 11 ちょうどではない。PR に点数を書く)。
- [ ] `annotation(experiment(StopTime=5))` を付けたモデルを開くと Setup の Stop time が 5 になる。
- [ ] CLI の既存テスト・CI の omc-integration が今まで通り通る(CLI 挙動不変)。
- [ ] 上記テストが通る。README 共通コマンド 5 つが成功。

## やってはいけないこと

- method 名をユーザー入力のまま検証せずにスクリプトへ埋め込まない。
- 「Translate」ボタンの実装はこの issue の範囲外(別 issue にする)。
