param(
    [string]$Installer = 'src-tauri/target/release/bundle/nsis/Codex Usage Widget_0.1.2_x64-setup.exe',
    [string]$Executable = 'src-tauri/target/release/codex-usage-widget.exe',
    [string]$Version = '0.1.2',
    [string]$UpgradeInstaller = '',
    [string]$UpgradeExecutable = '',
    [string]$UpgradeVersion = '',
    [ValidateSet('None', 'Windows', 'Notifications')][string]$NativeCheck = 'None'
)
$ErrorActionPreference = 'Stop'
if (-not [Environment]::Is64BitProcess) { throw 'Use 64-bit PowerShell for the x64 package check.' }
$widgetRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$widgetTarget = [IO.Path]::GetFullPath((Join-Path $widgetRoot 'src-tauri/target'))
$widgetInstallRoot = [IO.Path]::GetFullPath((Join-Path $widgetTarget 'validation/package install'))
if (-not $widgetInstallRoot.StartsWith($widgetTarget + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) { throw 'Install target must remain inside the project target directory.' }
$widgetInstaller = (Resolve-Path (Join-Path $widgetRoot $Installer)).Path
$widgetExecutable = (Resolve-Path (Join-Path $widgetRoot $Executable)).Path
$widgetArtifacts = @($widgetInstaller, $widgetExecutable)
if ($UpgradeInstaller -or $UpgradeExecutable -or $UpgradeVersion) {
    if (-not ($UpgradeInstaller -and $UpgradeExecutable -and $UpgradeVersion)) { throw 'Supply all three upgrade fixture arguments.' }
    $widgetUpgradeInstaller = (Resolve-Path (Join-Path $widgetRoot $UpgradeInstaller)).Path
    $widgetUpgradeExecutable = (Resolve-Path (Join-Path $widgetRoot $UpgradeExecutable)).Path
    $widgetArtifacts += @($widgetUpgradeInstaller, $widgetUpgradeExecutable)
}
foreach ($widgetArtifact in $widgetArtifacts) {
    if (-not $widgetArtifact.StartsWith($widgetTarget + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) { throw 'Use artifacts in this project target directory.' }
}
if ($NativeCheck -ne 'None') {
    $widgetDebugText = [Text.Encoding]::ASCII.GetString([IO.File]::ReadAllBytes($widgetExecutable))
    if (-not $widgetDebugText.Contains('--check-native-notifications') -or -not $widgetDebugText.Contains('native-check result=pass')) { throw 'Native checks require a debug executable with the isolated validation hooks.' }
}
if (Test-Path -LiteralPath $widgetInstallRoot) { throw 'The isolated installation target already exists; inspect it before rerunning.' }
if (Get-Process -Name 'codex-usage-widget*' -ErrorAction SilentlyContinue) { throw 'Close the widget before package validation.' }
$widgetProduct = 'Codex Usage Widget'
$widgetUninstallKey = 'Software\Microsoft\Windows\CurrentVersion\Uninstall\' + $widgetProduct
$widgetLocationKey = 'Software\codexusagewidget\' + $widgetProduct
$widgetRunKey = 'Software\Microsoft\Windows\CurrentVersion\Run'
$widgetApprovalKey = 'Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run'
$widgetIdentityKey = 'Software\Classes\AppUserModelId\dev.codexusagewidget.desktop'
$widgetUser = [Microsoft.Win32.Registry]::CurrentUser
$widgetMachine = [Microsoft.Win32.Registry]::LocalMachine
$widgetParent = $widgetUser.OpenSubKey('Software\codexusagewidget')
$widgetHadParent = $null -ne $widgetParent
if ($null -ne $widgetParent) { $widgetParent.Dispose() }
foreach ($widgetBase in @($widgetUser, $widgetMachine)) {
    foreach ($widgetKeyName in @($widgetUninstallKey, $widgetLocationKey, $widgetIdentityKey)) {
        $widgetKey = $widgetBase.OpenSubKey($widgetKeyName)
        if ($null -ne $widgetKey) { $widgetKey.Dispose(); throw 'An existing product registration must be preserved; use a clean validation user.' }
    }
    foreach ($widgetKeyName in @($widgetRunKey, $widgetApprovalKey)) {
        $widgetKey = $widgetBase.OpenSubKey($widgetKeyName)
        if ($null -ne $widgetKey) {
            try { if ($widgetKey.GetValueNames() -contains $widgetProduct) { throw 'Existing product startup state must be preserved; use a clean validation user.' } }
            finally { $widgetKey.Dispose() }
        }
    }
}
$widgetStartShortcut = Join-Path ([Environment]::GetFolderPath('Programs')) ($widgetProduct + '.lnk')
$widgetDesktopShortcut = Join-Path ([Environment]::GetFolderPath('DesktopDirectory')) ($widgetProduct + '.lnk')
foreach ($widgetShortcut in @($widgetStartShortcut, $widgetDesktopShortcut)) {
    if (Test-Path -LiteralPath $widgetShortcut) { throw 'An existing product shortcut must be preserved; use a clean validation user.' }
}
$widgetState = Join-Path ([Environment]::GetFolderPath('LocalApplicationData')) 'dev.codexusagewidget.desktop/state.json'
function Get-WidgetDigest([string]$Path) {
    if (-not [IO.File]::Exists($Path)) { return $null }
    $widgetHash = [Security.Cryptography.SHA256]::Create()
    $widgetStream = [IO.File]::OpenRead($Path)
    try { return [Convert]::ToBase64String($widgetHash.ComputeHash($widgetStream)) }
    finally { $widgetStream.Dispose(); $widgetHash.Dispose() }
}
$widgetStateBefore = Get-WidgetDigest $widgetState
$widgetPortableFixtures = @{}
function Assert-WidgetPortableData {
    foreach ($widgetPath in $widgetPortableFixtures.Keys) {
        if ((Get-WidgetDigest $widgetPath) -ne $widgetPortableFixtures[$widgetPath]) { throw 'Installer changed a beside-EXE configuration preservation fixture.' }
    }
}
function Assert-WidgetArtifact([string]$Installed, [string]$Expected) {
    if ((Get-WidgetDigest $Installed) -eq (Get-WidgetDigest $Expected)) { return }
    # The pinned Tauri bundler patches UNK to NSS while building NSIS and then
    # restores the standalone executable. Permit exactly that documented marker.
    $widgetExpected = [IO.File]::ReadAllBytes($Expected)
    $widgetAscii = [Text.Encoding]::ASCII.GetString($widgetExpected)
    $widgetMarker = '__TAURI_BUNDLE_TYPE_VAR_UNK'
    $widgetOffset = $widgetAscii.IndexOf($widgetMarker, [StringComparison]::Ordinal)
    if ($widgetOffset -lt 0 -or $widgetAscii.LastIndexOf($widgetMarker, [StringComparison]::Ordinal) -ne $widgetOffset) { throw 'Cannot identify exactly one standalone bundle marker.' }
    $widgetReplacement = [Text.Encoding]::ASCII.GetBytes('__TAURI_BUNDLE_TYPE_VAR_NSS')
    [Array]::Copy($widgetReplacement, 0, $widgetExpected, $widgetOffset, $widgetReplacement.Length)
    $widgetHash = [Security.Cryptography.SHA256]::Create()
    try { $widgetExpectedDigest = [Convert]::ToBase64String($widgetHash.ComputeHash($widgetExpected)) } finally { $widgetHash.Dispose() }
    if ((Get-WidgetDigest $Installed) -ne $widgetExpectedDigest) { throw 'Installed executable differs beyond the expected NSIS bundle marker.' }
}
function Invoke-WidgetInstaller([string]$Path, [string[]]$Arguments, [bool]$ExpectFailure = $false) {
    $widgetChild = Start-Process -FilePath $Path -ArgumentList $Arguments -PassThru -WindowStyle Hidden
    [void]$widgetChild.Handle
    try {
        if (-not $widgetChild.WaitForExit(60000)) { $widgetChild.Kill(); $widgetChild.WaitForExit(); throw 'Validation installer timed out; inspect its isolated target and registration.' }
        if ($ExpectFailure) { if ($widgetChild.ExitCode -eq 0) { throw 'Expected the downgrade installer to reject the operation.' } }
        elseif ($widgetChild.ExitCode -ne 0) { throw "Installer exited with code $($widgetChild.ExitCode)." }
    } finally { $widgetChild.Dispose() }
}
function Assert-WidgetInstalled([string]$ExpectedVersion = $Version, [string]$ExpectedExecutable = $widgetExecutable) {
    $widgetRegistration = $widgetUser.OpenSubKey($widgetUninstallKey)
    if ($null -eq $widgetRegistration) { throw 'Missing current-user uninstall registration.' }
    try {
        if ($widgetRegistration.GetValue('DisplayVersion') -ne $ExpectedVersion -or $widgetRegistration.GetValue('InstallLocation').Trim('"') -ne $widgetInstallRoot) { throw 'Installed version/location mismatch.' }
    } finally { $widgetRegistration.Dispose() }
    $widgetGlobal = $widgetMachine.OpenSubKey($widgetUninstallKey)
    if ($null -ne $widgetGlobal) { $widgetGlobal.Dispose(); throw 'Unexpected machine-wide registration.' }
    $widgetIdentity = $widgetUser.OpenSubKey($widgetIdentityKey)
    if ($null -eq $widgetIdentity) { throw 'Missing current-user notification identity metadata.' }
    try {
        if ($widgetIdentity.GetValue('DisplayName') -ne $widgetProduct -or $widgetIdentity.GetValue('WidgetInstallLocation') -ne $widgetInstallRoot -or $widgetIdentity.GetValue('IconUri') -ne (Join-Path $widgetInstallRoot 'notification.png') -or -not [IO.File]::Exists((Join-Path $widgetInstallRoot 'notification.png'))) { throw 'Notification identity metadata/icon mismatch.' }
    } finally { $widgetIdentity.Dispose() }
    Assert-WidgetArtifact (Join-Path $widgetInstallRoot 'codex-usage-widget.exe') $ExpectedExecutable
    if (-not (Test-Path -LiteralPath $widgetStartShortcut)) { throw 'Missing installed Start menu identity shortcut.' }
    # Read only this .lnk file's metadata; no shell UI or input is automated.
    $widgetShell = [Activator]::CreateInstance([Type]::GetTypeFromProgID('Shell.Application'))
    $widgetFolder = $null
    $widgetLink = $null
    try {
        $widgetFolder = $widgetShell.Namespace((Split-Path -Parent $widgetStartShortcut))
        $widgetLink = $widgetFolder.ParseName((Split-Path -Leaf $widgetStartShortcut))
        if ($widgetLink.ExtendedProperty('System.AppUserModel.ID') -ne 'dev.codexusagewidget.desktop' -or $widgetLink.ExtendedProperty('System.Link.TargetParsingPath') -ne (Join-Path $widgetInstallRoot 'codex-usage-widget.exe')) { throw 'Shortcut identity/target mismatch.' }
    } finally {
        foreach ($widgetCom in @($widgetLink, $widgetFolder, $widgetShell)) { if ($null -ne $widgetCom) { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($widgetCom) } }
    }
    Assert-WidgetPortableData
    if ((Get-WidgetDigest $widgetState) -ne $widgetStateBefore) { throw 'Installer changed existing widget state.' }
    if ((Get-WidgetDigest $widgetSentinel) -ne $widgetSentinelBefore) { throw 'Installer removed app-data preservation fixture.' }
}
function Invoke-WidgetNativeCheck {
    $widgetArgument = if ($NativeCheck -eq 'Notifications') { '--check-native-notifications' } else { '--check-native-windows' }
    $widgetLogRoot = Join-Path $widgetTarget 'validation'
    $widgetStdout = Join-Path $widgetLogRoot 'package-native.stdout.log'
    $widgetStderr = Join-Path $widgetLogRoot 'package-native.stderr.log'
    $widgetObserved = @{}
    $widgetChild = Start-Process -FilePath (Join-Path $widgetInstallRoot 'codex-usage-widget.exe') -ArgumentList $widgetArgument -WindowStyle Hidden -PassThru -RedirectStandardOutput $widgetStdout -RedirectStandardError $widgetStderr
    [void]$widgetChild.Handle
    $widgetObserved[$widgetChild.Id] = $widgetChild.StartTime.ToUniversalTime().Ticks
    $widgetWatch = [Diagnostics.Stopwatch]::StartNew()
    try {
        while (-not $widgetChild.HasExited -and $widgetWatch.Elapsed.TotalSeconds -lt 60) {
            $widgetRows = @(Get-CimInstance -Query 'SELECT ProcessId, ParentProcessId FROM Win32_Process')
            $widgetCurrent = [Collections.Generic.HashSet[int]]::new()
            [void]$widgetCurrent.Add($widgetChild.Id)
            do {
                $widgetAdded = $false
                foreach ($widgetRow in $widgetRows) { if ($widgetCurrent.Contains([int]$widgetRow.ParentProcessId) -and $widgetCurrent.Add([int]$widgetRow.ProcessId)) { $widgetAdded = $true } }
            } while ($widgetAdded)
            foreach ($widgetChildId in $widgetCurrent) {
                $widgetProcess = Get-Process -Id $widgetChildId -ErrorAction SilentlyContinue
                if ($null -eq $widgetProcess) { continue }
                try { $widgetObserved[$widgetChildId] = $widgetProcess.StartTime.ToUniversalTime().Ticks } catch {} finally { $widgetProcess.Dispose() }
            }
            Start-Sleep -Milliseconds 500
            $widgetChild.Refresh()
        }
        if (-not $widgetChild.HasExited) { throw 'Installed native check timed out.' }
        $widgetChild.WaitForExit()
        $widgetText = Get-Content -Raw -LiteralPath $widgetStdout
        if ($widgetChild.ExitCode -ne 0 -or $widgetText -notmatch 'native-check result=pass' -or ($NativeCheck -eq 'Notifications' -and $widgetText -notmatch 'notification-test-outcome-persisted=pass')) { throw "Installed native check failed; inspect $widgetStdout and $widgetStderr" }
        Write-Output "package-check native$NativeCheck=pass (coordinator/queue only; no visual display proof)"
    } finally {
        Start-Sleep -Seconds 3
        $widgetRemaining = 0
        foreach ($widgetChildId in $widgetObserved.Keys) {
            $widgetProcess = Get-Process -Id $widgetChildId -ErrorAction SilentlyContinue
            if ($null -eq $widgetProcess) { continue }
            try { if ($widgetProcess.StartTime.ToUniversalTime().Ticks -eq $widgetObserved[$widgetChildId]) { $widgetRemaining++; Stop-Process -Id $widgetChildId -Force -ErrorAction SilentlyContinue } } catch {} finally { $widgetProcess.Dispose() }
        }
        $widgetChild.Dispose()
        Write-Output "package-check remainingOwnedProcessesBeforeCleanup=$widgetRemaining"
        if ($widgetRemaining -gt 0) { throw 'Installed native check required forced process cleanup.' }
    }
}
$widgetInstalled = $false
$widgetDataDirectory = Split-Path -Parent $widgetState
$widgetHadDataDirectory = [IO.Directory]::Exists($widgetDataDirectory)
$widgetSentinel = Join-Path $widgetDataDirectory ('installer-validation-' + [Guid]::NewGuid().ToString('N') + '.txt')
[void][IO.Directory]::CreateDirectory($widgetDataDirectory)
[IO.File]::WriteAllText($widgetSentinel, 'Synthetic installer-preservation fixture; contains no quota or account data.')
$widgetSentinelBefore = Get-WidgetDigest $widgetSentinel
try {
    Invoke-WidgetInstaller $widgetInstaller @('/S', "/D=$widgetInstallRoot")
    $widgetInstalled = $true
    Assert-WidgetInstalled
    Write-Output 'package-check install=pass currentUser=true artifactMatch=true statePreserved=true'
    if ($NativeCheck -ne 'None') { Invoke-WidgetNativeCheck; Assert-WidgetInstalled }
    foreach ($widgetName in @('state.json', 'state.backup.json', 'storage-location.json')) {
        $widgetPath = Join-Path $widgetInstallRoot $widgetName
        $widgetBytes = [Text.Encoding]::UTF8.GetBytes('{"installerPreservationFixture":"' + [Guid]::NewGuid().ToString('N') + '"}')
        $widgetFile = [IO.File]::Open($widgetPath, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::None)
        try { $widgetFile.Write($widgetBytes, 0, $widgetBytes.Length); $widgetFile.Flush($true) }
        finally { $widgetFile.Dispose() }
        $widgetPortableFixtures[$widgetPath] = Get-WidgetDigest $widgetPath
    }
    Invoke-WidgetInstaller $widgetInstaller @('/S', '/UPDATE', "/D=$widgetInstallRoot")
    Assert-WidgetInstalled
    Write-Output 'package-check sameVersionUpdate=pass statePreserved=true'
    if ($UpgradeInstaller) {
        Invoke-WidgetInstaller $widgetUpgradeInstaller @('/S', '/UPDATE', "/D=$widgetInstallRoot")
        Assert-WidgetInstalled $UpgradeVersion $widgetUpgradeExecutable
        Write-Output 'package-check versionUpgrade=pass statePreserved=true'
        Invoke-WidgetInstaller $widgetInstaller @('/S', '/UPDATE', "/D=$widgetInstallRoot") $true
        Assert-WidgetInstalled $UpgradeVersion $widgetUpgradeExecutable
        Write-Output 'package-check downgradeRejected=pass'
    }
} finally {
    try {
        # Invoke only the uninstaller belonging to our checked absolute workspace target.
        $widgetRegistration = $widgetUser.OpenSubKey($widgetUninstallKey)
        $widgetOwned = $false
        if ($null -ne $widgetRegistration) {
            try { $widgetOwned = $widgetRegistration.GetValue('InstallLocation').Trim('"') -eq $widgetInstallRoot }
            finally { $widgetRegistration.Dispose() }
        }
        if ($widgetOwned -and [IO.File]::Exists((Join-Path $widgetInstallRoot 'uninstall.exe'))) {
            Invoke-WidgetInstaller (Join-Path $widgetInstallRoot 'uninstall.exe') @('/S', "_?=$widgetInstallRoot")
            $widgetRegistration = $widgetUser.OpenSubKey($widgetUninstallKey)
            if ($null -ne $widgetRegistration) { $widgetRegistration.Dispose(); throw 'Uninstall registration survived cleanup.' }
            $widgetIdentity = $widgetUser.OpenSubKey($widgetIdentityKey)
            if ($null -ne $widgetIdentity) { $widgetIdentity.Dispose(); throw 'Notification identity metadata survived cleanup.' }
            if ([IO.File]::Exists((Join-Path $widgetInstallRoot 'codex-usage-widget.exe')) -or (Test-Path -LiteralPath $widgetStartShortcut) -or (Test-Path -LiteralPath $widgetDesktopShortcut)) { throw 'Installed executable/shortcuts survived cleanup.' }
            Assert-WidgetPortableData
            if ((Get-WidgetDigest $widgetState) -ne $widgetStateBefore) { throw 'Uninstall changed existing widget state.' }
            if ((Get-WidgetDigest $widgetSentinel) -ne $widgetSentinelBefore) { throw 'Uninstall removed the app-data preservation fixture.' }
            $widgetUninstaller = Join-Path $widgetInstallRoot 'uninstall.exe'
            if (Test-Path -LiteralPath $widgetUninstaller) { Remove-Item -LiteralPath $widgetUninstaller }
            if ((Test-Path -LiteralPath $widgetInstallRoot) -and @(Get-ChildItem -LiteralPath $widgetInstallRoot -Force).Count -eq 0) { Remove-Item -LiteralPath $widgetInstallRoot }
            # Tauri intentionally retains install-location preferences when data is kept.
            # This key did not exist before this test; remove only our exact test value.
            $widgetLocation = $widgetUser.OpenSubKey($widgetLocationKey, $true)
            if ($null -ne $widgetLocation) {
                try {
                    if ($widgetLocation.GetValue('') -eq $widgetInstallRoot) { $widgetLocation.DeleteValue('', $false); $widgetLocation.DeleteValue('Installer Language', $false) }
                    $widgetEmpty = $widgetLocation.ValueCount -eq 0 -and $widgetLocation.SubKeyCount -eq 0
                } finally { $widgetLocation.Dispose() }
                if ($widgetEmpty) { $widgetUser.DeleteSubKey($widgetLocationKey, $false) }
            }
            if (-not $widgetHadParent) {
                $widgetParent = $widgetUser.OpenSubKey('Software\codexusagewidget')
                if ($null -ne $widgetParent) {
                    try { $widgetEmpty = $widgetParent.ValueCount -eq 0 -and $widgetParent.SubKeyCount -eq 0 } finally { $widgetParent.Dispose() }
                    if ($widgetEmpty) { $widgetUser.DeleteSubKey('Software\codexusagewidget', $false) }
                }
            }
            Write-Output 'package-check uninstall=pass statePreserved=true'
        } elseif ($widgetInstalled) { throw 'Cannot verify ownership for package cleanup.' }
    } finally {
        # Delete only the unique fixture created by this invocation, never user state.
        foreach ($widgetPath in $widgetPortableFixtures.Keys) { if ((Get-WidgetDigest $widgetPath) -eq $widgetPortableFixtures[$widgetPath]) { Remove-Item -LiteralPath $widgetPath } }
        if ((Test-Path -LiteralPath $widgetInstallRoot) -and @(Get-ChildItem -LiteralPath $widgetInstallRoot -Force).Count -eq 0) { Remove-Item -LiteralPath $widgetInstallRoot }
        if ([IO.File]::Exists($widgetSentinel)) { Remove-Item -LiteralPath $widgetSentinel }
        if (-not $widgetHadDataDirectory -and [IO.Directory]::Exists($widgetDataDirectory) -and @(Get-ChildItem -LiteralPath $widgetDataDirectory -Force).Count -eq 0) { Remove-Item -LiteralPath $widgetDataDirectory }
    }
}
Write-Output "package-check result=pass versionUpgradeChecked=$([bool]$UpgradeInstaller) preexistingStatePresent=$($null -ne $widgetStateBefore) appDataSentinelPreserved=true portableConfigPreserved=true (application UI and notifications not checked)"
