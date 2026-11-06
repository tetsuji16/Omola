# G15: リポジトリ整理(不要ファイル削除・改行コード固定・ドキュメントの現状反映)

- 難易度: 易 / 依存: なし / 触るファイル: `b`, `c`(削除), `.gitattributes`, `docs/issues/gui/README.md`, `docs/reference/dymola-gui.md`
- 先に `docs/issues/gui/README.md` を読むこと。**コードは変更しない issue です。**

## 目的

1. リポジトリ直下の空ファイル `b` と `c` を削除する(誤ってコミットされたもの。中身は 0 バイト)。
2. Rust ソースの改行コードを LF に固定する。Windows で編集すると `.rs` が丸ごと CRLF になり、差分が数千行に膨らむ事故が起きている。
3. 古くなった進捗表を、2026-11-06 時点の実装に合わせる。

## 実装手順

1. `git rm b c`
2. `.gitattributes` の末尾に次の 4 行を追加する(既存行は消さない):
   ```
   *.rs   text eol=lf
   *.toml text eol=lf
   *.md   text eol=lf
   *.json text eol=lf
   ```
   追加後に `git add --renormalize .` を実行し、`git status` で**改行だけが変わったファイルが出ないこと**を確認する(出た場合は、その差分もこの PR に含めてよい。`git diff --ignore-cr-at-eol --stat` が空であることを PR に貼る)。
3. `docs/issues/gui/README.md`:
   - 「5.5 進捗メモ」の見出し日付を 2026-11-06 にし、次の箇条を**追記**する(既存の箇条は消さない):
     - `G06/G07/G09/G10/G12/G13: 実装済み(PR #54〜#59)。`
     - `G11: テキストエディタは syntect で描画するだけになっており、ユーザーモデルが編集できない回帰がある(G23 で修正)。エラー行ジャンプは未完成(G25)。`
     - `G14: issue #42 は閉じられたが未実装(Keep/Close/Load Result が無い)。#42 を再オープンした。`
     - `GUI が重い(#60): 原因の内訳と対策は G16〜G22。`
   - 「6. issue 一覧」の表の末尾に G15〜G34 の行を追加する(このPRで追加された各 `G*.md` の 1 行目タイトル・難易度・依存をそのまま転記)。
4. `docs/reference/dymola-gui.md` の「3.2 実装状況」の表で、次の 2 行だけを書き換える:
   - `Diagram 層` の状態: `座標系グリッドのみ(G12)` → `getModelInstance の Placement と connect の Line で部品配置と結線を読み取り専用表示。ズーム・パン可。編集は不可(G28〜G33)`
   - `Text 層` の状態の末尾に `。ただし現在ユーザーモデルの編集ができない回帰あり(G23)` を追加。

## 受け入れ条件

- [ ] `b`, `c` がリポジトリから消えている。
- [ ] `.gitattributes` に 4 行が追加され、`git ls-files --eol crates | grep crlf` が何も出さない。
- [ ] README と dymola-gui.md の記述が上記どおり。
- [ ] README 共通コマンド 5 つが成功。

## やってはいけないこと

- ソースコード(`.rs`)の中身を変更しない(改行コードの正規化を除く)。
- 「Dymola 互換」と書かない。
