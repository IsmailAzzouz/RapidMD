<#
.SYNOPSIS
  Fair cold-start comparison across the fast / lean / release binaries.

.DESCRIPTION
  First-run of any freshly built binary is dominated by cold page cache, not by
  code. This script interleaves profiles round-robin (so cache state hits every
  profile equally) and discards a warmup launch, then reports min/median/max
  time-to-first-window plus idle working set and idle CPU.

  Usage: powershell -File tools/startup_compare.ps1 -Runs 4
#>
param(
    [string[]]$Profiles = @("fast", "lean", "release"),
    [int]$Runs = 4,
    [int]$SettleMs = 1200,
    [int]$IdleMs = 2500
)

$ErrorActionPreference = "Stop"

function Get-BenchPath($name) {
    $p = Join-Path $PSScriptRoot "..\target\$name\rapidmd.exe"
    if (-not (Test-Path $p)) { return $null }
    return (Resolve-Path $p).Path
}

# Warmup: pay the cold-cache cost once, for whichever profile runs first.
$warmExe = Get-BenchPath $Profiles[0]
if ($warmExe) {
    $wp = Start-Process -FilePath $warmExe -PassThru
    while ($wp.MainWindowHandle -eq 0 -and -not $wp.HasExited) { Start-Sleep -Milliseconds 5; $wp.Refresh() }
    Start-Sleep -Milliseconds 800
    if (-not $wp.HasExited) { $wp.CloseMainWindow() | Out-Null; if (-not $wp.WaitForExit(3000)) { $wp.Kill() } }
}

$results = @{}

function Measure-Launch($exe) {
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    $p = Start-Process -FilePath $exe -PassThru
    while ($sw.ElapsedMilliseconds -lt 30000) {
        Start-Sleep -Milliseconds 2
        $p.Refresh()
        if ($p.HasExited) { return $null }
        if ($p.MainWindowHandle -ne [IntPtr]::Zero) { break }
    }
    $sw.Stop()
    $t = $sw.Elapsed.TotalMilliseconds
    if ($p.MainWindowHandle -eq [IntPtr]::Zero) { try { $p.Kill() } catch {}; return $null }

    Start-Sleep -Milliseconds $SettleMs
    $p.Refresh()
    $rss0 = [double]$p.WorkingSet64 / 1MB
    $priv = [double]$p.PrivateMemorySize64 / 1MB

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
    $cpuPct = 100.0 * ($p.TotalProcessorTime.TotalMilliseconds - $c0) / $wall
    $rssMax = if ($rss.Count) { ($rss | Measure-Object -Maximum).Maximum } else { $rss0 }

    $p.CloseMainWindow() | Out-Null
    if (-not $p.WaitForExit(3000)) { try { $p.Kill() } catch {} }

    return [pscustomobject]@{
        FirstWindowMs = $t
        RssMiB = $rss0
        RssMaxMiB = $rssMax
        PrivateMiB = $priv
        IdleCpuPct = $cpuPct
    }
}

Write-Host "interleaved runs: $Runs per profile (round-robin), warmup discarded"
Write-Host ""

for ($i = 1; $i -le $Runs; $i++) {
    foreach ($prof in $Profiles) {
        $exe = Get-BenchPath $prof
        if (-not $exe) { Write-Host "  skip $prof (not built)" -ForegroundColor Yellow; continue }
        $r = Measure-Launch $exe
        if (-not $r) { Write-Host ("  {0,-8} run {1}: FAILED" -f $prof, $i) -ForegroundColor Red; continue }
        if (-not $results.ContainsKey($prof)) { $results[$prof] = @() }
        $results[$prof] += $r
        Write-Host ("  {0,-8} run {1}: window {2,6:N0} ms | rss {3,6:N1} MiB | private {4,6:N1} MiB | idle cpu {5,5:N2}%" -f `
                $prof, $i, $r.FirstWindowMs, $r.RssMiB, $r.PrivateMiB, $r.IdleCpuPct)
    }
}

Write-Host ""
Write-Host "=== summary (median of runs) ==="
foreach ($prof in $results.Keys) {
    $a = $results[$prof]
    $w = $a | ForEach-Object { $_.FirstWindowMs } | Sort-Object
    $rss = $a | ForEach-Object { $_.RssMiB } | Measure-Object -Average
    $priv = $a | ForEach-Object { $_.PrivateMiB } | Measure-Object -Average
    $cpu = $a | ForEach-Object { $_.IdleCpuPct } | Measure-Object -Average
    $med = $w[[int][Math]::Floor($w.Count / 2)]
    $size = (Get-Item (Get-BenchPath $prof)).Length
    Write-Host ("  {0,-8} window min {1,5:N0} med {2,5:N0} max {3,5:N0} ms | rss {4,6:N1} MiB | private {5,6:N1} MiB | idle cpu {6,4:N2}% | exe {7,6:N2} MiB" -f `
            $prof, $w[0], $med, $w[-1], $rss.Average, $priv.Average, $cpu.Average, ($size / 1MB))
}
