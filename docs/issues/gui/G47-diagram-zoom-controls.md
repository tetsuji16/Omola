# G47: Diagram の表示操作(100% / Fit ボタン)とリボンの折りたたみ

- 難易度: 易 / 依存: なし(`diagram_layer` を触る他の Issue と同時に進めない) / 触るファイル: `crates/omola-gui/src/ui/chrome.rs`(`graphics_ribbon`), `src/app.rs`, `src/ui/views.rs`(`diagram_layer`), `src/ui/mod.rs`
- 先に `docs/issues/gui/README.md` を読むこと。

## 背景(2026-11-12 のコード全体見直しで判明)

- Graphics リボンの倍率ボタン(`"100%  ▾"`、`chrome.rs` の 611 行付近)と Fit ボタンは無効の飾り。Diagram はホイールで拡大できるが、**元の倍率に戻す手段が無い**(`diagram_zoom` / `diagram_pan` は `OmolaApp` にある)。
- リボン右端の折りたたみボタンも無効の飾り。小さい画面では編集領域が狭い。

## 実装手順

### A. 倍率

1. `app.rs` に `pub fn reset_diagram_view(&mut self)`(`diagram_zoom = 1.0; diagram_pan = egui::Vec2::ZERO;`)と `pub fn zoom_diagram(&mut self, factor: f32)`(`diagram_zoom = (diagram_zoom * factor).clamp(MIN, MAX)`。`MIN`/`MAX` は `diagram_layer` のホイール処理が使っている値と同じ定数にまとめる)を追加する。
2. 倍率ボタンを有効にし、文字を `format!("{:.0}%", self.diagram_zoom * 100.0)` にする。押すとメニュー(`egui::Popup` ではなく既存のメニューボタンの書き方、`ui.menu_button`)で `50%` / `100%` / `200%` / `Fit to window` を出す。
3. Fit ボタン: `reset_diagram_view()` を呼ぶ(`zoom == 1.0` が「座標系全体が収まる」倍率であることを `views.rs` の `diagram_scale()` で確認すること。違う場合は収まる倍率を計算する)。
4. クラスを切り替えたとき(`active` が変わったとき)に `reset_diagram_view()` を呼ぶ。
5. Icon 層では倍率ボタンを無効のままにする。

### B. リボンの折りたたみ

1. `app.rs` に `pub ribbon_collapsed: bool` を足し、`PersistedState` に `ribbon_collapsed: Option<bool>` として保存・復元する(G02 の他の項目と同じ書き方。古い保存データで壊れないよう `Option`)。
2. `chrome.rs` の折りたたみボタンを有効にして `ribbon_collapsed` を反転する。折りたたみ中はリボン本体(グループの並び)を描かず、タブの行だけ残す。ボタンの場所は `grep -n "Collapse the ribbon" crates/omola-gui/src/ui/chrome.rs` で分かる。ツールチップは状態に合わせて `Collapse the ribbon` / `Expand the ribbon` にする(新しいアイコンは足さなくてよい)。
3. 折りたたみ中にタブ名をクリックしたら、展開する(`select_tab` の中で `ribbon_collapsed = false`)。

## テスト

- `app.rs`: `zoom_diagram_is_clamped_and_reset_restores_defaults`。
- `app.rs`: `persisted_state_roundtrips_ribbon_collapsed`(既存の `persisted_state_roundtrips_setup_keep_results_and_tab` を手本に)。古い保存データ(項目なし)で `false` になることも確認する。

## 受け入れ条件

- [ ] 倍率ボタンに現在の倍率が出て、メニューから 50 / 100 / 200% と Fit を選べる。Fit ボタンで全体表示に戻る。
- [ ] リボンを折りたためる。再起動後も状態が残る。タブをクリックすると開く。
- [ ] 上のテストが通る。README 共通コマンド 5 つが成功。折りたたみ前後のスクリーンショットを PR に付ける。
