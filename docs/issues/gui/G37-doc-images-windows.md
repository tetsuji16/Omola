# G37: 【不具合】Documentation 層の画像が Windows で表示されない(警告アイコンになる)

- 難易度: 易 / 依存: なし / 触るファイル: `crates/omola-gui/src/ui/doc_view.rs`(必要なら `src/library.rs`)
- 先に `docs/issues/gui/README.md` を読むこと。

## 不具合の内容(2026-11-09 メンテナ報告、Windows)

`Modelica.Blocks.Examples.PID_Controller` の Documentation 層で、`<img src="modelica://Modelica/Resources/Images/Blocks/PID_controller.png">` などの画像が描かれず、**赤い警告三角**だけが出る。

## 原因の見込み

`doc_view.rs` の `image()`:

```rust
LinkTarget::File(path) if path.is_file() => {
    let uri = format!("file://{}", path.display());
```

`path.is_file()` は通っている(通らなければ `[image: ...]` の文字になる)ので、**ファイルは見つかっているが egui の画像ローダーが読めていない**。Windows では `path.display()` が `C:\Users\...` になり、`file://C:\Users\...` という不正な URI になる。egui_extras の `file` ローダーは `file://` の後ろをそのままパスとして扱うため、区切り文字やドライブ文字の扱いで失敗している可能性が高い。

## 実装手順

1. egui_extras 0.37 の file ローダーが URI をどうパスに戻すかを `~/.cargo/registry/src/*/egui_extras-0.37*/src/loaders/file_loader.rs` で読み、PR に要点を書く。
2. パス → URI の変換を関数にする: `fn file_uri(path: &Path) -> String`。ローダーの実装に合わせ、Windows でも読める形にする(例: `file://` + パスそのまま、`file:///C:/...` のどちらがローダーに合うかを 1 で確認して決める。推測で決めない)。
3. 画像の読み込みに失敗したとき、警告三角ではなく `[image: <alt または src>]` と失敗理由を tooltip で出す(`egui::Image` の `load_for_size` の結果か、`ui.ctx().try_load_image` を使う)。
4. SVG(`.svg`)の画像も同じ経路で表示されること(egui_extras の `svg` 機能は有効)。

## テスト

- `file_uri` のテスト: Unix パス、`C:\Users\a b\x.png` 形式のパス(`#[cfg(windows)]` でなく、文字列として変換関数を単体テストできるようにする)。

## 受け入れ条件

- [ ] **Windows** で PID_Controller の Documentation 層に 2 枚の画像が表示される(スクリーンショット)。Windows で確認できない場合はその旨を明記し、Linux のスクリーンショットと単体テストで代える。
- [ ] 存在しない画像は文字で表示され、パニックしない。
- [ ] テストが通る。README 共通コマンド 5 つが成功。
