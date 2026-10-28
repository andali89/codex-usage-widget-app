# Build public artifacts without embedding this machine's user/workspace paths.
# Environment changes apply only to this process and are restored on completion.
$ErrorActionPreference = 'Stop'
$widgetRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
if ((Get-Location).Path -ne $widgetRoot) { throw 'Run this command from the repository root.' }
if (-not [Environment]::Is64BitProcess) { throw 'Use 64-bit PowerShell for the x64 release.' }
$widgetProfile = [Environment]::GetFolderPath('UserProfile')
if (-not $widgetProfile) { throw 'Cannot determine the user-profile path for remapping.' }
$widgetTarget = Join-Path $widgetRoot 'src-tauri/target/publication-remapped'
$widgetConfig = Get-Content -Raw -LiteralPath (Join-Path $widgetRoot 'src-tauri/tauri.conf.json') | ConvertFrom-Json
$widgetVersion = $widgetConfig.version
$widgetName = "$($widgetConfig.productName)_${widgetVersion}_x64-setup.exe"
$widgetOldTarget = [Environment]::GetEnvironmentVariable('CARGO_TARGET_DIR', 'Process')
$widgetOldFlags = [Environment]::GetEnvironmentVariable('CARGO_ENCODED_RUSTFLAGS', 'Process')
$widgetFlags = @()
if ($widgetOldFlags) {
    $widgetFlags += $widgetOldFlags.Split([char]31)
} elseif ($env:RUSTFLAGS) {
    $widgetFlags += $env:RUSTFLAGS -split '\s+' | Where-Object { $_ }
}
$widgetFlags += "--remap-path-prefix=$widgetProfile=/builder"
$widgetFlags += "--remap-path-prefix=$widgetRoot=/workspace/codex-usage-widget"
$widgetFlags += '-C', 'link-arg=/PDBALTPATH:%_PDB%'
try {
    $env:CARGO_TARGET_DIR = $widgetTarget
    $env:CARGO_ENCODED_RUSTFLAGS = $widgetFlags -join [char]31
    & npm.cmd run desktop:package
    if ($LASTEXITCODE -ne 0) { throw 'Release build failed; no assets were staged.' }

    $widgetBinary = Join-Path $widgetTarget 'release/Usage.exe'
    $widgetBytes = [IO.File]::ReadAllBytes($widgetBinary)
    $widgetTexts = @([Text.Encoding]::UTF8.GetString($widgetBytes), [Text.Encoding]::Unicode.GetString($widgetBytes))
    foreach ($widgetPrefix in @($widgetProfile, $widgetRoot)) {
        foreach ($widgetVariant in @($widgetPrefix, $widgetPrefix.Replace('\', '/'), $widgetPrefix.Replace('\', '\\'))) {
            foreach ($widgetText in $widgetTexts) {
                if ($widgetText.IndexOf($widgetVariant, [StringComparison]::OrdinalIgnoreCase) -ge 0) {
                    throw 'A local source path remains in the executable; review it before staging public assets.'
                }
            }
        }
    }
    $widgetInstaller = Join-Path $widgetTarget "release/bundle/nsis/$widgetName"
    $widgetSha = [Security.Cryptography.SHA256]::Create()
    try {
        $widgetHash = ([BitConverter]::ToString($widgetSha.ComputeHash([IO.File]::ReadAllBytes($widgetInstaller)))).Replace('-', '').ToLowerInvariant()
    } finally {
        $widgetSha.Dispose()
    }
    $widgetAssets = Join-Path $widgetRoot "src-tauri/target/release-assets/v$widgetVersion"
    New-Item -ItemType Directory -Force -Path $widgetAssets | Out-Null
    $widgetAsset = Join-Path $widgetAssets $widgetName
    Copy-Item -LiteralPath $widgetInstaller -Destination $widgetAsset -Force
    [IO.File]::WriteAllText((Join-Path $widgetAssets 'SHA256SUMS.txt'), "$widgetHash  $widgetName`n", [Text.UTF8Encoding]::new($false))
    Write-Output "Public artifacts: src-tauri/target/release-assets/v$widgetVersion"
    Write-Output 'Local source-path check: pass'
} finally {
    [Environment]::SetEnvironmentVariable('CARGO_TARGET_DIR', $widgetOldTarget, 'Process')
    [Environment]::SetEnvironmentVariable('CARGO_ENCODED_RUSTFLAGS', $widgetOldFlags, 'Process')
}
