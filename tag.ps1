Set-Location $PSScriptRoot

$versionJson = pnpm pkg get version --json
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

try {
    $version = $versionJson | ConvertFrom-Json
}
catch {
    Write-Error 'Unable to read the package version.'
    exit 1
}

if ([string]::IsNullOrWhiteSpace($version)) {
    Write-Error 'Package version is empty.'
    exit 1
}

# 三段式版本（如 2026.9.14）补 .0，统一为发布流水线要求的四段式
if ($version -match '^\d+\.\d+\.\d+$') {
    $version = "$version.0"
}
if ($version -notmatch '^\d+\.\d+\.\d+\.\d+$') {
    Write-Error "Package version is not in the four-segment format: $version"
    exit 1
}

git rev-parse --is-inside-work-tree *> $null
if ($LASTEXITCODE -ne 0) {
    Write-Error 'The script must run from a Git repository.'
    exit $LASTEXITCODE
}

$tag = "v$version"
git show-ref --verify --quiet "refs/tags/$tag"
if ($LASTEXITCODE -eq 0) {
    Write-Error "Tag already exists: $tag"
    exit 1
}
if ($LASTEXITCODE -ne 1) { exit $LASTEXITCODE }

Write-Host "Create and push tag: $tag"
$confirmation = Read-Host 'Continue? [y/N]'
if ($confirmation -notmatch '^[yY]$') {
    Write-Host 'Tag creation cancelled.'
    exit 0
}

git tag $tag
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

git push origin $tag
exit $LASTEXITCODE