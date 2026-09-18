# 将 package.json 的 version 同步到 src-tauri 侧的版本声明。
# 用法:
#   .\version.ps1           # 同步版本（package.json → Cargo.toml / tauri.conf.json / Cargo.lock）
#   .\version.ps1 -Check    # 仅检查是否一致，不写入
# 说明:
#   - package.json 的 version 为唯一来源；接受 x.y.z 或 x.y.z.w（CalVer）
#   - 只替换版本行，不重排目标文件格式
param(
    [switch]$Check
)

$ErrorActionPreference = "Stop"
$RepoRoot = $PSScriptRoot

$pkgPath = Join-Path $RepoRoot "package.json"
$pkg = Get-Content $pkgPath -Raw -Encoding UTF8 | ConvertFrom-Json
$version = [string]$pkg.version
if ([string]::IsNullOrWhiteSpace($version)) {
    Write-Error "package.json 的 version 为空。"
    exit 1
}
if ($version -notmatch '^\d+\.\d+\.\d+(\.\d+)?$') {
    Write-Error "package.json 的版本格式非法（应为 x.y.z 或 x.y.z.w，CalVer）：$version"
    exit 1
}

# (路径, 匹配目标行的正则, 替换模板; $1/$2 为保留组, ${version} 为新版本)
$targets = @(
    @{
        Path     = Join-Path $RepoRoot "src-tauri\Cargo.toml"
        Pattern  = '(?m)^version = "[^"]*"'
        Template = 'version = "${version}"'
        Label    = "src-tauri/Cargo.toml"
    },
    @{
        Path     = Join-Path $RepoRoot "src-tauri\tauri.conf.json"
        Pattern  = '(?m)^(\s*"version"\s*:\s*)"[^"]*"'
        Template = '${1}"${version}"'
        Label    = "src-tauri/tauri.conf.json"
    },
    @{
        Path     = Join-Path $RepoRoot "src-tauri\Cargo.lock"
        Pattern  = '(?s)(name = "ipabuyer"\r?\nversion = ")[^"]*(")'
        Template = '${1}${version}${2}'
        Label    = "src-tauri/Cargo.lock"
    }
)

$regex = [System.Text.RegularExpressions.Regex]
$utf8 = New-Object System.Text.UTF8Encoding($false)
$pending = @()

foreach ($target in $targets) {
    $content = [System.IO.File]::ReadAllText($target.Path)
    $updated = $regex::Replace($content, $target.Pattern, {
        param($match)
        $match.Result($target.Template.Replace('${version}', $version))
    })
    if ($updated -eq $content) {
        Write-Host "已是最新: $($target.Label)"
    }
    else {
        if ($Check) {
            Write-Host "需要更新: $($target.Label) -> $version"
        }
        else {
            [System.IO.File]::WriteAllText($target.Path, $updated, $utf8)
            Write-Host "已更新: $($target.Label) -> $version"
        }
        $pending += $target.Label
    }
}

if ($Check) {
    if ($pending.Count -gt 0) { exit 1 }
    Write-Host "所有版本声明均已同步为 $version。"
}
elseif ($pending.Count -eq 0) {
    Write-Host "所有版本声明均已同步为 $version，无需更改。"
}
