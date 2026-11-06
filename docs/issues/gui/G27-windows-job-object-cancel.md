# G27: Windows でシミュレーション中断時に子プロセスまで止める(Job Object)

- 難易度: 中 / 依存: なし(G13 は実装済み) / 触るファイル: `crates/omola-core/src/lib.rs`, `crates/omola-core/Cargo.toml`, `licenses/inventory.toml`, `docs/provenance-licenses.md`, `README.md`
- 先に `docs/issues/gui/README.md` を読むこと。

## 背景

G13(PR #59)で Stop ボタンを実装しました。Unix はプロセスグループごと止めますが、Windows は `Child::kill()` で **omc.exe だけ**を止めるため、omc が起動したシミュレーション実行ファイル(`<Model>.exe`)が残って CPU を使い続けることがあります(README と `kill_tree` のコメントに記載あり)。

## 実装手順

1. 依存: `windows-sys`(MIT OR Apache-2.0)を **Windows のときだけ**追加する:
   ```toml
   [target.'cfg(windows)'.dependencies]
   windows-sys = { version = "0.59", features = ["Win32_Foundation", "Win32_System_JobObjects", "Win32_System_Threading", "Win32_Security"] }
   ```
   - Cargo.lock に既に `windows-sys` の別バージョンがあれば、**そのバージョンに合わせる**(`grep -A1 'name = "windows-sys"' Cargo.lock`)。新しいバージョンを増やさない。
   - README §5 の手順で `licenses/inventory.toml` と `docs/provenance-licenses.md` を更新。
2. `lib.rs` の cancel 対応の起動処理(`invoke_in` の cancel が `Some` の分岐)で、Windows のとき:
   1. `CreateJobObjectW(null, null)` でジョブを作る。
   2. `SetInformationJobObject` で `JOBOBJECT_EXTENDED_LIMIT_INFORMATION.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` を設定。
   3. `child = Command::spawn()` の直後に `AssignProcessToJobObject(job, child.as_raw_handle() as HANDLE)`(`std::os::windows::io::AsRawHandle`)。
   4. キャンセル時は `TerminateJobObject(job, 1)` を呼び、その後 `child.wait()`。
   5. 正常終了・エラー・キャンセルのいずれでも最後に `CloseHandle(job)`(`Drop` を実装した小さな構造体 `JobHandle(HANDLE)` で包むと漏れない)。
   - 各 API が失敗したら(戻り値 0 / null)、ジョブを使わず従来どおり `Child::kill()` にフォールバックし、`eprintln!` ではなく既存のエラー/警告の仕組みで「job object unavailable」を返すか無視する(シミュレーション自体は止めない)。
   - `unsafe` ブロックには必ず `// SAFETY:` コメントを書く。
3. `#[cfg(windows)]` のテストを追加: 偽 omc として `cmd /C ping -n 30 127.0.0.1 > NUL` を起動するスクリプト(`.bat` をテスト内で一時ディレクトリに書く)を `OMOLA_OMC` 相当に指定し、0.3 秒後にキャンセル → 2 秒以内に `Cancelled` が返り、`tasklist /FI "IMAGENAME eq PING.EXE"` に残っていないことを確認する。
   - CI(`.github/workflows/rust.yml`)の windows-latest で `cargo test --workspace` に含まれて実行される。
4. README の G13 関連の注意書き(「Windows は OMC のみ ... 残る可能性」)を「Windows は Job Object でまとめて停止」に更新。`kill_tree` のコメントも更新。

## 受け入れ条件

- [ ] Windows の CI で新しいテストが通る(CI のログのリンクを PR に貼る)。
- [ ] 実 OMC で `stopTime=1e7` の BouncingBall を Stop → タスクマネージャに `BouncingBall.exe` が残らない(できれば確認。できなければ「CI のテストのみ」と明記)。
- [ ] Linux/macOS の動作は変わらない。README 共通コマンド 5 つが成功。

## やってはいけないこと

- `windows` クレート(大きい方)は使わない。`windows-sys` のみ。
- cancel を使わない CLI の経路(`Command::output()`)は変更しない。
