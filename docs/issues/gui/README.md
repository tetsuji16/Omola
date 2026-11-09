# Omola ネイティブ GUI(egui)issue 一覧と共通ルール

`crates/omola-gui` の残作業を、小さく独立した issue に分けたものです。
**各 issue に着手する前に、このファイルを最後まで読んでください。** 各 issue 本文はこのファイルのルールを前提にしています。

## 0. 方針(2026-11-02 決定)

- Omola の GUI は **Rust + egui(eframe 0.37, egui_extras 0.37)のネイティブアプリ**として作ります。
  以前の「OMEdit を改造する」方針(`docs/gui-roadmap.md`)は、この決定で置き換えました。OMEdit 連携の CLI コマンドは残しますが、新規 GUI 機能は `omola-gui` に実装します。
- **計算(構文チェック・翻訳・シミュレーション)は OpenModelica の `omc` に委譲**します。Rust でソルバや Modelica コンパイラを書いてはいけません(README の Architecture 節)。
- 画面構成は `docs/reference/dymola-gui.md` を参照。**Dymola の画像・アイコン・文言はコピー禁止。**
- 「Dymola 互換」と書いてはいけません。測定していない機能を「対応済み」と書いてはいけません。

## 1. コードの地図

```
crates/omola-core/src/lib.rs     OMC プロセス境界(Omc::discover, check_model, simulate_project_model_with_parameters, run_script)
crates/omola-core/src/mat.rs     OpenModelica .mat(v4, binTrans/binNormal)リーダ/ライタ。std のみ
crates/omola-gui/src/main.rs     起動(eframe::run_native)
crates/omola-gui/src/app.rs      OmolaApp の状態、毎フレームのレイアウト、結果の保持(add_run)
crates/omola-gui/src/backend.rs  ワーカースレッド。Job を受けて OMC を呼び、Event を返す
crates/omola-gui/src/model.rs    Document(開いた .mo)と、クラス一覧の簡易スキャナ(表示用。パーサではない)
crates/omola-gui/src/library.rs  OMC へのライブラリ/クラス照会スクリプトと出力の解析、modelica:// の解決
crates/omola-gui/src/html.rs     Documentation の HTML を Block/Inline に変換(描画は ui/doc_view.rs)
crates/omola-gui/src/theme.rs    配色・フォント(OS から実行時に読む)・アイコンフォント
crates/omola-gui/src/ui/mod.rs       全体レイアウト(パネル配置)、枠なしウィンドウのリサイズ
crates/omola-gui/src/ui/chrome.rs    タイトル行(クイックアクセス)、リボンタブ、各リボン
crates/omola-gui/src/ui/browsers.rs  Package/Component Browser、Variable Browser
crates/omola-gui/src/ui/views.rs     モデルタブ、編集ウィンドウ(各層)、プロット
crates/omola-gui/src/ui/doc_view.rs  Documentation 層
crates/omola-gui/src/ui/bottom.rs    Logs/Commands 窓、最下段バー
crates/omola-gui/src/ui/widgets.rs   リボン部品
crates/omola-gui/src/ui/dialogs.rs   Simulation Setup、パス入力ダイアログ
crates/omola-gui/examples/demo_mat.rs  OMC なしで GUI を試すためのデモ結果生成
scripts/fake-omc.sh              開発用の偽 omc(.mat をコピーするだけ)
scripts/gui-screenshot.sh        Xvfb でヘッドレス起動してスクリーンショット
```

データの流れ: UI → `Worker::submit(Job)` → ワーカースレッドで `omola_core::Omc` → `Event` → `OmolaApp::handle_events()`(毎フレーム)。
**UI スレッドから OMC を直接呼んではいけません**(画面が固まります)。新しい OMC 呼び出しは必ず `backend.rs` に `Job` と `Event` を追加して行います。

## 2. 必ず実行するコマンド(全 issue 共通の受け入れ条件)

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p omola -- licenses check      # 依存クレートを追加・更新したときは必須
git diff --check
```

5 つすべて成功しない PR はマージしません。

## 3. OMC なしで GUI を動かす方法

```sh
cargo run -p omola-gui --example demo_mat -- target/demo_res.mat
OMOLA_FAKE_MAT="$PWD/target/demo_res.mat" OMOLA_OMC="$PWD/scripts/fake-omc.sh" \
  cargo run -p omola-gui -- examples/BouncingBall.mo
