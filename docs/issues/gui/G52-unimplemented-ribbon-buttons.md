# G52: 動かないリボンボタンの整理(押しても何も起きないボタンを無くす)

- 難易度: 易 / 依存: G45, G46, G47, G48, G49 が先に入っていると作業が減る(入っていなくても進められる) / 触るファイル: `crates/omola-gui/src/ui/chrome.rs`, `src/app.rs`
- 先に `docs/issues/gui/README.md` を読むこと。

## 背景(2026-11-12 のコード全体見直しで判明)

リボンには、(a) 常に無効の飾りボタン、(b) 押すと `not_implemented` のログだけ出すボタン、が混ざっている。利用者には「壊れている」のか「まだ無い」のか区別が付かない。

2026-11-12 時点の一覧(`grep -n "not_implemented\|add_enabled(false" crates/omola-gui/src/ui/chrome.rs` で最新を確認すること):

| ボタン | 現状 | 方針 |
|---|---|---|
| Simulation: Continue, Linearize | 常に無効 | **リボンから外す**(OMC 委譲では当面実装しない) |
| Simulation: New Script | 常に無効 | 外す |
| Tools: Animation, Visualize 3D, 3D View | `not_implemented` | 外す(3D 表示は範囲外) |
| Tools: New Table | `not_implemented` | 外す |
| Documentation リボンの編集ボタン群 | 常に無効 | 外し、`Documentation is read-only` の 1 行にする |
| ウィンドウ右上の飾りアイコン(ヘルプ等) | 常に無効 | 動作が無いものは外す |
| Translate / Save in Model / Sweep Parameters / Go To / Replace / 倍率 / Fit / 折りたたみ | それぞれ G46 / G48 / G49 / G45 / G45 / G47 / G47 / G47 | **この Issue では触らない**。未着手なら無効のまま残し、ツールチップを `"Planned: GUI Gxx"` にする |

## 実装手順

1. 上の表で「外す」としたボタンを `chrome.rs` から削除する。グループが空になったらグループごと消す。区切り線が連続しないようにする。
2. 「この Issue では触らない」ボタンで、まだ無効のものは `on_disabled_hover_text("Planned: GUI Gxx")` を付ける(すでに実装済みなら何もしない)。
3. `not_implemented` を呼ぶ箇所が 0 になったら、`OmolaApp::not_implemented` 自体を削除する。
4. 回帰防止のテストを `chrome.rs` の `tests`(無ければ `app.rs`)に足す: リボンを headless に 1 フレーム描画し(既存の `egui::Context::default().run(...)` を使うテストを手本に)、**有効なボタンを全て列挙する仕組みは作らない**。代わりに単純に、ソースに対する検査 `include_str!("chrome.rs")` に `not_implemented(` が含まれないことを確認するテスト `ribbon_has_no_not_implemented_buttons` を置く。
5. `docs/reference/dymola-gui.md` の対応表で、外したボタンを「対象外」と明記する(G51 と重なる場合は後から入る側が合わせる)。

## 受け入れ条件

- [ ] リボン上の有効なボタンは、押すと必ず何かが起きる。
- [ ] 無効なボタンには、理由または予定の Issue がツールチップで出る。
- [ ] `grep -n "not_implemented" crates/omola-gui/src` が 0 件。
- [ ] 各リボンタブのスクリーンショット(Graphics / Text / Documentation / Simulation / Tools)を PR に付ける。
- [ ] README 共通コマンド 5 つが成功。
