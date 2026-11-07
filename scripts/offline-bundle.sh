#!/usr/bin/env bash
# The air-gapped install bundle (roadmap L7): everything an offline site needs to install
# and license Kshana Pro, checked with no network.
#
#   scripts/offline-bundle.sh <kshana-pro binary> <out-dir> [--sign <key file> --signer <name>]
#                            [--revocations <revocations.txt>] [--deb <package.deb>]
#
# <out-dir> must not exist. It receives:
#   bin/kshana-pro               the binary
#   packages/                    the .deb when given (Linux)
#   deploy/                      the systemd unit and its settings file
#   docs/                        install, licensing, service and payments guides
#   licence/revocations.txt      the signed revocation list, when given
#   sbom.cdx.json                CycloneDX bill of materials of the locked build graph
#   install.sh                   installs the binary (and the service), activates a licence offline
#   SHA256SUMS                   every file's SHA-256
#   signature.json               with --sign: every file under one Ed25519 signature (the
#                                licence for signing: $KSHANA_PRO_VENDOR_LICENSE)
#                                (kshana-pro dossier sign; verify with
#                                `kshana-pro dossier verify <dir> --trust <public key>` or
#                                scripts/verify-dossier-signature.py, both offline)
# Without --sign the bundle is unsigned and says so in BUNDLE.txt.
set -euo pipefail
BIN=${1:?usage: offline-bundle.sh <kshana-pro binary> <out-dir> [options]}
OUT=${2:?usage: offline-bundle.sh <kshana-pro binary> <out-dir> [options]}
shift 2
SIGN_KEY="" SIGNER="" REVS="" DEB=""
while [ $# -gt 0 ]; do
  case "$1" in
    --sign) SIGN_KEY=$2; shift 2 ;;
    --signer) SIGNER=$2; shift 2 ;;
    --revocations) REVS=$2; shift 2 ;;
    --deb) DEB=$2; shift 2 ;;
    *) echo "unknown option $1" >&2; exit 2 ;;
  esac
done
cd "$(dirname "$0")/.."
[ ! -e "$OUT" ] || { echo "$OUT exists; a bundle goes to a new directory" >&2; exit 1; }
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)
mkdir -p "$OUT"/{bin,deploy,docs,licence}
install -m 0755 "$BIN" "$OUT/bin/kshana-pro"
cp deploy/linux/kshana-pro.service deploy/linux/serve.env "$OUT/deploy/"
cp docs/LICENSING.md docs/SERVICE.md "$OUT/docs/"
[ -z "$REVS" ] || cp "$REVS" "$OUT/licence/revocations.txt"
if [ -n "$DEB" ]; then mkdir -p "$OUT/packages"; cp "$DEB" "$OUT/packages/"; fi
cargo metadata --format-version 1 --locked --offline 2>/dev/null | python3 scripts/sbom.py > "$OUT/sbom.cdx.json" \
  || cargo metadata --format-version 1 --locked | python3 scripts/sbom.py > "$OUT/sbom.cdx.json"
cat > "$OUT/install.sh" <<'INSTALL'
#!/usr/bin/env bash
# Install Kshana Pro from this bundle, with no network.
#   ./install.sh [--prefix <dir>] [--licence <token file>] [--service]
# --prefix   where bin/kshana-pro goes (default /usr/local)
# --licence  activate this token for the installing user (or $KSHANA_PRO_HOME)
# --service  also install the systemd unit (Linux, as root)
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
PREFIX=/usr/local LICENCE="" SERVICE=0
while [ $# -gt 0 ]; do
  case "$1" in
    --prefix) PREFIX=$2; shift 2 ;;
    --licence) LICENCE=$2; shift 2 ;;
    --service) SERVICE=1; shift ;;
    *) echo "unknown option $1" >&2; exit 2 ;;
  esac
done
# Check every file first: nothing is installed from a damaged bundle.
(cd "$HERE" && if command -v sha256sum >/dev/null; then sha256sum -c --quiet SHA256SUMS; else shasum -a 256 -c --quiet SHA256SUMS; fi)
install -d "$PREFIX/bin"
install -m 0755 "$HERE/bin/kshana-pro" "$PREFIX/bin/kshana-pro"
echo "installed $PREFIX/bin/kshana-pro"
if [ -f "$HERE/licence/revocations.txt" ]; then
  "$PREFIX/bin/kshana-pro" license revocations install "$HERE/licence/revocations.txt"
fi
if [ -n "$LICENCE" ]; then
  "$PREFIX/bin/kshana-pro" license activate --file "$LICENCE"
fi
if [ "$SERVICE" = 1 ]; then
  install -D -m 0644 "$HERE/deploy/kshana-pro.service" /etc/systemd/system/kshana-pro.service
  install -D -m 0644 "$HERE/deploy/serve.env" /etc/kshana-pro/serve.env
  sed -i "s#/usr/bin/kshana-pro#$PREFIX/bin/kshana-pro#" /etc/systemd/system/kshana-pro.service
  getent passwd kshana >/dev/null || useradd --system --home /var/lib/kshana-pro --shell /usr/sbin/nologin kshana
  install -d -o kshana -g kshana -m 0750 /var/lib/kshana-pro /var/lib/kshana-pro/data /var/lib/kshana-pro/licence
  systemctl daemon-reload
  echo "service installed: activate the licence as the kshana user, then systemctl enable --now kshana-pro"
fi
INSTALL
chmod 0755 "$OUT/install.sh"
{
  echo "Kshana Pro $VERSION offline bundle"
  echo "Built from commit $(git rev-parse --short HEAD 2>/dev/null || echo unknown)."
  echo "Check it: sha256sum -c SHA256SUMS (macOS: shasum -a 256 -c SHA256SUMS)."
  if [ -n "$SIGN_KEY" ]; then
    echo "Signed: kshana-pro dossier verify <this directory> --trust <the publisher's bundle public key>"
  else
    echo "UNSIGNED: this bundle carries no signature (no bundle signing key was given)."
  fi
  echo "Install: ./install.sh --prefix <dir> --licence <token file> [--service]"
} > "$OUT/BUNDLE.txt"
(cd "$OUT" && find . -type f ! -name SHA256SUMS ! -name signature.json | LC_ALL=C sort | sed 's#^\./##' \
  | while read -r f; do
      if command -v sha256sum >/dev/null; then sha256sum "$f"; else shasum -a 256 "$f"; fi
    done > SHA256SUMS)
if [ -n "$SIGN_KEY" ]; then
  # Signing is the Pro signed-dossier feature: the publisher's licence comes from
  # $KSHANA_PRO_VENDOR_LICENSE (a release build), or a debug build's --dev for tests.
  if [ -n "${KSHANA_PRO_VENDOR_LICENSE:-}" ]; then
    "$BIN" dossier sign "$OUT" --signer "${SIGNER:-Ashforde OÜ}" --key-file "$SIGN_KEY" --license "$KSHANA_PRO_VENDOR_LICENSE"
  else
    "$BIN" dossier sign "$OUT" --signer "${SIGNER:-Ashforde OÜ}" --key-file "$SIGN_KEY" --dev
  fi >/dev/null
fi
echo "$OUT"
