# G01: 実 OMC で GUI バックエンドを通す結合テストを CI に追加

- 難易度: 易 / 依存: なし / 触るファイル: `crates/omola-gui/src/backend.rs`, `.github/workflows/rust.yml`
- 先に `docs/issues/gui/README.md` を読むこと。

## 目的

いまの GUI バックエンドのテスト(`backend.rs` の `simulate_job_reads_result_through_fake_omc`)は**偽の omc** でしか動いていません。
本物の OpenModelica 1.27.1 で `examples/BouncingBall.mo` を GUI の `Worker` 経由でシミュレーションし、`.mat` を `omola_core::mat` で読めることを CI で保証します。
これが通ると「GUI から計算が実際に動く」ことの証拠になります。

## 実装手順

1. `crates/omola-gui/src/backend.rs` の `#[cfg(all(test, unix))] mod tests` に、次のテストを**追加**する(既存テストは消さない)。
   - 名前: `simulate_bouncing_ball_with_real_omc`
   - 属性: `#[test]` と `#[ignore = "needs OpenModelica; run with --ignored and OMOLA_OMC set"]`
   - 先頭で `std::env::var_os("OMOLA_OMC")` が無ければ `eprintln!` して `return`(失敗にしない)。
   - `Worker::spawn(一時ディレクトリ, || {})` を作り、`Job::Simulate { source_text: include_str!("../../../examples/BouncingBall.mo").into(), class: "BouncingBall".into(), setup: SimulationSetup { stop_time: 3.0, ..Default::default() } }` を submit。
   - 既存の fake テストと同じ方法で `Event` を最大 **180 秒**待つ(OMC の C コンパイルは遅い)。`Event::Log(LogLevel::Error, msg)` が来たら `panic!("{msg}")`。
   - 受け取った `SimulationOutcome` について次を `assert!`:
     - `result.time.first() == Some(&0.0)`、`result.time.last()` が 3.0 に `1e-6` 以内
     - `result.series("height")` が存在し、最初の値が 1.0 に `1e-6` 以内(`initialHeight = 1`)
     - `t <= 2.0` のサンプルで `height >= -1e-4`、`t` が 0.44〜0.47 に `|height| < 1e-3` のサンプルがあり(初回衝突は約 0.4515 s)、`t` が 0.5〜0.8 の `height` の最大が 0.4〜0.6(反発後のピーク約 0.49)
     - **全区間で `height >= 0` を要求してはいけない**: `BouncingBall` は Zeno 対策が無く、バウンドが収束する約 2.56 s 以降は実 OMC の結果でもボールが床を突き抜ける(CI で実測: stop 3 s で最小約 -0.955)。これはモデルの挙動で、バグではない。
     - `result.index_of("gravity")` の変数が `is_parameter == true`
   - **注意**: 偽 omc テストは `std::env::set_var("OMOLA_OMC", ...)` で環境変数を書き換えます。同じプロセスで並列に走ると衝突するので、新テストは `#[ignore]` にして単独実行します(手順 2)。
2. `.github/workflows/rust.yml` の `omc-integration` ジョブの `sh -lc '...'` の中で、`cargo check --workspace &&` の**直後**に次の 1 行を追加:
   ```
   OMOLA_OMC="$(command -v omc)" cargo test -p omola-gui -- --ignored --test-threads=1 simulate_bouncing_ball_with_real_omc &&
   ```
   (シングルクォートの中なので、`'` を使わないこと。)
3. Windows ジョブ(`omc-integration-windows`)は今回は変更しない(テストが `cfg(unix)` のため)。

## 受け入れ条件

- [ ] `cargo test --workspace`(OMC なし)は今まで通り成功し、新テストは ignored と表示される。
- [ ] 開発コンテナで `OMOLA_OMC=$(command -v omc) cargo test -p omola-gui -- --ignored --test-threads=1` が成功する(PR にログ抜粋を貼る)。
- [ ] CI の `omc-integration` ジョブが緑。
- [ ] README 共通コマンド 5 つが成功。

## やってはいけないこと

- テストの中で `omc` のパスを決め打ちしない(`OMOLA_OMC` を使う)。
- 値の比較を `==` で行わない(浮動小数は許容誤差で比較)。
- 期待値を「Dymola と一致」と書かない。
