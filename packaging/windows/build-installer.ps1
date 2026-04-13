param(
    [switch]$SkipTests,
    [switch]$RefreshEngine,
    [ValidateSet("bundled", "no-engine", "all")]
    [string]$Variant = "bundled"
)

$ErrorActionPreference = "Stop"

$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = (Resolve-Path (Join-Path $scriptRoot "..\\..")).Path
$windowsDistRoot = Join-Path $repoRoot "dist\\windows"
$defaultBundledStageDir = Join-Path $windowsDistRoot "Compressi.ty"
$bundledVariantStageDir = Join-Path $windowsDistRoot "Compressi.ty-bundled"
$noEngineStageDir = Join-Path $windowsDistRoot "Compressi.ty-no-engine"
$installerDir = Join-Path $windowsDistRoot "installer"
$engineCache = Join-Path $windowsDistRoot "engine-cache"
$setupIconPath = Join-Path $repoRoot "assets\\icon\\icon.ico"

function Get-AppVersion {
    $line = Select-String -Path (Join-Path $repoRoot "Cargo.toml") -Pattern '^version = "(.*)"$' | Select-Object -First 1
    if (-not $line) {
        throw "Could not read version from Cargo.toml."
    }

    return $line.Matches[0].Groups[1].Value
}

function Find-Iscc {
    $candidates = @(
        "${env:ProgramFiles(x86)}\\Inno Setup 6\\ISCC.exe",
        "${env:ProgramFiles}\\Inno Setup 6\\ISCC.exe"
    )

    foreach ($candidate in $candidates) {
        if ($candidate -and (Test-Path $candidate)) {
            return $candidate
        }
    }

    return $null
}

function Get-RequestedVariants {
    switch ($Variant) {
        "all" { return @("bundled", "no-engine") }
        default { return @($Variant) }
    }
}

function Get-VariantStageDir([string]$VariantName) {
    switch ($VariantName) {
        "bundled" {
            if ($Variant -eq "all") {
                return $bundledVariantStageDir
            }

            return $defaultBundledStageDir
        }
        "no-engine" {
            return $noEngineStageDir
        }
        default {
            throw "Unknown variant '$VariantName'."
        }
    }
}

function Get-VariantOutputBaseName([string]$VariantName, [string]$AppVersion) {
    switch ($VariantName) {
        "bundled" {
            if ($Variant -eq "all") {
                return "Compressi.ty-Setup-$AppVersion-Bundled"
            }

            return "Compressi.ty-Setup-$AppVersion"
        }
        "no-engine" {
            return "Compressi.ty-Setup-$AppVersion-NoEngine"
        }
        default {
            throw "Unknown variant '$VariantName'."
        }
    }
}

function Ensure-EngineCache {
    $downloadRoot = Join-Path $engineCache "download"
    $archivePath = Join-Path $downloadRoot "ffmpeg-release-essentials.zip"

    if ($RefreshEngine) {
        Remove-Item $downloadRoot -Recurse -Force -ErrorAction SilentlyContinue
    }

    New-Item -ItemType Directory -Force -Path $downloadRoot | Out-Null

    if (-not (Test-Path $archivePath)) {
        $url = "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip"
        Write-Host "Downloading bundled FFmpeg for Windows..."
        Invoke-WebRequest -Uri $url -OutFile $archivePath
    }

    $ffmpeg = Get-ChildItem -Path $downloadRoot -Filter ffmpeg.exe -Recurse | Select-Object -First 1
    $ffprobe = Get-ChildItem -Path $downloadRoot -Filter ffprobe.exe -Recurse | Select-Object -First 1
    $ffplay = Get-ChildItem -Path $downloadRoot -Filter ffplay.exe -Recurse | Select-Object -First 1

    if (-not $ffmpeg -or -not $ffprobe) {
        Write-Host "Extracting cached FFmpeg runtime..."
        Expand-Archive -Path $archivePath -DestinationPath $downloadRoot -Force

        $ffmpeg = Get-ChildItem -Path $downloadRoot -Filter ffmpeg.exe -Recurse | Select-Object -First 1
        $ffprobe = Get-ChildItem -Path $downloadRoot -Filter ffprobe.exe -Recurse | Select-Object -First 1
        $ffplay = Get-ChildItem -Path $downloadRoot -Filter ffplay.exe -Recurse | Select-Object -First 1
    }

    if (-not $ffmpeg -or -not $ffprobe) {
        throw "The cached FFmpeg archive did not contain ffmpeg.exe and ffprobe.exe."
    }

    return @{
        Ffmpeg = $ffmpeg.FullName
        Ffprobe = $ffprobe.FullName
        Ffplay = if ($ffplay) { $ffplay.FullName } else { $null }
    }
}

