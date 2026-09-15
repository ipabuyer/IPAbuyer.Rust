# 将 Tauri 构建产物打包为 .msixbundle（用于微软商店上传与 GitHub Release）
# 用法:
#   scripts/make-msix.ps1                    # 打包 x64（宿主默认目标）并合并 bundle
#   scripts/make-msix.ps1 -TargetArch x64,arm64 `
#       -RustTarget 'x86_64-pc-windows-msvc','aarch64-pc-windows-msvc'
#   scripts/make-msix.ps1 -TargetArch arm64 `
#       -RustTarget aarch64-pc-windows-msvc -SkipBundle
# 说明:
#   - RustTarget 与 TargetArch 一一对应；留空的项表示宿主默认目标（target/ 根目录）
#   - -SkipBundle 仅生成各架构 .msix，供双架构流程最后统一合并
# 注意: 本文件必须保留 UTF-8 BOM
param(
    # 商店要求包版本必须高于已发布版本；源工程采用 年.月.日 CalVer
    [string]$Version = "2026.9.14.0",
    [string]$Configuration = "release",
    # 目标架构，逗号分隔（x64 / x64,arm64）
    [string]$TargetArch = "x64",
    # 与 TargetArch 一一对应的 Rust target triple，逗号分隔；空段 = 宿主默认目标
    [string]$RustTarget = "",
    # 仅生成各架构 .msix，跳过 .msixbundle 合并
    [switch]$SkipBundle
)

$ErrorActionPreference = 'Stop'
$RepoRoot = Split-Path -Parent $PSScriptRoot
$MsixDir  = Join-Path $RepoRoot "msix"
$OutDir   = Join-Path $MsixDir "out"
$AssetsDir = Join-Path $MsixDir "assets"

$ValidArms = @('x64', 'arm64')
$ArchList = $TargetArch -split ',' | ForEach-Object { $_.Trim() } | Where-Object { $_ }
foreach ($a in $ArchList) {
    if ($ValidArms -notcontains $a) { Write-Error "无效的目标架构: $a（仅支持 x64 / arm64）" }
}
$TripleList = if ($RustTarget.Trim()) { $RustTarget -split ',' | ForEach-Object { $_.Trim() } } else { @() }
$ArchToProc = @{ 'x64' = 'x64'; 'arm64' = 'arm64' }
$RequiredAssets = @(
    "StoreLogo.png", "Square150x150Logo.png", "Square44x44Logo.png",
    "Wide310x150Logo.png", "SmallTile.png", "LargeTile.png", "SplashScreen.png"
)

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

# ---- 清理输出目录 ----
if (Test-Path $OutDir) { Remove-Item -Recurse -Force $OutDir }
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

