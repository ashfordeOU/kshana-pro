#!/usr/bin/env bash
# PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
# Build an RPM package of Kshana Pro (roadmap L6), the same contents as build-deb.sh:
# the binary, the systemd unit, its settings file and the documentation. Needs rpmbuild
# (Fedora, RHEL and its rebuilds, openSUSE; on Debian and Ubuntu the `rpm` package).
#
#   scripts/build-rpm.sh <kshana-pro binary> <out-dir> [arch]
#
# arch is the RPM architecture (x86_64, aarch64); it defaults to this machine's. The
# binary is packaged as given (no rebuild, no stripping). The package is unsigned; a
# signed dnf/zypper repository is the publisher's step.
set -euo pipefail
BIN=${1:?usage: build-rpm.sh <kshana-pro binary> <out-dir> [arch]}
OUT=${2:?usage: build-rpm.sh <kshana-pro binary> <out-dir> [arch]}
ARCH=${3:-$(uname -m)}
BIN=$(cd "$(dirname "$BIN")" && pwd)/$(basename "$BIN")
mkdir -p "$OUT"
OUT=$(cd "$OUT" && pwd)
cd "$(dirname "$0")/.."
SRC=$(pwd)
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)
# RPM versions may not contain '-': a pre-release 0.41.0-rc.1 becomes 0.41.0~rc.1, which
# sorts before 0.41.0 as it should.
RPMVER=${VERSION//-/\~}
TOP=$(mktemp -d)
trap 'rm -rf "$TOP"' EXIT
mkdir -p "$TOP"/{BUILD,BUILDROOT,RPMS,SOURCES,SPECS,SRPMS}
cat > "$TOP/SPECS/kshana-pro.spec" <<SPEC
Name:           kshana-pro
Version:        $RPMVER
Release:        1
Summary:        Kshana Pro: PNT resilience studies, job service and Station fleet
License:        Proprietary
URL:            https://kshana.dev
Vendor:         Ashforde OÜ
Packager:       Ashforde OÜ <contact@ashforde.org>
Group:          Applications/Engineering
Requires(pre):  shadow-utils
%{?systemd_requires}
AutoReqProv:    yes

# The binary is prebuilt: no debuginfo subpackage, no stripping, no build-id links.
%global debug_package %{nil}
%global __os_install_post %{nil}
%define _build_id_links none

%description
The proprietary Pro edition of the Kshana engine: the command line, the on-premises
job service (systemd unit kshana-pro) and the licence tools. Needs a licence token.

%install
install -D -m 0755 "$BIN" %{buildroot}/usr/bin/kshana-pro
install -D -m 0644 "$SRC/deploy/linux/kshana-pro.service" %{buildroot}/usr/lib/systemd/system/kshana-pro.service
install -D -m 0644 "$SRC/deploy/linux/serve.conf" %{buildroot}/etc/kshana-pro/serve.conf
install -D -m 0644 "$SRC/docs/SERVICE.md" %{buildroot}/usr/share/doc/kshana-pro/SERVICE.md
install -D -m 0644 "$SRC/docs/LICENSING.md" %{buildroot}/usr/share/doc/kshana-pro/LICENSING.md
install -D -m 0644 "$SRC/LICENSE" %{buildroot}/usr/share/licenses/kshana-pro/LICENSE
install -d -m 0750 %{buildroot}/var/lib/kshana-pro %{buildroot}/var/lib/kshana-pro/data %{buildroot}/var/lib/kshana-pro/licence

%pre
getent group kshana >/dev/null || groupadd --system kshana
getent passwd kshana >/dev/null || useradd --system --gid kshana --home-dir /var/lib/kshana-pro --no-create-home --shell /sbin/nologin kshana
exit 0

%post
if command -v systemctl >/dev/null 2>&1; then systemctl daemon-reload || true; fi
if [ "\$1" -eq 1 ]; then
  echo "kshana-pro installed. Activate a licence: sudo -u kshana KSHANA_PRO_HOME=/var/lib/kshana-pro/licence kshana-pro license activate --product service --file <token.txt>"
  echo "Then: sudo systemctl enable --now kshana-pro"
fi

%preun
# On removal (not upgrade), stop the service first.
if [ "\$1" -eq 0 ] && command -v systemctl >/dev/null 2>&1; then
  systemctl --no-reload disable --now kshana-pro.service >/dev/null 2>&1 || true
fi

%postun
if command -v systemctl >/dev/null 2>&1; then systemctl daemon-reload || true; fi
# On upgrade, restart a running service on the new binary.
if [ "\$1" -ge 1 ] && command -v systemctl >/dev/null 2>&1; then
  systemctl try-restart kshana-pro.service >/dev/null 2>&1 || true
fi

%files
/usr/bin/kshana-pro
/usr/lib/systemd/system/kshana-pro.service
%dir /etc/kshana-pro
%config(noreplace) /etc/kshana-pro/serve.conf
%doc /usr/share/doc/kshana-pro/SERVICE.md
%doc /usr/share/doc/kshana-pro/LICENSING.md
%license /usr/share/licenses/kshana-pro/LICENSE
%attr(0750,kshana,kshana) %dir /var/lib/kshana-pro
%attr(0750,kshana,kshana) %dir /var/lib/kshana-pro/data
%attr(0750,kshana,kshana) %dir /var/lib/kshana-pro/licence
SPEC
rpmbuild --quiet -bb --target "$ARCH" \
  --define "_topdir $TOP" \
  --define "_rpmdir $OUT" \
  --define "_build_name_fmt %%{NAME}-%%{VERSION}-%%{RELEASE}.%%{ARCH}.rpm" \
  "$TOP/SPECS/kshana-pro.spec" >/dev/null
echo "$OUT/kshana-pro-$RPMVER-1.$ARCH.rpm"
