<!--
PRIVATE — Ashforde internal. kshana-pro repo only. Contact: contact@ashforde.org
-->

# AO/1-13695 (EL1-132): risk register

L = likelihood, I = impact (1 low – 5 high). Score = L × I. Bid-phase risks (B) decide whether we bid;
contract-phase risks (C) go into the proposal's risk section, in edited form.

| ID | Phase | Risk | L | I | Score | Mitigation | Owner | Trigger / date |
|---|---|---|---|---|---|---|---|---|
| B1 | Bid | **Ashforde UG not in the Handelsregister or not validated in esa-star before submission** | 4 | 5 | 20 | Finish capital and notary filing now; helpdesk question on a UG i.G. (§A); Plan B (supplier to an eligible prime) | Founder | Not validated by Mon 14 Dec → Plan B |
| B2 | Bid | **No German NAVISP delegate funding statement** (KAF 8) | 3 | 5 | 15 | Contact the German Space Agency at DLR by 27 Nov; brief on the five follow-on eLoran ITTs too; with a non-German partner, its delegate is needed as well | Founder | No positive signal by 14 Dec → Plan B |
| B3 | Bid | No RF/eLoran hardware partner in time; incumbents team with each other | 3 | 5 | 15 | Approach 2–3 partner profiles in parallel (founder's go); offer the simulator core as their software layer; German partner preferred (one delegation) | Founder | No LOI by 14 Dec → no-go as prime |
| B4 | Bid | Estonian OÜ or a not-yet-registered UG is not allowed even as subcontractor/supplier (KAF 1 covers subcontractors) | 3 | 4 | 12 | Clarification Q1; licensing as a commercial off-the-shelf item may be treated differently (unverified) | Founder | Answer to Q1 |
| B5 | Bid | The SoW requires something we cannot meet (a delivered hardware product at high TRL; on-air transmission; a facility we lack) | 2 | 5 | 10 | Read the SoW by 27 Nov (G4); partner fills hardware | Analyst | 27 Nov |
| B6 | Bid | Founder capacity: a 24-month, ≈ 0.86 FTE commitment collides with the seed raise (first close target 29 Jan 2027) and Arksa pilots | 4 | 4 | 16 | Decide at gate G5; name a second engineer only if real; scope Ashforde's share honestly | Founder | 7 Dec |
| B7 | Bid | Deadline confusion (report 01 said closed 18 Nov; snapshot says 4 Jan 2027) | 2 | 5 | 10 | Confirm on the live esa-star page on 20 Nov | Founder | 20 Nov |
| B8 | Bid | Disclosure of unfiled patent-candidate methods in the proposal | 2 | 4 | 8 | Describe outcomes only; methods stay behind `unfiled-methods` | Analyst | Drafting |
| B9 | Bid | Founder's current ESA/ESOC relationship creates a conflict-of-interest or non-benefits issue (KAF 4) (unverified) | 2 | 5 | 10 | Founder confirms status; if still at or contracting to ESA, check the rules before tendering | Founder | 27 Nov |
| B10 | Bid | Holiday period: delegate letters and partner signatures slow down after ~18 Dec | 4 | 3 | 12 | Ask for letters in early December; submit 30 Dec | Founder | 18 Dec |
| C1 | Contract | Live-signal fidelity is poor for land paths or night skywave (models are MODELLED today) | 3 | 4 | 12 | Pre-registered bars; inverse modelling on calibration data, scored on withheld data; report misses honestly; scope of "validated" stated per condition | Ashforde | DR / campaign |
| C2 | Contract | Hardware drivers have never run on hardware; LF chain integration slips | 3 | 4 | 12 | Bench-readiness check from datasheets before purchase; virtual bench in CI; partner leads WP4; early chain characterisation by MTR | Partner | MTR |
| C3 | Contract | Access to live eLoran transmissions is limited (Anthorn is the only active European transmitter found; schedules can change) | 3 | 4 | 12 | Clarification Q5; alternative site or test transmitter via partner; record over several days; use the Korean or other datasets only if licensed | Partner | DR |
| C4 | Contract | GNSS simulator vendor command interfaces differ from our profiles | 3 | 2 | 6 | Profiles are configurable; confirm against manuals in WP2; sync is *measured* with a TIC, not assumed | Ashforde | WP4 |
| C5 | Contract | Signal standards (SAE AS9990-family, 10th pulse) not accessible or still changing | 3 | 3 | 9 | Q6; signal definitions are configuration; track the ROK–UK–FR working group | Ashforde | RR |
| C6 | Contract | Key-person dependency (solo founder) | 4 | 4 | 16 | Documented engine, CI and reproducible scenarios; partner's key persons; named back-up if hired | Founder | throughout |
| C7 | Contract | Export control of threat scenarios (LF jamming/spoofing) or of the simulator itself | 2 | 3 | 6 | Offer threat content only if the SoW asks (Q9); export-control review (EU 2021/821) before release | Founder | RR |
| C8 | Contract | IP terms give ESA rights that conflict with the commercial licence of the background engine | 2 | 4 | 8 | Read `ecc1-13695.pdf`; declare background IP; Q2 | Founder | before submission |
| C9 | Contract | Cash flow: an SME carrying 24 months of cost before milestone payments | 3 | 3 | 9 | Use the advance payment if offered (C04 notes 35% for SMEs, to confirm); milestone plan weighted early | Founder | contract negotiation |
