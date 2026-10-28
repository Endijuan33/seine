#!/bin/bash
# Samples THP coverage of running seine processes during a live arm: per process
# AnonHugePages / Rss (smaps_rollup) and the global thp_* vmstat counters, at the
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
    done
    grep -E '^(thp_fault_alloc|thp_fault_fallback|thp_collapse_alloc|thp_collapse_alloc_failed|thp_split_pmd|thp_deferred_split_page|compact_stall|compact_fail|numa_hint_faults|numa_pages_migrated) ' /proc/vmstat | tr '\n' ' '; echo
    grep -E '^(MemFree|MemAvailable|AnonHugePages|HugePages_Total|HugePages_Free|HugePages_Rsvd|HugePages_Surp):' /proc/meminfo | tr -s ' ' | tr '\n' ' '; echo
  } >> "$OUT"
done
