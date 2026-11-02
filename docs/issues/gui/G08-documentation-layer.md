# G08: Documentation 層の表示

- 難易度: 易 / 依存: G01 / 触るファイル: 新規 `src/doc_text.rs`, `src/backend.rs`, `src/app.rs`, `src/ui/views.rs`
- 先に `docs/issues/gui/README.md` を読むこと。

## 目的

Documentation タブで、選択クラスの `annotation(Documentation(info="<html>...</html>", revisions="..."))` を**読みやすいプレーンテキスト**で表示します。HTML レンダラは入れません(依存を増やさないため)。

## 実装手順

1. OMC 関数: `getDocumentationAnnotation(cl)` は `{info, revision, infoHeader}` の String 配列を返す(1.27.1 scripting API で確認済み)。スクリプトは
   ```
   (ライブラリ読み込み) loadFile("...");
   docs := getDocumentationAnnotation(CLASS);
   print("OMOLA_DOC_INFO_BEGIN\n" + docs[1] + "\nOMOLA_DOC_INFO_END\n");
   print("OMOLA_DOC_REV_BEGIN\n" + docs[2] + "\nOMOLA_DOC_REV_END\n");
   ```
   マーカーの間を取り出す純粋関数 `fn between(output: &str, begin: &str, end: &str) -> Option<String>` を書く。
2. 新規 `src/doc_text.rs` に `pub fn html_to_text(html: &str) -> DocText` を書く:
   ```rust
   pub struct DocText { pub paragraphs: Vec<DocPara> }
   pub enum DocPara { Heading(String), Text(String), Bullet(String), Code(String), Link { text: String, url: String } }
   ```
   - `<h1>`〜`<h6>` → Heading、`<p>` と `<br>` → 段落区切り、`<li>` → Bullet、`<pre>` → Code(中の改行を保持)、`<a href="http...">text</a>` → Link。
   - `<script>`, `<style>` の中身は捨てる。その他のタグは削除して中身の文字だけ残す。
   - 実体参照 `&lt; &gt; &amp; &quot; &#39; &nbsp;` と数値参照 `&#NNN;` `&#xHH;` を変換。
   - 連続空白は 1 個に、空の段落は除去。
   - `modelica://` リンクは Link にせず Text(「(link: modelica://...)」)にする。
   - 正規表現クレートは使わない(1 文字ずつ走査する小さなパーサで書く)。
3. `backend.rs`: `Job::LoadDocumentation { source_text, class, libraries }` → `Event::Documentation { class, info: DocText, revisions: DocText }`。
4. `ui/views.rs` の Documentation 分岐: Heading は `ui.heading`、Text は `ui.label`、Bullet は `"• "` ではなく `"- "` を前に付けた label(既定フォントに • が無い可能性があるので ASCII にする)、Code は monospace の `egui::Frame` 内、Link は `ui.hyperlink_to(text, url)`。全体を `ScrollArea::vertical`。Revisions は折りたたみ(`CollapsingHeader`)。
5. Documentation タブを開いたとき、または選択クラスが変わったときに 1 回だけ Job を送る(結果はクラス名をキーにキャッシュ)。
6. テスト(`doc_text.rs`): 少なくとも次の 6 ケース。
   - `<p>a &lt; b</p><p>c</p>` → Text("a < b"), Text("c")
   - `<ul><li>x</li><li>y</li></ul>` → Bullet×2
   - `<pre>l1\nl2</pre>` → Code("l1\nl2")
   - `<a href="https://x.org">X</a>` → Link
   - `<script>alert(1)</script>ok` → Text("ok") のみ
   - 閉じタグ欠落 `<p>abc` でもパニックしない

## 受け入れ条件

- [ ] MSL の `Modelica.Blocks.Continuous.FirstOrder` の Documentation が読める形で出る(スクリーンショット。G04 未完了ならテキストに `annotation(Documentation(info=...))` を書いた自作モデルで確認)。
- [ ] テストが通る。README 共通コマンド 5 つが成功。

## やってはいけないこと

- HTML レンダリング用の大きな依存(webview 等)を追加しない。
- ドキュメント内のスクリプトやリンク先を自動で実行・取得しない。
