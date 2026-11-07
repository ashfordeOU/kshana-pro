#!/usr/bin/env python3
# A CycloneDX 1.5 software bill of materials for the kshana-pro binary, from the locked
# dependency graph (`cargo metadata --locked`): every crate the build resolves, with its
# version, licence and package URL. Reads the metadata JSON on stdin, writes the SBOM.
#
#   cargo metadata --format-version 1 --locked | python3 scripts/sbom.py > sbom.cdx.json
import json
import sys


def main():
    meta = json.load(sys.stdin)
    root_id = meta.get("resolve", {}).get("root")
    pkgs = {p["id"]: p for p in meta["packages"]}
    # Only what the build resolves (the resolve graph), not every package in the index.
    reachable = [n["id"] for n in meta.get("resolve", {}).get("nodes", [])] or list(pkgs)
    comps = []
    for pid in sorted(reachable, key=lambda i: (pkgs[i]["name"], pkgs[i]["version"])):
        p = pkgs[pid]
        if pid == root_id:
            continue
        c = {
            "type": "library",
            "bom-ref": f"pkg:cargo/{p['name']}@{p['version']}",
            "name": p["name"],
            "version": p["version"],
            "purl": f"pkg:cargo/{p['name']}@{p['version']}",
        }
        if p.get("license"):
            c["licenses"] = [{"expression": p["license"]}]
        comps.append(c)
    root = pkgs.get(root_id, {"name": "kshana-pro", "version": "0"})
    sbom = {
        "bomFormat": "CycloneDX",
        "specVersion": "1.5",
        "version": 1,
        "metadata": {
            "component": {
                "type": "application",
                "bom-ref": f"pkg:cargo/{root['name']}@{root['version']}",
                "name": root["name"],
                "version": root["version"],
                "supplier": {"name": "Ashforde OÜ"},
            }
        },
        "components": comps,
    }
    json.dump(sbom, sys.stdout, indent=2, sort_keys=True)
    sys.stdout.write("\n")


if __name__ == "__main__":
    main()
