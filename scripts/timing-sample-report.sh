#!/usr/bin/env bash
# Rebuild the sample timing audit report (roadmap 0.42 T3) from the demo fleet, signed with the
# published DEMO key (seed = SHA-256 of the phrase below; not a vendor key, never used for a
# customer report). Needs a debug build: `cargo build`.
#
#   scripts/timing-sample-report.sh [path/to/kshana-pro]
set -euo pipefail
here="$(cd "$(dirname "$0")/.." && pwd)"
bin="${1:-${CARGO_TARGET_DIR:-$here/target}/debug/kshana-pro}"
out="$here/report-templates/samples/timing-audit-rts25"
work="$(mktemp -d)"; trap 'rm -rf "$work"' EXIT
seed="$(printf '%s' 'kshana-pro timing audit sample: demo key, not a vendor key' | sha256sum | cut -c1-64)"
printf 'private_key=%s\n' "$seed" > "$work/demo.key"
fleet="$here/examples/timing-fleet/fleet.toml"
"$bin" timing assess "$fleet" --evidence "$work/ev" --utc 2026-11-14T09:00:00Z --dev --json /dev/null
"$bin" timing assess "$fleet" --evidence "$work/ev" --utc 2026-11-15T09:00:00Z --dev --json /dev/null
rm -rf "$out"
"$bin" timing audit "$fleet" --profile rts25-traceability --evidence "$work/ev" \
  --operator "$here/examples/timing-fleet/operator.example.toml" \
  --signer "Kshana Pro sample (demo key)" --key-file "$work/demo.key" \
  --utc 2026-11-15T09:30:00Z --out "$out" --dev 2>&1 | sed -n 's/.*public key \([0-9a-f]*\)\]/\1/p' > "$out.demo-public-key.txt"
echo "sample written to $out; demo public key $(cat "$out.demo-public-key.txt")"
