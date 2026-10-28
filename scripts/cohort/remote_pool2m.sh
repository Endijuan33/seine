#!/bin/bash
# Set the explicit 2 MiB HugeTLB pool for a live run after dropping caches and
# compacting memory. <pages> is a page count, "auto" (everything but 6 GiB, rounded
# down to whole arenas, plus 64 pages of slack) or 0 to release the pool.
# Usage (root, /opt/cohort): remote_pool2m.sh <pages|auto|0> [tag]
set -u
want=$1; tag=${2:-pool}
mkdir -p /opt/cohort/results
if [ "$want" = auto ]; then
  total_kib=$(grep MemTotal /proc/meminfo | tr -s " " | cut -d" " -f2)
  arenas=$(( (total_kib - 6 * 1024 * 1024) / (2 * 1024 * 1024) ))
  want=$(( arenas * 1024 + 64 ))
fi
if [ "$want" != 0 ]; then sync; echo 3 > /proc/sys/vm/drop_caches; echo 1 > /proc/sys/vm/compact_memory; fi
echo "$want" > /proc/sys/vm/nr_hugepages
echo "$tag requested=$want $(grep -E "^(HugePages_Total|HugePages_Free|MemAvailable)" /proc/meminfo | tr -s " " | tr "\n" " ")" >> /opt/cohort/results/pools.txt
cat /proc/sys/vm/nr_hugepages
