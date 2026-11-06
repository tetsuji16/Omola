# G31: Diagram 層で部品を削除・名前変更する

- 難易度: 易〜中 / 依存: G28, G29 / 触るファイル: `crates/omola-gui/src/ui/views.rs`, `src/ui/chrome.rs`, `src/ui/dialogs.rs`, `src/app.rs`
- 先に `docs/issues/gui/README.md` を読むこと。

## 目的

1. 選択中の部品を `Delete` キー、または右クリックメニュー「Delete」で削除。その部品につながる結線も消す。
2. 右クリックメニュー「Rename...」で名前変更ダイアログを出し、OK で名前を変える(結線の端の名前も OMC が更新する)。
3. Graphics リボンの既存の無効ボタン `Delete`(`chrome.rs` の Edit グループ、`small_button(ui, ICON_CLOSE, RED, "Delete", false, false)`)を、ユーザーモデルで部品選択中なら有効にして同じ動作をさせる。

## 実装手順

1. 削除: 部品 `c` につながる結線(`diagram.connections` のうち、`from` または `to` が `c` か `c.` で始まるもの)をすべて `EditOp::DeleteConnection` にし、最後に `EditOp::DeleteComponent { name: c }` を付けて**1 回の** `apply_edit` で送る(OMC は deleteComponent だけでは connect 文を消さないため。消すかどうかを G28 の API 確認と同様に実 OMC で確かめて PR に書く)。
   - 結線抽出は純粋関数 `fn connections_of(diagram: &Diagram, component: &str) -> Vec<(String, String)>` にする。`c1` と `c10` を取り違えないこと(`starts_with("c1.")` か完全一致)。
2. 名前変更ダイアログ(`dialogs.rs` に既存の `PathDialog` と同じ作りで `RenameDialog { class, old, new_text }` を追加):
   - 入力が識別子でなければ OK を無効にし、赤字で「Not a valid Modelica identifier」。
   - 既存の部品名と重なれば OK を無効にし「Name already used」。
   - Modelica の予約語(G24 の Keyword 一覧)も不可。
   - OK で `EditOp::RenameComponent { old, new }`。
3. 右クリックメニュー: 部品の上で `response.context_menu(|ui| { ... })`(egui 0.37 の API を確認)。項目は `Rename...`, `Delete`, 区切り, `Rotate 90` (G29 の回転)。ボタン文字は ASCII のみ。
4. ライブラリクラスではメニューの項目を無効表示にする。

## テスト

- `connections_of`: `c1`/`c10` の区別、配列コネクタ `c1.p[2]`、つながっていない部品で空。
- 名前の検証関数: `abc` OK, `1a` NG, `model` NG, `a b` NG, 既存名 NG, 空 NG。
- 削除で作られる `Vec<EditOp>` の順序(DeleteConnection が先、DeleteComponent が最後)。

## 受け入れ条件

- [ ] 結線された部品を削除 → Text 層から宣言と connect 文の両方が消え、Check が通る(**実 OMC**)。
- [ ] 名前変更 → connect 文の名前も変わる。
- [ ] テストが通る。README 共通コマンド 5 つが成功。
