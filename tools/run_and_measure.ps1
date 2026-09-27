<#
.SYNOPSIS
  Runs a benchmark binary and reports its peak working set.

.DESCRIPTION
  The benchmark prints stage timings; this wrapper samples the child process so
  the memory number comes from the OS rather than from in-process code, which
  cannot read a correct peak from inside a child.

  Usage:
    powershell -File tools/run_and_measure.ps1 -Exe <path> -Sizes 1,10
#>
param(
    [Parameter(Mandatory = $true)][string]$Exe,
    [string]$OutFile = "",
    [int[]]$Sizes = @()
)

$ErrorActionPreference = "Stop"
$exe = (Resolve-Path $Exe).Path

$argList = @()
if ($Sizes.Count -gt 0) { $argList = $Sizes | ForEach-Object { "$_" } }

$outPath = Join-Path $env:TEMP "bench_out.txt"
$p = Start-Process -FilePath $exe -PassThru -NoNewWindow -RedirectStandardOutput $outPath -ArgumentList $argList

$peak = 0.0
while (-not $p.HasExited) {
    try {
        $p.Refresh()
        if ($p.PeakWorkingSet64 -gt $peak) { $peak = $p.PeakWorkingSet64 }
    } catch {}
    Start-Sleep -Milliseconds 50
}
$p.WaitForExit()

$out = ""
if (Test-Path $outPath) { $out = Get-Content $outPath -Raw }
if ($OutFile -ne "") { $out | Out-File -FilePath $OutFile -Encoding utf8 }
Write-Output $out
Write-Host ("PEAK WORKING SET: {0:N1} MiB" -f ($peak / 1MB))
