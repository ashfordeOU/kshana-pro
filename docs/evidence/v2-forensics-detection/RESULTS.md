# V2 results: incident forensics detection bars (roadmap 0.42 E1)

Scored **once** on 2026-11-15 (12:40 UTC) against `2026-11-15_PREREGISTRATION.md` (committed
alone in e5a3369b, before the labelled set or any scoring code existed). Method
`kshana-pro/incident-forensics/v1`, unchanged. Scorer: `tests/forensics_v2.rs`; every case with
its parameters and outcome is in `results.json` (the test re-runs the set and must reproduce
that file exactly). Data class: **SYNTHETIC** throughout.

## Outcome

**V2 is not met.** Ten of the eleven bars pass on the synthetic part; **B3 (cross-attribution)
misses**; and the public-event part (P) was empty at registration and is not scored. Under the
pre-registered claim rule **E1 stays unclaimed**: no E1 detection claim is made anywhere, and
every E1 report keeps its "V2 not met" limit.

| Bar | Rule | Result | |
|---|---|---|---|
| B1 | injected category indicated in ≥ 22 of 24 positives | 24 / 24 | met |
| B1j | jamming indicated in ≥ 10 of 12 jamming cases | 12 / 12 | met |
| B1s | spoofing indicated in ≥ 10 of 12 spoofing cases | 12 / 12 | met |
| B2 | jamming or spoofing indicated in ≤ 1 of 24 negatives (clean, fjord, container terminal) | 0 / 24 | met |
| **B3** | the category not injected indicated in ≤ 2 of 24 positives | **4 / 24** | **missed** |
| B4 | onset within [start, start + ramp + 60 s] in ≥ 90 % of detected positives | 24 / 24 | met |
| B5 | no high-confidence finding in a receiver-only case | 0 / 48 | met |
| B6 | AIS: spoofing in ≥ 9 of 10 positives; in ≤ 1 of 10 negatives | 10 / 10; 0 / 10 | met |
| B7 | ADS-B: jamming in ≥ 9 of 10 positives; in ≤ 1 of 10 negatives | 10 / 10; 0 / 10 | met |
| P | public-event part | not scored (no data) | — |

## The miss, plainly

All four cross-attributions are **jamming cases also indicated as spoofing** (S04, S08, S32,
S40). Their injected C/N0 drops were 24.9, 27.9, 25.6 and 25.5 dB; every jamming case with a
drop of 29.5 dB or more, and the one with 20.6 dB, was attributed to jamming alone. The
spoofing-like family that fired is the receiver's response while partly jammed, which the v1
attribution rule counts as spoofing-like. The jamming itself was found in all four (B1), with a
correct onset (B4); the report's verdict for them reads "jamming and spoofing indicated", which
over-states spoofing.

Not done in response (the bar is not loosened and the method is not changed in this run): a
successor method that separates partial-jamming receiver behaviour from spoofing would get a new
method id and a new pre-registration with a fresh labelled set.

## What this does and does not support

On a pre-registered synthetic labelled set the method found every injected jamming and
spoofing event, raised no false indication on clean or obstructed runs, and flagged every AIS
jump, AIS circle and ADS-B integrity drop with no false indications; it over-attributed spoofing
in 4 of 12 moderate-jamming cases. It says nothing about any real receiver, vessel, aircraft or
event: no public-event data were scored.
