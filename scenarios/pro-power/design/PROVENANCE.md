# Provenance of the design-study scenarios

Every study in this folder runs the open Kshana engine on public, generic inputs. Nothing
here comes from a customer, a partner or a workshop, and no study uses the open engine's
optional low-Earth-orbit preset files.

## Files copied from the open engine

Open engine repository, branch `feat/pro-engine-base`, commit
`39afbd712df32c0252f82763358f19c91f725d0f`
(`git -C ../kshana-wt-pro-engine rev-parse HEAD`). The copies in `engine/` are byte for
byte the engine's files; `tests/design_optimise.rs` checks each SHA-256 below and, when
the engine checkout is present, compares the bytes.

| File in `engine/` | SHA-256 | Path in the open engine |
|---|---|---|
| `link-budget.toml` | `1c75d05b9fc5e9374a896c944d6495cf972cd373fd6922ac1acbef0d3588df7f` | `scenarios/link-budget.toml` |
| `lunar-relay-constellation.toml` | `d626dcb56d0749b58bae19d446a5d869cecc3126ce499d0862c2be2a2665dff3` | `scenarios/lunar-relay-constellation.toml` |

## How each study uses them

| Study | Base scenario | Reference design |
|---|---|---|
| `lunar-navigation-global.toml` | Written for this study: one Walker shell around the Moon under the run conditions of `engine/lunar-relay-constellation.toml` (one day, 1 800 s steps, 10 deg mask, position dilution of precision at or below 6, 10 deg grid, one shared receiver clock). | `engine/lunar-relay-constellation.toml`, embedded byte for byte. |
| `lunar-navigation-south-polar.toml` | The same base scenario with the grid restricted to latitude -90 to -70 deg at 5 deg spacing. | `engine/lunar-relay-constellation.toml` with three lines changed so it is scored over the same region: `grid_step_deg = 10.0` becomes `grid_step_deg = 5.0`, and `lat_min_deg = -90.0` and `lat_max_deg = -70.0` are added after it. Nothing else differs. |
| `leo-positioning-earth.toml` | Written for this study: one generic Walker shell in low Earth orbit. It is not any operator's constellation. | None. |
| `mars-navigation-shell.toml` | Written for this study: one Walker shell around Mars. | None. |
| `link-budget-trade.toml` | `engine/link-budget.toml` with its comments removed (the same keys and values). | The same link budget (55 dBW, 1 Mbit/s), given as variable values. |

The published figures a test compares against (the Global Positioning System Standard
Positioning Service Performance Standard, 5th edition, April 2020, Appendix B: global
horizontal dilution of precision median 0.94, 90 % 1.16, 95 % 1.25) are quoted in
`tests/design_optimise.rs` with the engine run that reproduces them.
