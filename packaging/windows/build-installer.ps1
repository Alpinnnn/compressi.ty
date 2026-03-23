param(
    [switch]$SkipTests,
    [switch]$RefreshEngine
)

$ErrorActionPreference = "Stop"

$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = (Resolve-Path (Join-Path $scriptRoot "..\\..")).Path
$stageDir = Join-Path $repoRoot "dist\\windows\\Compressity"
$installerDir = Join-Path $repoRoot "dist\\windows\\installer"
$engineCache = Join-Path $repoRoot "dist\\windows\\engine-cache"

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

function Ensure-BundledEngine {
    if (
        -not $RefreshEngine `
        -and (Test-Path (Join-Path $stageDir "ffmpeg.exe")) `
        -and (Test-Path (Join-Path $stageDir "ffprobe.exe"))
    ) {
        return
    }

    $url = "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip"
    $downloadRoot = Join-Path $engineCache "download"
    $archivePath = Join-Path $downloadRoot "ffmpeg-release-essentials.zip"

    Remove-Item $downloadRoot -Recurse -Force -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force -Path $downloadRoot | Out-Null

    Write-Host "Downloading bundled FFmpeg for Windows..."
    Invoke-WebRequest -Uri $url -OutFile $archivePath
    Expand-Archive -Path $archivePath -DestinationPath $downloadRoot -Force

    $ffmpeg = Get-ChildItem -Path $downloadRoot -Filter ffmpeg.exe -Recurse | Select-Object -First 1
    $ffprobe = Get-ChildItem -Path $downloadRoot -Filter ffprobe.exe -Recurse | Select-Object -First 1
    $ffplay = Get-ChildItem -Path $downloadRoot -Filter ffplay.exe -Recurse | Select-Object -First 1

    if (-not $ffmpeg -or -not $ffprobe) {
        throw "The downloaded FFmpeg archive did not contain ffmpeg.exe and ffprobe.exe."
    }

    Copy-Item $ffmpeg.FullName (Join-Path $stageDir "ffmpeg.exe") -Force
    Copy-Item $ffprobe.FullName (Join-Path $stageDir "ffprobe.exe") -Force
    if ($ffplay) {
        Copy-Item $ffplay.FullName (Join-Path $stageDir "ffplay.exe") -Force
    }
}

Push-Location $repoRoot

if (-not $SkipTests) {
    cargo test
}

cargo build --release

$version = Get-AppVersion

Remove-Item $stageDir -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $stageDir | Out-Null
New-Item -ItemType Directory -Force -Path $installerDir | Out-Null

Copy-Item "target\\release\\compressity.exe" (Join-Path $stageDir "compressity.exe") -Force
if (Test-Path "target\\release\\compressity.pdb") {
    Copy-Item "target\\release\\compressity.pdb" (Join-Path $stageDir "compressity.pdb") -Force
}
Copy-Item "LICENSE" (Join-Path $stageDir "LICENSE.txt") -Force

Ensure-BundledEngine

$iscc = Find-Iscc
if ($iscc) {
    & $iscc `
        "/DMyAppVersion=$version" `
        "/DStageDir=$stageDir" `
        (Join-Path $scriptRoot "compressity.iss")
    Write-Host "Windows installer created in $installerDir"
} else {
    Write-Warning "Inno Setup 6 was not found. The bundled app is staged at $stageDir."
}

Pop-Location
