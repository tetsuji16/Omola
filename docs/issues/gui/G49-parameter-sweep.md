# G49: Sweep Parameters(1 つのパラメータを変えて連続シミュレーション)

- 難易度: 難 / 依存: G14(結果の Keep/Close)、G44 / 触るファイル: `crates/omola-gui/src/backend.rs`, `src/app.rs`, `src/ui/dialogs.rs`, `src/ui/chrome.rs`
- 先に `docs/issues/gui/README.md` を読むこと。

## 背景(2026-11-12 のコード全体見直しで判明)

Simulation リボンの `Sweep Parameters` は `not_implemented` を呼ぶだけ。パラメータを変えた比較は、Variable Browser で値を書き換えて Simulate を繰り返すしかない。

## 範囲(これ以上は広げない)

- 掃引するパラメータは **1 つだけ**、値は「開始・終了・点数」の等間隔。
- 各点を**順番に** 1 回ずつシミュレーションする(並列にしない)。再翻訳を避ける最適化はしない(既存の `Job::Simulate` をそのまま使う)。
- 結果は通常の結果と同じく Variable Browser に並び、ラベルに `<param>=<value>` が付く。掃引で作った結果は自動で Keep にする(`keep_results` で消えないように)。

## 実装手順

1. `backend.rs` に純関数 `pub fn sweep_values(from: f64, to: f64, points: u32) -> Result<Vec<f64>, String>`(`points` は 2〜50、`from`/`to` は有限で異なること。両端を含む等間隔)。
2. `app.rs`:
   - `pub struct SweepPlan { pub class: String, pub parameter: String, pub values: Vec<f64>, pub next: usize }` と `pub sweep: Option<SweepPlan>`、ダイアログの入力用に `pub sweep_dialog: Option<SweepDialog>`(`parameter: String, from: String, to: String, points: u32`)。
   - `pub fn start_sweep(&mut self, plan: SweepPlan)`: `sweep = Some(plan)` にして `submit_next_sweep_run()` を呼ぶ。
   - `fn submit_next_sweep_run(&mut self)`: 既存の `simulate()` が `Job::Simulate` を組み立てる部分を、上書き値(`overrides`。Variable Browser の編集値を渡している項目。`grep -n "overrides\|parameter_edits" crates/omola-gui/src/app.rs` で確認)を引数に取る内部関数に切り出して使う。掃引の値は、利用者の編集値に**上書き**して渡す(利用者の編集値そのものは変えない)。
   - `Event::SimulationFinished` を処理する場所で、`sweep` が `Some` なら、追加された結果の `label` に ` [<param>=<value>]` を足して `kept = true` にし、`next += 1`。残りがあれば次を投入、無ければ `sweep = None` にして `"Sweep finished: N runs."` をログに出す。
   - シミュレーションの失敗、または Stop(取り消し)のときは `sweep = None` にして `"Sweep stopped after K of N runs."` を出す(残りは実行しない)。
   - 掃引中は `Simulate` / `Sweep Parameters` を無効にする(`busy` と同じ扱い)。
3. `dialogs.rs` に `sweep_dialog_window`: パラメータ名(アクティブなクラスのパラメータ一覧から `ComboBox` で選ぶ。一覧は Variable Browser / Parameter Dialog が使っているものを再利用)、From / To / Number of points。`sweep_values` のエラーは赤字で出し `Run` を無効にする。
4. `chrome.rs` の `Sweep Parameters` を `sweep_dialog = Some(...)` に変える(モデルが開いていて busy でないときだけ有効)。
5. 上限: 掃引 1 回で最大 50 点。結果のメモリ使用量が増えるので、ダイアログに `"Each run is kept until you close it."` と書く。

## テスト

- `backend.rs`: `sweep_values_are_evenly_spaced_and_validated`(`(0, 1, 5) → [0, 0.25, 0.5, 0.75, 1]`、`points = 1` と `51` は `Err`、`from == to` は `Err`、NaN は `Err`)。
- `app.rs`: `sweep_submits_the_next_run_after_each_result`(`Worker` を使わず、`SimulationFinished` を手で流して `next` が進み、結果のラベルと `kept` が設定されること。既存の `add_run` を使うテストを手本に)。
- `app.rs`: `a_failed_run_stops_the_sweep`。
- `backend.rs`(`#[ignore]`、実 OMC): `sweep_bouncing_ball_e_with_real_omc`(2 点。結果の最終値が異なること)。CI の omc-integration の行に足す。

## 受け入れ条件

- [ ] BouncingBall の `e` を 0.5〜0.9 の 3 点で掃引すると、結果が 3 つ並び、ラベルに値が出て、同じ変数を重ねてプロットできる(スクリーンショットを PR に付ける)。
- [ ] 途中で Stop すると残りは実行されない。失敗したときも止まる。
- [ ] 掃引の後、Variable Browser の利用者の編集値は掃引前のまま。
- [ ] 上のテストが通る。README 共通コマンド 5 つが成功。
