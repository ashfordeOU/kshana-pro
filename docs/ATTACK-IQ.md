# Synthetic attack IQ files: guardrails (roadmap 0.42 F7)

PRIVATE. Kshana Pro only; never in the open crate.

**Decision.** Owner gate K6 (ROADMAP-0.42 §1e, 15 November 2026): build synthetic attack IQ
in 0.42 with guardrails: Pro only, licence-gated, files only (never drives a radio),
watermarked and logged per licensee, lab or test-range use attested, vetted buyers. Counsel's
export approval of 7 November stands as stated; this page does not change or restate it, and
it is not a legal opinion. The export-control record is in `commercial/export-control/`.

**What it is.** `kshana-pro attack-iq generate` (`src/attack_iq.rs`,
`src/cli/attack_iq_cmd.rs`) writes a complex-baseband recording (16-bit interleaved I/Q,
little-endian) of a generic interference class over thermal noise, for testing receivers and
detectors: a continuous-wave tone, a linear chirp, band-limited noise, a pulsed tone, or a
delayed replay of a synthetic spread signal (the meaconing class). The spreading code is
pseudo-random from the seed; no real GNSS ranging code or navigation message is produced.
Each file comes with `<file>.attack-iq.json`: the notice, parameters, licensee ID (a salted
hash, not the name), token ID, attestation hash, environment, watermark and the file's SHA-256.

## Guardrails and where each is enforced

| # | Guardrail | Enforced by | Test (`tests/attack_iq.rs`) |
|---|---|---|---|
| 1 | Pro only, never in the open crate | The module exists only in this private crate; the leak gate and copy check keep Pro names out of the open repository | (repository structure) |
| 2 | Licence-gated | A verified, unexpired, non-trial token from `--license` (never the stored licence, never `--dev`) that allows feature `pro-attack-iq`. The feature is at Enterprise tier in `license::explicit_tier`, so a Pro licence gets it only when the vendor names it (`license issue --feature pro-attack-iq`). Listed at `pro` in the signed tier manifest | `only_a_vetted_licence_with_the_feature_generates`, `the_cli_takes_the_licence_from_the_command_only_and_ignores_dev` |
| 3 | Vetted buyers | Claim `vetted_buyer: true`, set only by the vendor (`license issue --vetted-buyer`) after vetting the buyer (identity, end use, sanctions screening, as counsel advises). Without it, refused | same |
| 4 | Lab or test-range attestation before generation | `Attestation::check`: format, the licensee named on the licence, facility, environment (`shielded-lab`, `conducted`, `authorised-test-range`), authorisation reference, purpose, validity window covering today, the person who signs it off, and the use statement word for word. `attack-iq attestation-template --licensee <name>` prints one | `the_attestation_must_hold_today_for_this_licensee` |
| 5 | Files only, never a radio | `check_output_path`: a new regular file in an existing folder; standard output, `/dev`, `/proc`, `/sys`, device namespaces, FIFOs, sockets, existing files, URLs and SDR-driver-like targets refused; written through a temporary file and a rename. The module contains no radio, device, network or process code | `files_only_never_a_device_pipe_or_radio`, `the_module_has_no_radio_network_or_process_code` |
| 6 | Watermarked per licensee | The licence's mark in the sample least significant bits (changes each sample by at most one count) and in the sidecar. The vendor traces a file from its samples alone with `attack-iq trace` and the issuance register | `every_file_is_watermarked_described_and_logged` |
| 7 | Logged per licensee | One hash-chained line per file in `<log dir>/attack-iq-generations.log` (default `$KSHANA_PRO_HOME/attack-iq`): time, licensee ID, token ID, attestation hash, environment, file hash, parameters, mark. A generation that cannot be logged is not made | same |
| 8 | Bounded | At most 60 s and 100 million samples per file; sample rate 1 kHz to 100 MHz; interference-to-noise ratio -30 to 60 dB | same |

## Vendor steps before a sale

1. Confirm the buyer is a vetted buyer (owner and counsel). Record the decision.
2. Issue the licence with `--feature pro-attack-iq --vetted-buyer` (and the buyer's other
   claims). The trial and the hosted trial never carry the feature.
3. Give the buyer the attestation template; keep their signed-off copy with the order.

## Honest limits

* The file mark can be removed by anyone who rewrites the least significant bits; it traces
  honest leaks, it does not lock a file. The real controls are vetting, the licence and the
  attestation.
* The log lives on the buyer's machine; it is evidence for an audit under the licence terms,
  not a report to the vendor.
* The attestation is the buyer's declaration; Kshana checks its form and dates, not the
  facility.
* Files must never be committed to a repository: `tests/lab_campaign_no_client_data.rs`
  forbids raw IQ in this tree, and the tests write only to temporary folders.
