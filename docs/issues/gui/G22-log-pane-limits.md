# G22: ログ窓の上限と、見えている行だけの描画

- 難易度: 易 / 依存: なし / 触るファイル: `crates/omola-gui/src/app.rs`, `src/ui/bottom.rs`
- 先に `docs/issues/gui/README.md` を読むこと。親 issue: #60。

## 背景(現在のコード)

- `app.rs` の `logs: Vec<LogEntry>` は無制限に増えます(`info` / `error` / `handle_events` で `push`)。
- `ui/bottom.rs` の `logs_pane()` は選択タブの全エントリを毎フレーム `ui.label` で描きます。シミュレーションを繰り返すと、1 回で数千行のログが積み上がります。

## 実装手順

1. `app.rs` に定数 `pub const MAX_LOG_ENTRIES: usize = 2000;` と、1 エントリの最大文字数 `pub const MAX_LOG_ENTRY_CHARS: usize = 200_000;` を追加。
2. `logs.push(...)` をしている箇所をすべて新しいメソッド `fn push_log(&mut self, entry: LogEntry)` 経由にする(`grep -n "logs.push" crates/omola-gui/src` で全部探す)。`push_log` は:
   - `entry.text` が `MAX_LOG_ENTRY_CHARS` を超えたら、**先頭側を捨てて**末尾 `MAX_LOG_ENTRY_CHARS` 文字だけ残し、先頭に `"... (earlier output truncated)\n"` を付ける。文字境界で切ること(`char_indices` を使う。バイト位置で切るとパニックする)。
   - 追加後 `logs.len() > MAX_LOG_ENTRIES` なら古いものから削除(`drain(..n)`)。
3. `bottom.rs` の描画:
   - `self.logs.iter().filter(|e| e.tab == self.log_tab)` の結果のインデックスを `Vec<usize>` に集め、`ScrollArea::vertical().stick_to_bottom(true)` を付ける(新しいログが来たら下に追従)。
   - エントリは高さが一定でないため `show_rows` は使えません。代わりに `ScrollArea::show_viewport` を使い、**各エントリの高さを `galley` の高さで計算してキャッシュ**する方式は難しいので、この issue では次の簡易方式にする: 選択タブのエントリが 300 件を超えたら、古い方を描かずに先頭へ `"(N older entries hidden — use Copy All to see everything)"` を 1 行出す。
   - ログ窓の右クリックメニュー(または既存のボタン列)に `Copy All`(選択タブの全文を `ui.ctx().copy_text(...)`)と `Clear`(選択タブのエントリを削除)を追加する。ボタン文字は ASCII のみ。
4. G25 でエラー行の抽出に `logs` を使うため、`LogEntry` の構造は変えない。

## テスト(`app.rs` の tests)

- 2001 件 push すると 2000 件になり、最初の 1 件が消えている。
- 200_001 文字の日本語を含むテキストを push してもパニックせず、末尾が残り、先頭に truncated の印がある。
- Clear で選択タブだけ消える。

## 受け入れ条件

- [ ] 同じモデルを 20 回シミュレーションした後でも、G16 で測ったログ窓表示中の avg が 1 回目と大きく変わらない(PR に数値)。
- [ ] テストが通る。README 共通コマンド 5 つが成功。
