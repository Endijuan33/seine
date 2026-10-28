#!/bin/bash
# AMD (HIP/ROCm) backend bring-up on a rented ROCm box, following docs/AMD_BRINGUP.md.
# Usage (as root, from /opt/cohort with seine-src.tar.gz beside it):
#   bash remote_amd_bringup.sh [lane-sweep...]      default sweep: 1 2 4 8 16 32 64
# Writes results/ (device identity, every step's log, bench JSON) and touches AB-DONE.
set -uo pipefail
cd /opt/cohort
mkdir -p results
export DEBIAN_FRONTEND=noninteractive
export LD_LIBRARY_PATH=/opt/rocm/lib:${LD_LIBRARY_PATH:-}
export PATH=/opt/rocm/bin:$HOME/.cargo/bin:$PATH
step() { echo "== $(date -u +%FT%TZ) $*" | tee -a results/progress.txt; }

step identity
{ uname -a; cat /etc/os-release | head -3; nproc; free -g | head -2; } > results/host.txt 2>&1
{ rocminfo 2>&1 | grep -E 'Marketing Name|Name:.*gfx|Wavefront Size|Compute Unit|Max Clock' ; } > results/rocminfo.txt 2>&1
rocm-smi --showproductname --showmeminfo vram --showclocks > results/rocm-smi.txt 2>&1
{ hipconfig --version; ls -la /opt/rocm/lib/libamdhip64.so* /opt/rocm/lib/libhiprtc.so*; cat /opt/rocm/.info/version; } > results/rocm-versions.txt 2>&1
ls -la /dev/kfd /dev/dri > results/devices.txt 2>&1

step toolchain
if ! command -v cargo >/dev/null 2>&1; then
  curl -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain 1.93.0 --profile minimal > results/rustup.log 2>&1
fi
apt-get update -qq > /dev/null 2>&1; apt-get install -y -qq build-essential pkg-config libssl-dev > results/apt.log 2>&1
rustup toolchain install 1.93.0 --profile minimal >> results/rustup.log 2>&1
cargo --version > results/cargo.txt 2>&1

step source
rm -rf src && mkdir src && tar -xzf seine-src.tar.gz -C src
cd src
cat SOURCE_COMMIT 2>/dev/null > ../results/source-commit.txt
T="cargo +1.93.0 test --locked --no-default-features --features amd"

step a1-unit-tests
$T backend::amd:: -- --test-threads=1 > ../results/a1-unit.log 2>&1; echo "exit=$?" >> ../results/a1-unit.log
step a2-device-props
$T backend::amd::engine::tests::hip_device_props_parse_sanely -- --ignored --exact --nocapture > ../results/a2-props.log 2>&1; echo "exit=$?" >> ../results/a2-props.log
step a3-hiprtc-compile
$T backend::amd::engine::tests::hiprtc_kernel_compiles_for_device_arch -- --ignored --exact --nocapture > ../results/a3-hiprtc.log 2>&1; echo "exit=$?" >> ../results/a3-hiprtc.log
step b-differential
$T backend::amd::tests::gpu_solution_target_bracket_matches_cpu_reference -- --ignored --exact --nocapture > ../results/b-differential.log 2>&1; echo "exit=$?" >> ../results/b-differential.log

step release-build
cargo +1.93.0 build --locked --release --no-default-features --features amd > ../results/build-release.log 2>&1; echo "exit=$?" >> ../results/build-release.log
BIN=target/release/seine
if [ -x "$BIN" ]; then
  step c-1lane-kernel
  $BIN --bench --bench-kind kernel --backend amd --amd-max-lanes 1 --bench-secs 20 --bench-rounds 3 \
    --bench-output ../results/bench-amd-kernel-1lane.json > ../results/c-1lane.log 2>&1; echo "exit=$?" >> ../results/c-1lane.log
  $BIN --bench --bench-kind kernel-effective --backend amd --amd-max-lanes 1 --bench-secs 20 --bench-rounds 3 \
    --bench-output ../results/bench-amd-kerneleff-1lane.json > ../results/c-1lane-effective.log 2>&1; echo "exit=$?" >> ../results/c-1lane-effective.log
  for lanes in "${@:-1 2 4 8 16 32 64}"; do
    step "d-backend-${lanes}lanes"
    $BIN --bench --bench-kind backend --backend amd --amd-max-lanes "$lanes" --bench-secs 20 --bench-rounds 3 \
      --bench-warmup-rounds 1 --bench-output "../results/bench-amd-backend-${lanes}lane.json" > "../results/d-${lanes}lane.log" 2>&1
    echo "exit=$?" >> "../results/d-${lanes}lane.log"
  done
fi
step done
cd /opt/cohort && touch AB-DONE
