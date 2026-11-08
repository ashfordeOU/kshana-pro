#!/usr/bin/env python3
"""Write the sector console of the Missions page (src/pages/missions.html, between the
<!-- SECTORS:BEGIN --> and <!-- SECTORS:END --> markers) from the table below.

Each sector names its question, threat, figures of merit and standards (the page's own words),
the recorded run its chart replays, and the scenario files that demonstrate it:
Both lists name scenario files of the site's one engine checkout (src/data/sources.json: repo),
and every file of both must be in the Studio's library (src/data/sources.json: studio), so every
card opens that exact file in Kshana Studio:
  - files      plain cards: the whole card is the link to the file's recorded or live run.
  - next       the engine-run cards (dashed edge): cards that also show the run's headline figures,
               read from src/js/missions-runs.mjs (src/tools/gen_missions_runs.py runs the engine).
               The page script adds their "Open in Kshana Studio" link, on the view that reproduces
               the run, from js/studio-links.mjs (build.py writes it from the Studio's own link list).
A file the engine or the Studio's library does not carry stops the script.
Card text is written here in plain words, without figures: every figure on a card is drawn at run
time from a recorded run, so check.py's hand-typed-number rule holds.

  /opt/homebrew/bin/python3 src/tools/gen_missions_page.py [--check]
"""
import html, os, re, sys, tomllib

HERE = os.path.dirname(os.path.abspath(__file__))
SRC = os.path.dirname(HERE)
PAGE = os.path.join(SRC, "pages", "missions.html")
sys.path.insert(0, HERE)
from _sources import engine_tree, studio_dir, read_recorded, recorded_names  # noqa: E402  (src/data/sources.json: the one engine checkout)
REPO = engine_tree()
PG_REC = os.path.join(studio_dir(), "recorded")
PG_LIB = os.path.join(studio_dir(), "scenarios")  # the Studio's library (read only)
esc = lambda s: html.escape(s, quote=True)

# failure mode of each kind: the colour of a card and the column of the hero matrix
MODES = {
    "int": ("Interference", "--amber", ["jamming", "lunar-jamming", "spectrum", "tracking-loop", "impairment-eval", "conflict-resilience"]),
    "spf": ("Spoofing", "--magenta", ["spoof", "spoof-detect", "lunar-attack-surface"]),
    "tim": ("Timing", "--lime", ["clock", "sweep", "slot-timing", "telecom-timing", "timetransfer", "quantum-time-transfer", "lunar-time-offset", "lunar-time-budget", "quantum-trade", "hybrid"]),
    "orb": ("Geometry", "--cyan", ["orbit", "ephemeris", "passes", "constellation-design", "moonlight-service-volume", "earth-gnss-lunar", "cislunar-observability",
                                   "cislunar-arc-recovery", "mars-pnt", "body-pnt", "solar-system", "link-budget", "space-weather", "lunar-vlbi", "lunar-beacon", "pvt",
                                   "leo-signal", "leo-pass", "leo-navmsg", "leo-pvt", "leo-ppp", "ntn-positioning", "leo-pnt-chain"]),
    "itg": ("Integrity", "--coral", ["integrity", "araim-reference-check", "lunar-integrity", "gnss-sim"]),
    "nav": ("Coasting", "--tim", ["gnss-ins", "ins-trn-coast", "inertial", "gravity-map", "terrain-nav", "terrain-slam", "combined-altpnt", "sweep-nd"]),
}
KIND_MODE = {k: m for m, (_, _, ks) in MODES.items() for k in ks}
# campaigns take the mode of what they chain
FILE_MODE = {"campaign-jam-spoof-holdover-integrity": "int", "campaign-sweep-jammer-power": "int", "campaign-shared-jammer-sea-road": "int",
             "campaign-monte-carlo-clock-holdover": "tim", "campaign-spectrum-holdover-integrity": "int",
             "leo-resilience-gnss-jammed-leo-carries": "int", "leo-resilience-spoof-monitors": "spf", "leo-timing-utc": "tim",
             "leo-vertical-critical-infrastructure-timing": "tim", "leo-vertical-5g-network-timing": "tim",
             "leo-vertical-autonomous-vehicle": "nav", "leo-vertical-rail-maritime": "nav"}

