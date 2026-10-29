#!/bin/bash
# One-time driver + ROCm install on a fresh Ubuntu 24.04 host with an AMD Radeon card, then reboot.
# Run as root. Bare metal (Hostkey R9700, gfx1201) and Azure NVads V710 (gfx1101, SR-IOV VF) use the
# AMD stable repo (MODE=rocm, default). Azure NGads V620 (gfx1030) only supports the Microsoft-published
# amdgpu-install 6.4.2.2 build (MODE=azure-v620). After the reboot run remote_amd_setup.sh as usual.
#   GFX=gfx1201 bash remote_rocm_install.sh         # R9700 / RX 9070
#   GFX=gfx1101 bash remote_rocm_install.sh         # V710 / RX 7700-7800
#   MODE=azure-v620 bash remote_rocm_install.sh     # V620 (Secure Boot must be off)
set -uo pipefail
GFX=${GFX:-gfx1201}
MODE=${MODE:-rocm}
export DEBIAN_FRONTEND=noninteractive
mkdir -p /opt/cohort/results && cd /opt/cohort
log=/opt/cohort/results/rocm-install.log
{
  echo "mode=$MODE gfx=$GFX kernel=$(uname -r)"; lspci -d 1002: ; cat /etc/os-release | head -2
  apt-get update -qq
  apt-get install -y -qq "linux-headers-$(uname -r)" "linux-modules-extra-$(uname -r)" build-essential pkg-config \
      libssl-dev curl wget gpg libatomic1 libquadmath0
  usermod -a -G render,video root
  if [ "$MODE" = azure-v620 ]; then
    base=https://repo.radeon.com/.hidden/4beb847a345ee8f56d60594fcd2babe1
    wget -q "$base/amdgpu-install/6.4.2.2/ubuntu/noble/amdgpu-install_6.4.2.2.60402-1_all.deb"
    apt-get install -y -qq ./amdgpu-install_6.4.2.2.60402-1_all.deb
    amdgpu-setup -b "$base"
    sed -i "s|https://repo\.radeon\.com/amdgpu/6\.4\.2|$base/amdgpu/6.4.2.2|" /etc/apt/sources.list.d/amdgpu-proprietary.list
    gpg --keyserver keyserver.ubuntu.com --recv-keys 9386B48A1A693C5C
    gpg --export --armor 9386B48A1A693C5C > /etc/apt/trusted.gpg.d/amdgpu.asc
    amdgpu-install -y --usecase=rocm --no-32 --accept-eula
  else
    # Kernel driver from the amdgpu-install helper repo, ROCm 10.1 from the per-target stable repo.
    wget -q https://repo.radeon.com/amdgpu-install/31.60/ubuntu/noble/amdgpu-install_31.60.316000-1_all.deb
    apt-get install -y -qq ./amdgpu-install_31.60.316000-1_all.deb
    apt-get update -qq
    apt-get install -y -qq amdgpu-dkms
    mkdir -p -m 0755 /etc/apt/keyrings
    wget -qO- https://stable.repo.amd.com/rocm/gpg/packages.gpg | gpg --dearmor > /etc/apt/keyrings/amdrocm.gpg
    cat > /etc/apt/sources.list.d/amdrocm-stable.sources <<'SRC'
X-Repo-Id: amdrocm-stable
Types: deb
URIs: https://stable.repo.amd.com/rocm/core/packages/ubuntu2404/
Suites: stable
Components: main
Architectures: amd64
Signed-By: /etc/apt/keyrings/amdrocm.gpg
Enabled: yes
SRC
    apt-get update -qq
    apt-get install -y -qq "amdrocm10.1-$GFX"
    apt-get install -y -qq "amdrocm-core-dev10.1-$GFX" || apt-get install -y -qq "amdrocm-core-devel10.1-$GFX" || echo "no core-dev package for $GFX (hipcc may be missing; the miner only needs the runtime)"
  fi
  ls -d /opt/rocm* ; dkms status
  grep -rn amdgpu /etc/modprobe.d/ 2>/dev/null | grep -i blacklist && echo "WARNING: amdgpu blocklisted"
  echo "install-exit=$?"
} > "$log" 2>&1
tail -5 "$log"
touch /opt/cohort/ROCM-INSTALLED
echo "rebooting"; systemctl reboot
