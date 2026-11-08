# Omola のインストール(Modelica コンパイラ込み)

Omola はチェック・シミュレーション・ライブラリ照会を **OpenModelica のコンパイラ `omc`** に任せます。
次のスクリプトは、`omc`、Modelica Standard Library 4.1.0、Omola 本体を一度に導入し、最後に `examples/BouncingBall.mo` を実際にシミュレーションして動作を確認します。

| OS | コマンド(リポジトリのルートで実行) | 導入される OpenModelica |
|---|---|---|
| Windows 10/11 (x64) | `powershell -ExecutionPolicy Bypass -File scripts\setup-windows.ps1` | 公式インストーラ v1.27.1(`C:\Program Files\OpenModelica1.27.1-64bit`) |
| Debian / Ubuntu | `scripts/setup-linux.sh` | 公式 apt リポジトリ `build.openmodelica.org/apt`(1.27.1 があれば固定、なければ stable の `omc`。2026-11-02 時点の Ubuntu 24.04 では 1.27.2) |
| macOS / Docker | `scripts/setup-docker.sh` | 固定イメージ `openmodelica/openmodelica:v1.27.1-minimal` を元にした `.devcontainer/Dockerfile` |

終わったら、ワークベンチを起動します。

```sh
target/release/omola-gui                      # Windows: target\release\omola-gui.exe
target/release/omola-gui examples/Pendulum.mo
```

ソースから動かすときは `cargo run --release -p omola-gui` を使ってください。debug ビルドは描画が非常に遅く、GUI が重く感じます(依存クレートは `[profile.dev.package."*"]` で最適化していますが、release には及びません)。

起動すると MSL の Documentation が開きます。Package Browser から `Modelica > Blocks > Examples > PID_Controller` などを選び、Simulation タブの **Simulate** を押すと計算され、Variable Browser で変数を選ぶとプロットされます。

## スクリプトが行うこと

1. **OpenModelica の導入**(すでに `omc` が見つかる場合は省略)
   - Windows: `https://build.openmodelica.org/omc/builds/windows/releases/1.27/1/64bit/OpenModelica-v1.27.1-64bit.exe` を取得し、Authenticode 署名が有効で署名者が Open Source Modelica Consortium であることを確認してから(違えば中止。`-AllowUnsignedInstaller` で無視)、`/S` でサイレントインストールします。管理者権限が必要なので、UAC の確認が出ます。見つかった `omc.exe` をユーザー環境変数 `OMOLA_OMC` に保存します。
   - Debian/Ubuntu: 公式の鍵(`https://build.openmodelica.org/apt/openmodelica.asc`)と apt ソースを登録し、`omc` と C コンパイラ(`build-essential`)を入れます。`sudo` を使います。
   - Docker: イメージをビルドし、`scripts/omc-docker.sh` を `omc` の代わりに使います。ホーム・一時フォルダ・カレントフォルダを同じパスでマウントするので、Omola からは普通の `omc` に見えます。
2. **Modelica ライブラリ**: `omc scripts/install-libraries.mos` で、OpenModelica のパッケージマネージャから Modelica 4.1.0(必須)と ModelicaReference(任意)を入れます。保存先は `~/.openmodelica/libraries` です(Docker の場合はボリューム `omola-omc-home`)。
3. **Omola のビルド**: Rust が無ければ rustup で入れ、`cargo build --locked --release -p omola -p omola-gui` を実行します。Linux では GUI 用のランタイム(OpenGL・X11/Wayland・Liberation フォント)も入れます。
4. **動作確認**: `omola doctor` を実行し、`omola simulate` で BouncingBall を 1 秒分計算して、結果ファイルができたことを確かめます。

オプションは次のとおりです。

- Windows: `-SkipOmc` / `-SkipLibraries` / `-SkipBuild` / `-Version 1.27.1` / `-AllowUnsignedInstaller`
- Linux: `--skip-omc` / `--skip-libraries` / `--skip-build` / `--no-rust`、および環境変数 `OMOLA_OM_VERSION`
- Docker: `--skip-build` / `--no-rust`

## Omola が `omc` を探す順番

1. 環境変数 `OMOLA_OMC`(空でなければ、そのファイルを使う)
2. `PATH` 上の `omc`(Windows は `omc.exe`)
3. `OPENMODELICAHOME\bin\omc(.exe)`
4. Windows のみ: `C:\Program Files\OpenModelica*\bin\omc.exe`(新しいバージョンから順に)

GUI は `omc` が見つからないと、Logs の Version タブに導入方法を表示します。インストール後は **Tools > Locate OMC** で再検出できます。ライブラリが入っていない場合は、Package Browser に「Modelica (not installed)」と表示されるので、**Tools > Install Libraries** で導入できます。

## 補足

- **Windows でのビルド**: Rust は Microsoft C++ Build Tools(`link.exe`)を使います。見つからない場合、スクリプトは次のコマンドを表示して止まります。入れたあと `-SkipOmc -SkipLibraries` を付けて再実行してください。

  ```powershell
  winget install Microsoft.VisualStudio.2022.BuildTools --override "--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
  ```

- **ビルド済みバイナリ**: GitHub の Actions で **Release** ワークフローを手動実行するか、`v*` タグを付けると、Windows/Linux/macOS 向けの zip/tar.gz が作られます。中身はバイナリ・スクリプト・例題・ライセンス表記です。OpenModelica は同梱しないので、展開後に同じスクリプトを `-SkipBuild` / `--skip-build` 付きで実行して導入してください。
- **既存の OpenModelica を使う**: `OMOLA_OMC` にその `omc` のパスを設定し、`omc scripts/install-libraries.mos` を一度実行すれば十分です。
- **ライセンス**: OpenModelica は OSMC Public License / AGPLv3、MSL は 3-Clause BSD です。Omola のリポジトリと配布物はどちらも同梱せず、公式の配布元から利用者の環境に直接取得します。詳しくは [provenance-licenses.md](provenance-licenses.md) を見てください。
- **検証**: これらのスクリプトは GitHub Actions(`.github/workflows/setup-scripts.yml`)で、Windows・Ubuntu・Docker の各環境について、導入からビルド、シミュレーションまで通しで実行しています。