function Stage-AppBundle([string]$StageDir) {
    Remove-Item $StageDir -Recurse -Force -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force -Path $StageDir | Out-Null

    Copy-Item "target\\release\\compressity.exe" (Join-Path $StageDir "compressity.exe") -Force
    if (Test-Path "target\\release\\compressity.pdb") {
        Copy-Item "target\\release\\compressity.pdb" (Join-Path $StageDir "compressity.pdb") -Force
    }
    Copy-Item "LICENSE" (Join-Path $StageDir "LICENSE.txt") -Force
}

function Sync-SetupIconFromExe([string]$ExePath) {
    Add-Type -AssemblyName System.Drawing

    $resolvedExe = (Resolve-Path $ExePath).Path
    $iconDir = Split-Path -Parent $setupIconPath
    New-Item -ItemType Directory -Force -Path $iconDir | Out-Null

    $icon = [System.Drawing.Icon]::ExtractAssociatedIcon($resolvedExe)
    if (-not $icon) {
        throw "Could not extract the app icon from $resolvedExe."
    }

    $stream = [System.IO.File]::Open($setupIconPath, [System.IO.FileMode]::Create)
    try {
        $icon.Save($stream)
    }
    finally {
        $stream.Dispose()
        $icon.Dispose()
    }
}

function Copy-BundledEngine([string]$StageDir, $EngineArtifacts) {
    Copy-Item $EngineArtifacts.Ffmpeg (Join-Path $StageDir "ffmpeg.exe") -Force
    Copy-Item $EngineArtifacts.Ffprobe (Join-Path $StageDir "ffprobe.exe") -Force
    if ($EngineArtifacts.Ffplay) {
        Copy-Item $EngineArtifacts.Ffplay (Join-Path $StageDir "ffplay.exe") -Force
    }
}

Push-Location $repoRoot

if (-not $SkipTests) {
    cargo test
}

cargo build --release
Sync-SetupIconFromExe "target\\release\\compressity.exe"

$version = Get-AppVersion
$requestedVariants = Get-RequestedVariants
$iscc = Find-Iscc
$createdOutputs = New-Object System.Collections.Generic.List[string]
$engineArtifacts = $null

New-Item -ItemType Directory -Force -Path $installerDir | Out-Null

if ($requestedVariants -contains "bundled") {
    $engineArtifacts = Ensure-EngineCache
}

foreach ($variantName in $requestedVariants) {
    $stageDir = Get-VariantStageDir $variantName
    $outputBaseName = Get-VariantOutputBaseName $variantName $version

    Write-Host "Building Windows package variant '$variantName'..."
    Stage-AppBundle $stageDir

    if ($variantName -eq "bundled") {
        Copy-BundledEngine $stageDir $engineArtifacts
    } else {
        Write-Host "Skipping bundled FFmpeg for variant '$variantName'."
    }

    if ($iscc) {
        & $iscc `
            "/DMyAppVersion=$version" `
            "/DStageDir=$stageDir" `
            "/F$outputBaseName" `
            (Join-Path $scriptRoot "compressi.ty.iss")

        $createdOutputs.Add((Join-Path $installerDir ($outputBaseName + ".exe")))
    } else {
        Write-Warning "Inno Setup 6 was not found. The staged app is available at $stageDir."
        $createdOutputs.Add($stageDir)
    }
}

if ($createdOutputs.Count -gt 0) {
    Write-Host "Created outputs:"
    foreach ($output in $createdOutputs) {
        Write-Host " - $output"
    }
}

Pop-Location
