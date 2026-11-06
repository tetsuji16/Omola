# G16: フレーム時間の計測機能(性能改善 #60 の物差し)

- 難易度: 易 / 依存: なし / 触るファイル: 新規 `crates/omola-gui/src/perf.rs`, `src/main.rs`(`mod perf;` の追加のみ), `src/app.rs`
- 先に `docs/issues/gui/README.md` を読むこと。
- 親 issue: #60「gui処理が重い」。G17〜G22 の効果をこの機能で測ります。**G17〜G22 より先にマージしてください。**

## 目的

環境変数 `OMOLA_FRAME_STATS=1` を付けて起動すると、1 秒ごとに標準エラーへ「そのフレーム数・平均・最大の描画時間」を出す。
既定(未設定)では何もしない(オーバーヘッドなし)。

出力例(1 行 / 秒):
```
[omola-frame] frames=58 avg=4.21ms max=11.80ms tab=Simulation layer=Diagram
```

## 実装手順

1. 新規 `src/perf.rs`:
   ```rust
   //! Optional per-frame timing (`OMOLA_FRAME_STATS=1`), printed to stderr once a second.
   use std::time::{Duration, Instant};

   pub struct FrameStats {
       enabled: bool,
       window_start: Instant,
       frames: u32,
       total: Duration,
       max: Duration,
   }

   impl FrameStats {
       pub fn from_env() -> Self { /* enabled = std::env::var_os("OMOLA_FRAME_STATS").is_some_and(|v| v == "1") */ }
       pub fn disabled() -> Self { /* テスト用。enabled=false */ }
       /// 1 フレームの所要時間を記録する。1 秒以上たっていたら集計結果を返し、カウンタを 0 に戻す。
       pub fn record(&mut self, elapsed: Duration, now: Instant) -> Option<Summary> { ... }
   }

   #[derive(Debug, Clone, PartialEq)]
   pub struct Summary { pub frames: u32, pub avg_ms: f64, pub max_ms: f64 }
   ```
   - `record` は `enabled == false` なら即 `None` を返す。
   - `now` を引数で受けるのはテストのため(`Instant::now()` を中で呼ばない)。
2. `app.rs`:
   - `OmolaApp` に `pub frame_stats: crate::perf::FrameStats` を追加。`with_worker` では `FrameStats::disabled()`、`new` では `FrameStats::from_env()` を入れる。
   - `impl eframe::App for OmolaApp` の `fn ui(...)` の**先頭**で `let started = std::time::Instant::now();`、**末尾**で
     ```rust
     if let Some(s) = self.frame_stats.record(started.elapsed(), std::time::Instant::now()) {
         eprintln!("[omola-frame] frames={} avg={:.2}ms max={:.2}ms tab={} layer={}",
             s.frames, s.avg_ms, s.max_ms, self.tab.label(), self.layer().label());
     }
     ```
     (`self.layer()` は既存のメソッド。無い場合は layer 部分を省略してよい)
3. `README.md` の GUI 節(`omola-gui` を説明している箇所)に 1 行: ``性能を調べるときは `OMOLA_FRAME_STATS=1` を付けて起動すると、1 秒ごとの描画時間が標準エラーに出ます。``
4. テスト(`perf.rs` の `#[cfg(test)]`):
   - disabled は常に `None`。
   - enabled で 0.5 秒後の record は `None`、1.0 秒後で `Some`、frames と max が正しい。
   - Summary を返した後はカウンタが 0 から再開する。
   (テストでは `FrameStats` のフィールドに直接 `enabled: true` を入れるか、`pub(crate) fn enabled_for_test(start: Instant)` を用意する)

## 受け入れ条件

- [ ] `OMOLA_FRAME_STATS=1 cargo run --release -p omola-gui` で 1 秒ごとに行が出る(PR に出力を 5 行ほど貼る)。
- [ ] 環境変数なしでは何も出ない。
- [ ] テストが通る。README 共通コマンド 5 つが成功。

## PR に必ず書くこと(後続 issue の基準値になります)

次の 4 つの操作中の `avg` / `max` を**release ビルドで**測って表にしてください(fake-omc で可。どの環境で測ったか明記):
1. 起動直後、マウスを動かし続ける
2. Package Browser で `Modelica` → `Blocks` → `Examples` を展開した状態でマウスを動かす
3. `Modelica.Blocks.Examples.PID_Controller` をシミュレーションし、10 本の変数をプロットした状態でマウスを動かす
4. Text タブで 1000 行以上の .mo を開いてスクロール

## やってはいけないこと

- 新しい依存クレート(puffin, tracing 等)を追加しない。
