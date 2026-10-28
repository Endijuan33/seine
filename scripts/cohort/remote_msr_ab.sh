#!/bin/bash
# Hardware-prefetcher screen (bare metal only): candidate/seine bench with AMD
# MSR C001_0108 (PrefetchControl; a set bit disables that prefetcher) set on every
# core, each setting bracketed by the default. Restores the original value on exit.
# Bits: 0 L1 stream, 1 L1 stride, 2 L1 region, 3 L2 stream, 5 L2 up/down.
# Usage (root, /opt/cohort): remote_msr_ab.sh <kernel> <threads...>
set -u
W=/opt/cohort
R=$W/results/msr-ab
KERNEL=$1; shift
SECS=${SECS:-20}
MSR=0xc0000108
SETTINGS=${SETTINGS:-"0x1 0x8 0x24 0x2f"}
mkdir -p "$R"; chown -R bench:bench "$W"
command -v wrmsr >/dev/null || DEBIAN_FRONTEND=noninteractive apt-get install -y -qq msr-tools > "$R/apt.log" 2>&1
modprobe msr
ORIG=$(rdmsr -p 0 $MSR) || { echo "MSR-UNAVAILABLE"; touch "$R/COMPLETE"; exit 1; }
trap 'wrmsr -a $MSR 0x$ORIG' EXIT
echo "original 0x$ORIG" | tee "$R/summary.txt"
isolated() {
  local dir=$1; shift
  unshare -n setpriv --reuid=bench --regid=bench --init-groups --reset-env -- \
    env HOME=/home/bench PATH=/usr/bin:/bin bash -c 'cd "$0" && exec "$@"' "$dir" "$@"
}
one() { # one <msr value> <threads> <tag>
  wrmsr -a $MSR "$1"
  sync; echo 3 > /proc/sys/vm/drop_caches
  isolated "$W/candidate" ./seine --bench --bench-kind backend --backend cpu --ui plain \
    --data-dir /home/bench/seine-msr --threads "$2" --disable-cpu-autotune-threads --bench-cpu-kernel "$KERNEL" \
    --bench-secs "$SECS" --bench-rounds 1 --bench-warmup-rounds 1 --bench-output "$R/$3.json" > "$R/$3.log" 2>&1
  python3 -c "import json,sys; d=json.load(open(sys.argv[1])); print(sys.argv[2], round(d['avg_hps'], 3))" "$R/$3.json" "$3"
}
for t in "$@"; do
  one "0x$ORIG" "$t" "t$t-default-a"
  for s in $SETTINGS; do
    one "$s" "$t" "t$t-$s"
  done
  one "0x$ORIG" "$t" "t$t-default-b"
  for s in $(echo $SETTINGS | tr ' ' '\n' | tac); do
    one "$s" "$t" "t$t-$s-r"
  done
  one "0x$ORIG" "$t" "t$t-default-c"
done | tee -a "$R/summary.txt"
touch "$R/COMPLETE"