DESC = {
    "jamming-demo": "A ground jammer near a receiver: every satellite's signal-to-noise and lock state as it comes on.",
    "spoof-detect": "A fused spoof detector: consistency across satellites, receiver gain and signal shape, and which monitor fires.",
    "spoof-attack": "An active time-spoofing attack: does each clock's integrity monitor catch the push?",
    "spoof-meaconing": "A delayed rebroadcast of real signals drags the time solution; the detector has its own bound.",
    "conflict-resilience": "Layered navigation under rising threat intensity: the chance of losing every layer at once.",
    "combined-altpnt": "Gravity, magnetic and terrain matching fused into one navigator that needs no satellites.",
    "gps-denied-gravity-nav": "An hour without satellites, navigating on a gravity map with a quantum sensor.",
    "terrain-nav": "Terrain-referenced navigation: measured heights matched to a map while satellites are denied.",
    "orbit-gnss-challenged": "Navigation-satellite availability for a spacecraft inside the constellation shell, from real geometry.",
    "orbit-molniya": "The same question for a highly eccentric orbit that climbs above the constellation.",
    "orbit-sgp4-gps": "Availability from the real GPS constellation, propagated from two-line element sets.",
    "orbit-multignss": "GPS and Galileo together: how much a second constellation adds overhead.",
    "slot-timing-ocxo-leo": "How long an oven-controlled crystal clock on a small low-orbit satellite stays inside its routing slot.",
    "ephemeris": "Where the satellite is right now, and the ground track it draws.",
    "passes": "Ground-station pass prediction for the operations team.",
    "link-budget": "A one-way link budget, from transmitter power to receiver margin.",
    "space-weather": "The space-weather environment a mission flies through.",
    "moonlight-service-volume": "Coverage and protection levels of a lunar navigation service across its service volume.",
    "lunanet-araim": "Integrity for a landed receiver in the lunar south-pole region.",
    "earth-gnss-lunar": "How much of Earth's navigation signal still reaches a receiver at the Moon.",
    "lunar-jamming": "Jamming a lunar surface user, satellite by satellite.",
    "lunar-time-offset": "The relativistic rate between Earth and Moon clocks, against the published band.",
    "lunar-time-budget": "The end-to-end time-error budget for coordinated lunar time, clock class by clock class.",
    "lunar-vlbi": "Two Earth radio telescopes observing a beacon on the lunar near side.",
    "cislunar-observability": "Can a small constellation near the Moon be tracked, and over how long an arc?",
    "lunar-attack-surface": "The signal-security attack surface for navigation on the lunar surface.",
    "lunar-beacon": "Surface beacons that augment a lunar orbital navigation service.",
    "mars-pnt-lmo": "A low Mars orbiter navigating against a relay constellation.",
    "mars-pnt-surface": "A lander on the rotating surface of Mars.",
    "mars-pnt-transfer": "A spacecraft arriving at Mars on a high, eccentric arc.",
    "small-uas-jammed-nav": "A small drone loses satellites to a jammer: how long each inertial unit keeps it in budget.",
    "gnss-ins": "Satellite navigation blended with a strapdown inertial navigator, then coasting through an outage.",
    "ins-trn-coast": "Inertial coasting with terrain fixes: position error against coast time.",
    "integrity-raim": "Receiver integrity-monitoring availability over a GPS-like constellation.",
    "araim-gps-galileo": "Advanced integrity monitoring with GPS and Galileo together.",
    "araim-reference-check": "The engine's protection levels against the working group's own worked examples.",
    "gnss-sim-raim": "Pseudoranges simulated with ionosphere and troposphere, then checked for integrity.",
    "maritime-strait-jamming": "A shore or ship jammer across a strait: jammer-to-signal ratio and satellites kept.",
    "maritime-spoof-position-push": "A spoofer drags a ship's reported position; which detectors catch it, and when.",
    "maritime-port-approach-coast": "Satellites jammed on the port approach: how long each inertial grade can coast.",
    "imu-deadreckoning": "Dead reckoning through an outage: a cold-atom accelerometer against a navigation-grade unit.",
    "clock-holdover": "The reference case: a clock synchronised to satellites, then free-running when they are denied.",
    "automotive-urban-canyon": "A car through an urban canyon, an underpass and a roadside jammer, against a lane budget.",
    "rail-tunnel-coast": "A train in a tunnel: seconds of inertial coast before it can no longer tell its track.",
    "terrain-slam": "Recursive terrain-referenced navigation that keeps bounding the inertial drift.",
    "clock-holdover-labsr": "A laboratory strontium optical clock against a chip-scale atomic clock.",
    "clock-ensemble": "A Monte Carlo ensemble of holdover runs: the spread, not one lucky draw.",
    "sweep-clock-stability": "How holdover scales with clock stability, as a trade study.",
    "timetransfer": "Optical time transfer between satellites against classical two-way radio transfer.",
    "telecom-prtc-holdover-24h": "A rubidium timing reference loses satellites for a day: time error against the telecom masks.",
    "telecom-tie-ingest": "Bring your own time-error series and score it against the telecom masks.",
    "quantum-time-transfer": "A trusted quantum timing chain: transfer, secure dissemination and anomaly detection.",
    "quantum-trade": "Quantum against classical navigation, with measured clock-stability data ingested.",
    "hybrid-pnt": "An all-quantum navigation suite against an all-classical one, with satellites denied.",
    "pvt-abmf": "Single-point positioning from real measurements logged at a surveyed ground station.",
    "impairment-eval": "A testbed that scores radio-impairment detectors on labelled cases.",
    "sweep-nd-inertial": "A many-axis parameter sweep over an inertial scenario.",
}
NEXT_DESC = {
    "campaign-jam-spoof-holdover-integrity": "One chained run: nominal, jamming, spoofing, holdover, an integrity alarm and recovery.",
    "l-band-waterfall-jamming": "The L-band spectrum as a waterfall under several jammers, band by band.",
    "campaign-sweep-jammer-power": "The same receiver while the jammer's power is swept: where tracking gives out.",
    "constellation-multi-gnss-coverage": "All four global systems together, scored over the whole Earth for a sidereal day.",
    "leo-pnt-mega-shell": "A low-orbit navigation mega-constellation in four shells, scored as a coverage and geometry map.",
    "lunar-relay-constellation": "Relays in frozen elliptical orbits that linger over the lunar south pole, scored over the whole Moon.",
    "mars-orbit-pnt": "A low Mars orbiter with a relay constellation and a deep-space range from Earth.",
    "europa-surface-pnt": "A lander on Europa, with relays and an Earth link that rises and sets once a Europan day.",
    "solar-system-tour": "Every body the engine knows at one epoch, from the Sun to Titan.",
    "campaign-shared-jammer-sea-road": "One jammer, a ship and a car: the same transmitter, two very different outcomes.",
    "campaign-monte-carlo-clock-holdover": "A seeded Monte Carlo ensemble of a free-running clock, against its closed form.",
    "campaign-spectrum-holdover-integrity": "Jamming read from the spectrum, clock holdover, then integrity on Galileo alone.",
    "leo-pass-vs-gnss-cn0": "One low-orbit navigation pass beside the Galileo satellites in view: signal strength through the pass.",
    "leo-doppler-positioning": "A position fix from the Doppler shift of low-orbit satellites alone.",
    "meo-leo-fused-pvt": "GPS and Galileo fused with a low-orbit navigation layer: what the extra layer does to the fix.",
    "leo-ppp-convergence": "Precise point positioning: how much sooner it converges once low-orbit satellites are added.",
    "leo-pnt-end-to-end": "One low-orbit navigation system end to end: signal design, pass link budget, navigation message and the fix.",
    "leo-resilience-gnss-jammed-leo-carries": "GNSS is jammed and a low-orbit layer carries the user, with integrity maintained.",
    "leo-resilience-spoof-monitors": "Low-orbit signals under spoofing: which monitor sees which spoofer.",
    "leo-timing-utc": "Time transfer to Coordinated Universal Time from low-orbit satellites, against signal strength and the receiver's oscillator.",
    "leo-vertical-critical-infrastructure-timing": "Critical-infrastructure timing through holdover, with low-orbit time as the second source.",
    "ntn-5g-positioning": "Positioning from a 5G non-terrestrial network downlink in the mobile-satellite S band.",
    "leo-vertical-5g-network-timing": "Base-station timing for a mobile network: holdover, and time from low orbit.",
    "leo-vertical-autonomous-vehicle": "An autonomous vehicle through an urban canyon, with a low-orbit layer added.",
    "leo-vertical-rail-maritime": "The bundled rail and maritime cases again, with a low-orbit layer added.",
    "leo-navmsg-fit-interval-trade": "A low-orbit navigation message: range error against how long one message is fitted for.",
    "leo-band-trade": "A band trade for low-orbit navigation signals, from ultra-high frequency to a wide C band.",
}
NEW = {"maritime-strait-jamming", "maritime-spoof-position-push", "maritime-port-approach-coast", "automotive-urban-canyon", "rail-tunnel-coast"}

