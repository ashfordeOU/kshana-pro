Receiver resilience test report (kshana-pro/receiver-scorecard/v2), campaign E3B-HELDOUT-2026-11.

MODELLED verdicts of the open receiver-trust engine and of this scorecard, from MEASURED receiver logs (fix, flags, C/N0), scored under bars stated before scoring. Test evidence for the cases run, under the conditions stated; not a statement about cases not run, and not a certification.

receiver-scorecard.json   the scorecard (frozen format kshana-pro-receiver-scorecard/1)
report.html      the readable test report
campaign.toml    the campaign file as run (its SHA-256 is in the scorecard)
cases.csv        one row per event
mapping/         the level and topic templates the mapping used
signature.json   the signer's Ed25519 signature over every file

Verify offline, no licence needed: kshana-pro dossier verify <this directory> --trust <signer key>
Log files are not included; their SHA-256 are in the scorecard.
