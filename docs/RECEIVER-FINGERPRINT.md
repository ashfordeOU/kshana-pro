# Receiver behaviour fingerprint (population-benchmark give-back)

Research 2026-11-20 gaps, backlog B2603 (Pro side). Module `src/fingerprint.rs`, command
`kshana-pro fingerprint build` (`src/cli/fingerprint_cmd.rs`), tests in
`tests/receiver_scorecard.rs` (module `fingerprint`).

Already there: the hosted population aggregate, which the client reaches as the service's
`benchmark` verdict (contributor licences only; `kshana-pro verdict benchmark`). What this adds is
the Pro side that feeds it: a signed, per-firmware fingerprint built from the customer's own
scorecards, with the customer's consent.

`fingerprint build <request.toml> --out <dir> --signer <name> --key-file <file>`
(`pro-receiver-fingerprint`, Pro):

* reads signed scorecard directories and verifies each signature against the keys in `trust`;
  a directory that fails, or holds no recognised scorecard, is refused;
* recognises the receiver scorecard (`kshana-pro-receiver-scorecard/1`: `rx.rpcf_level`,
  `rx.misleading_unwarned_epochs`, `rx.monitor_detection_rate`, `rx.receiver_flag_rate`,
  `rx.recovery_rate`) and the OSNMA scorecard (`kshana-pro-osnma-scorecard/1`: `osnma.met_share`
  and the worst result per test kind, `osnma.<kind>`);
* requires every scorecard to name the same maker, model and firmware (a fingerprint is per
  firmware), and each feature to come from one scorecard;
* gives the fingerprint a stable identifier (SHA-256 of identity and features) and lists every
  source scorecard with its SHA-256 and signer key.

**Consent** (`[consent]`: `give_back`, `scope`, `granted_by`, `granted_utc`). The only scope
accepted is `population-benchmark`. With `give_back = true` the signed directory also holds
`contribution.json` (`kshana-pro-fingerprint-contribution/1`): the firmware identity, the features,
the source formats and a salted pseudonym of the customer; never the customer's name, sites, logs
or scorecards. With `give_back = false` the fingerprint stays local and nothing to give back is
written. The aggregate is computed by the hosted service, not here.