T_FREE = '<div class="tier free"><span>Free core</span><p>Run every scenario file in this sector in {{STUDIO}} or from any install, read the ledger and export reports.%s</p></div>'
LIC = " Commercial licence of the open core: closed or proprietary integration without AGPL obligations."
T_PRO = '<div class="tier pro"><span>Pro adds</span><p>%s</p><a class="tier-mail" href="mailto:contact@ashforde.org?subject=Kshana%%20Pro%%20evaluation">Request a Pro evaluation</a></div>'
T_CUS = '<div class="tier custom"><span>Custom study adds</span><p>%s</p><a class="tier-mail" href="mailto:contact@ashforde.org?subject=Kshana%%20study%%20request">Request a study</a></div>'
P_PACK = "Pro: audit-grade, reproducible evidence packs for a review."
P_TWIN = "Pro: clock digital twins calibrated to your device&#x27;s published Allan budget."
P_TRADE = "Pro: architecture trade studies ranked on a figure of merit."
C_EXPORT = "Custom: export-controlled resilience work under the appropriate clearance and NDA (non-disclosure agreement)."
C_HW = "Custom: sensor and resilience models calibrated to your hardware."
C_TRAIN = "Custom study: training and consulting on quantum and classical PNT performance analysis."

VIZ = lambda cid, h=250, w=560: f'<figure class="viz" data-viz="{cid}" data-w="{w}" data-h="{h}"></figure>'


