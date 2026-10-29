#!/usr/bin/env bash
# Builds the HiveOS custom miner package from a seine Linux x86_64 release.
#
#   packaging/hiveos/build.sh --archive seine-linux-x86_64.tar.gz --version v0.2.46
#
# Writes <out-dir>/seine-hiveos-<version>.tar.gz. HiveOS takes the miner name
# and version from that file name (the version is the text after the last
# dash), and the archive holds one top-level directory with the same name.
set -euo pipefail

usage() {
	cat <<'EOF'
Usage: build.sh --archive PATH --version VERSION [options]

  --archive PATH        seine-linux-x86_64.tar.gz from a release (or a directory
                        or file holding the Linux x86_64 seine binary)
  --version VERSION     release version, for example v0.2.46
  --out-dir DIR         where to write the package (default: current directory)
  --nvrtc-archive PATH  CUDA NVRTC redistributable archive already downloaded
                        (cuda_nvrtc-linux-x86_64-12.8.61-archive.tar.xz)
  --no-nvrtc            leave NVRTC out (for tests; NVIDIA rigs need it)
EOF
}

NVRTC_VERSION=12.8.61
NVRTC_ARCHIVE_NAME="cuda_nvrtc-linux-x86_64-${NVRTC_VERSION}-archive.tar.xz"
NVRTC_URL="https://developer.download.nvidia.com/compute/cuda/redist/cuda_nvrtc/linux-x86_64/${NVRTC_ARCHIVE_NAME}"
NVRTC_SHA256=df1df922d6af3a01ccfbf4db9125b2c20b9b23bbd4a737be9d2261b241ae77c0

archive=""
version=""
out_dir=$PWD
nvrtc_archive=""
with_nvrtc=1
while [[ $# -gt 0 ]]; do
	case $1 in
	--archive) archive=$2; shift 2 ;;
	--version) version=$2; shift 2 ;;
	--out-dir) out_dir=$2; shift 2 ;;
	--nvrtc-archive) nvrtc_archive=$2; shift 2 ;;
	--no-nvrtc) with_nvrtc=0; shift ;;
	-h | --help) usage; exit 0 ;;
	*) echo "unknown option: $1" >&2; usage >&2; exit 2 ;;
	esac
done
[[ -n $archive && -n $version ]] || { usage >&2; exit 2; }

# HiveOS reads the version after the last dash, so the version itself can't
# contain one (v0.3.0-rc1 becomes 0.3.0_rc1).
version=${version#v}
version=${version//-/_}
[[ $version =~ ^[0-9A-Za-z._+]+$ ]] || { echo "invalid version: $version" >&2; exit 2; }

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
repo=$(cd "$here/../.." && pwd)
name=seine-hiveos
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
stage="$work/stage/$name"
mkdir -p "$stage" "$work/release" "$out_dir"

# The seine binary and its license, from the release archive.
if [[ -d $archive ]]; then
	cp -R "$archive/." "$work/release/"
elif [[ $archive == *.tar.gz || $archive == *.tgz ]]; then
	tar -xzf "$archive" -C "$work/release"
else
	cp "$archive" "$work/release/seine"
fi
binary=$(find "$work/release" -type f -name seine | head -n 1)
[[ -n $binary ]] || { echo "no seine binary in $archive" >&2; exit 1; }
install -m 0755 "$binary" "$stage/seine"
if [[ -f $work/release/SEINE-LICENSE.txt ]]; then
	install -m 0644 "$work/release/SEINE-LICENSE.txt" "$stage/SEINE-LICENSE.txt"
else
	install -m 0644 "$repo/LICENSE" "$stage/SEINE-LICENSE.txt"
fi
install -m 0644 "$repo/PRIVACY.md" "$stage/PRIVACY.md"
install -m 0644 "$repo/docs/guides/hiveos.md" "$stage/README.md"

# The HiveOS scripts. HiveOS needs Unix line endings and executable scripts.
for script in h-config.sh h-run.sh h-stats.sh; do
	tr -d '\r' <"$here/$script" >"$stage/$script"
	chmod 0755 "$stage/$script"
done
tr -d '\r' <"$here/h-manifest.conf" | sed "s/@SEINE_VERSION@/$version/" >"$stage/h-manifest.conf"
chmod 0644 "$stage/h-manifest.conf"

# NVRTC compiles seine's NVIDIA kernels at startup; HiveOS doesn't ship it.
if [[ $with_nvrtc -eq 1 ]]; then
	if [[ -z $nvrtc_archive ]]; then
		nvrtc_archive="$work/$NVRTC_ARCHIVE_NAME"
		curl --fail --silent --show-error --location --retry 3 -o "$nvrtc_archive" "$NVRTC_URL"
	fi
	echo "$NVRTC_SHA256  $nvrtc_archive" | sha256sum --check --quiet -
	nvrtc_root="cuda_nvrtc-linux-x86_64-${NVRTC_VERSION}-archive"
	tar -xJf "$nvrtc_archive" -C "$work" \
		"$nvrtc_root/LICENSE" \
		"$nvrtc_root/lib/libnvrtc.so.${NVRTC_VERSION}" \
		"$nvrtc_root/lib/libnvrtc-builtins.so.${NVRTC_VERSION}"
	mkdir -p "$stage/lib"
	install -m 0755 "$work/$nvrtc_root/lib/libnvrtc.so.${NVRTC_VERSION}" "$stage/lib/"
	install -m 0755 "$work/$nvrtc_root/lib/libnvrtc-builtins.so.${NVRTC_VERSION}" "$stage/lib/"
	ln -s "libnvrtc.so.${NVRTC_VERSION}" "$stage/lib/libnvrtc.so.12"
	ln -s "libnvrtc-builtins.so.${NVRTC_VERSION}" "$stage/lib/libnvrtc-builtins.so.${NVRTC_VERSION%.*}"
	install -m 0644 "$work/$nvrtc_root/LICENSE" "$stage/NVIDIA-CUDA-NVRTC-LICENSE.txt"
fi

(cd "$stage" && find . -type f -printf '%P\n' | LC_ALL=C sort | xargs sha256sum) >"$work/SHA256SUMS.txt"
mv "$work/SHA256SUMS.txt" "$stage/SHA256SUMS.txt"
chmod 0644 "$stage/SHA256SUMS.txt"
find "$stage" -type d -exec chmod 0755 {} +

package="$out_dir/$name-$version.tar.gz"
tar -C "$work/stage" --sort=name --owner=0 --group=0 --numeric-owner \
	--mtime="@${SOURCE_DATE_EPOCH:-$(date +%s)}" -czf "$work/package.tar.gz" "$name"
mv "$work/package.tar.gz" "$package"
echo "$package"
