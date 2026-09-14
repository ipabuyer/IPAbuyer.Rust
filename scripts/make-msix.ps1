# 将 Tauri 构建产物打包为 .msixbundle（用于微软商店上传）
# 用法: powershell -ExecutionPolicy Bypass -File scripts/make-msix.ps1 [-Version 2026.9.14.0]
param(
    # 商店要求包版本必须高于已发布版本；源工程采用 年.月.日 CalVer
    [string]$Version = "2026.9.14.0",
    [string]$Configuration = "release"
)

$ErrorActionPreference = "Stop"
$RepoRoot = Split-Path -Parent $PSScriptRoot
$MsixDir  = Join-Path $RepoRoot "msix"
$OutDir   = Join-Path $MsixDir "out"
$StageDir = Join-Path $OutDir "staging"
$AssetsDir = Join-Path $MsixDir "assets"
$ExePath  = Join-Path $RepoRoot "src-tauri\target\$Configuration\IPAbuyer.exe"

if (-not (Test-Path $ExePath)) {
    Write-Error "找不到 $ExePath，请先运行: npm run build"
}

# ---- 定位 MakeAppx（Windows SDK），不假设安装盘符 ----
$CandidateRoots = @()
if ($env:WindowsSdkDir) { $CandidateRoots += (Join-Path $env:WindowsSdkDir "bin") }
$CandidateRoots += (Join-Path $env:ProgramFiles "Windows Kits\10\bin")
$ProgramFilesX86 = ${env:ProgramFiles(x86)}
if ($ProgramFilesX86) { $CandidateRoots += (Join-Path $ProgramFilesX86 "Windows Kits\10\bin") }
# SDK 可安装在任意盘符：扫描所有固定磁盘根目录下的 Windows Kits\10\bin
$CandidateRoots += [System.IO.DriveInfo]::GetDrives() |
    Where-Object { $_.DriveType -eq 'Fixed' -and $_.IsReady } |
    ForEach-Object { Join-Path $_.RootDirectory.FullName "Windows Kits\10\bin" }

$MakeAppx = $null
foreach ($root in ($CandidateRoots | Select-Object -Unique)) {
    if (-not (Test-Path $root)) { continue }
    $versionDir = Get-ChildItem -Path $root -Directory -ErrorAction SilentlyContinue |
        Where-Object { $_.Name -like "10.*" } |
        Sort-Object { try { [version]$_.Name } catch { [version]'0.0' } } -Descending |
        Select-Object -First 1
    if ($versionDir) {
        $p = Join-Path $versionDir.FullName "x64\makeappx.exe"
        if (Test-Path $p) { $MakeAppx = $p; break }
    }
}
if (-not $MakeAppx) { Write-Error "未找到 makeappx.exe（Windows SDK）" }
Write-Host "MakeAppx: $MakeAppx"

# ---- 准备暂存目录 ----
if (Test-Path $OutDir) { Remove-Item -Recurse -Force $OutDir }
New-Item -ItemType Directory -Force -Path $StageDir | Out-Null
New-Item -ItemType Directory -Force -Path (Join-Path $StageDir "Assets") | Out-Null

Copy-Item $ExePath $StageDir

# ---- ipatool sidecar（tauri build 已将 externalBin 复制到 release 目录，去 triple 后缀）----
$IpatoolPath = Join-Path $RepoRoot "src-tauri\target\$Configuration\ipatool.exe"
if (-not (Test-Path $IpatoolPath)) {
    Write-Error "找不到 sidecar $IpatoolPath，请先运行: powershell -File scripts/fetch-ipatool.ps1 && npm run build"
}
Copy-Item $IpatoolPath $StageDir

# ---- 由模板生成 AppxManifest（校验版本号格式 x.y.z.w）----
if ($Version -notmatch '^\d+\.\d+\.\d+\.\d+$') {
    Write-Error "版本号必须为 x.y.z.w 四段格式: $Version"
}
$manifest = (Get-Content (Join-Path $MsixDir "AppxManifest.template.xml") -Raw) -replace '@VERSION@', $Version
Set-Content -Path (Join-Path $StageDir "AppxManifest.xml") -Value $manifest -Encoding UTF8

# ---- 复制商店图标资源（scale-100 基准版；包内不含 resources.pri，需使用清单引用的原始文件名）----
$RequiredAssets = @(
    "StoreLogo.png", "Square150x150Logo.png", "Square44x44Logo.png",
    "Wide310x150Logo.png", "SmallTile.png", "LargeTile.png", "SplashScreen.png"
)
foreach ($asset in $RequiredAssets) {
    $src = Join-Path $AssetsDir $asset
    if (-not (Test-Path $src)) { Write-Error "缺少资源文件: $src" }
    Copy-Item $src (Join-Path (Join-Path $StageDir "Assets") $asset)
}

# ---- makeappx pack -> .msix ----
$MsixName    = "IPAbuyer_$($Version)_x64.msix"
$MsixPath    = Join-Path $OutDir $MsixName
$PackMapping = Join-Path $OutDir "pack-mapping.txt"
$lines = @('[Files]')
$lines += ('"{0}" "AppxManifest.xml"' -f (Join-Path $StageDir "AppxManifest.xml"))
$lines += ('"{0}" "IPAbuyer.exe"' -f (Join-Path $StageDir "IPAbuyer.exe"))
$lines += ('"{0}" "ipatool.exe"' -f (Join-Path $StageDir "ipatool.exe"))
foreach ($asset in $RequiredAssets) {
    $lines += ('"{0}" "Assets\{1}"' -f (Join-Path (Join-Path $StageDir "Assets") $asset), $asset)
}
Set-Content -Path $PackMapping -Value $lines -Encoding Ascii

Write-Host "正在打包 $MsixName ..."
& $MakeAppx pack /f $PackMapping /p $MsixPath /o
if ($LASTEXITCODE -ne 0) { Write-Error "makeappx pack 失败" }

# ---- makeappx bundle -> .msixbundle ----
$BundleName     = "IPAbuyer_$($Version)_x64.msixbundle"
$BundlePath     = Join-Path $OutDir $BundleName
$BundleMapping  = Join-Path $OutDir "bundle-mapping.txt"
Set-Content -Path $BundleMapping -Encoding Ascii -Value @(
    '[Files]'
    ('"{0}" "{1}"' -f $MsixPath, $MsixName)
)
Write-Host "正在打包 $BundleName ..."
# /bv 必须显式指定：缺省时 makeappx 会用当前 UTC 时间（年.月日.时分.0）生成 bundle 版本，
# 导致商店显示的版本号与包内实际版本不一致（如 2026.9.14.0 显示为 2026.914.940.0）
& $MakeAppx bundle /f $BundleMapping /p $BundlePath /bv $Version /o
if ($LASTEXITCODE -ne 0) { Write-Error "makeappx bundle 失败" }

Write-Host ""
Write-Host "完成: $BundlePath" -ForegroundColor Green
Write-Host "Identity: IPAbuyer.IPAbuyer, CN=68F867E4-B304-4B5D-9818-31B1910E0771, Version=$Version"
