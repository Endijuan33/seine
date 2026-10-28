# Windows placement investigation (item 8): guest core/L3 topology, then
# candidate\seine.exe (L3-interleaved pin order) vs extra\base\seine.exe at each
# thread count (ABBA, fixed kernel), then --cpu-affinity auto vs off on the base
# build at -AffinityThreads. Power plan and top CPU consumers are recorded so a
# fresh guest's background load cannot masquerade as a placement effect.
# Usage: powershell -File remote_win_item8.ps1 -Kernel avx512-stream -Threads 4,8,14 -AffinityThreads 8
param([string]$Kernel = 'avx512-stream', [string]$Threads = '4,8,14', [int]$AffinityThreads = 8, [int]$Secs = 20)
$ThreadList = $Threads -split '[, ]+' | Where-Object { $_ } | ForEach-Object { [int]$_ }
$W = 'C:\cohort'
$R = "$W\results\item8"
New-Item -ItemType Directory -Force $R | Out-Null

# Topology as Windows sees it: cores, logical processors, L3 masks.
Get-CimInstance Win32_Processor | Select-Object Name, NumberOfCores, NumberOfLogicalProcessors, NumberOfEnabledCore |
  Format-List | Out-File "$R\win32_processor.txt"
Get-CimInstance Win32_CacheMemory | Select-Object Level, InstalledSize, Associativity, NumberOfBlocks |
  Format-Table | Out-File "$R\win32_cache.txt"
try {
  Invoke-WebRequest -UseBasicParsing 'https://live.sysinternals.com/Coreinfo64.exe' -OutFile "$W\coreinfo64.exe"
  & "$W\coreinfo64.exe" -accepteula -c -l -n *> "$R\coreinfo.txt"
} catch { "coreinfo unavailable: $_" | Out-File "$R\coreinfo.txt" }
powercfg /getactivescheme | Out-File "$R\powercfg.txt"

& "$W\candidate\verify_cpu_kernels.exe" *> "$R\parity.log"
if (-not (Select-String -Path "$R\parity.log" -Pattern "^${Kernel}: PASS" -Quiet)) {
  'PARITY-FAILED' | Tee-Object "$R\summary.txt"; exit 1
}

function One($exe, $extra, $threads, $tag) {
  $a = @('--bench', '--bench-kind', 'backend', '--backend', 'cpu', '--ui', 'plain', '--data-dir', "$W\data-item8",
    '--threads', $threads, '--disable-cpu-autotune-threads', '--bench-cpu-kernel', $Kernel,
    '--bench-secs', $Secs, '--bench-rounds', 1, '--bench-warmup-rounds', 1, '--bench-output', "$R\$tag.json") + ($extra -split ' ' | Where-Object { $_ })
  & $exe @a *> "$R\$tag.log"
  if (Test-Path "$R\$tag.json") {
    $d = Get-Content "$R\$tag.json" -Raw | ConvertFrom-Json
    "$tag $([math]::Round($d.avg_hps, 3))"
  } else { "$tag FAILED" }
}
$base = "$W\extra\base\seine.exe"
$cand = "$W\candidate\seine.exe"
$out = foreach ($t in $ThreadList) {
  One $base '' $t "t$t-1-base"; One $cand '' $t "t$t-2-cand"; One $cand '' $t "t$t-3-cand"; One $base '' $t "t$t-4-base"
}
$t = $AffinityThreads
$out += One $base '--cpu-affinity auto' $t "aff$t-1-auto"
$out += One $base '--cpu-affinity off' $t "aff$t-2-off"
$out += One $base '--cpu-affinity off' $t "aff$t-3-off"
$out += One $base '--cpu-affinity auto' $t "aff$t-4-auto"
Get-Process | Sort-Object CPU -Descending | Select-Object -First 10 Name, CPU, WorkingSet64 | Format-Table | Out-File "$R\top-processes.txt"
$out | Tee-Object "$R\summary.txt"
