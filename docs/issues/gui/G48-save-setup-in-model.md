# G48: Save in Model(Simulation Setup を experiment アノテーションに書き戻す)

- 難易度: 中 / 依存: G44 を先に(同じ `setup_dialog` を触る) / 触るファイル: `crates/omola-gui/src/edit.rs`, `src/backend.rs`(テストのみ), `src/app.rs`, `src/ui/chrome.rs`, `src/ui/dialogs.rs`
- 先に `docs/issues/gui/README.md` を読むこと。

## 背景(2026-11-12 のコード全体見直しで判明)

モデルの `annotation(experiment(StartTime=…, StopTime=…, Tolerance=…, Interval=…))` は読み込んで Setup に反映している(G05)が、**GUI で変えた設定をモデルに書き戻す手段が無い**。Simulation リボンの `Save in Model` ボタンは常に無効。

## 方針

モデルの書き換えは、他の編集(G28〜G34)と同じく **OMC の編集 API を通す**(`edit.rs` の `EditOp` → `edit_script` → `Job::EditModel` → `Event::ModelEdited`)。Rust 側で Modelica のテキストを直接書き換えない。Undo(G34)は `Event::ModelEdited` の処理が既に面倒を見ている。

## 実装手順

1. `edit.rs` の `EditOp` に追加する:

   ```rust
   SetExperiment { start: f64, stop: f64, tolerance: f64, interval: f64 },
   ```

2. `edit_script` の `match` に腕を足す。生成する呼び出しは次の形(数値は既存の `n()` を通す。非有限なら `Err`):

   ```
   addClassAnnotation(<class>, annotate=experiment(StartTime=<start>, StopTime=<stop>, Tolerance=<tolerance>, Interval=<interval>))
   ```

   成功判定は他の腕と同じ書き方(`omolaEdit{i}`)に合わせる。`interval <= 0` または `stop <= start` は `Err("experiment values are not valid")`。
3. `app.rs` に `pub fn save_setup_in_model(&mut self)`:
   - アクティブなクラスが無い、またはユーザークラスでない → `self.error(LogTab::Syntax, "Only a user model can store the simulation setup.".into())`。
   - `self.setup.validate()` が `Err` → そのメッセージを出して終わる。
   - `interval = (stop - start) / intervals as f64` を計算し、`self.apply_edit(&class, vec![EditOp::SetExperiment { .. }])` を呼ぶ。
   - `apply_edit` の `#[allow(dead_code)]` は外す。
4. `Event::ModelEdited` で書き換わったテキストから experiment が読み直されるとき、**利用者が Setup に入れた値が変わらない**ことを確認する(書いた値と同じはずだが、`setup_edited_by_user` の扱いで Method などが既定に戻らないこと)。
5. `chrome.rs` の `Save in Model` ボタンを「ユーザークラスが開いていて、`pending_edit` が `None`」のときだけ有効にして `save_setup_in_model()` を呼ぶ。無効時のツールチップは `"Open a user model to store the setup"`。`setup_dialog` にも `Store in model` ボタンを足す(同じ条件)。

## 確認が必要な点(実 OMC で調べて PR に結果を書く)

- `addClassAnnotation(..., annotate=experiment(...))` が、クラスの**他のアノテーション**(`Icon(...)`、`Documentation(...)`、`__OpenModelica_simulationFlags(...)`)を消さないこと。消える場合はこの方法を使わず、issue にコメントして止まる(テキストの直接書き換えには進まない)。
- `experiment(...)` の中の未知の引数(例: `__Dymola_Algorithm="Dassl"`)は、この呼び出しで消える可能性がある。消える場合は、`Save in Model` の実行前に `"Other experiment settings in the model will be replaced."` を Info ログに出す。

## テスト

- `edit.rs`: `set_experiment_script_calls_add_class_annotation`(生成文字列に `addClassAnnotation(M, annotate=experiment(StartTime=0` が含まれる)。
- `edit.rs`: `set_experiment_rejects_non_finite_and_reversed_times`。
- `app.rs`: `save_setup_in_model_is_refused_for_library_classes`(ジョブが投入されず、エラーログが出る)。
- `backend.rs`(`#[ignore]`、実 OMC。既存の `Job::EditModel` の実 OMC テストを手本に): `set_experiment_keeps_other_annotations_with_real_omc` — `model M Real x; equation der(x)=1; annotation(Icon(graphics={Rectangle(extent={{-10,-10},{10,10}})}), experiment(StopTime=1)); end M;` に適用し、結果のテキストに `StopTime=5` と `Icon(` の両方があること。`.github/workflows/rust.yml` の omc-integration の `cargo test -p omola-gui -- --ignored ...` の並びにこのテスト名を足す。

## 受け入れ条件

- [ ] ユーザーモデルで Stop time を変えて `Save in Model` → Text 層の `experiment(...)` が更新され、ドキュメントが dirty になる。Undo で戻る。
- [ ] 保存して開き直すと、Setup がその値で始まる。
- [ ] ライブラリのクラスではボタンが無効。
- [ ] 上のテストが通る(実 OMC のテストは CI の omc-integration で通る)。README 共通コマンド 5 つが成功。
