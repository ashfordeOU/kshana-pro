# Operating envelope of Pro outputs

Research 2026-11-20 gaps, backlog B2508. Module `src/envelope.rs`, command `kshana-pro envelope`
(`src/cli/envelope_cmd.rs`), schema `docs/schemas/operating-envelope.schema.json`, tests
`tests/operating_envelope.rs`.

Every Pro output states where it may be relied on:

* **valid inputs**: the data and declarations the method was built for;
* **ranges**: the quantities and ranges it was built and exercised over;
* **exclusions**: what it does not cover. Every envelope carries the common exclusions: no
  emitter location, antenna control, navigation sensor fusion or transmission; not an approval,
  licence, type approval or regulatory finding; MODELLED unless the output names a validation
  data set; inputs taken as supplied.

## How it is attached

`signed_dossier::sign` calls `envelope::attach` first: when a top-level JSON file of the
directory carries a Kshana `format` tag, `operating-envelope.json` is written and then signed with
everything else, so changing it fails verification. A directory without a Kshana format (a
user's own files signed with `dossier sign`) is signed as it is.

The envelope is chosen by the output's format tag through nine families (interference and
forensics; maritime; aviation and airspace; land transport, agriculture and cellular; timing;
insurance and finance; eLoran and terrestrial; receivers, space and test; evidence and
governance). For a tag, the earliest token of its name that a family claims decides.

`kshana-pro envelope <format-tag|evidence-dir>` prints the envelope (free).

## Checks

`tests/operating_envelope.rs` scans the source for every `kshana…/N` format tag (over a hundred)
and fails if one has no family; every envelope is checked against the schema's required and
allowed keys; a signed directory carries its envelope under the signature and a user directory
does not get one.

Not covered: outputs printed to the terminal without a signed directory carry no envelope
file; `kshana-pro envelope <format>` gives theirs.
