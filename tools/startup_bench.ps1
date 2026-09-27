<#
.SYNOPSIS
  Cold start -> first window, idle RSS and idle CPU for the real RapidMD binary.

.DESCRIPTION
  Launches the built binary as a real GUI process, polls for the OS main
  window handle (first paint proxy), then samples working set and CPU time
  over a quiet idle window. Reports min/avg/max RSS and idle CPU percent.

  Usage: powershell -File tools/startup_bench.ps1 -Profile fast -Runs 3
#>
param(
    [string]$Profile = "fast",
    [int]$Runs = 3,
    [int]$SettleMs = 1500,
    [int]$IdleMs = 3000
)

$ErrorActionPreference = "Stop"
$exe = Join-Path $PSScriptRoot "..\target\$Profile\rapidmd.exe"
$exe = (Resolve-Path $exe).Path

Write-Host "binary: $exe"
Write-Host ("size:   {0:N0} bytes ({1:N2} MiB)" -f (Get-Item $exe).Length, ((Get-Item $exe).Length / 1MB))
Write-Host "runs:   $Runs   settle: ${SettleMs}ms   idle sample: ${IdleMs}ms"
Write-Host ""

$to_first_window = @()
$idle_rss_avg = @()
$idle_rss_max = @()
$idle_cpu = @()
$cpu_total = @()

for ($i = 1; $i -le $Runs; $i++) {
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    $p = Start-Process -FilePath $exe -PassThru

    # Poll for the first real window: egui cannot paint before this.
    $handle = [IntPtr]::Zero
    while ($sw.ElapsedMilliseconds -lt 30000) {
        Start-Sleep -Milliseconds 5
        $p.Refresh()
        if ($p.HasExited) { break }
        if ($p.MainWindowHandle -ne [IntPtr]::Zero) { $handle = $p.MainWindowHandle; break }
    }
    $sw.Stop()
    $first = $sw.Elapsed.TotalMilliseconds
    if ($handle -eq [IntPtr]::Zero) {
        Write-Host ("  run {0}: NO WINDOW (exited={1})" -f $i, $p.HasExited) -ForegroundColor Red
        try { if (-not $p.HasExited) { $p.Kill() } } catch {}
        continue
    }
    $to_first_window += $first

    # Settle: let the deferred recovery scan + first frames complete.
    Start-Sleep -Milliseconds $SettleMs

    # Idle window: sample working set and CPU.
    $rss = New-Object System.Collections.Generic.List[double]
    $c0 = $p.TotalProcessorTime.TotalMilliseconds
    $w0 = Get-Date
    while (((Get-Date) - $w0).TotalMilliseconds -lt $IdleMs) {
        Start-Sleep -Milliseconds 100
        $p.Refresh()
        if ($p.HasExited) { break }
        $rss.Add([double]$p.WorkingSet64 / 1MB)
    }
    $wall = ((Get-Date) - $w0).TotalMilliseconds
    $c1 = $p.TotalProcessorTime.TotalMilliseconds
    $cpu_ms = $c1 - $c0
    $cpu_total += $cpu_ms

    $avg = ($rss | Measure-Object -Average).Average
    $max = ($rss | Measure-Object -Maximum).Maximum
    $min = ($rss | Measure-Object -Minimum).Minimum
    $pct = 100.0 * $cpu_ms / $wall
    $idle_rss_avg += $avg
    $idle_rss_max += $max
    $idle_cpu += $pct

    Write-Host ("  run {0}: first window {1,7:N0} ms | idle RSS {2,6:N1} MiB (min {3:N1} max {4:N1}) | idle CPU {5,5:N2}% ({6:N0} ms of {7:N0} ms)" -f `
            $i, $first, $avg, $min, $max, $pct, $cpu_ms, $wall)

    # Clean shutdown, then force-kill if the graceful close does not land.
    $closed = $p.CloseMainWindow()
    if (-not $p.WaitForExit(3000)) { try { $p.Kill(); $p.WaitForExit(2000) } catch {} }
    if ($closed) { Write-Host "         closed gracefully" }
}

Write-Host ""
if ($to_first_window.Count -gt 0) {
    function Stat($a) {
        $s = $a | Measure-Object -Minimum -Maximum -Average
        return ("min {0:N1}  avg {1:N1}  max {2:N1}" -f $s.Minimum, $s.Average, $s.Maximum)
    }
    Write-Host ("first window   ms : " + (Stat $to_first_window))
    Write-Host ("idle RSS     MiB : " + (Stat $idle_rss_avg))
    Write-Host ("idle CPU       % : " + (Stat $idle_cpu))
    Write-Host ("idle CPU      ms : " + (Stat $cpu_total))
}