def grades(file, cid):
    return (f'<figure class="mx-fig" data-mx="grades" data-mx-file="{file}" data-viz="{cid}"><div class="viz-plot mx-plot" role="img" aria-label="Free-inertial coast time by IMU grade, {file}">'
            f'<p class="mx-wait">Drawn from the recorded run of <code>{file}.toml</code> when the page script loads.</p></div>'
            f'<figcaption class="prov"><span class="prov-src">Engine v{{{{FIG sec-maritime engine 0}}}} · <code>{file}.toml</code> · deterministic, no seed</span>'
            f'<a class="prov-open" href="studio/index.html?scenario={file}&amp;tab=overview">{{{{STUDIO_OPEN}}}}<svg aria-hidden="true"><use href="#i-ext"/></svg></a>'
            f'<span class="prov-tools"><a class="prov-x" href="studio/index.html?scenario={file}&amp;tab=exports" title="Open the run\'s exports in {{{{STUDIO}}}}: reproducible HTML report, JSON, CSV">Report</a>'
            f'<button class="prov-x" type="button" data-mx-csv="{cid}" title="Download the plotted data as comma-separated values"><abbr title="comma-separated values">CSV</abbr></button>'
            f'<button class="prov-x" type="button" data-dl-svg title="Download this chart as scalable vector graphics"><abbr title="scalable vector graphics">SVG</abbr></button></span></figcaption></figure>')


