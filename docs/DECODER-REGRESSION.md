# Correction-decoder interface-change regression

Research 2026-11-20 gaps, backlog B1604. Module `src/interface_drift.rs`, command
`kshana-pro iface` (`src/cli/iface_cmd.rs`), feature `pro-decoder-regression` (Pro; `template`
free), tests `tests/decoder_regression.rs`.

Correction services change their interfaces (a constellation added to a mask, a new message
subtype, a new transmitting satellite, as with QZSS IS-QZSS-L6 streams such as MADOCA-PPP and
CLAS and the move to seven QZSS satellites). The harness is decoder-neutral:

* **Interface profile** (`kshana-pro-interface-profile/1`, TOML): stream, version, source
  (specification, revision, section), message types, subtypes, GNSS identifiers allowed in masks,
  transmitting PRNs, and allowed values of header fields. Kshana ships only `iface template`,
  whose values are placeholders: a profile without a named source is refused.
* **Drift check** (`iface check <profile> <records.csv>`): decoded header records from any
  decoder (`timestamp,stream,message_type,subtype,gnss_ids,prn,fields`, GNSS ids separated by
  `|`, fields as `k=v;k=v`) against the profile; every unknown element raises an alarm with its
  first and last time and occurrences; the command exits non-zero on drift.
* **Regression vectors** (`iface vectors <profile>`): records to accept (every known message
  type, subtype and PRN; all GNSS ids together) and mutations to flag (unknown message type,
  subtype, GNSS id added to a mask, PRN, field value), self-checked against the profile.
  `iface score <vectors.json> <output.csv>` scores a decoder's `id,result` answers and lists
  unknown elements it accepted silently.

The tests use SYNTHETIC profiles shaped like a constellation addition in a PPP stream's mask and
a new transmitting satellite; no value is taken from IS-QZSS-L6. MODELLED: a harness; it decodes
no signal.
