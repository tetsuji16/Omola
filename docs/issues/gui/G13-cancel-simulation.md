# G13: 実行中のシミュレーションの中断

- 難易度: 中 / 依存: なし / 触るファイル: `crates/omola-core/src/lib.rs`, `crates/omola-cli/src/*.rs`(match 追加のみ), `crates/omola-gui/src/backend.rs`, `src/app.rs`, `src/ui/chrome.rs`
- 先に `docs/issues/gui/README.md` を読むこと。

## 目的

長いシミュレーションを GUI の **Stop** ボタンで中断できるようにします。今は `Command::output()` で OMC の終了を待つだけなので止められません。

## 背景

- `omola-core` の `Omc::invoke_in` が `Command::new(..).output()` で OMC を起動しています。
- OMC はシミュレーション実行ファイルを**子プロセス**として起動します。OMC だけ kill しても孫プロセス(シミュレーション本体)が残ることがあります。
  - Unix: OMC を**新しいプロセスグループ**で起動し、グループごと終了させる。
  - Windows: この issue では OMC の `Child::kill()` のみ。孫が残る可能性を README とコードコメントに明記(Job Object 対応は別 issue)。

## 実装手順

1. omola-core:
   - `BackendError` に `Cancelled` を追加。`Display` は `"Operation cancelled by the user."`。
     `grep -rn "BackendError::" crates/omola-cli` で網羅的 match があれば `Cancelled` の腕を追加してコンパイルを通す(CLI の挙動は変えない)。
   - `Omc` にフィールド `cancel: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>` を追加し、
     `pub fn with_cancel_flag(mut self, flag: Arc<AtomicBool>) -> Self` を追加。`Omc` は `Clone` なのでそのまま derive でよい。
   - `invoke_in` を次の方針で書き換える(cancel が `None` のときは**今と同じ** `output()` を使い、挙動を変えない):
     ```rust
     let mut command = Command::new(&self.executable);
     command.args(args).stdout(Stdio::piped()).stderr(Stdio::piped());
     if let Some(dir) = current_dir { command.current_dir(dir); }
     #[cfg(unix)]
     { use std::os::unix::process::CommandExt; command.process_group(0); }
     let mut child = command.spawn().map_err(|source| BackendError::LaunchFailed { .. })?;
     // stdout / stderr はパイプが詰まらないよう別スレッドで最後まで読む
     let mut out = child.stdout.take().unwrap();
     let mut err = child.stderr.take().unwrap();
     let out_t = std::thread::spawn(move || { let mut b = Vec::new(); let _ = std::io::Read::read_to_end(&mut out, &mut b); b });
     let err_t = std::thread::spawn(move || { let mut b = Vec::new(); let _ = std::io::Read::read_to_end(&mut err, &mut b); b });
     loop {
         if let Some(status) = child.try_wait()? { /* out_t/err_t を join して Output を作って返す */ }
         if flag.load(Ordering::Relaxed) {
             kill_tree(&mut child);
             let _ = child.wait();
             return Err(BackendError::Cancelled);
         }
         std::thread::sleep(std::time::Duration::from_millis(50));
     }
     ```
   - `kill_tree`: Unix では `child.id()` をプロセスグループ ID として `Command::new("kill").args(["-TERM", &format!("-{}", pid)])` を実行し、200ms 待って生きていれば `-KILL`。Windows では `child.kill()`。
   - `std::process::Output` は `ExitStatus` から直接作れるので、`Output { status, stdout, stderr }` で組み立てる。
2. テスト(omola-core, `#[cfg(unix)]`): 既存の fake 実行ファイルのテストを参考に、`sleep 30` するシェルスクリプトを偽 omc として作る(`--version` には即答)。別スレッドで 0.3 秒後にフラグを立て、`run_script` が **2 秒以内**に `Err(BackendError::Cancelled)` で戻ることを確認。さらにスクリプトが起動した子 `sleep` も終了していること(`pgrep -f` などで確認できなければ、このアサートは省略して理由をコメント)。
3. GUI:
   - `Worker` に `cancel: Arc<AtomicBool>` を持たせ、Job 開始時に `false` に戻し、`Omc::discover()?.with_cancel_flag(cancel.clone())` を使う。
   - `pub fn cancel(&self)` でフラグを立てる。
   - `Event` に変換: `BackendError::Cancelled` は `LogLevel::Warning` で「Simulation stopped.」。
   - リボン(Simulation タブ)とステータスバーの「Running OMC…」の横に `Stop` ボタン(`busy` のときだけ有効)。
4. 中断後に run ディレクトリが残っても構わない(次回の run は別ディレクトリ)。

## 受け入れ条件

- [ ] 実 OMC で `stopTime=1e6` の重いモデル(例: BouncingBall を stopTime 1e7、`numberOfIntervals` 大)を Simulate → Stop で 2 秒以内に Ready に戻り、`ps` にシミュレーション実行ファイルが残っていない(Linux。PR に `ps` の結果)。
- [ ] cancel を使わない CLI の全テスト・CI が今まで通り通る。
- [ ] テストが通る。README 共通コマンド 5 つが成功。

## やってはいけないこと

- `cancel == None` の経路(CLI)を変えない。
- `libc` など新しい依存を追加しない(`kill` コマンドで足りる)。