SECTORS = [
    dict(id="defence", n="Defence and security", c="--amber",
         q="How long do our own navigation and timing stay trustworthy under jamming and spoofing, and which layers keep us in spec?",
         threat="Jamming, spoofing, meaconing, GNSS denial",
         fom=["Effective C/N₀ (carrier-to-noise density) and lock state per satellite under a jammer", "Spoof verdict and which detection layer fired",
              "Total-loss probability against threat intensity for layered PNT", "Bounded alternative-PNT position error through the outage"],
         std=["RAIM / ARAIM", "IS-GPS-200", "IEEE 1139"], chart_l="Every satellite&#x27;s C/N₀ under a jammer", fig=VIZ("wf-jamming", 0),
         files=["jamming-demo", "spoof-detect", "spoof-attack", "spoof-meaconing", "conflict-resilience", "combined-altpnt", "gps-denied-gravity-nav", "terrain-nav"],
         next=["campaign-jam-spoof-holdover-integrity", "l-band-waterfall-jamming", "campaign-sweep-jammer-power",
               "leo-resilience-gnss-jammed-leo-carries", "leo-resilience-spoof-monitors"],
         tiers=[T_FREE % "", T_PRO % P_PACK, T_CUS % C_EXPORT],
         note="Defensive use only: protecting your own navigation and timing, resilience assessment and holdover planning. Kshana does not model targeting, interception or guidance. It drives no radio hardware and transmits nothing: its signal-level output is baseband IQ sample files for software receivers."),
    dict(id="space", n="Space: Earth orbit", c="--cyan",
         q="Does my spacecraft see enough navigation satellites, how long does its clock hold time between fixes, and what would a low-orbit navigation layer add?",
         threat="GNSS outage above or inside the constellation, clock drift between fixes, sparse geometry",
         fom=["GNSS availability and dilution of precision from real geometry", "Clock time error through each gap in coverage",
              "Satellites in view and PDOP (position dilution of precision) over the whole Earth, for any constellation you design"],
         std=["TLE / SGP4", "SP3-c / d", "CCSDS OEM / OMM"], chart_l="GNSS availability for a spacecraft inside the shell", fig=VIZ("sec-space"),
         files=["orbit-gnss-challenged", "orbit-molniya", "orbit-sgp4-gps", "orbit-multignss", "slot-timing-ocxo-leo", "ephemeris", "passes", "link-budget", "space-weather"],
         next=["constellation-multi-gnss-coverage", "leo-pnt-mega-shell", "leo-pass-vs-gnss-cn0", "leo-doppler-positioning", "meo-leo-fused-pvt",
               "leo-ppp-convergence", "leo-pnt-end-to-end"], extra="leo",
         tiers=[T_FREE % "", T_PRO % f"{P_TRADE} {P_TWIN}"],
         build="A geostationary user: set the user orbit in an orbit-kind scenario, the way orbit-molniya sets a highly eccentric one."),
    dict(id="lunar", n="Lunar and deep space", c="--magenta",
         q="Will a lander, a rover or an orbiter at the Moon, at Mars or further out get a trustworthy position and time, and what does the local time scale add to the error budget?",
         threat="Weak signals at lunar distance, sparse relay geometry, light-time delay, lunar time offset, surface jamming",
         fom=["Lunar service-volume coverage and protection levels", "Earth-GNSS reception at lunar distance",
              "Lunar time rate against the published band, and the end-to-end time-error budget", "Relay-navigation error in Mars orbit, on the surface and in transfer"],
         std=["CCSDS 401 / DSN", "LunaNet", "IERS conventions"], chart_l="Lunar south-pole integrity, from a real run", fig=VIZ("sec-lunar"),
         files=["moonlight-service-volume", "lunanet-araim", "earth-gnss-lunar", "lunar-jamming", "lunar-time-offset", "lunar-time-budget", "lunar-vlbi",
                "cislunar-observability", "lunar-attack-surface", "lunar-beacon", "mars-pnt-lmo", "mars-pnt-surface", "mars-pnt-transfer"],
         next=["lunar-relay-constellation", "mars-orbit-pnt", "europa-surface-pnt", "solar-system-tour"],
         tiers=[T_FREE % "", T_PRO % f"{P_TWIN} {P_TRADE}"]),
    dict(id="aviation", n="Aviation and drones", c="--tim",
         q="After a jammer takes GNSS away, how long does my UAS (uncrewed aircraft system) stay inside its position budget, and is integrity available on my route?",
         threat="GNSS jamming, loss of integrity",
         fom=["Seconds inside the position budget, flight-controller MEMS (micro-electromechanical systems) against a tactical-grade IMU (inertial measurement unit)",
              "Horizontal and vertical protection levels against the alert limits", "Coast error against coast duration"],
         std=["RAIM / ARAIM", "SBAS / DO-229E", "IS-GPS-200"], chart_l="Dual-constellation integrity, from a real run", fig=VIZ("sec-aviation"),
         files=["small-uas-jammed-nav", "jamming-demo", "gnss-ins", "ins-trn-coast", "integrity-raim", "araim-gps-galileo", "araim-reference-check", "gnss-sim-raim"],
         next=["campaign-sweep-jammer-power"],
         tiers=[T_FREE % "", T_PRO % P_PACK, T_CUS % C_HW]),
    dict(id="maritime", n="Maritime", c="--cyan",
         q="How far across the water does a jammer take GNSS away, would a position push be caught, and how long can a ship coast into port?",
         threat="Jamming and spoofing near shore, GNSS loss in a harbour approach",
         fom=["Mean J/S (jammer-to-signal ratio) and satellites tracked under a shore jammer", "Spoof verdict and which detection layers fired for a position push",
              "Seconds of free-inertial coast before the harbour position budgets"],
         std=["RAIM / ARAIM", "IEEE 1139"], chart_l="Free-inertial coast by IMU grade", fig=grades("maritime-port-approach-coast", "mx-port"),
         files=["maritime-strait-jamming", "maritime-spoof-position-push", "maritime-port-approach-coast", "spoof-detect", "imu-deadreckoning", "clock-holdover"],
         next=["campaign-shared-jammer-sea-road", "leo-vertical-rail-maritime"], extra="searoad",
         tiers=[T_FREE % "", T_PRO % P_PACK, T_CUS % C_HW],
         build='The jamming footprint under <a href="#shows">What a run shows</a> maps maritime-strait-jamming over a grid of ship positions, one engine run per cell.'),
    dict(id="automotive", n="Road and automotive", c="--lime",
         q="How far does a car drift under an overpass or past a roadside jammer, and does it stay inside its lane-level budget?",
         threat="GNSS loss in urban canyons and underpasses, roadside jamming",
         fom=["Position error through the underpass and the jammer, automotive MEMS against a tactical-grade IMU, against a lane-level budget",
              "Protection level against an alert limit you set", "Lock state under the jammer"],
         std=["RAIM / ARAIM", "SBAS / DO-229E"], chart_l="Through the urban canyon, from a real run", fig=VIZ("sec-automotive"),
         files=["automotive-urban-canyon", "gnss-ins", "ins-trn-coast", "integrity-raim", "jamming-demo"],
         next=["campaign-shared-jammer-sea-road", "leo-vertical-autonomous-vehicle"], extra="searoad",
         tiers=[T_FREE % LIC, T_CUS % C_HW]),
    dict(id="rail", n="Rail", c="--tim",
         q="How many seconds into a tunnel can a train still tell which track it is on?",
         threat="GNSS loss in tunnels, cuttings and covered stations",
         fom=["Seconds of free-inertial coast before the track-discrimination budget and a wider one", "The error term that dominates at each crossing",
              "How a terrain or map fix bounds the coast"],
         std=["RAIM / ARAIM"], chart_l="Seconds into a tunnel, by IMU grade", fig=grades("rail-tunnel-coast", "mx-rail"),
         files=["rail-tunnel-coast", "ins-trn-coast", "terrain-slam", "gnss-ins", "integrity-raim"], next=["leo-vertical-rail-maritime"],
         tiers=[T_FREE % "", T_PRO % P_PACK, T_CUS % C_HW]),
    dict(id="cni", n="Critical infrastructure and timing", c="--lime",
         q="If GNSS time is lost or spoofed, how long do a power grid, a trading venue or a data centre keep traceable time, and which clock carries them?",
         threat="Loss of the GNSS time reference, time spoofing, clock drift in holdover",
         fom=["Clock time error through holdover against the guard you set", "Holdover time by oscillator class, from chip-scale to optical",
              "Monte Carlo ensemble spread", "Time-spoof detection by the clock&#x27;s integrity monitor"],
         std=["IEEE 1139", "ITU-T G.8272"], chart_l="A Monte Carlo clock ensemble, from a real run", fig=VIZ("sec-cni"),
         files=["clock-holdover", "clock-holdover-labsr", "clock-ensemble", "sweep-clock-stability", "spoof-attack", "spoof-meaconing", "timetransfer"],
         next=["campaign-monte-carlo-clock-holdover", "campaign-spectrum-holdover-integrity", "leo-timing-utc", "leo-vertical-critical-infrastructure-timing"],
         tiers=[T_FREE % "", T_PRO % f"{P_TWIN} {P_PACK}"],
         build="Power grid and finance: your sector&#x27;s time-accuracy limit goes in as threshold_ns."),
    dict(id="telecom", n="Telecom and 5G", c="--amber",
         q="If the GNSS time reference is lost, does my network clock stay inside the ITU-T masks, and for how long?",
         threat="Loss of the GNSS time reference at a primary reference time clock, time spoofing",
         fom=["MTIE (maximum time interval error) and TDEV (time deviation) against ITU-T masks, with a pass, a fail and a margin per mask",
              "Timing error 95th percentile, holdover time and availability", "Time-spoof detection by the clock&#x27;s integrity monitor"],
         std=["ITU-T G.8272", "IEEE 1139"], chart_l="Time error against the ITU-T masks", fig=VIZ("sec-telecom"),
         files=["telecom-prtc-holdover-24h", "telecom-tie-ingest", "clock-holdover", "clock-ensemble", "quantum-time-transfer", "timetransfer", "spoof-attack"],
         next=["ntn-5g-positioning", "leo-vertical-5g-network-timing"],
         tiers=[T_FREE % "", T_PRO % f"{P_TWIN} {P_PACK}"],
         note="The engine carries ITU-T G.8272, G.8272.1, G.8273.2 and G.8271.1 limits, including PRTC-A, PRTC-B and ePRTC (primary reference time clock, classes A and B, and enhanced)."),
    dict(id="research", n="Science and research", c="--magenta",
         q="Can I reproduce, extend and cite a PNT result, down to the seed?",
         threat="Any of the above, as an experiment",
         fom=["Every figure with its unit and provenance", "Deterministic from scenario, seed and engine version", "A citable DOI (Digital Object Identifier) for every release"],
         std=["RINEX 3", "SP3-c / d", "IGRF-14"], chart_l="Holdover against clock stability, a trade study", fig=VIZ("sec-research"),
         files=["sweep-clock-stability", "quantum-trade", "hybrid-pnt", "pvt-abmf", "cislunar-observability", "lunar-vlbi", "impairment-eval", "sweep-nd-inertial"],
         next=["leo-navmsg-fit-interval-trade", "leo-band-trade"],
         tiers=[T_FREE % LIC, T_CUS % C_TRAIN]),
]


