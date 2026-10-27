#!/bin/bash
# Same-host live mining comparison, each miner on its defaults, against a local
# all-target Stratum fixture (every hash is a share), network-isolated.
# bntpool.com resolves to the fixture, so Seine's fee work is captured as non-user
# shares; competitor fee connections fail offline, so apply their nominal fee.
# Usage (root, /opt/cohort): remote_live_compete.sh <seconds> [seine|ntmminer|longhorn ...]
# Seine is candidate/seine with no flags; NTM extra/NTMminer-BNT; Longhorn extra/longhorn-*.
set -u
SECS=$1; shift
MINERS=${*:-seine ntmminer longhorn}
W=/opt/cohort
R=$W/results/live-compete-$SECS
mkdir -p "$R/tls"
openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:P-256 -nodes -days 2 -subj /CN=localhost \
  -addext "subjectAltName=DNS:localhost,IP:127.0.0.1" -keyout "$R/tls/key.pem" -out "$R/tls/cert.pem" 2>/dev/null
printf '127.0.0.1 localhost bntpool.com\n' > "$R/hosts"
chown -R bench:bench "$W"
for m in $MINERS; do
  tls=() extra=()
  case $m in
    seine) exe=$W/candidate/seine; extra=(--ui plain --data-dir "/home/bench/live-$m" --telemetry off) ;;
    ntmminer) exe=$W/extra/NTMminer-BNT; tls=(--tls-cert "$R/tls/cert.pem" --tls-key "$R/tls/key.pem" --tls-name localhost) ;;
    longhorn) exe=$(ls -d $W/extra/longhorn-* | head -1)/longhorn ;;
  esac
  [ -x "$exe" ] || { echo "$m missing"; continue; }
  rm -rf "/home/bench/live-$m" "$R/$m"; mkdir -p "/home/bench/live-$m"; chown bench:bench "/home/bench/live-$m"
  sync; echo 3 > /proc/sys/vm/drop_caches
  unshare -n -m bash -c 'ip link set lo up && mount --bind "$0" /etc/hosts && cd "$1" && exec setpriv --reuid=bench --regid=bench --init-groups --reset-env -- env HOME=/home/bench PATH=/usr/bin:/bin "${@:2}"' \
    "$R/hosts" "/home/bench/live-$m" python3 "$W/pool_fixture.py" --output "$R/$m" --seconds "$SECS" --warmup 0 \
    --window-origin launch --startup-timeout 300 --listen-port 3333 --miner "$m" --executable "$exe" "${tls[@]}" -- "${extra[@]}" \
    > "$R/$m.out" 2>&1
  python3 - "$R/$m/work.json" "$m" "$SECS" <<'PY' | tee -a "$R/summary.txt"
import json, sys
d = json.load(open(sys.argv[1])); m, secs = sys.argv[2], float(sys.argv[3])
user = [s["elapsed"] for s in d["shares"] if s["eligible"] and s["user_work"] and s["elapsed"] < secs]
fee = sum(1 for s in d["shares"] if s["eligible"] and not s["user_work"] and s["elapsed"] < secs)
last = sum(1 for t in user if t >= secs - 300) / 300
print(f"{m} lifetime_user={len(user)/secs:.3f} last300_user={last:.3f} fee_shares={fee} "
      f"first_share={d['launch_to_first_share_seconds']} end={d['termination_reason']}")
PY
done
touch "$R/COMPLETE"
