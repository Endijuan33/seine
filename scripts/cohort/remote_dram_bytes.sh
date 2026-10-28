#!/bin/bash
# DRAM bytes per hash (bare-metal AMD Zen 4/5): seine bench at a fixed kernel and
# count under each AMD PrefetchControl setting (MSR C000_0108, a set bit disables
# that prefetcher; the first and last runs are the original value), then NTMminer's
# bench (extra/) at the same count, all under `perf stat -a` with every DRAM
# CAS / data-fabric DRAM event the host's perf knows. Each count is one 64-byte line,
# so bytes/hash = sum(reads or writes) * 64 / hashes; the Argon2 floor is ~2.15 GB
# read + ~2.15 GB written per hash. Restores the MSR on exit.
# Usage (root, /opt/cohort): remote_dram_bytes.sh <kernel> <threads> [settings...]
set -u
W=/opt/cohort
R=$W/results/dram
KERNEL=$1 T=$2; shift 2
SETTINGS=${*:-"0x8 0x24"}
SECS=${SECS:-30}
MSR=0xc0000108
mkdir -p "$R"; chown -R bench:bench "$W"
export DEBIAN_FRONTEND=noninteractive
apt-get install -y -qq "linux-tools-$(uname -r)" linux-tools-common msr-tools > "$R/apt.log" 2>&1
modprobe msr 2>/dev/null
modprobe amd_uncore 2>/dev/null  # amd_df / amd_umc PMUs are not always loaded at boot
{ uname -r; ls /sys/bus/event_source/devices; for f in /sys/bus/event_source/devices/amd_umc_0/format/*; do
  [ -e "$f" ] && echo "umc format $(basename "$f"): $(cat "$f")"; done; } > "$R/pmus.txt"
perf list --no-desc 2>/dev/null > "$R/perf-list.txt"
# Named DRAM events, plus raw UMC CAS reads/writes when an amd_umc PMU exists.
CAND=$(grep -oiE '\b(umc_cas_cmd\.(rd|wr)|[a-z_]*dram[a-z0-9_]*|[a-z_]*data_beats[a-z0-9_]*)\b' "$R/perf-list.txt" | grep -v '^cxl' | sort -u)
for u in /sys/bus/event_source/devices/amd_umc_*; do
  [ -e "$u/format/rdwrmask" ] || continue
  n=$(basename "$u")
  CAND="$CAND $n/event=0x0a,rdwrmask=0x1/ $n/event=0x0a,rdwrmask=0x2/"
done
EV=
for e in $CAND; do  # keep only events this kernel can count system-wide
  out=$(perf stat -a -x, -e "$e" -- sleep 0.1 2>&1)
  if [ $? = 0 ] && ! grep -q 'not supported\|not counted' <<<"$out"; then EV="${EV:+$EV,}$e"; fi
done
echo "candidates: $CAND" > "$R/events.txt"; echo "events: $EV" >> "$R/events.txt"
if [ -z "$EV" ] || ! perf stat -a -x, -e "$EV" -- sleep 1 > "$R/probe.log" 2>&1; then
  echo "NO-DRAM-EVENTS" | tee "$R/summary.txt"; cat "$R/probe.log" >> "$R/summary.txt"; touch "$R/COMPLETE"; exit 0
fi
perf stat -a -x, -e "$EV" -o "$R/idle.perf" -- sleep "$SECS"
ORIG=$(rdmsr -p 0 $MSR 2>/dev/null) && trap 'wrmsr -a $MSR 0x$ORIG' EXIT
isolated() {
  local dir=$1; shift
  unshare -n setpriv --reuid=bench --regid=bench --init-groups --reset-env -- \
    env HOME=/home/bench PATH=/usr/bin:/bin bash -c 'cd "$0" && exec "$@"' "$dir" "$@"
}
seine_run() { # seine_run <tag> [msr value]
  [ -n "${2:-}" ] && [ -n "${ORIG:-}" ] && wrmsr -a $MSR "$2"
  sync; echo 3 > /proc/sys/vm/drop_caches
  perf stat -a -x, -e "$EV" -o "$R/$1.perf" -- isolated "$W/candidate" ./seine --bench --bench-kind backend \
    --backend cpu --ui plain --data-dir /home/bench/seine-dram --threads "$T" --disable-cpu-autotune-threads \
    --bench-cpu-kernel "$KERNEL" --bench-secs "$SECS" --bench-rounds 1 --bench-warmup-rounds 1 \
    --bench-output "$R/$1.json" > "$R/$1.log" 2>&1
}
seine_run seine-default-a "0x${ORIG:-}"
if [ -n "${ORIG:-}" ]; then
  for s in $SETTINGS; do seine_run "seine-msr-$s" "$s"; done
  seine_run seine-default-b "0x$ORIG"
fi
if [ -x "$W/extra/NTMminer-BNT" ]; then
  sync; echo 3 > /proc/sys/vm/drop_caches
  perf stat -a -x, -e "$EV" -o "$R/ntm.perf" -- isolated "$W/extra" ./NTMminer-BNT --bench $((SECS * 2)) -t "$T" > "$R/ntm.log" 2>&1
fi
python3 - "$R" <<'PY' | tee "$R/summary.txt"
import json, pathlib, re, sys
r = pathlib.Path(sys.argv[1])
def counts(path):
    # Lines per source (umc = memory-controller CAS, df = data fabric), read / write / other.
    out = {}
    for line in path.read_text().splitlines():
        p = line.split(',')
        if len(p) < 3 or not p[0].replace('.', '').isdigit():
            continue
        v, name = float(p[0]), p[2].lower()
        src = 'umc' if 'umc' in name else 'df'
        kind = ('rd' if ('rdwrmask=0x1' in name or name.endswith('.rd') or 'read' in name) else
                'wr' if ('rdwrmask=0x2' in name or name.endswith('.wr') or 'write' in name) else 'other')
        out[f'{src}_{kind}'] = out.get(f'{src}_{kind}', 0) + v
    return out
idle = counts(r / 'idle.perf')
for perf in sorted(r.glob('*.perf')):
    tag = perf.stem
    if tag == 'idle': continue
    c = counts(perf)
    if tag == 'ntm':
        hps = [float(x) for x in re.findall(r'([0-9.]+) H/s', (r / 'ntm.log').read_text(errors='replace'))]
        hps = hps[-1] if hps else 0.0
    else:
        hps = json.load(open(r / f'{tag}.json'))['avg_hps']
    elapsed = [l.split(',')[0] for l in perf.read_text().splitlines() if 'seconds time elapsed' in l]
    print(tag, 'hps', round(hps, 3), {k: int(v) for k, v in sorted(c.items())}, 'elapsed', elapsed[:1])
print('idle', {k: int(v) for k, v in sorted(idle.items())})
PY
touch "$R/COMPLETE"
