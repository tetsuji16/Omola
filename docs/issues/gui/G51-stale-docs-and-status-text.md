# G51: 古くなった文書と状態表示の更新

- 難易度: 易 / 依存: なし(コードの挙動は変えない) / 触るファイル: `docs/gui-roadmap.md`, `docs/reference/dymola-gui.md`, `README.md`, `crates/omola-cli/src/gui.rs`, `crates/omola-gui/src/app.rs`
- 先に `docs/issues/gui/README.md` を読むこと。

## 背景(2026-11-12 のコード全体見直しで判明)

G01〜G41 がほぼ完了した後も、文書と CLI の状態表示が古いまま残っている。新しい実装者(人でもエージェントでも)が誤った前提で作業を始めてしまう。

## 実装手順

1. `docs/gui-roadmap.md`: 冒頭の「G01–G14」という範囲の記述を現状(G01〜G41 完了、G14 と G42 以降が残り)に直す。各段階の表に「完了」列が無ければ足し、完了済みの項目に Issue 番号を書く。残りの作業は `docs/issues/gui/README.md` §6 を指すだけにして、二重管理をしない。
2. `docs/reference/dymola-gui.md` §3.2(Simulation タブの対応表): 「未実装」となっている行を、現在のコードに合わせて直す。少なくとも次を確認する — Simulation Setup(実装済み)、Stop(実装済み)、結果の Keep / Close / Load(骨格あり、G14)、Translate(Check と同じ、G46)、Save in Model(G48)、Sweep Parameters(G49)、Continue / Linearize / Animation / 3D(対象外、G52)。**Dymola の画面や文言を新たに書き写さないこと**(README の規則)。
3. `crates/omola-cli/src/gui.rs`: `omola gui` の状態表示の文章が、クローズ済みの Issue(#15, #16, #19, #22, #25)を「これからの作業」として挙げている。その文を削り、`docs/issues/gui/README.md` を見るよう 1 行にする。この文字列を検査しているテスト(`grep -rn "#15\|#16\|#19\|#22\|#25" crates/omola-cli/`)があれば一緒に直す。
4. `crates/omola-gui/src/app.rs`: `apply_edit` に付いている `#[allow(dead_code)]` は、すでに使われているはずなので外す(G48 が先に外していれば何もしない)。外して `cargo clippy` が通ることを確認する。他の `#[allow(dead_code)]` も `grep -rn "allow(dead_code)" crates/` で一覧にし、**外しても clippy が通るものだけ**外す(通らないものは残し、PR に一覧を書く)。
5. `README.md` の GUI 節: 「できること / まだできないこと」を現状に合わせる。まだできないこととして、Translate の分離(G46)、Sweep(G49)、Animation / 3D(予定なし)を挙げる。

## テスト

- 文書のみの変更には新しいテストは不要。
- `gui.rs` の文字列を変えた場合は、既存のテストを新しい文面に合わせる。

## 受け入れ条件

- [ ] `grep -rn "G01–G14\|G01-G14" docs/` が 0 件。
- [ ] `omola gui --help`(または状態表示)に、クローズ済みの Issue 番号が「未着手」として出ない。
- [ ] 外せる `#[allow(dead_code)]` が外れている(残したものは PR に理由付きで列挙)。
- [ ] README 共通コマンド 5 つが成功。
