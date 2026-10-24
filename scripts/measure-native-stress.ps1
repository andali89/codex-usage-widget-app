param(
    [string]$Executable = 'src-tauri/target/debug/codex-usage-widget.exe',
    [string]$Output = 'src-tauri/target/validation/native-stress-samples.csv'
)
$ErrorActionPreference = 'Stop'
$widgetRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$widgetExecutable = (Resolve-Path (Join-Path $widgetRoot $Executable)).Path
if (-not $widgetExecutable.StartsWith($widgetRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) { throw 'Use a widget executable in this workspace.' }
$widgetOutput = [IO.Path]::GetFullPath((Join-Path $widgetRoot $Output))
$widgetTarget = Join-Path $widgetRoot 'src-tauri/target'
if (-not $widgetOutput.StartsWith($widgetTarget + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) { throw 'Write validation samples below src-tauri/target.' }
if (Get-Process -Name 'codex-usage-widget' -ErrorAction SilentlyContinue) { throw 'Close the running widget before this isolated validation run.' }
New-Item -ItemType Directory -Path (Split-Path -Parent $widgetOutput) -Force | Out-Null
$widgetStdout = [IO.Path]::ChangeExtension($widgetOutput, 'stdout.log')
$widgetStderr = [IO.Path]::ChangeExtension($widgetOutput, 'stderr.log')
$widgetSamples = [Collections.Generic.List[object]]::new()
$widgetObserved = @{}
$widgetPreviousCpu = @{}
$widgetTotalCpu = 0.0
$widgetHashProvider = [Security.Cryptography.SHA256]::Create()
$widgetExecutableStream = [IO.File]::OpenRead($widgetExecutable)
try { $widgetExecutableHash = [BitConverter]::ToString($widgetHashProvider.ComputeHash($widgetExecutableStream)).Replace('-', '') }
finally { $widgetExecutableStream.Dispose(); $widgetHashProvider.Dispose() }
$widgetLogicalProcessors = [int](Get-CimInstance Win32_ComputerSystem -Property NumberOfLogicalProcessors).NumberOfLogicalProcessors
if ($widgetLogicalProcessors -lt 1) { throw 'Cannot determine total logical processors.' }
$widgetTimer = [Diagnostics.Stopwatch]::StartNew()
$widgetProcess = Start-Process -FilePath $widgetExecutable -ArgumentList '--check-native-stress' -PassThru -WindowStyle Hidden -RedirectStandardOutput $widgetStdout -RedirectStandardError $widgetStderr
# Retain the native process handle while it is alive; Windows PowerShell otherwise
# may return a null ExitCode for a rapidly exiting Start-Process -PassThru child.
[void]$widgetProcess.Handle
$widgetObserved[$widgetProcess.Id] = $widgetProcess.StartTime.ToUniversalTime().Ticks
try {
    while (-not $widgetProcess.HasExited -and $widgetTimer.Elapsed.TotalSeconds -lt 180) {
        $widgetRows = @(Get-CimInstance -Query 'SELECT ProcessId, ParentProcessId FROM Win32_Process')
        $widgetCurrent = [Collections.Generic.HashSet[int]]::new()
        [void]$widgetCurrent.Add($widgetProcess.Id)
        do {
            $widgetAdded = $false
            foreach ($widgetRow in $widgetRows) {
                if ($widgetCurrent.Contains([int]$widgetRow.ParentProcessId) -and $widgetCurrent.Add([int]$widgetRow.ProcessId)) { $widgetAdded = $true }
            }
        } while ($widgetAdded)
        $widgetPrivateBytes = 0L
        $widgetLiving = 0
        foreach ($widgetChildId in $widgetCurrent) {
            $widgetChild = Get-Process -Id $widgetChildId -ErrorAction SilentlyContinue
            if ($null -eq $widgetChild) { continue }
            try {
                $widgetCreation = $widgetChild.StartTime.ToUniversalTime().Ticks
                $widgetObserved[$widgetChildId] = $widgetCreation
                $widgetCpu = $widgetChild.TotalProcessorTime.TotalSeconds
                $widgetIdentity = "$widgetChildId/$widgetCreation"
                $widgetPrevious = if ($widgetPreviousCpu.ContainsKey($widgetIdentity)) { $widgetPreviousCpu[$widgetIdentity] } else { 0.0 }
                $widgetTotalCpu += [Math]::Max(0.0, $widgetCpu - $widgetPrevious)
                $widgetPreviousCpu[$widgetIdentity] = $widgetCpu
                $widgetPrivateBytes += $widgetChild.PrivateMemorySize64
                $widgetLiving++
            } catch { } finally { $widgetChild.Dispose() }
        }
        $widgetCycle = 0
        $widgetText = Get-Content -Raw -LiteralPath $widgetStdout -ErrorAction SilentlyContinue
        if ($widgetText) {
            $widgetMatches = [regex]::Matches($widgetText, 'native-stress cycles=(\d+)')
            if ($widgetMatches.Count) { $widgetCycle = [int]$widgetMatches[$widgetMatches.Count - 1].Groups[1].Value }
        }
        $widgetSamples.Add([pscustomobject]@{ Seconds = [Math]::Round($widgetTimer.Elapsed.TotalSeconds, 3); CompletedCycles = $widgetCycle; PrivateMiB = [Math]::Round($widgetPrivateBytes / 1MB, 3); Processes = $widgetLiving; SampledCpuSeconds = [Math]::Round($widgetTotalCpu, 4) })
        Start-Sleep -Milliseconds 500
        $widgetProcess.Refresh()
    }
    $widgetSamples | Export-Csv -LiteralPath $widgetOutput -NoTypeInformation -Encoding utf8
    $widgetFinal = Get-Content -Raw -LiteralPath $widgetStdout
    if (-not $widgetProcess.HasExited -or $widgetFinal -notmatch 'native-stress result=pass cycles=100' -or $widgetFinal -notmatch 'native-check result=pass') { throw "Stress check failed; inspect $widgetStdout and $widgetStderr" }
    $widgetProcess.WaitForExit()
    if ($widgetProcess.ExitCode -ne 0) { throw "Stress process exited with code $($widgetProcess.ExitCode)." }
    $widgetStressSamples = @($widgetSamples | Where-Object { $_.CompletedCycles -ge 25 -and $_.CompletedCycles -lt 100 })
    if ($widgetStressSamples.Count -lt 2) { throw 'Insufficient plateau samples.' }
    $widgetPeak = ($widgetStressSamples | Measure-Object PrivateMiB -Maximum).Maximum
    $widgetMinimum = ($widgetStressSamples | Measure-Object PrivateMiB -Minimum).Minimum
    $widgetElapsed = $widgetTimer.Elapsed.TotalSeconds
    [pscustomobject]@{ Result = 'pass'; Cycles = 100; SampleCount = $widgetSamples.Count; Seconds = [Math]::Round($widgetElapsed, 3); StressPrivateMiBMin = $widgetMinimum; StressPrivateMiBMax = $widgetPeak; SampledCpuMachinePercent = [Math]::Round(100 * $widgetTotalCpu / $widgetElapsed / $widgetLogicalProcessors, 3); LogicalProcessors = $widgetLogicalProcessors; ExecutableSha256 = $widgetExecutableHash; Samples = $widgetOutput } | ConvertTo-Json -Compress
} finally {
    # Stop only this root/descendants, verifying creation times to avoid PID reuse.
    Start-Sleep -Seconds 3
    $widgetRemaining = 0
    foreach ($widgetChildId in $widgetObserved.Keys) {
        $widgetChild = Get-Process -Id $widgetChildId -ErrorAction SilentlyContinue
        if ($null -eq $widgetChild) { continue }
        try {
            if ($widgetChild.StartTime.ToUniversalTime().Ticks -eq $widgetObserved[$widgetChildId]) { $widgetRemaining++; Stop-Process -Id $widgetChildId -Force -ErrorAction SilentlyContinue }
        } catch { } finally { $widgetChild.Dispose() }
    }
    Write-Output "remainingOwnedProcessesBeforeCleanup=$widgetRemaining"
    $widgetProcess.Dispose()
    if ($widgetRemaining -gt 0) { throw 'Owned validation processes required forced cleanup after the grace period.' }
}
