<!--
PRIVATE — Ashforde internal. kshana-pro repo only. Contact: contact@ashforde.org
-->

# AO/1-13695 (EL1-132): work packages, schedule and cost skeleton

> **DRAFT NUMBERS: NOT A PRICE.** These are planning figures built on the C04 calculator assumptions
> ([`../cost-volume/ashforde-ecos-cost-volume.html`](../cost-volume/ashforde-ecos-cost-volume.html)):
> €750/day loaded labour (the market-researched recommendation in a €650–850 band, **not yet registered
> in an ECOS rate file**), fee ≤ 8% (7% used), no management reserve. The partner day rate, equipment
> and campaign costs are **placeholders**. The duration (24 months) and the €800k figure come from the
> NAVISP Work Plan via report 01; esa-star shows only "> 500 KEURO". Confirm the maximum price, price type
> and milestone/payment plan in `eli1-13695.pdf` and `ecc1-13695.pdf`.
>
> **Honesty guard (from C04):** the hours must reflect real capacity (solo founder plus a named partner).
> Do not pad hours to fill the budget. A lean, defensible price scores better on the 30% criterion.

## Work packages

| WP | Title | Lead | Ashforde days | Partner days | Main outputs (provisional) |
|---|---|---|---|---|---|
| WP0 | Management, PA/QA, reviews | Ashforde | 40 | 0 | Management plan, PA plan, progress reports, review packages |
| WP1 | Requirements, signal-definition review, state of the art | Ashforde | 35 | 10 | Technical note: signal definitions; requirements baseline; SoTA |
| WP2 | Architecture: software core + hardware chain | Ashforde | 40 | 25 | Architecture and design document; ICDs (GNSS coupling, hardware, data formats) |
| WP3 | Software core: signals, propagation, multi-chain/multi-time, data channel, GNSS coupling | Ashforde | 110 | 0 | Simulator software releases; scenario packs per SoW environment list; software test report |
| WP4 | Hardware integration and synchronisation with commercial GNSS simulators | Partner | 30 | 80 | Reference LF chain; characterisation report (delay, gain, spectral purity); sync verification reports for *[2]* GNSS simulators |
| WP5 | Receiver analysis and validation tools; record and replay | Ashforde | 60 | 20 | Tool releases; coherent LF + L-band capture sets; user manual |
| WP6 | Fidelity validation against live signals | Partner (field) / Ashforde (analysis) | 45 | 60 | Pre-registered campaign plan; live recordings; fidelity report |
| WP7 | EL1→EL2 roadmap, final review, dataset hand-over | Ashforde | 20 | 10 | Roadmap; final report; executive summary; datasets for EL3-036bis |
| | **Total** | | **380** | **205** | |

Ashforde: 380 days over 24 months ≈ **190 days a year ≈ 0.86 FTE** (at 220 working days). That is the
founder's main job for two years, or the founder plus a named second engineer. Gate G5.

## Schedule (24 months, KO = kick-off; provisional)

| Milestone | When | Content |
|---|---|---|
| KO | T0 (≈ Apr–May 2027 if the contract follows a typical 3–4 month evaluation; unverified) | Kick-off |
| RR — Requirements review | T0 + 3 | WP1 done; requirements baseline; signal definitions agreed |
| DR — Design review | T0 + 6 | WP2 done; architecture, ICDs; fidelity metrics and bars **pre-registered** |
| MTR — Mid-term review | T0 + 12 | WP3 first release; WP4 reference chain characterised; sync with the first GNSS simulator shown |
| TRR — Test readiness review | T0 + 16 | WP5 tools; record and replay on hardware; campaign plan frozen |
| Campaign | T0 + 16 to T0 + 20 | WP6 live recordings (day/night) and scoring |
| AR — Acceptance review | T0 + 22 | Fidelity report; final software release |
| FR — Final review | T0 + 24 | Roadmap, datasets, final report |

```
Month      0    3    6    9    12   15   18   21   24
WP0        ██████████████████████████████████████████
WP1        ██████
WP2             ██████
WP3             ████████████████████████
WP4                  ████████████████████
WP5                       ████████████████████
WP6                                 ██████████████
WP7                                           ██████
Reviews    KO   RR   DR        MTR    TRR      AR  FR
```

## Cost build-up (draft)

| Line | Basis | Amount (€) |
|---|---|---|
| Ashforde labour | 380 days × €750 | 285,000 |
| Partner (subcontract) labour | 205 days × €900 *(placeholder rate)* | 184,500 |
| Equipment / hardware for the reference LF chain | AWG/DAC, LF amplifier, antenna/H-field simulator, TIC access *(placeholder; assumes the partner owns the GNSS simulators and references)* | 60,000 |
| Live-signal campaign | Site access, logistics, partner travel *(placeholder)* | 25,000 |
| Travel and meetings | Reviews at ESTEC: video by default; founder travel kept minimal *(placeholder)* | 10,000 |
| **Direct cost** | | **564,500** |
| Fee | 7% of direct (ceiling 8%) | 39,515 |
| **Total (draft)** | | **604,015** |

- Against the €800k Work-Plan figure (unverified as the ITT ceiling), the draft is ≈ 75%. Do not raise
  it to fill the budget. If the SoW asks for more (e.g. a delivered hardware unit, several campaigns, a
  GUI), add those lines explicitly.
- Fee: C04 applies the fee to direct cost. Check whether the partner's subcontract carries its own fee
  and whether ESA's rules allow a fee on a fee (normally the prime's fee applies to its own costs).
  Resolve this with the PSS forms.
- Payment plan, advance payment (C04 shows 35% for an SME) and milestone split: from the contract
  conditions.
- **ECOS:** "ECOS required: True" on esa-star. The UG has no rate file yet. Use the ESA-accepted
  route for a new SME (unverified; ask in the helpdesk question).

## Recompute

Paste the WP rows into the C04 calculator (one row per WP, Phase 1 column only, since this ITT has no
phases) with rate 750, fee 7, ODC 95,000 and subcontract 184,500. The calculator's 13494/13503 cap checks
(€1.0M / €2.5M / €3.5M) do **not** apply to this ITT. Use the ITT's own ceiling.
