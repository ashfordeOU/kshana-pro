# Numbers in the 0.40 release video

Kshana 0.40.0, tag `v0.40.0`, commit `1395dc6803691aa9c008db5d369299e83c74ae43` (public repository ashfordeOU/kshana). **Not re-run for the video**: building and running the engine was not permitted in the environment the video was made in, so every number is read from engine output and results files that the repository commits at that tag, by `src/collect.py`. Each file is listed below with its SHA-256 at the tag. The Studio's recorded mission-thread result records its own engine version (0.40.0) and the commit it was recorded at (`339521841a159ffa0594dd73fe8a7c5483841676`). The commands that regenerate each output from a build of the tag are under "Live commands". Inputs marked synthetic are synthetic and say so on screen.

## Files read at the tag

| file | SHA-256 | taken from it |
|---|---|---|
| `web/studio/recorded/mission-thread-artemis-class.json` | `928ae413504feedb…` | recorded run of scenarios/mission-thread-artemis-class.toml (phases, peaks, margins, weakest link, evidence) |
| `xval/mission-thread/results_oracle.json` | `7309859857e10d13…` | row C1: cislunar propagation against the public Artemis I trajectory, 72 h |
| `xval/mission-thread/RESULTS.md` | `90784cbed776844b…` | diagnostic: median hourly residual between the five events |
| `xval/mission-thread-lunar/results_oracles.json` | `cb4f987183ce95a9…` | rows A1 (LRO, 24 h) and A2 (CAPSTONE, one revolution) |
| `examples/emitter-region/output/emitter-adsb-custom-2026-03-01.geojson` | `c15778d40cec1e41…` | regions at 50/90/95 %, the 95 % ellipse and the centroid (synthetic input) |
| `examples/emitter-region/output/emitter-adsb-custom-2026-03-01-summary.json` | `61000ae53551fe99…` | session, region areas, ellipse, input counts, evidence (synthetic input) |
| `docs/emitter-region/RESULTS.md` | `14dc5c986edc6406…` | Jammertest 2025 comparison: outcome per source |
| `README.md` | `6c8092dcae93abde…` | What's new in 0.40 (clock-fit misses, 92 kinds); README is the release text |
| `docs/CLOCK-LOG-FORMATS.md` | `6d858b874074c1b6…` | the eight clock-log formats |
| `examples/partner-submission/submission.toml` | `94448759c0019953…` | synthetic data-partner submission |
| `xval/bench/RESULTS-0.40.md` | `0955b9e902f2d1b7…` | 0.40 re-campaign verdicts A3, D3, R3, R4 |
| `xval/bench/RESULTS-v2.md` | `20366002788a60ba…` | A2 and D1 recorded verdicts (unchanged in 0.40) |
| `docs/BENCH.md` | `9e5ed65827017062…` | R1 verdict |
| `CHANGELOG.md` | `663d1d9f7311fba8…` | [0.40.0]: security entry, matrix growth, kinds growth |
| `Cargo.toml` | `e9945c7263fe4798…` | rust-version |
| `web/data/verification-matrix.json` | `18e9c66fd6a4cf4e…` | the ledger: rows and status counts |
| `i18n/<code>/engine.ftl` ×18 | (per file in `src/data/raw/raw.json`) | the advisory line `engine-receiver-trust-chart-advisory` |

## Numbers used on screen, and where each comes from

