#!/bin/bash
# Usage: remote_amd_bench.sh <binary> <tag> <secs> <rounds> <lanes...>   (extra args via EXTRA="...")
# Runs backend-kind benches with one warmup round, sampling rocm-smi power every 5 s; writes results/<tag>-<lanes>lane.{json,log} and results/<tag>-power.txt
set -uo pipefail
BIN=$1; TAG=$2; SECS=$3; ROUNDS=$4; shift 4
cd /opt/cohort; export PATH=/opt/rocm/bin:$PATH LD_LIBRARY_PATH=/opt/rocm/lib
(while [ ! -e /opt/cohort/STOP-POWER-$TAG ]; do echo "$(date -u +%T) $(rocm-smi --showpower 2>/dev/null | grep -o 'Package Power (W): [0-9.]*' | head -1)"; sleep 5; done) > results/$TAG-power.txt 2>&1 &
for lanes in "$@"; do
  echo "== $(date -u +%T) $TAG lanes=$lanes" >> results/progress-$TAG.txt
  "$BIN" --bench --bench-kind backend --backend amd --amd-max-lanes "$lanes" --bench-secs "$SECS" --bench-rounds "$ROUNDS" --bench-warmup-rounds 1 ${EXTRA:-} --bench-output "results/$TAG-${lanes}lane.json" > "results/$TAG-${lanes}lane.log" 2>&1
  echo "exit=$?" >> "results/$TAG-${lanes}lane.log"
done
touch /opt/cohort/STOP-POWER-$TAG; sleep 6
echo "== done $(date -u +%T)" >> results/progress-$TAG.txt
