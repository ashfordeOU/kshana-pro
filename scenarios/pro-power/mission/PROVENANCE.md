# Provenance of the worked mission programme

This directory is the worked example of `kshana-pro mission build` (see
`docs/MISSION-DOSSIER.md`). `tests/mission_dossier.rs` checks the SHA-256 values below
against the files, builds the dossier from them, and compares every expected value with
a direct run of the open engine.

## Scenario files copied from the open engine

Copied unchanged from `scenarios/` of the open Kshana repository at commit
`da55ae630b16dc93dc81119627b2445396782e3d` (the 75-kind engine this branch builds
against; it reports version 0.28.0). Each is one of the engine's own public examples and
uses public data only.

| File | Kind | SHA-256 |
|---|---|---|
| `leo-pnt-end-to-end.toml` | `leo-pnt-chain` | `4a0286b53161bff2257bb693b77f73c20632df077f0eb84a459d5a32ef21ebce` |
| `lunar-relay-constellation.toml` | `constellation-design` | `d626dcb56d0749b58bae19d446a5d869cecc3126ce499d0862c2be2a2665dff3` |
| `l-band-waterfall-jamming.toml` | `spectrum` | `204830758faac62c51e3b396ff425442fee7d781378790c9c4ca7173f6c4973d` |
| `campaign-jam-spoof-holdover-integrity.toml` | `campaign` | `cec1e10f8eb478bae58acebe9c2f06cfccf6572f6f6895dc633fb409b8d8ee5f` |
| `mars-orbit-pnt.toml` | `body-pnt` | `813ff12e5f8119636bd43bf164c08f061b97b6bb5e5da9362bec98f1e1d2f8e8` |

None of them uses the engine's optional system preset for a named low Earth orbit
demonstration mission, and no file here names it; a test scans for that.

## Hand-written inputs

Written for this example. The limits in the requirements and the second design in the
trade study are illustrative: they are no agency's and no operator's.

| File | What it is |
|---|---|
| `programme.toml` | the programme: five scenarios, their headline figures and charts, the trade study, the sign-off rows |
| `requirements.csv` | twelve requirements with their verification methods and acceptance criteria |
| `trade-lunar-relay.toml` | a `pro-trade-study` scenario with two variants. `south-biased-14` has the keys and values of `lunar-relay-constellation.toml`; `north-and-south-22` adds one shell of eight elliptical orbits with the argument of perilune at 270 degrees |

## Outputs

Nothing generated is kept here. `kshana-pro mission build programme.toml --out <dir>`
writes every output, and the tests build into the target directory.
