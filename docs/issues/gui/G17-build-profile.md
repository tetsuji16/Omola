# G17: 開発ビルドでも依存クレートを最適化する + release 実行の明記

- 難易度: 易 / 依存: なし(G16 があれば効果を数値で示す) / 触るファイル: `Cargo.toml`(ワークスペース直下), `README.md`, `docs/install.md`
- 先に `docs/issues/gui/README.md` を読むこと。親 issue: #60。

## 背景

`cargo run -p omola-gui`(debug ビルド)では egui / epaint / syntect / image などが**最適化なし**で動くため、release と比べて描画が 10 倍以上遅くなることがあります。「GUI が重い」という報告の一部はこれが原因の可能性があります。

## 実装手順

1. ワークスペース直下 `Cargo.toml` の末尾に追加:
   ```toml
   # Dependencies (egui, epaint, syntect, image, ...) are optimized even in dev builds so the
   # GUI stays responsive; our own crates keep fast incremental builds.
   [profile.dev.package."*"]
   opt-level = 2

   [profile.release]
   debug = "line-tables-only"
   ```
   - `[profile.release]` が既にある場合は `debug` 行だけ追加する(今は無い)。
2. `README.md` の Quick start 直後の「Then start the workbench with `target/release/omola-gui` ...」の文に続けて 1 文追加:
   ``When building from source, use `cargo run --release -p omola-gui`; debug builds are much slower to draw.``
3. `docs/install.md` に GUI の起動方法が書かれている箇所があれば、同じ注意を 1 行追加する(無ければ何もしない)。
4. `.github/workflows/*.yml` は変更しない。

## 受け入れ条件

- [ ] `cargo build -p omola-gui` が成功する(初回は依存の再ビルドで時間がかかるのは正常)。
- [ ] G16 がマージ済みなら、`cargo run -p omola-gui`(debug)での G16 の操作 2 の avg/max を、変更前後で PR に表で示す。
- [ ] README 共通コマンド 5 つが成功。

## やってはいけないこと

- `lto`, `codegen-units`, `panic = "abort"` など他のプロファイル設定を足さない(ビルド時間・CI に影響するため別途相談)。
