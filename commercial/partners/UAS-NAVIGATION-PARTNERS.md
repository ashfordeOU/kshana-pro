<!-- PROPRIETARY — Kshana Pro overlay. Internal commercial record. Not part of the open-source engine. -->

# Uncrewed-aircraft navigation partners: interceptor autonomy and radar makers

**Owner:** Ashforde OÜ · **Status:** DRAFT ONLY — nothing has been sent · **Date:** 2026-09-25

> **Two holds apply before anything here is sent.**
>
> 1. **Export control.** Every organisation below builds equipment for a force at
>    war. Anything beyond published open-source material, such as a study, a Kshana
>    Pro licence or tailored analysis, is presumed to need a determination from the
>    Estonian Strategic Goods Commission first (`legal/EXPORT-CONTROL-DETERMINATION.md`).
>    The open engine is public under the GNU Affero General Public License (AGPL), and
>    pointing someone to it is not a supply.
> 2. **The founder reads and approves each send.** They are written to go from
>    `contact@ashforde.org` only.
>
> **The scope boundary is the same for Kshana and Pavana.** Ashforde offers navigation
> resilience evidence (how a vehicle's position and time degrade under jamming, spoofing
> and loss of the Global Navigation Satellite System, GNSS), plus detection that reports
> to a command-and-control system. It offers nothing that selects, guides to or engages
> a target. No draft may offer a targeting, guidance or fire-control input. Any change
> to that rule is the founder's decision alone.

Abbreviations: UAS, uncrewed aircraft system; C-UAS, counter-UAS; EW, electronic
warfare; FPV, first-person-view drone; IMU, inertial measurement unit; MoU, memorandum
of understanding; SAPIENT, the UK Ministry of Defence sensor-integration standard;
CoT, Cursor-on-Target; ASTERIX, the EUROCONTROL surveillance data exchange format.

---

## Why this group

Ukrainian C-UAS work has moved to interceptor drones against one-way-attack drones.
Two public examples reached the founder on 2026-09-25:

- **Valkyrie Dynamics** builds miniature radar sensors to be carried on interceptors.
  It argues that on-board camera and infrared detection degrade in winter (longer
  nights, cloud), and it announced an MoU with Korven Systems on sensor fusion.
- **Korven Systems** is a Ukrainian–Estonian company. It moved from fintech and AI
  (active since 2021) to autonomy systems for interceptor drones, including an autopilot
  for a high-speed interceptor. Its founder also develops Teletactica, which makes
  EW-resistant communication systems for UAVs (uncrewed aerial vehicles).

What Ashforde has that fits, without crossing the scope boundary:

| Need these builders state or imply | Ashforde asset | Evidence tier today |
|---|---|---|
| Navigation that keeps working when GNSS is jammed or spoofed | Kshana: `small-uas-jammed-nav` (a Bosch BMI088 flight-controller IMU against a tactical-grade unit after the GNSS signal is jammed, from 0.27.3; on every channel from 0.27.4), plus `jamming-demo`, `spoof-detect`, `spoof-meaconing`, `terrain-nav` and `hybrid-pnt` | Modelled. The BMI088 figures come from its datasheet temperature coefficients plus two flagged assumptions (a 10 K warm-up, then 1 K per minute in flight). Validating them needs jammed-environment flight logs, which is the ask in draft 1. **Do not send before 0.27.4 is on every channel (crates.io, PyPI and npm):** 0.27.3 fixed a sign error in the navigation filter that had put a floor of tens of metres under every GNSS/INS coast figure. |
| Testing before flight, with no range time | Pavana Sim (a separate product): drone flight dynamics, four emulated radar classes writing ASTERIX, measured infrasound clutter, SAPIENT checked with Dstl Apex v4.2.0 | Modelled throughout; nothing is validated in the field |
| Detection when cameras are blind (night, cloud, fog) | Pavana Wake: a passive pressure tripwire covering the last few metres of an opening. It sees fibre-optic FPV drones, which give off no radio signal | Modelled. Range is about 2 m; it cannot see jet or Shahed-class drones at altitude and must never be pitched against them. |

**Not a fit, and never to be offered:** Pavana on an interceptor (its range falls as
airspeed rises, so it is blind in flight), and any feed from Pavana or Kshana into an
autopilot's targeting.

---

## 1. Korven Systems (Ukraine and Estonia), tailored

**To:** `[find the company's public contact route; none is recorded yet]`
**Subject:** Estonian open-source tool for navigation under jamming: evaluation data and validation

Dear Korven Systems team,

I read in Defender Media that Korven Systems is building autonomy systems for
interceptor drones, and that its team also develops communication systems designed to
keep working under electronic warfare. Ashforde OÜ is a Tallinn company that builds
Kshana, an open-source engine that predicts how a vehicle's position and time degrade
when the Global Navigation Satellite System (GNSS) signal is jammed, spoofed or lost.
Every figure it reports says whether it has been validated against a published
reference or is only modelled.

Two questions, both about navigation evidence rather than your product:

1. Would a small-drone navigation scenario be useful to you: a consumer-grade inertial
   sensor, a jammed GNSS receiver and a stated drift budget, published with its inputs
   so that anyone can rerun it?
2. If you hold flight logs from jammed environments that you are able to share, could
   we compare our predictions with them? That would let the result be labelled
   validated rather than modelled.

The engine is published under the GNU Affero General Public License at
github.com/ashfordeOU/kshana. As a fellow Estonian company, I would be glad to meet in
Tallinn or online.

Kind regards,
[Name]
Ashforde OÜ · contact@ashforde.org

---

## 2. Miniature-radar makers (template; Valkyrie Dynamics is the example)

Hold this draft until Pavana's time-cued radar handoff (WP6 in the Pavana plan) shows
a handoff for every emulated radar class. The draft offers interoperability testing
only: the radar supplies a cue, and Pavana confirms at an opening and reports to the
buyer's command-and-control system. It says nothing about fusion for targeting.

**Subject:** Testing a radar cue against a passive last-metres sensor, in simulation

`[Write this when WP6 is green. Quote only figures from Pavana's generated artefacts.]`

---

## Facts used here (checked 2026-09-25)

| Claim | Source |
|---|---|
| Korven Systems: Ukrainian–Estonian; fintech and AI since 2021; switched this year to interceptor autonomy; its founder also develops Teletactica (EW-resistant UAV communications) | Defender Media post on LinkedIn, short link `lnkd.in/drmxy6UN`, seen in a screenshot the founder shared. **Not yet read at the source: open the article before sending.** |
| Valkyrie Dynamics: miniature radar for interceptors; MoU with Korven Systems; the winter degradation argument | Valkyrie Dynamics LinkedIn posts, seen in the founder's screenshots. Their figures ("up to 40%", "3,000 a month") are the company's own and are not quoted here. |
| Kshana is public under the AGPL | GitHub API, `repos/ashfordeOU/kshana` |
| Pavana's range, blind spots and "modelled" status | the Pavana repository README and its generated limits section, 2026-09-25 |

## Send record

| Date | Organisation | Route | Sent by | Reply |
|---|---|---|---|---|
| — | — | — | — | nothing sent |
