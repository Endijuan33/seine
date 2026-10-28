#!/bin/bash
# Argument A/B on one build: candidate/seine with BASE_ARGS vs CAND_ARGS, one ABBA
# pair per thread count (bench, fixed kernel). Linux counterpart of remote_args_ab.ps1.
# Usage (root, /opt/cohort): BASE_ARGS="..." CAND_ARGS="--cpu-page-mode regular" \
#   remote_args_ab.sh <name> <kernel> <threads...>
set -u
W=/opt/cohort
R=$W/results/args-ab-$1
KERNEL=$2; shift 2
SECS=${SECS:-20}
mkdir -p "$R"; chown -R bench:bench "$W"
isolated() {
  local dir=$1; shift
  unshare -n setpriv --reuid=bench --regid=bench --init-groups --reset-env -- \
    env HOME=/home/bench PATH=/usr/bin:/bin bash -c 'cd "$0" && exec "$@"' "$dir" "$@"
}
one() { # one <args> <threads> <tag>
  sync; echo 3 > /proc/sys/vm/drop_caches
  # shellcheck disable=SC2086
  isolated "$W/candidate" ./seine --bench --bench-kind backend --backend cpu --ui plain \
    --data-dir /home/bench/seine-args --threads "$2" --disable-cpu-autotune-threads --bench-cpu-kernel "$KERNEL" \
    --bench-secs "$SECS" --bench-rounds 1 --bench-warmup-rounds 1 --bench-output "$R/$3.json" $1 > "$R/$3.log" 2>&1
  python3 -c "import json,sys; d=json.load(open(sys.argv[1])); print(sys.argv[2], round(d['avg_hps'], 3))" "$R/$3.json" "$3"
}
for t in "$@"; do
  one "${BASE_ARGS:-}" "$t" "t$t-1-base"; one "${CAND_ARGS:-}" "$t" "t$t-2-cand"
  one "${CAND_ARGS:-}" "$t" "t$t-3-cand"; one "${BASE_ARGS:-}" "$t" "t$t-4-base"
done | tee "$R/summary.txt"
touch "$R/COMPLETE"
