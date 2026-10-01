# Site coverage audit (2026-09-27, multi-page)

The redesign is now a multi-page site built by `build.py` from `src/` and the Kshana repository:
`index.html` (Home), `missions.html`, `capabilities.html`, `evidence.html`, `developers.html`,
`editions.html`, `docs/` and the redesigned `playground/`. This file records where each need the live
site (`~/Code/kshana/web/`) serves, each engine capability area and each founder request now lives.

`check.py` reads every `cover:` link below and fails if the target page or anchor does not exist.
All counts come from the repository at build time (currently v0.28.0, 63 scenario kinds,
83 scenario files (81 bundled), 174 capabilities: 66 Validated, 104 Modelled, 4 partner-owned).

## Refreshing after a Kshana release

One command, nothing typed by hand:

    python3 build.py && python3 check.py

`build.py` re-reads the version, package names, install channels, scenario files, verification
matrix, capability cards, MCP tools and git-tracked docs; `check.py` fails on any drift. New engine
output for the charts comes from re-recording the playground (`node tools_record.mjs` in
the Studio folder `src/data/sources.json` names, after `node tools_native.mjs`; see `ASSEMBLY-RUNBOOK.md`) before the build.

## Live-site needs

| Need | Where it lives now | Link |
|---|---|---|
| Capability explorer, 46 cards with a Run link each | Capabilities, cards with domain filters | `cover:capabilities.html#explorer` |
| Validation ledger, all 174 rows | Evidence, searchable and filterable | `cover:evidence.html#ledger` |
| Verification matrix at a glance | Evidence, ring and one cell per capability | `cover:evidence.html#matrix` |
| Standards grid | Evidence | `cover:evidence.html#standards` |
| Playground: every feature (picker, TOML editor, summary, figures of merit, time series, stability, 3-D orbit, JSON, compare, sweep, report, chart download, share link, embed, tour) | The redesigned playground; its PARITY.md maps all 26 live features | `cover:playground/index.html` |
| MCP server: tools, registry name, client configuration | Developers | `cover:developers.html#mcp` |
| JetBrains plugin | Developers | `cover:developers.html#jetbrains` |
| Install channels | Developers installer, generated from `channels.json` | `cover:developers.html#install` |
| Cite, BibTeX and APA from CITATION.cff; related publications | Evidence | `cover:evidence.html#cite` |
| Editions: open core, Pro, custom and studies | Editions | `cover:editions.html#editions` |
| "Not an RF simulator, not a receiver" | Home capability bento ("What Kshana is not"), Missions, Capabilities, footer | `cover:missions.html#who` |
| Every abbreviation spelled out, glossary | The glossary lives in Docs only (founder R3 #30), with the site's own abbreviations appended | `cover:docs/glossary.html#on-this-site` |
| Documentation | Docs, rendered from git-tracked Markdown | `cover:docs/index.html` |

## Engine capability areas

| Area | Where, with the recorded run behind it | Link |
|---|---|---|
| GNSS jamming and interference | Home jamming footprint tile (`maritime-strait-jamming`, 875 runs) and the chained campaign's jamming phase, band atlas, Defence sector | `cover:index.html#jam` |
| Spoofing and meaconing | Home spoofing tile (`spoof-attack`) and scroll story (the campaign's spoofing phase), Missions failure cards (`spoof-meaconing`) | `cover:index.html#fails` |
| GNSS/INS coasting | Home coasting-cone tile (`small-uas-jammed-nav`), Automotive nearest run (`terrain-slam`) | `cover:missions.html#aviation` |
| Clock holdover | Clock ladder (`clock-holdover`, `clock-holdover-labsr`, `orbit-molniya`), lunar clock classes (`lunar-time-budget`) | `cover:capabilities.html#clocks` |
| Slot timing | Timing wall (`slot-timing-ocxo-leo`) | `cover:capabilities.html#timing` |
| Orbits | Home hero globe (the 102-satellite constellation run) and SGP4 tile (`ephemeris`), Space sector (`orbit-gnss-challenged`) | `cover:missions.html#space` |
| Integrity (RAIM, ARAIM) | Home RAIM tile and hero protection-level card (the chained campaign), Aviation sector (`araim-gps-galileo`), lunar stop (`lunanet-araim`) | `cover:capabilities.html#journey` |
| Telecom timing masks | Timing wall and Telecom sector (`telecom-prtc-holdover-24h`) | `cover:missions.html#telecom` |
| Lunar and Mars PNT | Journey (`lunanet-araim`, `mars-pnt-surface`), Space sector | `cover:capabilities.html#journey` |
| Quantum-sensor trade studies | Research sector (`sweep-clock-stability`), capability cards | `cover:missions.html#research` |
| Exchange formats | Standards grid, capability cards | `cover:evidence.html#standards` |

## Founder requests

| Request | Where | Link |
|---|---|---|
| Missions for every sector, including Defence (defensive framing only) | Missions | `cover:missions.html#sectors` |
| Who uses it, by role | Missions | `cover:missions.html#who` |
| Ways to use it | Developers | `cover:developers.html#ways` |
| Free and Pro highlighted; why everyone needs it, why Pro matters more | Editions and the Home Pro band | `cover:editions.html#why` |
| Pro in the hero | Home hero, secondary line to Editions | `cover:editions.html#pro` |
| Pro and custom study presented as powerful, public facts only, no prices | Editions Pro band with four deliverables on real open-engine output | `cover:index.html#pro-band` |
| Band usage, clocks, timing | Band atlas, clock ladder, timing wall | `cover:capabilities.html#bands` |
| Everything real and reproducible in the playground | Every chart carries engine version, scenario and seed, and opens that run | `cover:index.html#jam` |
| Real planets and Earth | Home hero globe, journey (NASA imagery; see ../2026-09-27-audit/IMAGERY.md) | `cover:capabilities.html#journey` |
| Installation maintained smartly | Installer from `channels.json`, drift-checked | `cover:developers.html#install` |
| Docs generated from the repository | Docs | `cover:docs/index.html` |
| Multi-page structure | Six pages plus docs and playground, one shell | `cover:index.html` |
| Playground redesign | Playground, linked from every page's nav | `cover:playground/index.html` |

## Feature gaps worth building in the engine (proposals; not shown as existing)

1. ~~Maritime scenarios~~ and 2. ~~road and rail scenarios~~: shipped in Kshana commit aab844f (`maritime-strait-jamming`, `maritime-spoof-position-push`, `maritime-port-approach-coast`, `automotive-urban-canyon`, `rail-tunnel-coast`), recorded in the Studio and shown on Missions, Capabilities and the docs. Not yet pushed to GitHub, so their docs cards say "Not yet on GitHub" instead of linking.
3. **A geostationary user file.** The orbit kind supports any user orbit (see `orbit-molniya`), but no GEO file ships.
4. **Sector time-accuracy presets** for grids and finance, with sources.
5. **GLONASS, BeiDou and Galileo E6 signal models.** Ephemerides are parsed; the signals are not modelled.
6. **An azimuth per satellite in the jamming result.** It would allow an honest sky plot; today only elevation is published, so the site draws no sky plot.
7. **A time-varying jammer in a bundled scenario.** `jamming-demo`'s jammer is on for the whole run, so its waterfall shows the before and after as two panels (without and with the jammer) rather than an onset.
8. **Ledger mapping by kind.** Only 12 of the 63 kinds map to a ledger module by name.
9. **Clock presets through a recorded run.** The recorded runs cover the optical lattice, lab strontium and CSAC clocks, an OCXO (slot timing) and a rubidium (telecom); TCXO and caesium curves would need a bundled scenario each.