def kind_of(path):
    t = tomllib.loads(open(path, encoding="utf-8").read())
    return t.get("kind", "clock")


def card(f, kind, recorded):
    mode = FILE_MODE.get(f) or KIND_MODE.get(kind, "orb")
    new = '<b class="sc-new">New</b>' if f in NEW else ""
    tab = "overview"
    href = f"studio/index.html?scenario={f}&amp;tab={tab}"
    how = "Recorded run" if recorded else "Live run"
    return (f'<a class="sc" href="{href}" data-mode="{mode}" style="--c:var({MODES[mode][1]})">'
            f'<span class="sc-k"><i></i>{esc(kind)}{new}</span><code class="sc-f">{esc(f)}</code>'
            f'<span class="sc-d">{esc(DESC[f])}</span>'
            f'<span class="sc-go"><span>{how}</span>{{{{STUDIO_OPEN}}}}<svg aria-hidden="true"><use href="#i-ext"/></svg></span></a>')


def next_card(f, kind):
    mode = FILE_MODE.get(f) or KIND_MODE.get(kind, "orb")
    return (f'<article class="sc sc-next" data-run="{f}" data-mode="{mode}" style="--c:var({MODES[mode][1]})">'
            f'<span class="sc-k"><i></i>{esc(kind)}<b class="sc-new">New</b></span><code class="sc-f">{esc(f)}</code>'
            f'<span class="sc-d">{esc(NEXT_DESC[f])}</span><span class="sc-fig" data-fig="{f}"></span>'
            f'<span class="sc-acts" data-acts="{f}"></span></article>')


