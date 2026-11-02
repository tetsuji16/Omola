# G03: ネイティブのファイルダイアログと「最近使ったファイル」

- 難易度: 易 / 依存: なし(G02 と同時なら G02 を先にマージ) / 触るファイル: `crates/omola-gui/Cargo.toml`, `src/ui/dialogs.rs`, `src/ui/chrome.rs`, `src/app.rs`, `licenses/inventory.toml`, `docs/provenance-licenses.md`
- 先に `docs/issues/gui/README.md`(特に「5. 依存クレートを追加するとき」)を読むこと。

## 目的

いまの Open / Save as はパス文字列を打ち込む仮ダイアログ(`PathDialog`)です。OS 標準のファイル選択ダイアログに置き換え、File メニューに「最近使ったファイル」(最大 8 件)を追加します。

## 実装手順

1. 依存追加: `rfd = { version = "0.15", default-features = false, features = ["xdg-portal"] }`。
   - まず `cargo search rfd` で最新版を確認し、**egui/eframe 0.37 と同時にビルドできる最新版**を使う。
   - Linux で `gtk3` 機能は使わない(システムの GTK 開発パッケージが必要になり CI が壊れる)。`xdg-portal` で足りるか `cargo build` で確認。ビルドできない場合は PR で報告して止まる。
   - `licenses check` の指示どおり inventory を更新。
2. **UI スレッドをブロックしない**: `rfd::FileDialog::pick_file()` はブロッキングです。次のどちらかで実装する。
   - (推奨)`backend.rs` とは別に、`std::thread::spawn` でダイアログを開き、結果を `std::sync::mpsc::Sender<DialogResult>` で返す。`OmolaApp` に `dialog_rx: Option<Receiver<DialogResult>>` を持たせ、毎フレーム `try_recv` する。
   - `enum DialogResult { Open(Option<PathBuf>), SaveAs(Option<PathBuf>) }`
   - フィルタ: `.add_filter("Modelica", &["mo"])`。Save as は `.set_file_name(現在のファイル名 or "Unnamed.mo")`。
3. `PathDialog` と `path_dialog_window` は削除せず、**環境変数 `OMOLA_GUI_TEXT_DIALOGS=1` のときだけ**使うフォールバックとして残す(ヘッドレス環境・ポータルが無い環境用)。
4. 最近使ったファイル:
   - `OmolaApp` に `recent_files: Vec<PathBuf>` を追加。`open_file` 成功時と Save as 成功時に先頭へ追加(重複は削除、最大 8 件)。
   - File メニューに `ui.menu_button("Recent files", ...)` を追加し、各パスをボタンにする。存在しないファイルは `add_enabled(false, ...)` で灰色表示。
   - G02 がマージ済みなら `PersistedState` に `recent_files` を追加して保存する。未マージならメモリ上だけでよい。
5. テスト: 「最近使ったファイル」の追加ロジックを `fn push_recent(list: &mut Vec<PathBuf>, path: PathBuf)` という純粋関数にし、重複除去と 8 件上限をテストする。

## 受け入れ条件

- [ ] Linux(デスクトップ環境あり)と Windows で File > Open… が OS のダイアログを開き、選んだ .mo が編集ウィンドウに表示される(PR にスクリーンショット)。
- [ ] ダイアログを開いている間も GUI が再描画される(固まらない)。
- [ ] `OMOLA_GUI_TEXT_DIALOGS=1` で従来のテキスト入力ダイアログになる。
- [ ] `push_recent` のテストが通る。README 共通コマンド 5 つ(`licenses check` 含む)が成功。

## やってはいけないこと

- GTK などシステムライブラリ必須の機能を有効にしない。
- UI スレッドで `pick_file()` を直接呼ばない。
