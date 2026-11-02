<#
.SYNOPSIS
  Set up Omola on Windows: OpenModelica (omc) from the official installer, the Modelica
  Standard Library 4.1.0, Rust (if missing), a release build of omola + omola-gui, and a
  smoke simulation.

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File scripts\setup-windows.ps1
  powershell -ExecutionPolicy Bypass -File scripts\setup-windows.ps1 -SkipBuild

.NOTES
  The installer must carry a valid Authenticode signature of the Open Source Modelica
  Consortium (override with -AllowUnsignedInstaller). It needs administrator rights; Windows asks for confirmation
  (UAC) when the script is not already elevated. OMOLA_OMC is stored as a user
  environment variable so Omola finds the compiler in new terminals.
#>
param(
    [string]$Version = "1.27.1",
    [switch]$SkipOmc,
    [switch]$SkipLibraries,
    [switch]$SkipBuild,
    [switch]$AllowUnsignedInstaller
)
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot

function Step($message) { Write-Host "`n==> $message" -ForegroundColor Cyan }

function Find-Omc {
    $cmd = Get-Command omc.exe -ErrorAction SilentlyContinue
    if ($cmd) { return $cmd.Source }
    if ($env:OPENMODELICAHOME) {
        $candidate = Join-Path $env:OPENMODELICAHOME "bin\omc.exe"
        if (Test-Path $candidate) { return $candidate }
    }
    $installs = Get-ChildItem -Path $env:ProgramFiles -Directory -Filter "OpenModelica*" -ErrorAction SilentlyContinue |
        Sort-Object { [version](($_.Name -replace '^OpenModelica([0-9.]+).*$', '$1')) } -Descending
    foreach ($dir in $installs) {
        $candidate = Join-Path $dir.FullName "bin\omc.exe"
        if (Test-Path $candidate) { return $candidate }
    }
    return $null
}

# 1. OpenModelica compiler -----------------------------------------------------------------
$omc = Find-Omc
if (-not $omc -and -not $SkipOmc) {
    $parts = $Version.Split(".")
    $url = "https://build.openmodelica.org/omc/builds/windows/releases/$($parts[0]).$($parts[1])/$($parts[2])/64bit/OpenModelica-v$Version-64bit.exe"
    $installer = Join-Path $env:TEMP "OpenModelica-v$Version-64bit.exe"
    Step "Downloading OpenModelica $Version from $url"
    if (Get-Command curl.exe -ErrorAction SilentlyContinue) {
        & curl.exe -L --fail -o $installer $url
        if ($LASTEXITCODE -ne 0) { throw "Download failed: $url" }
    } else {
        Invoke-WebRequest -Uri $url -OutFile $installer -UseBasicParsing
    }
    $signature = Get-AuthenticodeSignature $installer
    Write-Host "Installer signature: $($signature.Status) $($signature.SignerCertificate.Subject)"
    $trusted = $signature.Status -eq "Valid" -and $signature.SignerCertificate.Subject -match "OPEN SOURCE MODELICA CONSORTIUM"
    if (-not $trusted -and -not $AllowUnsignedInstaller) {
        Remove-Item $installer -ErrorAction SilentlyContinue
        throw "The installer is not validly signed by the Open Source Modelica Consortium. Re-run with -AllowUnsignedInstaller only if you trust the download."
    }
    Step "Running the OpenModelica installer silently (administrator rights required)"
    $isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole(
        [Security.Principal.WindowsBuiltInRole]::Administrator)
    if ($isAdmin) {
        $process = Start-Process -FilePath $installer -ArgumentList "/S" -Wait -PassThru
    } else {
        $process = Start-Process -FilePath $installer -ArgumentList "/S" -Verb RunAs -Wait -PassThru
    }
    if ($process.ExitCode -ne 0) { throw "OpenModelica installer exited with code $($process.ExitCode)" }
    Remove-Item $installer -ErrorAction SilentlyContinue
    $omc = Find-Omc
}
if (-not $omc) {
    if ($SkipOmc) { Write-Warning "omc not found; set OMOLA_OMC later (see docs\install.md)." }
    else { throw "omc.exe was not found after installation. See docs\install.md." }
} else {
    $env:OMOLA_OMC = $omc
    [Environment]::SetEnvironmentVariable("OMOLA_OMC", $omc, "User")
    Step "Using $omc ($(& $omc --version)); OMOLA_OMC saved for new terminals"
}

# 2. Modelica libraries --------------------------------------------------------------------
if ($omc -and -not $SkipLibraries) {
    Step "Installing Modelica Standard Library 4.1.0 (and ModelicaReference)"
    Push-Location $env:TEMP
    try { $out = (& $omc (Join-Path $Root "scripts\install-libraries.mos")) -join "`n" } finally { Pop-Location }
    $out -split "`n" | Where-Object { $_ -like "OMOLA_*" } | ForEach-Object { Write-Host $_ }
    if ($out -notmatch "OMOLA_INSTALLED Modelica 4.1.0") { Write-Host $out; throw "Installing the Modelica Standard Library failed." }
}

# 3. Build Omola ---------------------------------------------------------------------------
if (-not $SkipBuild) {
    $cargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue) -and (Test-Path (Join-Path $cargoBin "cargo.exe"))) {
        $env:PATH = "$cargoBin;$env:PATH"
    }
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        Step "Installing Rust with rustup (https://rustup.rs)"
        $rustup = Join-Path $env:TEMP "rustup-init.exe"
        Invoke-WebRequest -Uri "https://win.rustup.rs/x86_64" -OutFile $rustup -UseBasicParsing
        & $rustup -y --profile minimal
        if ($LASTEXITCODE -ne 0) { throw "rustup-init failed" }
        $env:PATH = "$cargoBin;$env:PATH"
    }
    $vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
    $hasMsvc = (Test-Path $vswhere) -and (& $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath)
    if (-not $hasMsvc) {
        Write-Warning ("Rust on Windows needs the Microsoft C++ build tools (link.exe). Install them with:`n" +
            "  winget install Microsoft.VisualStudio.2022.BuildTools --override `"--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended`"`n" +
            "then run this script again with -SkipOmc -SkipLibraries.")
        throw "Microsoft C++ build tools not found."
    }
    Step "Building omola and omola-gui (release)"
    Push-Location $Root
    try {
        & cargo build --locked --release -p omola -p omola-gui
        if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
    } finally { Pop-Location }

    # 4. Smoke test ------------------------------------------------------------------------
    if ($omc) {
        Step "Smoke test: simulate examples\BouncingBall.mo"
        $outDir = Join-Path $env:TEMP ("omola-setup-" + [guid]::NewGuid().ToString("N"))
        New-Item -ItemType Directory -Path $outDir | Out-Null
        $omola = Join-Path $Root "target\release\omola.exe"
        & $omola doctor
        if ($LASTEXITCODE -ne 0) { throw "omola doctor failed" }
        & $omola simulate --model (Join-Path $Root "examples\BouncingBall.mo") --class BouncingBall --stop-time 1 --output (Join-Path $outDir "BouncingBall")
        if ($LASTEXITCODE -ne 0) { throw "smoke simulation failed" }
        if (-not (Test-Path (Join-Path $outDir "BouncingBall_res.mat"))) { throw "smoke simulation produced no result" }
        Remove-Item $outDir -Recurse -Force -ErrorAction SilentlyContinue
    }
    Step "Done. Start the workbench with:  $(Join-Path $Root 'target\release\omola-gui.exe')"
} else {
    Step "Done (build skipped)."
}