- Mission thread (`mission-thread-artemis-class.toml`, recorded run): "Artemis-class mission thread (ILLUSTRATIVE)", 11 phases: ascent (launch-ascent), rdv-1 (leo-rendezvous), rdv-2 (leo-rendezvous), rdv-3 (leo-rendezvous), cruise (tli-and-cruise), lunar-orbit (lunar-orbit-rendezvous), descent (lunar-descent), surface (lunar-surface), ascent-lunar (lunar-ascent), redock (lunar-orbit-rendezvous), entry (entry). Peak 1-sigma position per phase (m): ascent 60.7, rdv-1 18.9, rdv-2 19.5, rdv-3 19.5, cruise 451.1, lunar-orbit 451.2, descent 82.4, surface 36.6, ascent-lunar 187.8, redock 187.8, entry 21.1. Weakest link: phase `descent`, margin 1.21 against its stated limit, set by "inertial drift, no aid, east". Tier MODELLED; evidence rows VM-cf3cdcd3, VM-dd872eab, VM-141c6467, VM-3285c018, VM-09d31ea9, VM-fe6d68d3, VM-36619fa3, VM-5c095f48, VM-0002d522, VM-bb8b8191, VM-5bd5efbe, VM-576667f2, VM-85ad15fc (tier MODELLED). Every number MODELLED; the example's inputs are ILLUSTRATIVE.
- Scenario kinds: 92 (README "What's new in 0.40"; CHANGELOG [0.40.0]: from 87 to 92).
- C1, cislunar propagation against the public Artemis I trajectory, 72 h (`xval/mission-thread/results_oracle.json`): largest position difference 45325.2 m = 45.3 km against 5 km; velocity at 72 h 0.2489 m/s (shown as 0.25 m/s) against 0.05 m/s; per epoch (h, km) [[12, 3.74], [24, 12.04], [36, 21.36], [48, 32.11], [60, 35.78], [72, 45.33]]: **NOT MET**. Diagnostic (after the verdict, not part of it): median hourly residual 0.38 m between the events.
- A1, lunar propagator against the public LRO reconstructed trajectory, 24 h (`xval/mission-thread-lunar/results_oracles.json`): max 3-D 33.11 m against 500 m, RMS radial 2.39 m against 60 m: **MET**.
- A2, the same propagator against the public CAPSTONE orbit, one revolution: end position 136822.9 m = 136.8 km against 25 km; periapsis radius 28371.8 m = 28.4 km against 10 km: **NOT MET**.
- Emitter region (`examples/emitter-region/output/`, the committed synthetic example, engine 0.40.0): one session 2026-03-01T10:00Z to 2026-03-01T10:30Z (30 min), 21 affected of 80 distinct aircraft, 2480 reports; regions 50 % 176 km2, 90 % 577 km2, 95 % 761 km2; 95 % ellipse 16.3 by 15.0 km, azimuth 82 deg; evidence VM-bc70cb57 (MODELLED). Input: "Synthetic data made up for this example; not a measurement". Jammertest 2025 comparison (`docs/emitter-region/RESULTS.md`): ADS-B 2 scheduled sessions, NOT ASSESSABLE; AIS 0 scheduled sessions, NOT ASSESSABLE.
- Measured clocks: eight formats (CHANGELOG [0.40.0]); the fit misses F1(a) on five of six records and F2 on one (README); both MODELLED.
- Benchmark re-campaign (`xval/bench/RESULTS-0.40.md`): bar "A3 (EXPLORATORY) the 2.5 % quantile of the folder-clustered bootstrap AUC of `agc_k_sigma` >= 0.80": 2.5 % = 0.8502, 97.5 % = 0.9995, median 0.9377; point estimate 0.9332 (equals the recorded pooled 0.933): MET; bar "D3 (EXPLORATORY) at scale 0.25 on `test`, at least one detector has `optimism_gap` > 0.05": gaps 0.0961 (`agc-excess`), 0.0860 (`energy`), 0.0704 (`fused`), 0.0494 (`parity`), 0.0187 (`sqm`): three of five exceed 0.05: MET; bar "R3 `cn0_drop` AUC >= 0.95 on the 18 event windows against null-recording windows": AUC 1.0: MET; bar "R4 at h = 5, P_fa <= 0.05 and P_d >= 0.90": P_fa 0.0, P_d 1.0: MET. Unchanged and still NOT MET: A2 AUC 0.766 against 0.80 (`xval/bench/RESULTS-v2.md`), D1 largest gap 0.0301, R1 (NOT MET, `docs/BENCH.md`).
- Languages: 18 catalogues under `i18n/` (cs, da, de, en, es, et, fi, fr, ja, nb, nl, pl, pt-BR, pt-PT, ru, sv, uk, zh-Hans), English and 17 machine translations; the advisory line shown in each is the catalogue's `engine-receiver-trust-chart-advisory`.
- Ledger (`web/data/verification-matrix.json`): 401 rows, 176 VALIDATED, 221 MODELLED, 4 PARTNER; CHANGELOG [0.40.0]: from 388 to 401 rows, no row promoted.
- Security: RUSTSEC-2026-0387 is fixed in the CHANGELOG section [0.39.0] - 2026-10-22, not [0.40.0]; minimum Rust 1.88 (`Cargo.toml` rust-version at the tag). Shown on screen only as "since 0.39".

## Live commands

From a build of the tag (`cargo build --release --bin kshana`), these regenerate the outputs above:

- `kshana scenarios/mission-thread-artemis-class.toml`
- `cargo run --release --example mission_thread_oracle -- <dir>   # xval/mission-thread/fetch_data.sh first`
- `kshana interference-map locate examples/emitter-region/input/adsb-synthetic.csv --source adsb --dataset custom --licence CC0-1.0 ... --out out`
- `kshana partner check examples/partner-submission/submission.toml`
- `kshana --lang <code> scenarios/<file>.toml`
