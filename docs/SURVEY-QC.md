# Marine survey and construction positioning QC under interference

Research 2026-11-20 gaps, backlog B0807. Module `src/survey_qc.rs`, command
`kshana-pro survey-qc` (`src/cli/survey_cmd.rs`), tests `tests/survey_qc.rs`.

`survey-qc run <request.toml> --out <dir> --signer <name> --key-file <file>` (`pro-survey-qc`, Pro)
reads the survey positioning log (`timestamp,line_id,lat,lon` plus any of
`fix_quality,num_sats,cn0_db,agc_db,diff_age_s,sigma_h_m`) and flags each epoch where:

| indicator | rule (thresholds in the request) |
|---|---|
| `cn0-drop` | mean C/N0 below the line's calibration-window median by `cn0_drop_db` (6 dB) |
| `agc-drop` | AGC below its baseline by `agc_drop_db` (3 dB) |
| `few-satellites` | fewer than `min_sats` (6) |
| `fix-degraded` | GGA fix quality below `required_fix` (4 RTK fixed > 5 float > 2 differential > 1 autonomous) |
| `correction-age` | correction age above `max_diff_age_s` (10 s) |
| `sigma-over-tolerance` | reported horizontal sigma above the horizontal tolerance |
| `cross-track` | distance from the line's straight track above the horizontal tolerance |

Flagged epochs within `merge_s` form intervals. Lines are **PASS** (no interval), **DEGRADED**
(flagged share up to `reject_share`, 5 %) or **RE-RUN**. The annex lists every interval with the
indicators that fired; the **site risk plan** ranks UTC hours of day and 0.01-degree cells by
flagged time and names windows and places where indicators recurred on two or more lines.

The horizontal tolerance and its source are the user's (`horizontal_tolerance_m`,
`tolerance_source`, e.g. the survey specification or the IHO S-44 order the contract names);
no tolerance table is bundled. The annex is signed and carries its operating envelope where
that feature is merged. Engineering QC for the survey record; acceptance is the client's and
the surveyor's. MODELLED: exercised on a synthetic survey with injected intervals
(`survey-qc synth <dir>`). Nothing here locates an emitter.
