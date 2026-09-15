# 构建并运行 IPAbuyer
# 用法:
#   .\run.ps1              # 默认：debug 开发模式（vite 热重载 + 增量编译，改前端即时生效）
#   .\run.ps1 -SkipBuild   # 跳过构建直接启动
#   .\run.ps1 -Release     # 发布模式：release 构建 + 内嵌前端资产
#   .\run.ps1 -Release -Msix  # 发布构建并打包 msixbundle
param(
    [switch]$Release,    # 发布模式（默认为 debug 开发模式）
    [switch]$SkipBuild,  # 跳过构建直接启动
    [switch]$Msix        # 打包 msixbundle（需 -Release）
)

$ErrorActionPreference = "Stop"
$RepoRoot = $PSScriptRoot
$TargetDir = if ($Release) { "release" } else { "debug" }
$ExePath = Join-Path $RepoRoot "src-tauri\target\$TargetDir\IPAbuyer.exe"

# 运行中的实例会锁住 exe 导致链接器拒绝写入，必须先退出
Get-Process IPAbuyer -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Milliseconds 500

if ($Msix -and -not $Release) {
    Write-Warning "-Msix 需要 -Release（发布构建），本次按 -Release 执行"
    $Release = $true
    $SkipBuild = $false
    $TargetDir = "release"
}

if (-not $SkipBuild) {
    if ($Release) {
        # 前端 dist 变化不会触发 cargo 重编（资产编译期嵌入），touch 强制重编
        (Get-Item (Join-Path $RepoRoot "src-tauri\src\lib.rs")).LastWriteTime = Get-Date
        Push-Location (Join-Path $RepoRoot "src-tauri")
        cargo build --release --features custom-protocol
        $buildExit = $LASTEXITCODE
        Pop-Location
    }
    else {
        # 开发模式：前端由 vite dev server 提供（热重载），无需嵌入资产
        $portListening = Get-NetTCPConnection -LocalPort 1420 -State Listen -ErrorAction SilentlyContinue
        if (-not $portListening) {
            Write-Host "启动 vite dev server (localhost:1420)..."
            Start-Process -FilePath "cmd.exe" -ArgumentList "/c npm run dev" `
                -WorkingDirectory $RepoRoot -WindowStyle Hidden
            $ready = $false
            for ($i = 0; $i -lt 30; $i++) {
                Start-Sleep -Milliseconds 500
                if (Get-NetTCPConnection -LocalPort 1420 -State Listen -ErrorAction SilentlyContinue) {
                    $ready = $true; break
                }
            }
            if (-not $ready) { Write-Error "vite dev server 启动超时"; exit 1 }
        }
        else {
            Write-Host "vite dev server 已在运行"
        }
        Push-Location (Join-Path $RepoRoot "src-tauri")
        cargo build
        $buildExit = $LASTEXITCODE
        Pop-Location
    }
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
