# 一键构建并运行 IPAbuyer（release 版）
# 用法: powershell -ExecutionPolicy Bypass -File run.ps1 [-SkipBuild] [-Msix]
param(
    [switch]$SkipBuild,  # 跳过构建，直接启动现有 exe
    [switch]$Msix        # 构建后顺带打包 msixbundle
)

$ErrorActionPreference = "Stop"
$RepoRoot = $PSScriptRoot
$ExePath = Join-Path $RepoRoot "src-tauri\target\release\IPAbuyer.exe"

# 运行中的实例会锁住 exe 导致链接器拒绝写入，必须先退出
Get-Process IPAbuyer -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Milliseconds 500

if (-not $SkipBuild) {
    # 前端 dist 变化不会触发 cargo 重编（资产编译期嵌入），touch 强制重编
    $libRs = Join-Path $RepoRoot "src-tauri\src\lib.rs"
    (Get-Item $libRs).LastWriteTime = Get-Date

    Push-Location (Join-Path $RepoRoot "src-tauri")
    cargo build --release --features custom-protocol
    $buildExit = $LASTEXITCODE
    Pop-Location
    if ($buildExit -ne 0) {
        Write-Error "构建失败（exit $buildExit）"
        exit 1
    }
}

if (-not (Test-Path $ExePath)) {
    Write-Error "找不到 $ExePath，请先构建"
    exit 1
}

if ($Msix) {
    & (Join-Path $RepoRoot "scripts\make-msix.ps1")
}

Write-Host "启动 $ExePath" -ForegroundColor Green
Start-Process $ExePath
