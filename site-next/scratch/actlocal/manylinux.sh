#!/usr/bin/env bash
# Stand-in for the maturin-action jobs act cannot host (ci.yml test-python-bindings,
# wheels.yml Linux legs, wheel-repro): the same quay.io/pypa/manylinux_2_28 container,
# driven directly on the host daemon, x86-64 by default (GitHub's runner arch; Rosetta here).
# Builds the wheel twice with SOURCE_DATE_EPOCH (wheel-repro), checks the auditwheel tag,
# installs it, runs tests/python and the stubtest check (test-python-bindings), and builds
# the sdist. usage: manylinux.sh <checkout> [x86_64|aarch64]
set -euo pipefail
SRC="$1"; ARCH="${2:-x86_64}"
case "$ARCH" in x86_64) PLAT=linux/amd64 ;; aarch64) PLAT=linux/arm64 ;; *) exit 2 ;; esac
TC="$(grep -E '^channel' "$SRC/rust-toolchain.toml" | sed -E 's/.*"([^"]+)".*/\1/')"
OUT="$SRC/target/manylinux-$ARCH"; rm -rf "$OUT"; mkdir -p "$OUT"
docker run --rm --platform "$PLAT" -e SOURCE_DATE_EPOCH=1700000000 -e CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-6}" \
  -e ARCH="$ARCH" -e TC="$TC" -v "$SRC:/io" -w /io --entrypoint bash quay.io/pypa/manylinux_2_28_"$ARCH" -c '
set -euo pipefail
curl -sSf https://sh.rustup.rs -o /tmp/r.sh && sh /tmp/r.sh -y -q --profile minimal --default-toolchain "$TC" >/dev/null
. "$HOME/.cargo/env"
PY=/opt/python/cp312-cp312/bin/python
$PY -m pip -q install "maturin>=1.9.2,<2" auditwheel pytest numpy mypy
export CARGO_TARGET_DIR=/io/target/manylinux-$ARCH/cargo
for n in 1 2; do
  CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1 $PY -m maturin build --release --features python --manylinux 2_28 --out /io/target/manylinux-$ARCH/dist$n
done
h1=$(sha256sum /io/target/manylinux-$ARCH/dist1/*.whl | cut -d" " -f1); h2=$(sha256sum /io/target/manylinux-$ARCH/dist2/*.whl | cut -d" " -f1)
echo "pass1 $h1"; echo "pass2 $h2"
[ "$h1" = "$h2" ] && echo "OK: wheel is byte-reproducible" || { echo "FAIL: wheel differs between two builds"; exit 1; }
for w in /io/target/manylinux-$ARCH/dist1/*.whl; do auditwheel show "$w"; auditwheel show "$w" | grep -q "manylinux_2_28_$ARCH" || { echo "FAIL: $w not manylinux_2_28_$ARCH"; exit 1; }; done
$PY -m maturin sdist --out /io/target/manylinux-$ARCH/sdist
$PY -m pip -q install --no-index --find-links /io/target/manylinux-$ARCH/dist1 kshana
(cd /io && $PY -m pytest -q tests/python)
cd /tmp && printf "kshana\\\\.kshana\n" > allow.txt && cat allow.txt && $PY -m mypy.stubtest kshana --allowlist allow.txt
echo "OK: manylinux_2_28 $ARCH wheel built twice identically, tagged, installed, tested, stub-checked"
'
