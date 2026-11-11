# G50: 一時作業フォルダの掃除(古い session フォルダが残り続ける)

- 難易度: 易 / 依存: なし / 触るファイル: `crates/omola-gui/src/backend.rs`, `src/app.rs`
- 先に `docs/issues/gui/README.md` を読むこと。

## 不具合の内容(2026-11-12 のコード全体見直しで判明)

`backend.rs` の `default_work_root()` は `%TEMP%/omola-gui/session-<pid>` を返し、シミュレーションのたびに生成物(C ソース、実行ファイル、`*_res.mat`)がそこに溜まる。**終了時にも次回起動時にも消さない**ため、使うほどディスクを消費する(1 回の実行で数 MB〜数十 MB)。

## 実装手順

1. `backend.rs` に純関数を追加する:

   ```rust
   /// 掃除してよい古い session フォルダを返す。
   pub fn stale_session_dirs(root: &Path, current: &Path, now: SystemTime, max_age: Duration) -> Vec<PathBuf>
   ```

   - `root`(`<temp>/omola-gui`)の直下で、名前が `session-` で始まるディレクトリだけを対象にする。
   - `current` と同じものは除く。
   - フォルダの更新時刻が `now - max_age` より古いものだけを返す(更新時刻が取れないものは返さない)。別の Omola が動作中かもしれないので、PID の生存確認はせず**時間だけ**で判断する。`max_age` は 7 日(`const STALE_SESSION_AGE`)。
   - シンボリックリンクは対象にしない(`symlink_metadata` で `is_dir()` を見る)。
2. `pub fn remove_stale_sessions(root: &Path, current: &Path) -> usize`: 上の一覧を `std::fs::remove_dir_all` で消し、消せた数を返す。失敗は無視する(次回また試す)。
3. 呼び出し: ワーカースレッドの開始時(`Worker::spawn` のスレッド内、最初のジョブを待つ前)に 1 回だけ呼ぶ。**UI スレッドでは呼ばない**。消した数が 1 以上なら `Event::Log(LogTab::Translation, LogLevel::Info, "Removed N old work folders.")` を送る。
4. 利用者が作業フォルダを自分で指定している場合(`default_work_root()` 以外の作業フォルダを渡す経路。`grep -n "default_work_root\|work_root" crates/omola-gui/src/` で確認。2026-11-12 時点では環境変数による上書きは無い)は、**そのフォルダには一切触らない**。掃除するのは既定の `<temp>/omola-gui` の下だけ。
5. 「結果を読み込む」(G14)で開いた `.mat` が古い session フォルダの中にある場合に消してしまわないこと: 現在のセッションで読み込んだ結果のパスは `current` の外でも消さない、とまではしない(7 日より古い作業フォルダの結果は対象外と README に書く)。代わりに G14 の `Save result as…` で残したい結果を別の場所に保存できることを README の同じ箇所に書く。

## テスト(`backend.rs`、一時フォルダを使う)

- `stale_session_dirs_skips_the_current_and_recent_sessions`: `session-1`(古い)、`session-2`(現在)、`session-3`(新しい)、`other`(名前が違う)、`session-file`(ファイル)を作り、`now` を 8 日後にして `session-1` と `session-3` のうち**更新時刻が古いものだけ**が返ることを確認する。更新時刻は実際に変えず、`now` 引数を動かして検証する。
- `remove_stale_sessions_deletes_only_listed_directories`。

## 受け入れ条件

- [ ] 7 日より古い `session-*` フォルダが、次回起動後の最初のジョブまでに消える。現在のセッションと 7 日以内のフォルダは残る。
- [ ] `<temp>/omola-gui` の外、および `session-` で始まらないものは決して消さない(テストで確認)。
- [ ] 上のテスト 2 つが通る。README 共通コマンド 5 つが成功。
