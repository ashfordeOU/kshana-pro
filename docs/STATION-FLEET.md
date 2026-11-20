# Station fleet management and calibration register

Research 2026-11-20 gaps, backlog B2604 (partly built before: the service's multi-station
registry, `src/serve/fleet.rs`). This adds module `src/station_fleet.rs` and command
`kshana-pro fleet` (`src/cli/fleet_cmd.rs`) under the same feature `pro-station-fleet` (Pro);
`fleet config-verify` is free. Tests `tests/fleet_records.rs` (SYNTHETIC records).

* **Configuration history** (`fleet config-add`, `config-verify`): one JSON line per change
  (receiver, antenna, cable, firmware, position, clock, site, other) with
  `hash = SHA-256(prev ‖ "\n" ‖ body)`, the chain rule of the calibration register. An edited,
  removed, inserted or re-ordered line breaks the chain, and no change is appended to a history
  that does not verify. Tamper-evident, not tamper-proof: keep the head hash elsewhere.
* **Calibration register** (`fleet cal-add`, `ageing`): the existing register
  `kshana_eloran::calreg` per asset serial and quantity; the ageing curve is its least-squares
  drift per year with standard error, the predicted value at a date, the first date the
  prediction leaves a stated tolerance, and the due date.
* **Site log** (`fleet site-log`): text in the section layout of an IGS site log (form, site
  identification, location, receiver, antenna, cable, frequency standard, firmware) generated
  from the configuration history, with installed and removed dates. Field names are to confirm
  against the current IGS template.
* **Monthly clock-quality statement** (`fleet statement`): from `timestamp,offset_s` against
  the stated reference, for one calendar month: samples, coverage, mean, RMS and peak-to-peak
  offset, MTIE, TDEV and overlapping ADEV (open engine) at stated windows; written and signed
  (Ed25519), verifiable offline with `dossier verify`. A statement of measurements, not a
  certificate.
* **Host dashboard** (`fleet dashboard`): one static HTML page of the stations' latest
  configuration, chain status and calibration ageing.

Feeder licences and receipts (PR #75) are not touched.
