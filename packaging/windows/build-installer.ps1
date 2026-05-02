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
$pdfEngineCache = Join-Path $windowsDistRoot "pdf-engine-cache"
$packageEngineCache = Join-Path $windowsDistRoot "package-engine-cache"
$setupIconPath = Join-Path $repoRoot "assets\\icon\\icon.ico"
$ghostscriptVersion = "10.07.0"
$ghostscriptTag = "gs10070"
$ghostscriptInstaller = "gs10070w64.exe"
$ghostscriptDownloadUrl = "https://github.com/ArtifexSoftware/ghostpdl-downloads/releases/download/$ghostscriptTag/$ghostscriptInstaller"
$qpdfVersion = "12.3.2"
$qpdfArchive = "qpdf-$qpdfVersion-mingw64.zip"
$qpdfDownloadUrl = "https://sourceforge.net/projects/qpdf/files/qpdf/$qpdfVersion/$qpdfArchive/download"
$sevenZipVersion = "26.01"
$sevenZipInstaller = "7z2601-x64.exe"
$sevenZipDownloadUrl = "https://github.com/ip7z/7zip/releases/download/$sevenZipVersion/$sevenZipInstaller"

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

function Ensure-PdfEngineCache {
    $downloadRoot = Join-Path $pdfEngineCache "download"
    $installRoot = Join-Path $pdfEngineCache "install"
    $installerPath = Join-Path $downloadRoot $ghostscriptInstaller
    $qpdfArchivePath = Join-Path $downloadRoot $qpdfArchive
    $qpdfInstallRoot = Join-Path $installRoot "qpdf"

    if ($RefreshEngine) {
        Remove-Item $downloadRoot -Recurse -Force -ErrorAction SilentlyContinue
        Remove-Item $installRoot -Recurse -Force -ErrorAction SilentlyContinue
    }

    New-Item -ItemType Directory -Force -Path $downloadRoot | Out-Null

    if (-not (Test-Path $installerPath)) {
        Write-Host "Downloading bundled Ghostscript $ghostscriptVersion for Windows..."
        Invoke-WebRequest -Uri $ghostscriptDownloadUrl -OutFile $installerPath
    }

    if (-not (Test-Path $qpdfArchivePath)) {
        Write-Host "Downloading bundled qpdf $qpdfVersion for Windows..."
        Invoke-WebRequest -Uri $qpdfDownloadUrl -OutFile $qpdfArchivePath
    }

    $ghostscript = Get-ChildItem -Path $installRoot -Filter gswin64c.exe -Recurse -ErrorAction SilentlyContinue | Select-Object -First 1

    if (-not $ghostscript) {
        New-Item -ItemType Directory -Force -Path $installRoot | Out-Null

        Write-Host "Installing cached Ghostscript runtime..."
        $process = Start-Process -FilePath $installerPath -ArgumentList @("/S", "/D=$installRoot") -NoNewWindow -Wait -PassThru
        if ($process.ExitCode -ne 0) {
            throw "Ghostscript installer failed with exit code $($process.ExitCode)."
        }

        $ghostscript = Get-ChildItem -Path $installRoot -Filter gswin64c.exe -Recurse | Select-Object -First 1
    }

    if (-not $ghostscript) {
        throw "The cached Ghostscript installer did not produce gswin64c.exe."
    }

    $qpdf = Get-ChildItem -Path $qpdfInstallRoot -Filter qpdf.exe -Recurse -ErrorAction SilentlyContinue | Select-Object -First 1
    if (-not $qpdf) {
        Remove-Item $qpdfInstallRoot -Recurse -Force -ErrorAction SilentlyContinue
        New-Item -ItemType Directory -Force -Path $qpdfInstallRoot | Out-Null
        Write-Host "Extracting cached qpdf runtime..."
        Expand-Archive -Path $qpdfArchivePath -DestinationPath $qpdfInstallRoot -Force
        $qpdf = Get-ChildItem -Path $qpdfInstallRoot -Filter qpdf.exe -Recurse | Select-Object -First 1
    }

    if (-not $qpdf) {
        throw "The cached qpdf archive did not produce qpdf.exe."
    }

    $binDir = Split-Path -Parent $ghostscript.FullName
    $engineRoot = Split-Path -Parent $binDir

    return @{
        GhostscriptRoot = $engineRoot
        GhostscriptBinary = $ghostscript.FullName
        QpdfRoot = $qpdfInstallRoot
        QpdfBinary = $qpdf.FullName
    }
}

