#!/usr/bin/env bash
# PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
#
# Regenerates the long-term signature test fixtures (tests/longterm_signature.rs) with
# OpenSSL 3. Two self-made test time-stamp authorities, no real one:
#   A: P-256 root -> P-256 TSA whose certificate lasts 40 days from generation
#      (so a record holding only stamp 0 expires soon after);
#   B: RSA-2048 root -> RSA-2048 TSA valid for 20 years (the renewal authority).
# Stamp 0 (TSA A) covers dossier/signature.json; stamp 1 (TSA B) covers the renewal text
# of src/longterm.rs (renewal_preimage), written here independently in shell so the test
# also cross-checks the format. Private keys stay in a temporary directory and are deleted.
# Running it again gives different keys, certificates and times; the test reads every time
# from the files, so nothing else needs to change.
#
# The dossier itself is signed by the test with the fixed test seed K1 (see the test), and
# committed; this script only stamps it:
#   kshana-pro dossier sign tests/fixtures/longterm/dossier --signer "Kshana Test Signer" \
#     --key-file <K1 seed file> --utc 2026-11-20T08:00:00Z --dev
#
# usage: tests/fixtures/longterm/gen.sh
set -euo pipefail
OUT="$(cd "$(dirname "$0")" && pwd)"
K="$(mktemp -d)"
trap 'rm -rf "$K"' EXIT
mkdir -p "$OUT/certs" "$OUT/tokens"
cd "$K"

cat > ext.cnf <<'X'
[req]
distinguished_name = dn
[dn]
[v3ca]
basicConstraints = critical,CA:TRUE
keyUsage = critical,keyCertSign,cRLSign
subjectKeyIdentifier = hash
[tsa]
basicConstraints = critical,CA:FALSE
keyUsage = critical,digitalSignature,nonRepudiation
extendedKeyUsage = critical,timeStamping
subjectKeyIdentifier = hash
authorityKeyIdentifier = keyid
X

ca_dates() { # issuer_cert issuer_key csr out start end
  rm -rf cadb; mkdir cadb; : > cadb/index.txt; openssl rand -hex 8 > cadb/serial
  cat > ca.cnf <<C
[ca]
default_ca = d
[d]
database = cadb/index.txt
serial = cadb/serial
new_certs_dir = cadb
default_md = sha256
policy = p
unique_subject = no
copy_extensions = none
[p]
commonName = supplied
C
  openssl ca -batch -notext -config ca.cnf -cert "$1" -keyfile "$2" -in "$3" -out "$4" \
    -extfile ext.cnf -extensions tsa -startdate "$5" -enddate "$6" 2>/dev/null
}
csr() { openssl req -new -key "$1" -subj "/CN=$2" -out "$3"; }
START="$(date -u -d '-1 day' +%Y%m%d%H%M%SZ)"
END_A="$(date -u -d '+40 days' +%Y%m%d%H%M%SZ)"
END_B="$(date -u -d '+20 years' +%Y%m%d%H%M%SZ)"

# Authority A (P-256).
openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:P-256 -out root-a.key
openssl req -x509 -new -key root-a.key -subj "/CN=Kshana Test Long-Term Root A" -days 9000 \
  -sha256 -extensions v3ca -config ext.cnf -out root-a.pem 2>/dev/null
openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:P-256 -out tsa-a.key
csr tsa-a.key "Kshana Test Long-Term TSA A" tsa-a.csr
ca_dates root-a.pem root-a.key tsa-a.csr tsa-a.pem "$START" "$END_A"

# Authority B (RSA).
openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:2048 -out root-b.key 2>/dev/null
openssl req -x509 -new -key root-b.key -subj "/CN=Kshana Test Long-Term Root B" -days 9000 \
  -sha256 -extensions v3ca -config ext.cnf -out root-b.pem 2>/dev/null
openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:2048 -out tsa-b.key 2>/dev/null
csr tsa-b.key "Kshana Test Long-Term TSA B" tsa-b.csr
ca_dates root-b.pem root-b.key tsa-b.csr tsa-b.pem "$START" "$END_B"

tscnf() { # signer_cert key
  cat > ts.cnf <<T
[tsa_config1]
serial = $K/tsaserial
crypto_device = builtin
signer_cert = $1
signer_key = $2
signer_digest = sha256
default_policy = 1.2.3.4.1
digests = sha256
accuracy = secs:1
ordering = no
tsa_name = no
ess_cert_id_chain = no
ess_cert_id_alg = sha256
T
  [ -f tsaserial ] || echo 01 > tsaserial
}
stamp() { # data out signer key
  tscnf "$K/$3" "$K/$4"
  openssl ts -query -data "$1" -sha256 -cert -out q.tsq
  openssl ts -reply -queryfile q.tsq -config ts.cnf -section tsa_config1 -out "$2" 2>/dev/null
}

sha() { sha256sum "$1" | cut -d' ' -f1; }

# Stamp 0 by A over signature.json.
stamp "$OUT/dossier/signature.json" "$OUT/tokens/stamp0-a.tsr" tsa-a.pem tsa-a.key
# Stamp 1 by B over the renewal text (no chain certificates were stored with stamp 0).
printf 'kshana-longterm/1 renewal\nseq=1\nhash_alg=sha256\nprev_imprint=%s\nprev_token_sha256=%s\nprev_chain_sha256=\n' \
  "$(sha "$OUT/dossier/signature.json")" "$(sha "$OUT/tokens/stamp0-a.tsr")" > renewal1.txt
stamp renewal1.txt "$OUT/tokens/stamp1-b.tsr" tsa-b.pem tsa-b.key
# A token by B over other data (a wrong imprint for stamp 0).
printf 'other data\n' > other.txt
stamp other.txt "$OUT/tokens/other-b.tsr" tsa-b.pem tsa-b.key

cp root-a.pem "$OUT/certs/root-a.crt"; cp root-b.pem "$OUT/certs/root-b.crt"   # .crt: *.pem is git-ignored
(cd "$OUT" && sha256sum certs/* tokens/* dossier/* > SHA256SUMS)
echo "fixtures written to $OUT"
