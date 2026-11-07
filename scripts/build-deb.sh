#!/usr/bin/env bash
# Build a Debian package of Kshana Pro (roadmap L6): the binary, the systemd unit, its
# settings file and the documentation. Needs dpkg-deb (Debian, Ubuntu).
#
#   scripts/build-deb.sh <kshana-pro binary> <out-dir> [arch]
#
# The package is unsigned; a signed apt repository is the publisher's step.
set -euo pipefail
BIN=${1:?usage: build-deb.sh <kshana-pro binary> <out-dir> [arch]}
OUT=${2:?usage: build-deb.sh <kshana-pro binary> <out-dir> [arch]}
ARCH=${3:-$(dpkg --print-architecture 2>/dev/null || echo amd64)}
cd "$(dirname "$0")/.."
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)
ROOT=$(mktemp -d)
trap 'rm -rf "$ROOT"' EXIT
install -D -m 0755 "$BIN" "$ROOT/usr/bin/kshana-pro"
install -D -m 0644 deploy/linux/kshana-pro.service "$ROOT/lib/systemd/system/kshana-pro.service"
install -D -m 0644 deploy/linux/serve.env "$ROOT/etc/kshana-pro/serve.env"
for d in SERVICE.md LICENSING.md; do install -D -m 0644 "docs/$d" "$ROOT/usr/share/doc/kshana-pro/$d"; done
install -D -m 0644 LICENSE "$ROOT/usr/share/doc/kshana-pro/copyright"
mkdir -p "$ROOT/DEBIAN"
cat > "$ROOT/DEBIAN/control" <<CONTROL
Package: kshana-pro
Version: $VERSION
Architecture: $ARCH
Maintainer: Ashforde OÜ <contact@ashforde.org>
Section: science
Priority: optional
Depends: adduser
Homepage: https://kshana.dev
Description: Kshana Pro: PNT resilience studies, job service and Station fleet
 The proprietary Pro edition of the Kshana engine: the command line, the on-premises
 job service (systemd unit kshana-pro) and the licence tools. Needs a licence token.
CONTROL
echo "/etc/kshana-pro/serve.env" > "$ROOT/DEBIAN/conffiles"
cat > "$ROOT/DEBIAN/postinst" <<'POST'
#!/bin/sh
set -e
if [ "$1" = configure ]; then
  getent group kshana >/dev/null || addgroup --system kshana
  getent passwd kshana >/dev/null || adduser --system --ingroup kshana --home /var/lib/kshana-pro --no-create-home kshana
  install -d -o kshana -g kshana -m 0750 /var/lib/kshana-pro /var/lib/kshana-pro/data /var/lib/kshana-pro/licence
  if command -v systemctl >/dev/null 2>&1; then systemctl daemon-reload || true; fi
  echo "kshana-pro installed. Activate a licence: sudo -u kshana KSHANA_PRO_HOME=/var/lib/kshana-pro/licence kshana-pro license activate --file <token.txt>"
  echo "Then: sudo systemctl enable --now kshana-pro"
fi
POST
chmod 0755 "$ROOT/DEBIAN/postinst"
mkdir -p "$OUT"
DEB="$OUT/kshana-pro_${VERSION}_${ARCH}.deb"
dpkg-deb --root-owner-group --build "$ROOT" "$DEB" >/dev/null
echo "$DEB"
