#!/bin/bash
# Samples THP coverage of running seine processes during a live arm: per process
# AnonHugePages / Rss (smaps_rollup), per-node arena placement (numa_maps), perf
# TLB counters when perf exists, and the global thp_* vmstat counters, at the
# given seconds after start. Run in the background before the arm:
#   (bash remote_thp_sample.sh <tag> 300 540 &)
set -u
TAG=$1; shift
OUT=/opt/cohort/results/thp-$TAG.txt
mkdir -p /opt/cohort/results
start=$(date +%s)
for at in "$@"; do
  sleep $(( at - ($(date +%s) - start) > 0 ? at - ($(date +%s) - start) : 0 ))
  {
    echo "== t=$at"
    for pid in $(pgrep -x seine); do
      echo "pid $pid $(grep -E '^(Rss|AnonHugePages):' /proc/$pid/smaps_rollup | tr -s ' ' | tr '\n' ' ')"
      echo "pid $pid hugetlb $(grep -E 'HugetlbPages' /proc/$pid/status | tr -s ' ')"
      # Worker pinning: threads allowed on exactly one CPU vs all threads.
      echo "pid $pid threads $(ls /proc/$pid/task | wc -l) single_cpu $(grep -h Cpus_allowed_list /proc/$pid/task/*/status | awk '$2 ~ /^[0-9]+$/' | wc -l)"
      # Per-node placement of each >= 1 GiB mapping (worker arenas): arenas wholly
      # on one node vs split across nodes, and GiB per node.
      awk '{ kb = 4; for (i = 1; i <= NF; i++) if ($i ~ /^kernelpagesize_kB=/) { split($i, a, "="); kb = a[2] }
             total = 0; delete per
             for (i = 1; i <= NF; i++) if ($i ~ /^N[0-9]+=/) { split($i, a, "="); per[a[1]] = a[2] * kb; total += a[2] * kb }
             if (total < 1048576) next
             nodes = 0; for (k in per) { nodes++; gib[k] += per[k] / 1048576 }
             if (nodes > 1) split_arenas++; else whole[k]++ }
           END { printf "pid arenas"; for (k in whole) printf " whole_%s=%d", k, whole[k]
                 printf " split=%d gib", split_arenas + 0; for (k in gib) printf " %s=%.1f", k, gib[k]; print "" }' \
        /proc/$pid/numa_maps 2>/dev/null
    done
    if command -v perf > /dev/null; then
      perf stat -a -x, -e cycles,instructions,dTLB-load-misses,dTLB-loads -- sleep 10 2>&1 | cut -d, -f1,3 | tr '\n' ' '; echo
    fi
    grep -E '^(thp_fault_alloc|thp_fault_fallback|thp_collapse_alloc|thp_collapse_alloc_failed|thp_split_pmd|thp_deferred_split_page|compact_stall|compact_fail|numa_hint_faults|numa_pages_migrated) ' /proc/vmstat | tr '\n' ' '; echo
    grep -E '^(MemFree|MemAvailable|AnonHugePages|HugePages_Total|HugePages_Free|HugePages_Rsvd|HugePages_Surp):' /proc/meminfo | tr -s ' ' | tr '\n' ' '; echo
  } >> "$OUT"
done