LEO = ('<article class="sc-wide sc-leo" style="--c:var(--cyan)">'
       '<div class="sc-k"><i></i>LEO-PNT</div>'
       '<h4>A low-orbit navigation layer, scored before it flies.</h4>'
       '<p>LEO-PNT (low Earth orbit positioning, navigation and timing): thousands of satellites a few hundred kilometres up, '
       'close enough to be heard indoors and moving fast enough to fix a position from Doppler. The engine designs and scores the constellation '
       '(the <code>leo-pnt-mega-shell</code> run, in the orbit console below) and follows the signal from its design to the fix, for any system: '
       'the cards around this one are recorded runs of those kinds.</p>'
       '<div class="leo-figs" data-leo></div>'
       '<ul class="leo-soon" aria-label="The engine\'s low-orbit navigation kinds">'
       '<li><b>The kinds behind it</b></li><li><code>leo-signal</code> signal design</li><li><code>leo-pass</code> pass link budget</li>'
       '<li><code>leo-navmsg</code> navigation message</li><li><code>leo-pvt</code> position and time</li><li><code>leo-ppp</code> precise positioning</li>'
       '<li><code>ntn-positioning</code> 5G from orbit</li><li><code>leo-pnt-chain</code> end to end</li></ul>'
       '<a class="sc-jump" href="#orbits">Open the orbit console <svg aria-hidden="true"><use href="#i-arrow"/></svg></a></article>')
SEAROAD = ('<article class="sc-wide sc-searoad" data-searoad style="--c:var(--amber)">'
           '<div class="sc-k"><i></i>Across sectors · campaign</div>'
           '<h4>One jammer, a ship and a car.</h4>'
           '<p>A campaign binds one jammer, its power and its position, into two ordinary jamming runs: a ship in a strait and a car on a coast road. '
           'Same transmitter, two very different outcomes, in one reproducible run.</p>'
           '<div class="sr-bars" data-sr-bars role="img" aria-label="Availability, mean jammer-to-signal ratio and fewest satellites tracked, ship against car"></div>'
           '<p class="prov" data-prov-run="campaign-shared-jammer-sea-road"></p></article>')
