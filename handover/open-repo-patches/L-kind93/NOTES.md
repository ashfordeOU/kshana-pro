# 93-kind site re-port (tip-and-cue): notes for review

Two patches on open `ws/tip-and-cue-kind` 0a53cb87e:

1. `web: re-ported at 93 scenario kinds (tip-and-cue), from a full site rebuild at engine 0a53cb87e`.
   This covers `web/` and `web/capabilities.json` (the proof line). The recorder-diff list is in the
   commit message.
2. `i18n: English page catalogue re-extracted at the 93-kind site; …`.

Sources: kshana-pro `site/desktop-studio` b2ed1332 (site and Studio, clean), after 751efa14 (the
same procedure at 3ff14e6a3; it carries the placements below).

## Feature register placements (site-next/features/placements.toml), for review

All rows have `studio = "none"`.

| Feature | Page | Section | Docs |
|---|---|---|---|
| `kind:tip-and-cue` | Capabilities | `atlas` | `docs/scenarios.html#tip-and-cue` |
| `scenario:tip-and-cue-baltic-m2` | Capabilities | `atlas` | `docs/scenario-reference.html#tip-and-cue-baltic-m2` |
| `scenario:tip-and-cue-short` | Capabilities | `atlas` | `docs/scenario-reference.html#tip-and-cue-short` |
| `cli:scenario`, `cli:obs`, `cli:ntrip`, `cli:device`, `cli:planner`, `cli:station`, `cli:connect` | Docs | `commands.html#every-command` | the same |
| `cli:convert`, `cli:check` (subcommands of `obs`) | Docs | `commands.html#every-command` | the same, but NOT_SHOWN |
| `cli:watch`, `cli:adapters`, `cli:nats`, `cli:ws`, `cli:mqtt`, `cli:map`, `cli:memory` (subcommands of `connect`) | Docs | `commands.html#every-command` | the same, but NOT_SHOWN |
| `cli:version`, `cli:migrate` (subcommands of `scenario`) | Docs | `commands.html#every-command` | the same, but NOT_SHOWN |
| `mcp-tool:referee_location_score`, `mcp-tool:interference_emitter_regions`, `mcp-tool:connect_map` | Developers | `mcp` | `docs/mcp.html#tools` |
| `binding:python.{env_baseline, planner, area_trust_timeline, referee_location_score, connect_map}` | Docs | `surfaces.html#where-each-capability-is-reachable` | the same |
| `binding:wasm.{env_baseline, planner, area_trust_timeline, referee_location_score, connect_map}` | Docs | `surfaces.html#where-each-capability-is-reachable` | the same |

New descriptions, in `site-next/features/gen_features.py`:
- `CLI_DESC` for the 18 CLI rows.
- `API_WORDS` for the 5 binding functions.

New entry in the expansions block of `placements.toml`: `"NATS"`, the product name.

Register state after placement:

| Class | Count | What it is |
|---|---|---|
| NOT_SHOWN | 11 | The subcommand rows above. |
| UNREGISTERED | 4 | Backticked words in docs: `device` and `connect` in commands, `adapter` in contributing, `cap` in trust-telemetry. |
| ALSO | 1 | The same item as the 0.40.0 build's register. |
| NO_STUDIO | 15 | Includes tip-and-cue. |
| ABBREVIATION | 583 | The same 583 items as the 0.40.0 build's register, compared item by item. |

**Follow-up, not done.** `gen_features.py` reads the nested `match` arms in `src/main.rs`
(`run_obs_cli`, `run_connect_cli`, `run_scenario_cli`) as top-level commands. Its scanner should
name these rows `obs convert`, `connect watch` and so on, under their parent command. Once it
does, the 11 NOT_SHOWN rows go away.

## Scenarios with no Studio library place (left unplaced)

- `clock-fitted-log`
- `clock-measured-log`
- `lunar-ascent`
- `lunar-descent`
- `lunar-descent-lola`
- `lunar-orbit-rendezvous`
- `lunar-orbit-rendezvous-elfo`
- `lunar-surface`
- `lunar-surface-terrain`
- `mission-thread-artemis-class`
- `tip-and-cue-baltic-m2`
- `tip-and-cue-short`

## Deploy size: a proposal for headroom (nothing changed for it)

check.py rule 12b measures the deploy tree at 498.1 MB of its 500 MB cap:

- 166.6 MB is English, including the engine package.
- 331.4 MB is 17 language trees, about 19.5 MB each, of which about 14 MB is docs pages.

Three changes would free the most, in this order:

1. **Serve one copy of the WebAssembly package.** `pkg/` and `studio/pkg/` deploy the same
   18 MB `kshana_bg.wasm`, byte for byte. Pointing the Studio at `/pkg/`, or the reverse, saves
   18 MB at no cost to any page.
2. **Compress the Studio's browser recordings.** `studio/recorded/` is 43 MB of plain JSON. These
   are the fallback runs shown when WebAssembly is blocked. The native recordings already ship
   gzipped and are decompressed in the page. Doing the same here, with `.json.gz` and
   `DecompressionStream`, would save an estimated 30–35 MB.
3. **Stop copying untranslated docs into every language tree.** Each tree carries about 14 MB of
   docs pages, and the docs are the least-translated part of the site. A docs page whose segments
   are all untranslated could fall back to the English page instead of being copied into the
   language tree. That saves up to the full 14 MB per language for a mostly-English docs set,
   and is by far the largest lever (17×). It needs a decision on how a language page links to an
   English docs page.

Items 1 and 2 together give about 50 MB, roughly the room for the next several kinds or one more
language. Item 3 is the long-term fix.
