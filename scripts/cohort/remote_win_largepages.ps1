# Windows large pages vs regular pages for one kernel, ABBA per thread count.
# Runs seine as LocalSystem (which holds SeLockMemoryPrivilege) via a one-shot
# scheduled task, so no account policy change or re-logon is needed.
# Usage: powershell -File remote_win_largepages.ps1 -Kernel avx512-stream -Threads 14
param([string]$Kernel, [string]$Threads, [int]$Secs = 20)
$ThreadList = $Threads -split '[, ]+' | Where-Object { $_ } | ForEach-Object { [int]$_ }
$W = 'C:\cohort'
$R = "$W\results\largepages-$Kernel"
New-Item -ItemType Directory -Force $R | Out-Null
function One($mode, $threads, $tag) {
  $cmd = "`"$W\candidate\seine.exe`" --bench --bench-kind backend --backend cpu --ui plain --data-dir `"$W\data-lp`" " +
    "--threads $threads --disable-cpu-autotune-threads --bench-cpu-kernel $Kernel --cpu-page-mode $mode " +
    "--bench-secs $Secs --bench-rounds 1 --bench-warmup-rounds 1 --bench-output `"$R\$tag.json`" > `"$R\$tag.log`" 2>&1"
  Set-Content "$W\lp-run.cmd" $cmd -Encoding ASCII
  $action = New-ScheduledTaskAction -Execute 'cmd.exe' -Argument "/c `"$W\lp-run.cmd`""
  $principal = New-ScheduledTaskPrincipal -UserId 'SYSTEM' -LogonType ServiceAccount -RunLevel Highest
  Register-ScheduledTask -TaskName 'CohortLargePages' -Action $action -Principal $principal -Force | Out-Null
  Start-ScheduledTask -TaskName 'CohortLargePages'
  Start-Sleep -Seconds 3
  while ((Get-ScheduledTask -TaskName 'CohortLargePages').State -eq 'Running') { Start-Sleep -Seconds 2 }
  Unregister-ScheduledTask -TaskName 'CohortLargePages' -Confirm:$false
  if (Test-Path "$R\$tag.json") {
    $d = Get-Content "$R\$tag.json" -Raw | ConvertFrom-Json
    "$tag $([math]::Round($d.avg_hps, 3))"
  } else { "$tag FAILED" }
}
$out = foreach ($t in $ThreadList) {
  One auto $t "t$t-1-auto"; One large $t "t$t-2-large"; One large $t "t$t-3-large"; One auto $t "t$t-4-auto"
}
$out | Tee-Object "$R\summary.txt"