function Ensure-PackageEngineCache {
    $downloadRoot = Join-Path $packageEngineCache "download"
    $installRoot = Join-Path $packageEngineCache "install"
    $installerPath = Join-Path $downloadRoot $sevenZipInstaller

    if ($RefreshEngine) {
        Remove-Item $downloadRoot -Recurse -Force -ErrorAction SilentlyContinue
        Remove-Item $installRoot -Recurse -Force -ErrorAction SilentlyContinue
    }

    New-Item -ItemType Directory -Force -Path $downloadRoot | Out-Null

    if (-not (Test-Path $installerPath)) {
        Write-Host "Downloading bundled 7-Zip $sevenZipVersion for Windows..."
        Invoke-WebRequest -Uri $sevenZipDownloadUrl -OutFile $installerPath
    }

    $sevenZip = Get-ChildItem -Path $installRoot -Filter 7z.exe -Recurse -ErrorAction SilentlyContinue | Select-Object -First 1
    if (-not $sevenZip) {
        Remove-Item $installRoot -Recurse -Force -ErrorAction SilentlyContinue
        New-Item -ItemType Directory -Force -Path $installRoot | Out-Null
        Write-Host "Installing cached 7-Zip runtime..."
        $process = Start-Process -FilePath $installerPath -ArgumentList @("/S", "/D=$installRoot") -NoNewWindow -Wait -PassThru
        if ($process.ExitCode -ne 0) {
            throw "7-Zip installer failed with exit code $($process.ExitCode)."
        }
        $sevenZip = Get-ChildItem -Path $installRoot -Filter 7z.exe -Recurse | Select-Object -First 1
    }

    if (-not $sevenZip) {
        throw "The cached 7-Zip installer did not produce 7z.exe."
    }

    return @{
        Root = $installRoot
        Binary = $sevenZip.FullName
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

function Copy-BundledEngine([string]$StageDir, $EngineArtifacts, $PdfEngineArtifacts, $PackageEngineArtifacts) {
    $videoEngineDir = Join-Path $StageDir "engine\\video-engine"
    Remove-Item $videoEngineDir -Recurse -Force -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force -Path $videoEngineDir | Out-Null
    Copy-Item $EngineArtifacts.Ffmpeg (Join-Path $videoEngineDir "ffmpeg.exe") -Force
    Copy-Item $EngineArtifacts.Ffprobe (Join-Path $videoEngineDir "ffprobe.exe") -Force
    if ($EngineArtifacts.Ffplay) {
        Copy-Item $EngineArtifacts.Ffplay (Join-Path $videoEngineDir "ffplay.exe") -Force
    }

    $pdfEngineDir = Join-Path $StageDir "engine\\pdf-engine"
    Remove-Item $pdfEngineDir -Recurse -Force -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force -Path $pdfEngineDir | Out-Null
    Copy-Item (Join-Path $PdfEngineArtifacts.GhostscriptRoot "*") $pdfEngineDir -Recurse -Force
    Copy-Item $PdfEngineArtifacts.QpdfRoot $pdfEngineDir -Recurse -Force

    $packageEngineDir = Join-Path $StageDir "engine\\package-engine"
    Remove-Item $packageEngineDir -Recurse -Force -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force -Path $packageEngineDir | Out-Null
    Copy-Item (Join-Path $PackageEngineArtifacts.Root "*") $packageEngineDir -Recurse -Force
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
$pdfEngineArtifacts = $null
$packageEngineArtifacts = $null

New-Item -ItemType Directory -Force -Path $installerDir | Out-Null

if ($requestedVariants -contains "bundled") {
    $engineArtifacts = Ensure-EngineCache
    $pdfEngineArtifacts = Ensure-PdfEngineCache
    $packageEngineArtifacts = Ensure-PackageEngineCache
}

foreach ($variantName in $requestedVariants) {
    $stageDir = Get-VariantStageDir $variantName
    $outputBaseName = Get-VariantOutputBaseName $variantName $version

    Write-Host "Building Windows package variant '$variantName'..."
    Stage-AppBundle $stageDir

    if ($variantName -eq "bundled") {
        Copy-BundledEngine $stageDir $engineArtifacts $pdfEngineArtifacts $packageEngineArtifacts
    } else {
        Write-Host "Skipping bundled FFmpeg and document engines for variant '$variantName'."
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
