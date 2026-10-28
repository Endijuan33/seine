# Windows argument A/B on one binary: candidate\seine.exe with -BaseArgs vs -CandArgs,
# one ABBA pair per thread count (bench, fixed kernel).
# Usage: powershell -File remote_args_ab.ps1 -Kernel avx512-stream -Threads 14 -Name aff -BaseArgs "--cpu-affinity auto" -CandArgs "--cpu-affinity off"
param([string]$Kernel, [string]$Threads, [string]$Name, [string]$BaseArgs, [string]$CandArgs, [int]$Secs = 20)
$ThreadList = $Threads -split '[, ]+' | Where-Object { $_ } | ForEach-Object { [int]$_ }
$W = 'C:\cohort'
$R = "$W\results\args-ab-$Name"
New-Item -ItemType Directory -Force $R | Out-Null
function One($extra, $threads, $tag) {
  $a = @('--bench', '--bench-kind', 'backend', '--backend', 'cpu', '--ui', 'plain', '--data-dir', "$W\data-args",
    '--threads', $threads, '--disable-cpu-autotune-threads', '--bench-cpu-kernel', $Kernel,
    '--bench-secs', $Secs, '--bench-rounds', 1, '--bench-warmup-rounds', 1, '--bench-output', "$R\$tag.json") + ($extra -split ' ' | Where-Object { $_ })
  & "$W\candidate\seine.exe" @a *> "$R\$tag.log"
  $d = Get-Content "$R\$tag.json" -Raw | ConvertFrom-Json
  "$tag $([math]::Round($d.avg_hps, 3))"
}
$out = foreach ($t in $ThreadList) {
  One $BaseArgs $t "t$t-1-base"; One $CandArgs $t "t$t-2-cand"; One $CandArgs $t "t$t-3-cand"; One $BaseArgs $t "t$t-4-base"
}
$out | Tee-Object "$R\summary.txt"
