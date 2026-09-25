# Changelog

All notable changes to the private Kshana Pro overlay. Each lane keeps its entry in its
own bullet so parallel work merges cleanly.

## [Unreleased]

- **Study dossier (`kshana-pro study --dossier`), and the README pack count fixed.**
  One scenario now becomes a reproducible evidence dossier, written as JSON
  (`dossier.json`), a self-contained HTML page (`dossier.html`) and the exact scenario
  text run (`scenario.run.toml`). The page is laid out like a European Space Agency
  (ESA) technical note: identification, scope, applicable and reference documents,
  method, results, limitations. It records the open engine and Pro versions, the SHA-256
  of the input, the engine's scenario hash, and a SHA-256 result hash over the whole
  dossier; the same scenario gives the same hash and byte-identical files (an optional
  `--utc` issue stamp is kept outside the hash). Every figure carries the VALIDATED or
  MODELLED tier the open verification matrix grants it, read through
  `kshana::fom_label::tier_for`; a figure with no owning row is MODELLED. A
  one-at-a-time sensitivity sweep re-runs the scenario with each key parameter changed
  alone and reports each figure's change and elasticity. For `clock` scenarios the
  dossier adds the Pro holdover band and its maximum time interval error (MTIE) check
  against the scenario's own time-error budget and the published clock-class limits
  already in `twin::holdover_uq`: the enhanced primary reference time clock (ePRTC) 30 ns
  and primary reference time clock (PRTC-A) 100 ns class limits, and the conventional
  1.5 µs / 24 h holdover budget (an industry convention, not an ITU-T clock-class limit). No new mask values
  were added: the MTIE check already existed. The run's behaviour is read against the
  four levels of the US Department of Homeland Security (DHS) Resilient PNT Conformance
  Framework (RPCF) v2.0, Table 1, with each of its eight minimum requirements recorded
  as not assessed where a simulation cannot verify it, and Prevent / Respond / Recover
  readings are offered for later mapping to IEEE P1952, whose draft is not public. All
  of it is labelled "tentative, simulation-derived, not a certified level". The Goal
  Structuring Notation (GSN) argument, Verification Control Document (VCD) and W3C-PROV
  record are built with the existing L3 types; a `pro-trade-study` scenario also gets
  the trade's own gate, GSN, VCD and provenance appended unchanged. The command needs
  the Pro tier (`pro-study-dossier`) and fails closed without a licence. Worked example:
  `scenarios/dossier-holdover-csac.toml`. The README said seven Pro packs; the registry
  holds sixteen, and the README and `docs/OVERLAY-ARCHITECTURE.md` now say so. The new
  `tests/readme_counts.rs` fails if either count, or the README's integration-test
  count, stops matching the registry or `tests/`. Tests: `tests/study_dossier.rs`
  (determinism, tier split, the binary end to end and its licence gate) and unit tests
  in `src/study_dossier.rs`.
