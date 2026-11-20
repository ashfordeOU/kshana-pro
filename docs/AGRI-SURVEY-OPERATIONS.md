# Agriculture, construction and survey operations

`kshana-pro agri` (module `kshana_pro::agri`): tools for machines that steer or grade by GNSS
(tractors, sprayers, forestry harvesters, construction machine control) and for the dealers,
contractors, surveyors and agencies who check their work. Research 2026-11-20 gaps, epic E35:
item B3501 in full, and the locally running part of B3502.

**Evidence label: MODELLED.** Thresholds and weights are named parameters with stated
demonstration defaults; nothing has been checked against a machine maker's own figures. The
drift, resilience, realignment and photo methods are shown on SYNTHETIC logs and photos. The
space-weather backtest runs on the real, published GFZ Kp series for May 2024 and compares the
advisory with the dates of reported farm outages only (qualitative, not receiver outcomes).
No output says that a machine, a receiver or a claim meets a standard or a rule.

**Out of scope by design:** locating an interference source, steering or controlling a
machine, and fusing GNSS with other sensors.

## Commands

| Command | What it does | Licence feature | Tier |
|---|---|---|---|
| `convert` | Guidance lines between the neutral form, GeoJSON, KML, ISOXML (subset) and CSV; or an A-B line from A plus B or A plus heading | none | free |
| `drift` | Pass-to-pass drift, within-pass drift, overlap and miss area from a log against an A-B line | `pro-agri-guidance` | Pro |
| `realign` | Shift stored guidance lines onto the rows actually planted, with uncertainty | `pro-agri-guidance` | Pro |
| `field` | Swath lines over a field boundary, working time, and a working window from an advisory | `pro-agri-guidance` | Pro |
| `resilience` | Autosteer GNSS-loss resilience score and indicative outage diagnosis | `pro-agri-resilience` | Pro |
| `advise` | Scintillation and PPP-versus-RTK advisory per field and hour | `pro-agri-advisor` | Pro |
| `translate` | Plain-language space weather for growers | none | free |
| `photo` | Consistency checks of geotagged photos against a parcel | `pro-agri-photo` | Enterprise |
| `sample` | SYNTHETIC log, line, field, profile or photo to try the commands | none | free |
| `verify` | Check a signed output directory | none | free |

`drift`, `resilience` and `photo` with `--out <dir> --signer <name> --key-file <file>` write
`report.json` and `report.md` into a directory signed with `signed_dossier` (Ed25519, SHA-256
per file), which is never overwritten; `kshana-pro agri verify <dir> --trust <key>` checks it.

```sh
kshana-pro agri sample log --out log.csv --drift 4:0.06 --outage 245:30:interference
kshana-pro agri sample line --out line.json
kshana-pro agri drift --log log.csv --line line.json --width 12
kshana-pro agri resilience --log log.csv --kp agri/data/gfz-kp-2024-05-07-to-14.txt
kshana-pro agri advise --at 49.9,-97.1 --kp agri/data/gfz-kp-2024-05-07-to-14.txt \
    --from 2024-05-09T00:00Z --hours 96 --base 49.95,-97.2 --out advisory.json
kshana-pro agri field --boundary agri/examples/field.example.geojson --width 12 \
    --line line.json --advisory advisory.json --to isoxml --out plan.xml
```

## Inputs

**Machine logs.** A neutral CSV with a header (`t_s`, `lat`, `lon` required; `h`, `fix`,
`sats`, `hdop`, `cn0`, `corr_age`, `steer`, `xte` optional; aliases accepted) or NMEA 0183
`GGA` sentences (checksum checked, midnight carried). Fix names: `rtk_fixed`, `rtk_float`,
`ppp`, `dgnss`, `sbas`, `autonomous`, `dr`, `none`, or the NMEA quality digit. In-cab
proprietary log formats with no public description are not read; export to CSV or NMEA.

**Guidance lines.** Neutral form `kshana-guidance-lines/1` (JSON), GeoJSON (`LineString`,
`MultiLineString`, features), KML (`LineString` coordinates), CSV (`name,lat,lon` rows) and a
subset of the ISO 11783-10 task data file: guidance patterns `GPN` (attribute `C` type 1 A-B,
2 A+heading with heading `G`, 3 curve) carrying a guidance line string `LSG A="5"` of points
`PNT` (`C` latitude, `D` longitude). Pivot and spiral patterns are refused with a message. The
ISOXML subset was written from the published element and attribute structure and has not been
imported into any terminal or farm-management system: check the result in yours.

**Field boundaries.** A GeoJSON polygon (first ring) or `lat,lon` rows.

**Kp.** The GFZ file `Kp_ap_Ap_SN_F107_since_1932.txt` (Matzka et al. 2021,
doi:10.1029/2020SW002641; CC BY 4.0; it also carries F10.7), NOAA SWPC
`noaa-planetary-k-index.json` (current object form and the older array form) and the NOAA SWPC
3-day forecast text ("NOAA Kp index breakdown"; storm annotations and a year boundary handled).