EXTRA = {"leo": LEO, "searoad": SEAROAD}


def build():
    rec = recorded_names(PG_REC)
    assert rec, "the Studio has no recordings"
    tabs, panels = [], []
    for i, s in enumerate(SECTORS):
        sid = s["id"]
        n = len(s["files"]) + len(s["next"])
        tabs.append(f'<button role="tab" type="button" id="st-{sid}" aria-controls="{sid}" aria-selected="{"true" if i == 0 else "false"}" tabindex="{0 if i == 0 else -1}" '
                    f'data-sector="{sid}" data-n="{n}" style="--c:var({s["c"]})"><i></i>{esc(s["n"])}</button>')
        cards = []
        for f in s["files"]:
            p = os.path.join(REPO, "scenarios", f + ".toml")
            assert os.path.exists(p), f"{sid}: {f} is not a scenario file of the engine"
            assert os.path.exists(os.path.join(PG_LIB, f + ".toml")), f"{sid}: {f} is not in the Studio's library"
            assert f in DESC, f"{f}: no card text"
            cards.append(card(f, kind_of(p), f in rec))
        for f in s["next"]:
            p = os.path.join(REPO, "scenarios", f + ".toml")
            assert os.path.exists(p), f"{sid}: {f} is not a scenario file of the engine"
            assert os.path.exists(os.path.join(PG_LIB, f + ".toml")), f"{sid}: {f} is not in the Studio's library, so its card cannot open in the Studio"
            assert f in NEXT_DESC, f"{f}: no card text"
            cards.append(next_card(f, kind_of(p)))
        extra = EXTRA.get(s.get("extra"), "")
        std = "".join(f"<span>{esc(x)}</span>" for x in s["std"])
        fom = "".join(f"<li>{x}</li>" for x in s["fom"])
        note = f'<p class="m-note">{s["note"]}</p>' if s.get("note") else ""
        bld = f'<p class="m-build">{s["build"]}</p>' if s.get("build") else ""
        panels.append(
            f'<div class="mission" role="tabpanel" id="{sid}" aria-labelledby="st-{sid}" style="--mc:var({s["c"]})"{"" if i == 0 else " hidden"}>\n'
            f'  <div class="m-head">\n    <h3 class="m-q">{esc(s["q"])}</h3>\n'
            f'    <div class="m-meta"><div class="m-threat"><span>Threat or failure</span><b>{esc(s["threat"])}</b></div><div class="m-std" aria-label="Standards involved">{std}</div></div>\n  </div>\n'
            f'  <div class="m-grid">\n'
            f'    <div class="sc-chart"><p class="m-chart-l"><i></i>{s["chart_l"]}</p>{s["fig"]}</div>\n'
            f'    {"".join(cards)}\n    {extra}\n  </div>\n'
            f'  <div class="m-foot">\n    <div class="m-block"><h4>Figures of merit</h4><ul>{fom}</ul>{bld}</div>\n'
            f'    <div class="tiers">{"".join(s["tiers"])}<a class="tier-link" href="editions.html">Compare the editions <svg width="14" height="14" aria-hidden="true"><use href="#i-arrow"/></svg></a></div>\n'
            f'  </div>\n  {note}\n</div>')
    block = (f'<div class="sectors" role="tablist" aria-label="Sectors" aria-orientation="vertical">{"".join(tabs)}</div>\n'
             f'<div class="m-panels">\n' + "\n".join(panels) + '\n</div>')
    return block


def main():
    t = open(PAGE, encoding="utf-8").read()
    a, b = "<!-- SECTORS:BEGIN -->", "<!-- SECTORS:END -->"
    assert a in t and b in t, "missions.html has no SECTORS markers"
    new = t[: t.index(a) + len(a)] + "\n" + build() + "\n" + t[t.index(b):]
    if "--check" in sys.argv:
        sys.exit(0 if new == t else "missions.html sector console is stale: run src/tools/gen_missions_page.py")
    open(PAGE, "w", encoding="utf-8").write(new)
    print(f"wrote {len(SECTORS)} sectors, {sum(len(s['files']) + len(s['next']) for s in SECTORS)} cards")


if __name__ == "__main__":
    main()
