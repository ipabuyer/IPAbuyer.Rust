# 下载 ipatool.exe 作为 Tauri sidecar（输出 Tauri target-triple 命名约定）
# 用法: powershell -ExecutionPolicy Bypass -File scripts/fetch-ipatool.ps1 [-Version 2.5.0] [-Force]
param(
    [ValidatePattern('^$|^\d+\.\d+\.\d+$')]
    [string]$Version = '2.5.0',
    [switch]$Force
)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$RepoRoot = Split-Path -Parent $PSScriptRoot
$OutputDir = Join-Path $RepoRoot "src-tauri\binaries"
# Tauri externalBin 要求 <名称>-<target-triple>(.exe) 命名
$Targets = @{
    'amd64' = 'x86_64-pc-windows-msvc'
    'arm64' = 'aarch64-pc-windows-msvc'
}

$TempRoot = Join-Path ([System.IO.Path]::GetTempPath()) "ipabuyer-ipatool-$([System.Guid]::NewGuid().ToString('N'))"

function Get-ArchiveSha256([string]$ChecksumPath) {
    $checksum = (Get-Content -LiteralPath $ChecksumPath -Raw -Encoding ASCII).Trim()
    if ($checksum -notmatch '^[A-Fa-f0-9]{64}$') { throw "无效的 SHA-256 校验文件: $ChecksumPath" }
    return $checksum.ToLowerInvariant()
}

function Test-PeFile([string]$Path) {
    if ((Get-Item -LiteralPath $Path).Length -lt 2) { return $false }
    $stream = [System.IO.File]::OpenRead($Path)
    try { return $stream.ReadByte() -eq 0x4D -and $stream.ReadByte() -eq 0x5A }
    finally { $stream.Dispose() }
}

try {
    if ([string]::IsNullOrWhiteSpace($Version)) {
        $release = Invoke-RestMethod -Uri 'https://api.github.com/repos/majd/ipatool/releases/latest' -Headers @{ 'User-Agent' = 'IPAbuyer ipatool fetcher' }
        $Version = [string]$release.tag_name -replace '^v', ''
        if ($Version -notmatch '^\d+\.\d+\.\d+$') { throw "Latest release tag has an unsupported format: $Version" }
    }
    $releaseBaseUrl = "https://github.com/majd/ipatool/releases/download/v$Version"
    Write-Host "Using ipatool v$Version"

    [System.IO.Directory]::CreateDirectory($OutputDir) | Out-Null
    [System.IO.Directory]::CreateDirectory($TempRoot) | Out-Null

    foreach ($arch in $Targets.Keys) {
        $archiveName = "ipatool-$Version-windows-$arch.tar.gz"
        $checksumName = "$archiveName.sha256sum"
        $executableName = "ipatool-$Version-windows-$arch.exe"
        $archTempDir = Join-Path $TempRoot $arch
        $archivePath = Join-Path $archTempDir $archiveName
        $checksumPath = Join-Path $archTempDir $checksumName
        $extractDir = Join-Path $archTempDir 'extract'
        $expectedExecutablePath = Join-Path $extractDir "bin/$executableName"
        $sidecarName = "ipatool-$($Targets[$arch]).exe"
        $destinationPath = Join-Path $OutputDir $sidecarName

        if ((Test-Path -LiteralPath $destinationPath) -and -not $Force) {
            Write-Host "已存在，跳过: $destinationPath（-Force 覆盖）"
            continue
        }
        [System.IO.Directory]::CreateDirectory($archTempDir) | Out-Null

        Write-Host "Downloading $releaseBaseUrl/$archiveName"
        Invoke-WebRequest -Uri "$releaseBaseUrl/$archiveName" -OutFile $archivePath
        Invoke-WebRequest -Uri "$releaseBaseUrl/$checksumName" -OutFile $checksumPath

        $expectedHash = Get-ArchiveSha256 $checksumPath
        $actualHash = (Get-FileHash -LiteralPath $archivePath -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($actualHash -ne $expectedHash) { throw "SHA-256 mismatch for ${archiveName}: expected $expectedHash, got $actualHash" }
        Write-Host "Verified archive SHA-256: $actualHash"

        [System.IO.Directory]::CreateDirectory($extractDir) | Out-Null
        # 必须用系统自带 bsdtar：PATH 里的 GNU tar (Git) 会把 C:\ 误判为远程主机
        & "$env:SystemRoot\System32\tar.exe" -xzf $archivePath -C $extractDir
        if ($LASTEXITCODE -ne 0) { throw "解压失败: $archiveName" }
        if (-not (Test-Path -LiteralPath $expectedExecutablePath -PathType Leaf)) { throw "压缩包内未找到 bin/$executableName" }
        if (-not (Test-PeFile $expectedExecutablePath)) { throw "非有效 Windows 可执行文件: $expectedExecutablePath" }

        Copy-Item -LiteralPath $expectedExecutablePath -Destination $destinationPath -Force
        $exeHash = (Get-FileHash -LiteralPath $destinationPath -Algorithm SHA256).Hash.ToLowerInvariant()
        Write-Host "Installed: $destinationPath"
        Write-Host "Executable SHA-256: $exeHash"
    }
}
finally {
    if (Test-Path -LiteralPath $TempRoot) {
        Remove-Item -LiteralPath $TempRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
}