**Resilience profile** (`kshana-agri-profile/1`, TOML; `kshana-pro agri sample profile`): the
machine class, the fix the work needs, the cross-track tolerance and every threshold and
weight below, with a `source` line saying where the values came from. `--class forestry` and
`--class construction` set other demonstration defaults (0.5 m with DGNSS; 0.03 m).

## Methods

### Pass-to-pass drift (`drift`)

Each epoch is put in the A-B line's frame (along `s`, across `x`, left positive). With swath
`w`, its pass is `k = round(x/w)` and its deviation `x - k w`. A pass is a run on one `k`, one
direction, at least 30 m, by default only with steering engaged. Per pass: a straight-line fit
of deviation against time (start, end, rate in m/min: the pass that "starts correctly then
slowly starts missing"). Per adjacent pair, sampled every metre over the shared along-track
range: spacing error `e = dev(k+1) - dev(k)`; RMS, 95th percentile, maximum; miss strip
`max(0, w + e - W)` and overlap `max(0, W - w - e)` for implement width `W`, integrated to
areas. Pairs more than 15 minutes apart (the usual pass-to-pass convention; `--max-gap-min`)
are reported as repeatability. Tolerance default 0.076 m (3 in), an operator setting. The
method measures what the log says the machine did: a receiver that reports a consistently
wrong position shows no drift here.

### Guidance-line realignment (`realign`)

After an outage or a change of correction source, stored lines can sit off the rows actually
planted. Drive along existing rows with a good fix (`--log`) and/or re-measure markers whose
stored position is known (`--marker stored_lat,stored_lon:now_lat,now_lon`). Each log epoch
within `--capture` (1 m) of a stored line gives a signed cross-track offset; per line the mean
offset is one observation along the line's normal, weighted by its scatter over an effective
sample count (duration over a 10 s correlation time); a marker gives an east and a north
observation (2 cm standard uncertainty assumed). The weighted least-squares horizontal shift
is applied to every line; its standard uncertainty is GUM type A from the normal equations.
With only parallel lines, the along-line component is not observable: only the cross-track
shift is applied and the output says so. Rotation is not estimated; a line whose residual
exceeds the tolerance is flagged so a rotation or distortion is not hidden.

### Field planner (`field`)

The boundary is put in a local plane at its first vertex and rotated so the working heading
(`--heading`, or the heading of `--line`) is "up". Centre lines at `w/2, 3w/2, ...` are clipped
to the polygon; each inside segment of a concave field is its own line; directions alternate.
Reported: passes, path length, field and covered area, working time at `--speed-kmh` with
`--turn-s` per turn. Headland passes are not planned. With `--advisory` (the JSON of
`advise`), the planner finds the earliest run of consecutive `go` hours (and `caution` hours
with `--allow-caution`) long enough for the work: the interference- and space-weather-aware
part.

### Resilience score and outage diagnosis (`resilience`)

A degraded episode is a run below the required fix while steering is engaged (when the log
records it), with runs closer than 10 s merged. Recovery time: from the episode start until the
required fix is back and held 10 s. Score 0-100 from four components, each 0-1:

| Component | Weight | Value |
|---|---|---|
| availability | 40 | share of engaged time at the required fix |
| recovery | 20 | median recovery: 1 at 10 s or less, 0 at 300 s or more, linear between |
| continuity | 20 | share of episodes without steering disengaging |
| degraded accuracy | 20 | share of degraded time with the guidance cross-track error within tolerance |

A component the log cannot inform is "not assessed" and its weight is shared over the others.
Grades A (85 or more), B (70), C (50), D.

Indicative cause per episode, from the signatures in this one log:

* **interference-like**: satellites down 40 % or more against the minute before, or C/N0 down
  6 dB or more, or the position lost;
* **correction-feed-like**: satellites and C/N0 hold, the fix falls below RTK and the
  correction age exceeds 10 s (or is not logged);
* **ionosphere-like**: satellites and C/N0 hold, corrections fresh, the fix sits in RTK float
  or toggles fixed/float three times or more; with `--kp`, the Kp during the episode is quoted;
* otherwise **undetermined** (a log without satellite counts or C/N0 often is).

This is a dealer's first look, not an attribution, and it locates nothing.

### Space-weather advisors (`advise`, `translate`)

* **NOAA G scale** from Kp: G1 at 5, G2 6, G3 7, G4 8 (including 9-), G5 9.
* **Location.** Geomagnetic latitude from the open engine's IGRF centred dipole
  (`kshana::igrf::geomagnetic_north_pole`); modified dip latitude from the IGRF inclination
  (`kshana::igrf::magnetic_field`, `tan(modip) = I / sqrt(cos lat)`).
* **Scintillation level per hour** (low, moderate, high):
  * low latitude (|modip| 30 deg or less): 19:00 to 02:00 local solar time is moderate, and
    high in the longitude sector's season (American sector September to March; elsewhere the
    equinox months) with F10.7 at or above 150 (from the GFZ file or `--f107`). After the
    published climatology (Aarons 1993; Basu et al. 2002), simplified;
  * auroral: high poleward of `68 - 2 Kp` deg geomagnetic, moderate within 3 deg of it (a
    linear simplification of the Feldstein-Starkov oval);
  * storm-time mid latitude: Kp 7 or more at 40 deg geomagnetic or more is high (ionospheric
    gradients that upset RTK).
