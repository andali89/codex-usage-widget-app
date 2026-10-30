param(
    [Parameter(Mandatory = $true)][string]$Directory,
    [switch]$AsJson
)
# Read-only; accepts only this recorder's five known filenames. No dependencies.
$ErrorActionPreference = 'Stop'
$records = New-Object 'System.Collections.Generic.List[object]'
$malformed = 0
foreach ($name in @('widget.4.jsonl', 'widget.3.jsonl', 'widget.2.jsonl', 'widget.1.jsonl', 'widget.jsonl')) {
    $path = Join-Path $Directory $name
    if (!(Test-Path -LiteralPath $path -PathType Leaf)) { continue }
    foreach ($line in Get-Content -LiteralPath $path -Encoding UTF8) {
        try {
            $record = $line | ConvertFrom-Json
            if ($record.format -eq 1) { $records.Add($record) } else { $malformed++ }
        } catch { $malformed++ }
    }
}
$open = @{}
$episodes = New-Object 'System.Collections.Generic.List[object]'
$views = @{}
$visibility = @{}
$runtime = @{}
$lastTimes = @{}
$failures = @{}
$triggers = @{}
$eventCounts = @{}
$dropMaximum = @{}
$clientDropped = 0
$tainted = @{}
$spans = @{}
$bits = [ordered]@{ shared_loading = 1; quota_loading = 2; refreshing = 4; stale = 8; partial = 16; unavailable = 32; blocked = 64 }
function Finish-Episode($key, $at, $complete) {
    if (!$open.ContainsKey($key)) { return }
    $episode = $open[$key]
    $duration = [Math]::Max(0, $at - $episode.start)
    $episodes.Add([pscustomobject]@{
        session = $episode.session; segment = $episode.segment; source = $episode.source
        condition = $episode.condition; origin = $episode.origin
        duration_ms = $duration; complete = $complete; left_censored = $episode.left
    })
    $open.Remove($key)
}
foreach ($record in $records) {
    $session = [string]$record.session
    $prefix = "$session/$($record.segment)"
    $data = $record.data
    $event = [string]$record.event
    $eventCounts[$event]++
    if ($null -ne $record.elapsed_ms) {
        $lastTimes[$prefix] = [long]$record.elapsed_ms
        $spanKey = "$prefix/native"
        if (!$spans.ContainsKey($spanKey)) { $spans[$spanKey] = @{ source = 'native'; start = [long]$record.elapsed_ms; end = [long]$record.elapsed_ms } }
        $spans[$spanKey].end = [long]$record.elapsed_ms
    }
    if ($event -eq 'usage_state' -and $data.runtime) { $runtime[$session] = $true }
    if ($event -eq 'state_enter') {
        $key = "$prefix/native/$($data.condition)"
        $open[$key] = @{ session = $session; segment = $record.segment; source = 'native'; condition = $data.condition; origin = $data.origin; start = [long]$record.elapsed_ms; left = [bool]$data.left_censored }
    }
    if ($event -eq 'state_exit') {
        Finish-Episode "$prefix/native/$($data.condition)" ([long]$record.elapsed_ms) $true
    }
    if ($event -eq 'frontend') {
        $detail = $data.detail
        $source = "$($data.source):$($detail.view)"
        $viewKey = "$prefix/$source"
        if ([int]$detail.view -gt 0) {
            if (!$spans.ContainsKey($viewKey)) { $spans[$viewKey] = @{ source = $source; start = [long]$detail.elapsedMs; end = [long]$detail.elapsedMs } }
            $spans[$viewKey].end = [long]$detail.elapsedMs
        }
        if ($detail.kind -eq 'display') {
            $old = $views[$viewKey]
            foreach ($condition in $bits.Keys) {
                $key = "$viewKey/$condition"
                $visible = ([int]$detail.flags -band 128) -ne 0
                $active = $visible -and (([int]$detail.flags -band $bits[$condition]) -ne 0)
                if ($active -and !$open.ContainsKey($key)) {
                    $origin = if ($null -eq $old) { 'view_initial' } elseif ($runtime[$session]) { 'runtime' } else { 'startup' }
                    $open[$key] = @{ session = $session; segment = $record.segment; source = $source; condition = $condition; origin = $origin; start = [long]$detail.elapsedMs; left = ($null -eq $old -or !$visibility[$viewKey]) }
                }
                if (!$active) { Finish-Episode $key ([long]$detail.elapsedMs) $visible }
            }
            $views[$viewKey] = [long]$detail.elapsedMs
            $visibility[$viewKey] = (([int]$detail.flags -band 128) -ne 0)
        }
        if ($detail.kind -eq 'unmount') {
            foreach ($condition in $bits.Keys) { Finish-Episode "$viewKey/$condition" ([long]$detail.elapsedMs) $false }
            $views.Remove($viewKey)
        }
        if ($detail.kind -eq 'client_dropped') { $clientDropped += [long]$detail.count; $tainted[$session] = $true }
    }
    if ($event -eq 'webview_page' -and $data.started) {
        $retired = @{}
        foreach ($key in @($open.Keys)) {
            $episode = $open[$key]
            if ($episode.session -eq $session -and $episode.source -like "$($data.source):*") {
                $viewKey = "$prefix/$($episode.source)"
                Finish-Episode $key ([long]$views[$viewKey]) $false
                $retired[$viewKey] = $true
            }
        }
        foreach ($key in $retired.Keys) { $views.Remove($key) }
    }
    if ($event -in @('refresh_result', 'protocol_stage') -and $data.error) {
        $key = "$event/$($data.stage)/$($data.error)"; $failures[$key]++
    }
    if ($data.trigger) { $triggers[[string]$data.trigger]++ }
    if ($event -eq 'events_dropped') { $dropMaximum[$session] = [Math]::Max([long]$dropMaximum[$session], [long]$data.total); if ($data.total -gt 0) { $tainted[$session] = $true } }
    if ($event -eq 'segment_end') {
        $dropMaximum[$session] = [Math]::Max([long]$dropMaximum[$session], [long]$data.dropped_total)
        if ($data.dropped_total -gt 0) { $tainted[$session] = $true }
        foreach ($key in @($open.Keys)) {
            if ($key.StartsWith("$prefix/")) {
                $episode = $open[$key]
                $at = if ($episode.source -eq 'native') { [long]$lastTimes[$prefix] } else { [long]$views["$prefix/$($episode.source)"] }
                Finish-Episode $key $at $false
            }
        }
    }
}
foreach ($key in @($open.Keys)) {
    $episode = $open[$key]; $prefix = "$($episode.session)/$($episode.segment)"
    $at = if ($episode.source -eq 'native') { [long]$lastTimes[$prefix] } else { [long]$views["$prefix/$($episode.source)"] }
    Finish-Episode $key $at $false
}
foreach ($episode in $episodes) {
    if ($tainted[$episode.session]) { $episode.complete = $false; $episode.duration_ms = $null }
}
$summary = @($episodes | Group-Object source, condition, origin | ForEach-Object {
    $group = @($_.Group)
    $exact = @($group | Where-Object { $_.complete -and !$_.left_censored } | ForEach-Object { $_.duration_ms } | Sort-Object)
    $median = $null; $p95 = $null; $maximum = $null
    if ($exact.Count) {
        $median = ($exact[[Math]::Floor(($exact.Count - 1) / 2)] + $exact[[Math]::Floor($exact.Count / 2)]) / 2
        $p95 = $exact[[Math]::Ceiling($exact.Count * 0.95) - 1]; $maximum = $exact[-1]
    }
    $observedMs = 0
    foreach ($span in $spans.Values) { if ($span.source -eq $group[0].source) { $observedMs += [Math]::Max(0, $span.end - $span.start) } }
    $entries = @($group | Where-Object { !$_.left_censored }).Count
    $frequency = if ($observedMs -gt 0) { [Math]::Round($entries * 3600000.0 / $observedMs, 3) } else { $null }
    [pscustomobject]@{
        source = $group[0].source; condition = $group[0].condition; origin = $group[0].origin
        episodes = $group.Count; entries = $entries; entries_per_observed_hour = $frequency; observed_span_ms = $observedMs; exact_completed = $exact.Count
        incomplete = @($group | Where-Object { !$_.complete -or $_.left_censored }).Count
        observed_total_ms = ($group | Measure-Object duration_ms -Sum).Sum
        median_ms = $median; p95_ms = $p95; maximum_ms = $maximum
    }
})
$result = [pscustomobject]@{
    records = $records.Count; malformed_lines = $malformed
    dropped_native_or_ingress = ($dropMaximum.Values | Measure-Object -Sum).Sum
    dropped_frontend_buffer = $clientDropped
    summary = $summary; failures = $failures; triggers = $triggers; event_counts = $eventCounts
    episodes = $episodes.ToArray()
}
if ($AsJson) { $result | ConvertTo-Json -Depth 8 } else {
    "Records: $($result.records); malformed/truncated lines: $malformed; dropped native/ingress: $($result.dropped_native_or_ingress); dropped frontend buffer: $clientDropped"
    $summary | Format-Table -AutoSize
    'Failures (refresh outcomes and individual stages counted separately):'; $failures | Format-Table -AutoSize
    'Observed triggers (requests can coalesce):'; $triggers | Format-Table -AutoSize
    'Incomplete episode totals are lower bounds. View statistics overlap native statistics; do not sum them together.'
}
