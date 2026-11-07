#!/usr/bin/env python3
# Verify a signed Kshana dossier (signature.json) without Kshana: every file's SHA-256
# and the Ed25519 signature over the signed statement. Needs the `cryptography` package.
#
#   python3 verify-dossier-signature.py <dossier-dir> --trust <public key hex>
#
# Exit 0 only when the signature holds, every file matches, no file is added, and the
# signing key is the one given with --trust (obtained from the signer by another route).
import hashlib
import json
import os
import sys

from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PublicKey


def files_under(root):
    out = {}
    for d, dirs, names in os.walk(root, followlinks=False):
        for n in names:
            p = os.path.join(d, n)
            if os.path.islink(p):
                raise SystemExit(f"symbolic link: {p}")
            rel = os.path.relpath(p, root).replace(os.sep, "/")
            if rel == "signature.json":
                continue
            with open(p, "rb") as f:
                out[rel] = hashlib.sha256(f.read()).hexdigest()
    return out


def main(argv):
    if len(argv) < 2:
        print(__doc__ or "usage: verify-dossier-signature.py <dir> --trust <hex>")
        return 2
    root = argv[1]
    trust = argv[argv.index("--trust") + 1] if "--trust" in argv else None
    with open(os.path.join(root, "signature.json"), encoding="utf-8") as f:
        doc = json.load(f)
    sig = bytes.fromhex(doc.pop("signature"))
    order = ["format", "signer", "utc", "public_key", "key_id", "files"]
    statement = {k: doc[k] for k in order}
    statement["files"] = [{"path": x["path"], "sha256": x["sha256"]} for x in doc["files"]]
    msg = json.dumps(statement, separators=(",", ":"), ensure_ascii=False).encode("utf-8")
    problems = []
    try:
        Ed25519PublicKey.from_public_bytes(bytes.fromhex(doc["public_key"])).verify(sig, msg)
    except Exception:
        problems.append("signature does not verify")
    if trust is None:
        problems.append("no --trust key given")
    elif trust.lower() != doc["public_key"].lower():
        problems.append("signed by a key that is not the trusted key")
    now = files_under(root)
    for x in statement["files"]:
        if x["path"] not in now:
            problems.append(f"{x['path']}: missing")
        elif now[x["path"]] != x["sha256"]:
            problems.append(f"{x['path']}: changed")
    signed = {x["path"] for x in statement["files"]}
    problems += [f"{p}: not covered by the signature" for p in now if p not in signed]
    print(f"signer: {doc['signer']}  signed at: {doc['utc']}  key id: {doc['key_id']}")
    for p in problems:
        print("problem:", p)
    print("VERIFIED" if not problems else "NOT VERIFIED")
    return 0 if not problems else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv))
