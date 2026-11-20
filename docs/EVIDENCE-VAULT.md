# Customer evidence vault (first slice)

Research 2026-11-20 gaps, backlog B2504 (first slice). Module `src/vault.rs`, command
`kshana-pro vault` (`src/cli/vault_cmd.rs`), tests `tests/evidence_vault.rs`. Feature
`pro-evidence-vault` (Enterprise); `vault import` is free so a customer can always bring an
export back.

A vault is a directory the customer controls:

* `index.json`: every pack held: id (SHA-256 of its `signature.json`), format, family, signer,
  key id, signing and ingest times, whether the key was trusted at ingest, tags, retention date,
  legal hold, deletion;
* `packs/<id>/`: the signed evidence directories, copied as they are;
* `ledger.json`: every action (init, ingest, hold, release, delete) as a hash-chained record; a
  changed, removed or re-ordered record shows in `vault verify`.

| command | what it does |
|---|---|
| `ingest <vault> <dir> [--tag] [--retain-years n] [--trust key]` | refuses a directory whose signature does not verify; records whether the key is trusted |
| `hold` / `release <vault> <id> --reason` | legal hold; `delete` is refused under hold or before the retention date |
| `trend <vault> [--html]` | packs per month of signing per family (first token of the format name) |
| `calendar <vault> <obligations.toml>` | per declared obligation (`monthly`, `quarterly`, `annual`; `served_by` format prefixes): next due date (month ends kept), latest serving pack, whether one was signed in the current period |
| `submission <vault> --title --packs` | a Markdown skeleton citing pack ids, formats, signers and key ids with sections to fill; no regulator's form is reproduced |
| `export <vault> <out>` / `import <out> <new>` | full open export (plain files and JSON with an `export-manifest.json` of SHA-256 digests); import checks every digest and the ledger, and round-trips byte for byte |

Not in this slice: a server, access control, multi-user audit, regulator-specific templates.