* **What the levels mean.** The open engine's Cornell scintillation model
  (`kshana::iq::channel::scint`) turns each level's representative S4 (0.1, 0.4, 0.8;
  tau0 1 s) into the share of time a signal fades deeper than the receiver's 10 dB margin.
* **PPP versus RTK.** RTK horizontal 1 sigma `10 mm + 1 ppm x baseline x m(G)` with storm
  multipliers 1, 2, 3, 6, 10, 20 for G0 to G5 (MODELLED); with `--ionex` and `--base`, the
  vertical TEC difference between field and base from the IONEX map (open engine
  `kshana::ionex`, rotated-map interpolation) is reported as an L1 delay difference and half of
  it is added. PPP 1 sigma 0.04 m after a 20 minute convergence; moderate scintillation
  doubles the convergence and raises the error by half; high makes PPP unavailable. A method
  is offered when twice its sigma meets `--accuracy` (default 0.05 m) and, for RTK,
  scintillation is not high. Status: no-go when no method qualifies or scintillation is high;
  caution at moderate or any G level; otherwise go.
* **Grower translation** (`translate`, free): a paragraph per UTC day naming the storm level
  and the hours when RTK-grade guidance is likely unreliable, with what to do about it.

**May 2024 backtest** (`tests/agri.rs`, `may_2024_storm_backtest`). On the GFZ Kp series for 7
to 14 May 2024 (`agri/data/gfz-kp-2024-05-07-to-14.txt`), fields in Manitoba, Iowa and
Saskatchewan are never no-go on 8 and 9 May, are no-go with high scintillation from 18 UT on
10 May to 21 UT on 11 May (Kp 9, G5 on 11 May), and are clear again on 14 May. Farmers in the
Canadian Prairies and the US Midwest reported autosteer misses and idle RTK on 10 and 11 May
(Western Producer, 21 May 2024; 404 Media, May 2024). A field in Mato Grosso, near the
magnetic equator, is untouched by the high-latitude rules and shows only the post-sunset
window. This matches dates, not any receiver's measured behaviour.

**What a hosted service would add.** Live regional TEC and rate-of-TEC maps from reference
station networks in place of the Kp proxy, thresholds calibrated against observed receiver
outcomes per region, and alerts pushed to dealers. None of that is in this build: the advisors
run locally on public indices.

### Geotagged-photo checks (`photo`)

For photos submitted as evidence of field work (for example under an agricultural subsidy
scheme), each JPEG's EXIF (TIFF IFD0, Exif and GPS IFDs, both byte orders) is read and checked:
GPS position present; inside the declared parcel (distance outside reported); GPS UTC against
camera time (with an offset tag within 120 s; without one, a whole number of quarter hours
within 120 s); inside the claimed window (`--from`, `--to`); stated horizontal error within
30 m; coordinates not rounded to four decimals or fewer; a software tag that names neither the
camera nor a firmware version is listed for review. Across the set: identical coordinates on
photos taken more than a minute apart, and travel faster than 50 m/s between consecutive
photos. EXIF can be written by anyone with free tools: passing every check does not show a
photo is genuine, and a flag is a reason to look, not a finding. Image content is not analysed.

## Files

* `agri/data/gfz-kp-2024-05-07-to-14.txt`: excerpt of the GFZ Kp file
  (https://kp.gfz.de/app/files/Kp_ap_Ap_SN_F107_since_1932.txt, downloaded 2026-11-20; CC BY
  4.0; the sunspot-number column set to -1 because its CC BY-NC licence is not taken on).
* `agri/data/swpc-3-day-forecast-2026-11-20.txt`, `agri/data/swpc-planetary-k-index-sample.json`:
  NOAA SWPC products as published on 20 Nov 2026 (US Government work), for the parsers.
* `agri/examples/`: SYNTHETIC A-B line, field, profile and photo from `kshana-pro agri sample`.

## Not built here

* The Arctic geometry and ionosphere availability planner (B3503) belongs in the open engine
  on top of `kshana sky`; it is not part of this Pro overlay.
* Reading proprietary in-cab formats; headland planning; rotation in the realignment; image
  forensics of photos; the hosted parts of B3502 (above).
