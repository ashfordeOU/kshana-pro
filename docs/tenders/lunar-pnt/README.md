<!--
PRIVATE — Ashforde OÜ internal positioning material. kshana-pro repo only; NEVER publish in the open kshana repo.
Program-area (not a single AO): anticipatory ESA lunar-PNT fit assets mapping Kshana's lunar capability
to the class of need expressed by ESA's Moonlight / LCNS and the NovaMoon-class lunar geodetic/timing/
differential PNT station concept, plus the LunaNet / IOAG lunar interoperability framework.
Contact for all external use: contact@ashforde.org
-->

# ESA lunar-PNT fit assets (PRIVATE)

**PRIVATE — Ashforde OÜ internal positioning material.** Shelf assets that position **Kshana's lunar
PNT capability** against the *class of need* expressed by ESA's lunar PNT programme. Not tied to a single
open AO — this is anticipatory program-area positioning (Moonlight/LCNS is an active services initiative;
a NovaMoon-class lunar geodetic/timing/differential station and the LunaNet/IOAG interoperability work are
the named concepts the LinkedIn-surfaced ESA activity describes). Never publish in the open kshana repo.
All external contact: **contact@ashforde.org**.

## What ESA is doing (public context these assets respond to)

| Programme / concept | What it is (public) | Kshana's responding capability |
|---|---|---|
| **Moonlight / LCNS** | ESA's lunar communications & navigation initiative; Lunar Communication and Navigation Services — a small lunar nav constellation + signal/service definition. | Nav service-volume DOP / coverage / availability + integrity protection level (`lunar_service`). |
| **NovaMoon-class station** | A lunar geodetic + timing + differential-PNT reference station: multi-technique geodesy (VLBI / ranging), joint orbit-and-clock determination, lunar reference frame, lunar time, differential corrections. | VLBI observable (`lunar_vlbi`), joint OD+clock (`lunar_combination`), frame realisation (`lunar_frame_realise`), lunar time (`lunar_time`), differential PNT (`lunar_dpnt`). |
| **Coordinated Lunar Time** | A lunar time scale and its relativistic rate vs terrestrial time (~57 µs/day). | Relativistic Earth↔Moon clock-rate model + LTC/TCL conversions + ensemble (`lunar_time`). |
| **LunaNet / IOAG interoperability** | Cross-agency lunar comm/nav/timing interoperability framework and specifications. | LunaNet/IOAG-aligned CCSDS OEM + time + ephemeris export, with round-trip conformance (`lunar_interop`). |

## Standing rules (apply to every file in this folder)

- **Kshana ≠ NovaMoon, ≠ Moonlight, ≠ LCNS, ≠ a flight product.** Kshana is an open, reproducible
  **simulator that models the *class* of system** these programmes build. Example lunar constellation /
  station parameters are **illustrative, public-source, and not affiliated with ESA**. Never imply
  Kshana is, powers, or is endorsed by any ESA programme.
- **Honest scope, identical to the code.** Every lunar capability is **MODELLED** (open simulator,
  calibrated to public oracles: DE ephemerides, GRAIL gravity, IAU/IERS conventions, LunaNet/IOAG,
  the published lunar-clock-rate band) — except where it **reuses an already externally-validated
  kernel** (e.g. DOP validated against `gnss_lib_py`; Earth orientation against SOFA/ERFA). Claim
  **validation and reproducibility — never TRL > 3, certification, flight heritage, or ESA endorsement.**
  This mirrors the invariant the open repo enforces in `src/verification.rs` and `tests/no_overclaims.rs`.
- **Role of Kshana in any bid = evidence of engineering & assurance capability**, plus a ready
  mission-analysis / benefits-case substrate — *not* the thing being flown. Keep that framing intact
  (same posture as the FutureNAV C3 assets in `../cross-tender/`).
- **Outreach framing is empowerment and collaboration.** ESA-facing copy positions Kshana as open,
  neutral, reproducible heritage that *strengthens* a lunar-PNT consortium. Internal commercial strategy
  (the broader market thesis) stays in `docs/strategy/` and the founder's messaging notes — it does **not**
  appear in any artifact that could reach ESA or a prime. See the messaging guardrails before reusing copy.
- **Outreach is PARKED** pending explicit founder go. These are shelf assets; producing them does not
  authorise contacting anyone. **No real prime/partner names** appear here.
- Business contact **contact@ashforde.org** only (never the personal address).

## Index

| File | One-line summary |
|---|---|
| [`CAPABILITY-STATEMENT.md`](CAPABILITY-STATEMENT.md) | Ready-to-paste lunar-PNT credibility copy (tight drop-in + half-page + per-concept tailoring lines) for ESA lunar outreach — honest, validated/modelled, non-endorsement. |
| [`FIT-MAP.md`](FIT-MAP.md) | Capability crosswalk: each Kshana lunar module + reused validated kernel → the ESA lunar-PNT need it addresses, with the runnable scenario, the oracle, and the Validated/Modelled label. |

## Status notes (founder)

- The seven lunar modules and their scenarios are **built and committed in the kshana working tree**
  (`lunar_time`, `lunar_vlbi`, `lunar_combination`, `lunar_frame_realise`, `lunar_service`, `lunar_dpnt`,
  `lunar_interop`) but **not yet pushed** to the public repo. The public-repo / crates.io / Zenodo
  evidence these assets cite becomes live on the next release once you authorise the push.
- A companion arXiv preprint (lunar geodetic-VLBI observability result) is drafted privately in
  `papers/lunar-pnt/` — arXiv-first, nothing public until you announce. Cite it in outreach only after
  it is posted.

---

*Ashforde OÜ · contact@ashforde.org · Kshana — open AGPL-3.0-only PNT performance & resilience simulator,
commercial licence available · models the class of lunar PNT systems, not a flight product · TRL ~3, no
flight heritage, no certification, no ESA endorsement · claims labelled validated (external oracle) or
modelled · outreach PARKED pending founder go.*
