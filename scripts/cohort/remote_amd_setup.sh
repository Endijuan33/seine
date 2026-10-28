#!/bin/bash
# One-time box setup: toolchain, source extract, release build. Run as root from /opt/cohort.
set -uo pipefail
cd /opt/cohort; mkdir -p results
export DEBIAN_FRONTEND=noninteractive PATH=/opt/rocm/bin:$HOME/.cargo/bin:$PATH LD_LIBRARY_PATH=/opt/rocm/lib
command -v cargo >/dev/null 2>&1 || curl -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain 1.93.0 --profile minimal > results/rustup.log 2>&1
apt-get update -qq >/dev/null 2>&1; apt-get install -y -qq build-essential pkg-config libssl-dev > results/apt.log 2>&1
rm -rf src && mkdir src && tar -xzf seine-src.tar.gz -C src && cd src
cargo +1.93.0 build --locked --release --no-default-features --features amd > ../results/build-base.log 2>&1; echo "build-exit=$?" | tee -a ../results/build-base.log
cp target/release/seine ../seine-base
cat > /opt/cohort/meminfo.hip <<'HIP'
#include <hip/hip_runtime.h>
#include <cstdio>
int main(){ size_t f=0,t=0; hipMemGetInfo(&f,&t); printf("hipMemGetInfo free=%zu MiB total=%zu MiB\n", f>>20, t>>20);
  hipDeviceProp_t p; hipGetDeviceProperties(&p,0); printf("totalGlobalMem=%zu MiB warpSize=%d multiProcessorCount=%d clockRate=%d kHz memoryClockRate=%d kHz memoryBusWidth=%d l2CacheSize=%d\n", (size_t)p.totalGlobalMem>>20, p.warpSize, p.multiProcessorCount, p.clockRate, p.memoryClockRate, p.memoryBusWidth, p.l2CacheSize);
  for (size_t gib : {64,128,160,192,224,240,248}) { void* ptr=nullptr; hipError_t e=hipMalloc(&ptr, gib<<30); printf("hipMalloc %zu GiB -> %s\n", gib, hipGetErrorString(e)); if(e==hipSuccess) hipFree(ptr); }
  return 0; }
HIP
hipcc -o /opt/cohort/meminfo /opt/cohort/meminfo.hip > /opt/cohort/results/meminfo-build.log 2>&1 && /opt/cohort/meminfo > /opt/cohort/results/meminfo.txt 2>&1
cat /opt/cohort/results/meminfo.txt
touch /opt/cohort/SETUP-DONE