# ---- 逐架构打包 .msix ----
$MsixPaths = @()
for ($i = 0; $i -lt $ArchList.Count; $i++) {
    $arch = $ArchList[$i]
    $rustTarget = if ($i -lt $TripleList.Count) { $TripleList[$i] } else { '' }

    # rustTarget 为空 = 宿主默认目标（target 根目录）；否则 target/<triple>/<Configuration>
    $targetRoot = if ($rustTarget) {
        Join-Path (Join-Path $RepoRoot "src-tauri\target") (Join-Path $rustTarget $Configuration)
    } else {
        Join-Path (Join-Path $RepoRoot "src-tauri\target") $Configuration
    }
    $exePath = Join-Path $targetRoot "IPAbuyer.exe"
    $ipatoolPath = Join-Path $targetRoot "ipatool.exe"
    if (-not (Test-Path $exePath)) { Write-Error "找不到 $exePath，请先构建 $arch 架构" }
    if (-not (Test-Path $ipatoolPath)) { Write-Error "找不到 sidecar $ipatoolPath" }

    # ---- 组装临时目录 ----
    $stageDir = Join-Path $OutDir "stage-$arch"
    New-Item -ItemType Directory -Force -Path (Join-Path $stageDir "Assets") | Out-Null
    Copy-Item $exePath (Join-Path $stageDir "IPAbuyer.exe")
    Copy-Item $ipatoolPath (Join-Path $stageDir "ipatool.exe")

    # ---- 由模板生成 AppxManifest（校验版本号格式 x.y.z.w）----
    if ($Version -notmatch '^\d+\.\d+\.\d+\.\d+$') {
        Write-Error "版本号必须为 x.y.z.w 四段格式: $Version"
    }
    $manifest = (Get-Content (Join-Path $MsixDir "AppxManifest.template.xml") -Raw) `
        -replace '@VERSION@', $Version -replace '@ARCH@', $arch
    Set-Content -Path (Join-Path $stageDir "AppxManifest.xml") -Value $manifest -Encoding UTF8

    # ---- 复制商店图标资源（scale-100 基准版；包内不含 resources.pri，需使用清单引用的原始文件名）----
    foreach ($asset in $RequiredAssets) {
        $src = Join-Path $AssetsDir $asset
        if (-not (Test-Path $src)) { Write-Error "缺少资源文件: $src" }
        Copy-Item $src (Join-Path (Join-Path $stageDir "Assets") $asset)
    }

    # ---- makeappx pack -> .msix ----
    $msixName = "IPAbuyer_$($Version)_$arch.msix"
    $msixPath = Join-Path $OutDir $msixName
    $packMapping = Join-Path $OutDir "pack-mapping-$arch.txt"
    $lines = @('[Files]')
    $lines += ('"{0}" "AppxManifest.xml"' -f (Join-Path $stageDir "AppxManifest.xml"))
    $lines += ('"{0}" "IPAbuyer.exe"' -f (Join-Path $stageDir "IPAbuyer.exe"))
    $lines += ('"{0}" "ipatool.exe"' -f (Join-Path $stageDir "ipatool.exe"))
    foreach ($asset in $RequiredAssets) {
        $lines += ('"{0}" "Assets\{1}"' -f (Join-Path (Join-Path $stageDir "Assets") $asset), $asset)
    }
    Set-Content -Path $packMapping -Value $lines -Encoding Ascii

    Write-Host "正在打包 $msixName ..."
    & $MakeAppx pack /f $packMapping /p $msixPath /o
    if ($LASTEXITCODE -ne 0) { Write-Error "makeappx pack 失败（$arch）" }
    $MsixPaths += $msixPath
}

if ($SkipBundle) {
    Write-Host ""
    Write-Host "完成（跳过 bundle）:" -ForegroundColor Green
    foreach ($p in $MsixPaths) { Write-Host "  $p" }
    exit 0
}

# ---- makeappx bundle -> .msixbundle ----
# 单架构保留架构后缀；多架构合并为单一 .msixbundle
$suffix = if ($ArchList.Count -gt 1) { "" } else { "_$($ArchList[0])" }
$bundleName    = "IPAbuyer_$($Version)$suffix.msixbundle"
$bundlePath    = Join-Path $OutDir $bundleName
$bundleMapping = Join-Path $OutDir "bundle-mapping.txt"
$lines = @('[Files]')
foreach ($p in $MsixPaths) {
    $lines += ('"{0}" "{1}"' -f $p, (Split-Path $p -Leaf))
}
Set-Content -Path $bundleMapping -Value $lines -Encoding Ascii
Write-Host "正在合并 $bundleName ..."
# /bv 必须显式指定：缺省时 makeappx 会用当前 UTC 时间（年.月日.时分.0）生成 bundle 版本，
# 导致商店显示的版本号与包内实际版本不一致（如 2026.9.14.0 显示为 2026.914.940.0）
& $MakeAppx bundle /f $bundleMapping /p $bundlePath /bv $Version /o
if ($LASTEXITCODE -ne 0) { Write-Error "makeappx bundle 失败" }

Write-Host ""
Write-Host "完成: $bundlePath" -ForegroundColor Green
Write-Host "Identity: IPAbuyer.IPAbuyer, CN=68F867E4-B304-4B5D-9818-31B1910E0771, Version=$Version"