```

Windows PowerShell では fake-omc.sh は動きません。WSL か Docker の開発コンテナを使うか、本物の OpenModelica を入れて `OMOLA_OMC` に `omc.exe` を設定してください。

本物の OMC がある場合: `OMOLA_OMC=/path/to/omc cargo run -p omola-gui -- examples/BouncingBall.mo`。
開発コンテナ(`.devcontainer/Dockerfile`)には OMC 1.27.1 が入っています。

ヘッドレスでスクリーンショット(Linux, Xvfb と ImageMagick が必要):

```sh
pip install python-xlib
scripts/gui-screenshot.sh target/shot.png click:283,37 sleep:1
```

座標は 1280x800 の画面上のピクセル。既定レイアウトでの目安: リボンのタブ Graphics(41,37) Documentation(135,37) Text(217,37) Simulation(283,37) Tools(354,37)。
UI の見た目を変える issue では、**変更後のスクリーンショットを PR に添付**してください(画像はリポジトリにコミットしない。`docs/images/omola-gui/` の既存画像を更新する場合のみコミット可)。

## 4. egui 0.37 の書き方メモ(古い記事のコードは動きません)

- アプリのトレイトは `fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame)`(`update(ctx, ..)` ではない)。
- パネルは `egui::Panel::left("id")`, `Panel::right`, `Panel::top`, `Panel::bottom` + `.show(ui, |ui| ...)`。
  `SidePanel` / `TopBottomPanel` / `show_inside` は使わない。サイズは `.default_size(260.0)`、リサイズは `.resizable(true)`。
- 中央は `egui::CentralPanel::default().show(ui, |ui| ...)`。**パネルはすべて CentralPanel より前に出す。**
- ウィンドウ(ダイアログ)は `egui::Window::new("title").open(&mut bool).show(ui.ctx(), |ui| ...)`。
- メニューは `egui::MenuBar::new().ui(ui, |ui| { ui.menu_button("File", |ui| { ... }); })`。
- プロット: `egui_plot` は使いません。`src/ui/views.rs` の `plot_window()` が `Painter` で直接描きます(目盛りは `nice_step` / `minor_step` / `nice_range`)。
- アイコン: `egui_material_icons::icons::ICON_*`。ボタンは `src/ui/widgets.rs` の `big_button` / `small_button` / `icon_button` を使う。座標を合わせる必要があるリボンは `fixed_group` + `place`。
- 既定フォントには絵文字/記号が一部しかありません。ボタン文字に絵文字(▶ ✔ 💾 等)を使うと豆腐(□)になるので、**ASCII の文字だけ**を使ってください。
- API が分からないときは `~/.cargo/registry/src/*/egui-0.37.0/src/` を grep して確認する。推測で書かない。

## 5. 依存クレートを追加するとき

1. `crates/omola-gui/Cargo.toml` に追加し、`cargo build` で `Cargo.lock` を更新。
2. `cargo run -p omola -- licenses check` が「unrecorded crate」を列挙するので、`licenses/inventory.toml` の末尾に
   `[[cargo_packages]]` ブロック(name / version / license / source)を **cargo metadata の値そのまま**で追加。
   `cargo metadata --format-version 1 --locked` の `packages[].license` を使う。手で推測しない。
3. GPL/AGPL/LGPL **のみ**のライセンスのクレートは追加禁止(`MIT OR ...` のように選択肢があれば可)。迷ったら追加せず PR で質問。
4. `docs/provenance-licenses.md` の「Native GUI」節に 1 行追記。

## 5.5 進捗メモ(2026-11-06)

Dymola 風レイアウト(タイトル行・リボン・ブラウザ・モデルタブ・ログ・下段バー)と、MSL の閲覧 → Documentation → ライブラリ例題のシミュレーション → 変数ブラウザ → プロットは実装済みです(`docs/reference/dymola-gui.md` 3.2)。そのため、以下の issue は前提が変わっています。着手前に現在のコードを確認してください。

- G04: ライブラリ読み込み・木表示・検索・読み取り専用テキストは実装済み。残りは遅延読み込み・キャッシュ・アイコン描画。
- G08: HTML 描画は `src/html.rs` + `src/ui/doc_view.rs` で実装済み。残りは編集(Info Editor)。
- G09: 宣言順・Value/Unit 列・値の編集は実装済み。残りは木構造・時刻指定・Advanced。
- G10: プロットは `egui_plot` ではなく `src/ui/views.rs` の自前描画。複数ダイアグラム・カーソル・CSV 出力はこの描画に足す。
- G05: Stop time は experiment 注釈から反映済み。他の Setup 項目は未。
- G06/G07/G09/G10/G12/G13: 実装済み(PR #54〜#59)。
- G11: テキストエディタは syntect で描画するだけになっており、ユーザーモデルが編集できない回帰がある(G23 で修正)。エラー行ジャンプは未完成(G25)。
- G14: issue #42 は閉じられたが未実装(Keep/Close/Load Result が無い)。#42 を再オープンした。
- GUI が重い(#60): 原因の内訳と対策は G16〜G22。

## 6. issue 一覧(依存順)

| ID | タイトル | 難易度 | 依存 |
|---|---|---|---|
| [G01](G01-real-omc-integration-test.md) | 実 OMC で GUI バックエンドを通す結合テストを CI に追加 | 易 | — |
| [G02](G02-persist-settings.md) | 設定・最後に開いたファイルの保存と復元 | 易 | — |
| [G03](G03-native-file-dialogs.md) | ネイティブのファイルダイアログと「最近使ったファイル」 | 易 | — |
| [G04](G04-library-package-browser.md) | MSL などのライブラリ読み込みと Package Browser の木表示 | 中 | G01 |
| [G05](G05-full-simulation-setup.md) | Simulation Setup の全項目を OMC に渡す + experiment 注釈の読み込み | 中 | G01 |
| [G06](G06-component-browser.md) | Component Browser(getModelInstance の JSON を使う) | 中 | G01 |
| [G07](G07-icon-layer-rendering.md) | Icon 層の描画(グラフィックス注釈の読み取り専用レンダリング) | 中〜難 | G06 |
| [G08](G08-documentation-layer.md) | Documentation 層の表示 | 易 | G01 |
| [G09](G09-variable-browser-tree.md) | Variable Browser の木構造化・パラメータ編集から再シミュレーション | 中 | — |
| [G10](G10-plot-window.md) | プロット強化(複数ダイアグラム・カーソル・CSV 出力) | 中 | — |
| [G11](G11-text-editor.md) | テキストエディタ(シンタックスハイライト・行番号・エラー行ジャンプ) | 中 | — |
| [G12](G12-diagram-layer-readonly.md) | Diagram 層の読み取り専用表示(部品配置と結線) | 難 | G06, G07 |
| [G13](G13-cancel-simulation.md) | 実行中のシミュレーションの中断 | 中 | — |
| [G14](G14-result-management.md) | 結果の読み込み・保持/破棄・エクスポート | 易 | — (#42 を再オープン。未実装) |
| [G15](G15-repo-housekeeping.md) | リポジトリ整理(不要ファイル削除・改行コード固定・ドキュメントの現状反映) | 易 | — |
| [G16](G16-frame-time-stats.md) | フレーム時間の計測機能(性能改善 #60 の物差し) | 易 | — |
| [G17](G17-build-profile.md) | 開発ビルドでも依存クレートを最適化する + release 実行の明記 | 易 | (G16) |
| [G18](G18-library-index.md) | ライブラリのクラス索引(class / children / has_children を O(1) に) | 易〜中 | — |
| [G19](G19-package-browser-per-frame.md) | Package Browser の毎フレームの無駄をなくす | 中 | G18 |
| [G20](G20-variable-browser-cache.md) | Variable Browser の木を毎フレーム作り直さない | 中 | — |
| [G21](G21-plot-no-copy-decimate.md) | プロットで曲線データを毎フレーム複製しない + 画素幅で間引く | 中 | — |
| [G22](G22-log-pane-limits.md) | ログ窓の上限と、見えている行だけの描画 | 易 | — |
| [G23](G23-text-editor-editable.md) | 【不具合・最優先】Text 層でユーザーモデルが編集できない(G11 の回帰)を直す | 中 | — |
| [G24](G24-modelica-tokenizer.md) | Modelica 専用のトークナイザで色分けする | 中 | G23 |
| [G25](G25-error-line-jump.md) | 【不具合】OMC のエラー行の抽出を直し、Messages からエラー行へジャンプ | 中 | G23 |
| [G26](G26-diagram-placement-origin.md) | 【不具合】Diagram 層: origin が省略された部品が原点に重なる | 易 | — |
| [G27](G27-windows-job-object-cancel.md) | Windows でシミュレーション中断時に子プロセスまで止める(Job Object) | 中 | — |
| [G28](G28-model-edit-backend.md) | モデル編集の基盤(OMC の編集 API を通してテキストを書き換える Job) | 中 | G23 |
| [G29](G29-diagram-select-move.md) | Diagram 層で部品を選択・移動・回転する | 中 | G28, G26 |
| [G30](G30-diagram-add-component.md) | Package Browser から Diagram 層へドラッグして部品を追加する | 中 | G28 |
| [G31](G31-diagram-delete-rename.md) | Diagram 層で部品を削除・名前変更する | 易〜中 | G28, G29 |
| [G32](G32-diagram-connect.md) | Diagram 層でコネクタ間をドラッグして結線する / 結線を削除する | 中〜難 | G28, G29 |
| [G33](G33-component-parameter-dialog.md) | 部品のパラメータダイアログ | 中 | G28 |
| [G34](G34-undo-redo.md) | モデル編集の Undo / Redo | 易〜中 | G28 |
| [G35](G35-plot-options-tab.md) | 【不具合】Plot Options タブがクリックできない → プロット設定のリボン | 中 | — |
| [G36](G36-diagram-component-icons.md) | 【不具合】Diagram 層で部品のアイコンが描かれず空の四角になる | 中 | — |
| [G37](G37-doc-images-windows.md) | 【不具合】Documentation 層の画像が Windows で表示されない | 易 | — |
| [G38](G38-variable-browser-value-overflow.md) | 【不具合】Variable Browser の値が長い指数表記で名前に重なる | 易 | — |
| [G39](G39-plot-legend.md) | プロットの凡例: 折り返し・見分けられる色と線種 | 易〜中 | — |
| [G40](G40-library-text-readonly-editor.md) | ライブラリのクラスの Text 層を行番号・色分け付きの読み取り専用エディタで | 易〜中 | G24 |
| [G41](G41-title-and-tab-overlap.md) | 【不具合】タイトル行・下段モデルタブの文字の重なり | 易 | — |

並行して進めてよいもの: G02, G03, G09, G10, G11, G13, G14 は互いに独立。

### 2026-11-06 以降の進め方(G14〜G34)

1. **最初に**: G15(整理)と G16(計測)。G16 の PR に載せる基準値を、性能 issue すべてが比較に使う。
2. **#60「GUI が重い」の対策**: G17 → G18 → G19、並行して G20 / G21 / G22。
3. **不具合**: G23 を最優先(ユーザーモデルが編集できない)。G23 の後に G24・G25。G26 は独立。
4. **残り機能**: G14(再)・G27 は独立。
5. **Diagram 編集**: G28 → (G29, G30, G33, G34) → G31, G32。

### 2026-11-09 追加(実 OMC・Windows での画面確認から、G35〜G41)

PID_Controller を MSL から開いて Graphics / Documentation / Text / Simulation を確認した結果です。

6. **不具合を先に**: G36(Diagram のアイコン)・G37(ドキュメント画像)・G38(値の重なり)・G41(タイトルの重なり)は互いに独立。
7. **プロット**: G35(Plot Options)→ G39(凡例)。どちらも `views.rs` の `plot_window()` を触るので同時に進めない。
8. **Text 層**: G40 は G24 のマージ後。

同じファイルを触るため、同時に進めると衝突しやすい組み合わせ: G19/G20(`browsers.rs`)、G20/G22/G14(`app.rs`)、G23/G24/G25(`text_editor.rs`)、G29〜G32・G36(`views.rs` の `diagram_layer` / `diagram_view.rs`)、G35/G39(`plot_window`)、G23/G24/G40(`text_editor.rs`)。
**同じファイルを触る issue を同時に進めると衝突します。** PR は小さく、1 issue = 1 PR にしてください。

## 7. PR の書き方

- タイトル: `GUI GXX: <issue タイトル>`(例: `GUI G18: ...`)
- 本文: 何を変えたか / 受け入れ条件のチェックリスト(すべてに [x] と根拠)/ 実行したコマンドと結果 / スクリーンショット(UI 変更時)/ 未対応のこと
- 実 OMC で確認していない場合は「fake-omc でのみ確認」と**明記**する。
