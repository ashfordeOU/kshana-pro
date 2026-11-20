# Insurance and finance analytics

Research 2026-11-20 gaps, epic E28 (Kshana Pro part: B2801 local part, B2802, B2803, B2805).
Code: `src/insurance/` (`trigger.rs`, `basis.rs`, `bi.rs`, `feed.rs`), CLI
`src/cli/insurance_cmd.rs`. Tests: `tests/insurance_analytics.rs` (10 tests) and unit tests in
each module. Examples: `examples/insurance/`.

**What this is.** Calculations an underwriter, broker, risk manager or claims handler runs on
their own data, with every assumption stated, every input hashed and a result fingerprint. Every
output is evidence, an indicator or a calculation for the user's own decision. It is **not**
insurance or financial advice, an underwriting, pricing or claims decision, a rating or a
guarantee of any payout or loss figure. Ashforde has no insurer partners; nothing here is
offered with or endorsed by an insurer. Cover schedules and areas in the examples and the sample
are synthetic and say so. Evidence: **MODELLED** throughout; no parameter is fitted to claims or
loss data.

| Command | Feature | Tier | Backlog |
|---|---|---|---|
| `insurance trigger` | `pro-parametric-trigger` | Pro | B2801 (local part) |
| `insurance basis` | `pro-basis-risk` | Pro | B2802 |
| `insurance accumulation` | `pro-accumulation` | Enterprise | B2802 |
| `insurance bi` | `pro-bi-quantification` | Pro | B2803 |
| `insurance feed` | `pro-underwriting-feed` | Pro | B2805 |
| `insurance triage` | `pro-claims-triage` | Pro | B2805 |
| `insurance schema`, `sample`, `bi models`, `verify` | none | free | |

Producing commands take `--out <dir> --signer <name> --key-file <file> [--utc <iso>]` and sign
the directory (`signature.json`); `kshana-pro insurance verify <dir> --trust <hex>` checks it
offline. Each directory holds the result JSON, `report.html` and a copy of the request.

## Parametric outage trigger (`insurance trigger`)

Evaluates a trigger definition on the user's own station series and reports what it would have
done. A hosted oracle would add independently operated stations, a published signed index and a
neutral record of the trigger's state; this command works only on the data supplied.

```toml
title = "..."
period_start_utc = "2026-09-01T00:00:00Z"
period_end_utc = "2026-09-03T00:00:00Z"
series = ["stations.csv"]        # CSV utc,site,cn0_dbhz or kshana.obs/1 files (.jsonl)

[[sites]]                        # optional: position (location basis) and a fixed baseline
id = "A"
lat_deg = 59.4
lon_deg = 24.7
baseline_dbhz = 45.0

[trigger]
cn0_drop_db = 6.0                # degraded: C/N0 below baseline minus this
min_sites = 2                    # k-of-n: sites degraded at once
min_duration_min = 30.0          # an event this long or longer fires
max_gap_s = 120.0                # a longer gap in the data ends an episode
merge_gap_min = 10.0             # episodes and events closer than this merge

[cover]                          # optional; synthetic must be stated
name = "Synthetic cover (illustrative)"
currency = "EUR"
synthetic = true
tiers = [{ min_hours = 0.5, amount = 10000.0 }, { min_hours = 2.0, amount = 50000.0 }]
per_event_limit = 50000.0
aggregate_limit = 100000.0

[[backtest]]                     # archived events with their documented basis
id = "ARCH-1"
start_utc = "..."
end_utc = "..."
basis = "where this event is documented"
```

Method: the baseline per site is the stated `baseline_dbhz`, else the median of the site's
samples in the period (unaffected while degraded time stays under half the period). For
`kshana.obs/1` input the epoch value is the mean C/N0 of the satellites tracked. **Missing data
is never degraded**: a gap longer than `max_gap_s` ends an episode and is listed as reduced
observed hours, because it is itself a source of basis risk. Site episodes are swept for the
times at least `min_sites` are degraded at once; merged events lasting `min_duration_min` fire.
Outage index per event: Σ over its sites' episodes of (baseline − C/N0) × time, dB·h; per
period: degraded site-hours over observed site-hours. Modelled payout: the highest tier the
event's duration reaches, capped per event and in aggregate (in time order).

## Basis risk (`insurance basis`)

```toml
title = "..."
trigger = "trigger.toml"         # the trigger request above (needs a [cover])
losses = ["losses.csv"]          # insured,start_utc,end_utc,loss[,lat,lon]
match_tolerance_h = 6.0
bootstrap = 1000
seed = 1
max_site_distance_km = 50.0
```

Fired events and losses overlapping within the tolerance form clusters; per cluster the payout
P and the loss L (the sum of the losses supplied; the cover is read as protecting that
aggregate). Reported: each cluster's class (paid with loss, payout without recorded loss, loss
without payout) and whether a trigger event below the duration threshold overlapped an unpaid
loss (duration basis); shortfall Σmax(L−P, 0), excess Σmax(P−L, 0); the basis-risk ratio
Σ|P−L| / ΣL with a seeded bootstrap 90 % interval over clusters; hit rate; payout-without-loss
rate; payout-loss correlation (three clusters or more); and, with positions, each loss's distance
to the nearest site and the share of loss beyond `max_site_distance_km` (location basis).

## Accumulation (`insurance accumulation`, Enterprise)

```toml
title = "..."
currency = "EUR"
portfolio = "portfolio.csv"      # insured,lat,lon,exposure[,dependency 0-1]
period_start_utc = "..."
period_end_utc = "..."
events = ["events.json"]         # kshana-pnt-interference-event/1 with a location
loss_ratio_per_hour = 0.002      # the user's assumption
max_loss_ratio = 0.1
footprint_buffer_km = 0.0
cell_deg = 1.0

[[scenario]]
name = "Gdansk Bay 24 h, 60 km"
lat_deg = 54.45
lon_deg = 18.6
radius_km = 60.0
duration_h = 24.0
```

Loss of an insured under a footprint: exposure × dependency × min(max_loss_ratio,
loss_ratio_per_hour × hours). Reported per footprint (historical events in the period and
scenarios), per grid cell (exposure, share, Herfindahl index over cells), the largest historical
event, the period aggregate and its annualised value, the largest scenario, and an empirical
exceedance table (return period = period in years / rank). Events without a location are counted
and left out. Not a probable-maximum-loss certification or a capital figure.

## Business interruption and loss (`insurance bi`)

```toml
title = "..."
model = "timing"                 # timing | yield | port | grounding | cascade
currency = "EUR"
step_pct = 20                    # sensitivity: each parameter ±20 %
preset = "svn23"                 # timing only
te_log = "te.csv"                # timing: utc,time_error_ns
outage_log = "outages.csv"       # any model with outage_h: start_utc,end_utc (summed)
vessels = "vessels.csv"          # port: vessel,delay_h

[params]
tolerance_ns = 1500
revenue_per_hour = 4000.0
```

`kshana-pro insurance bi models` lists every parameter with its unit, default and meaning. A
value derived from data (outage hours, breaches, vessel delay) takes precedence over a stated
one, and the result says so. Each result lists its parameters with their source (stated,
default, preset, derived from a file), the parts of the figure, the model's assumptions, and the
sensitivity table (also `sensitivity.csv`), sorted by swing.

| Model | Headline | Calculation |
|---|---|---|
| `timing` | modelled BI loss | (outage + recovery h) × revenue per hour × affected fraction + breaches × penalty + recovery h × recovery cost. Outage hours: samples of a time-error log above the tolerance (each sample's interval, capped at 600 s), or a UTC-level step scenario: preset `svn23` is the about 13 µs GPS UTC error of 26 January 2016 (research 07b [T69][T70]), 12 h by default; a step under the tolerance gives no outage. The tolerance is the user's own requirement (for example a phase budget at a radio, or a regulatory traceability limit for trading clocks). |
| `yield` | modelled BI loss | area delayed = min(area, outage h × rate); delay days = outage h / working hours per day; yield loss = area delayed × yield × price × min(1, loss %/day × days); plus idle cost. |
| `port` | modelled BI loss | vessel delay h (file, or vessels × mean delay) / 24 × demurrage per day + outage h × berth loss per hour. |
| `grounding` | modelled cost | kinetic energy ½ m v² (MJ) × bottom factor × environment factor = severity index, banded under 1 / 10 / 100 (minor, moderate, serious, severe; engineering choices); cost = base + cost per MJ × index + off-hire days × hire. |
| `cascade` | 1-in-100 fleet loss | beta-binomial count of units affected (per-unit probability, intra-fleet correlation; 0 = independent): expected units, P(at least the threshold), 99th percentile units × loss per unit. |

Every coefficient is the user's. A severity band or a loss figure here is a calculation, not a
loss adjustment.

## Underwriting feed (`insurance feed`) and schema

```toml
title = "..."
period_start_utc = "..."
period_end_utc = "..."
tracks = "tracks.csv"            # the forensics AIS (vessel_id) or ADS-B (aircraft_id) CSV
id_column = "vessel_id"
events = ["events.json"]
areas = "areas.toml"             # the briefing area file; kind = "listed" counts as listed
max_speed_kn = 50.0              # position jump: implied speed above this ...
min_jump_km = 1.0                # ... over more than this
max_gap_min = 30.0               # track segments longer than this are not counted

[[subject]]
id = "SYN-A"
declared_controls = ["multi-constellation"]
```

Per subject: track reports and hours, coverage and data quality (sufficient at 50 % or more,
limited at 10 %, else insufficient); RFI exposure hours (inside an active event's footprint),
split jamming and spoofing, with the events encountered; position jumps, per 1,000 track hours
and the largest; hours in each area and in listed areas (war-risk pricing inputs; no market list
ships with Kshana Pro, the user supplies one and its source is recorded in the header); a cyber
exposure indicator (`high`: spoofing encountered and no signal authentication declared;
`elevated`: spoofing with authentication declared, or unexplained jumps without it; `low`
otherwise) with its basis, an indicator and not a cyber rating.

The feed is JSON Lines (`kshana-pro-underwriting-feed/1`): a header record (label, inputs with
hashes, listed-area sources, fingerprint of the subject records) and one record per subject; plus
`feed.csv` and `schema.json`. `kshana-pro insurance schema` prints the JSON Schema;
`kshana-pro insurance sample --out <dir>` writes a SYNTHETIC sample (two vessels, a jamming and
a spoofing event, a synthetic listed area) and the unsigned feed built from it.

## Claims triage (`insurance triage`)

```toml
events = ["events.json"]
evidence = ["signed/forensics-run"]      # signed evidence directories (verified with --trust)
cause_verdict = "verdict.json"           # a signed hosted cause verdict (verified with --verdict-key)

[claim]
claim_id = "C-1"
subject_id = "SYN-A"
start_utc = "..."
end_utc = "..."
lat_deg = 34.0
lon_deg = 34.25
radius_km = 10.0
claimed_cause = "spoofing"               # jamming | spoofing | outage
time_tolerance_h = 2.0
```

Outcome: `conflicting evidence` (a verified cause verdict attributes the episode to track
falsification or an equipment fault), `corroborated` (an event of the claimed kind from two or
more observers overlaps in time and place, or a verified cause verdict states the claimed
cause), `partly corroborated` (related evidence: a single-observer event, an event of another
kind, a verified signed evidence directory), or `no Kshana evidence found`, which is **not**
evidence against the claim. A verdict whose signature does not verify, or with no key given, is
set aside. Every result carries the evidence considered with its bearing and a handler checklist.

### Training note for claims handlers

1. Triage first, then read: the outcome says where to look, not what to decide.
2. A single observer is weaker than two independent ones; a signed directory is only as good as
   its signer (pin the key with `--trust`).
3. "No Kshana evidence found" with no events supplied says nothing; ask for monitoring data for
   the area and window.
4. Spoofing claims: look for position jumps, impossible speeds and time offsets (the feed counts
   jumps) and an independent track (radar, coastal AIS, LRIT).
5. The policy wording is the policy holder's and the insurer's; this aid does not interpret it.

## Not built here

* **B2801 hosted oracle**: independently operated stations, a published signed index and
  signed trigger verdicts are a hosted service, outside this crate. The local trigger above is
  its user-side counterpart.
* **B2804 policy-wording evidence verdicts** (malicious electronic means exclusions, war versus
  cyber, cyber buy-back): a hosted method with labelled fixtures; not in this crate. The
  existing dispute evidence report (`forensics dispute`) maps evidence to generic contract
  questions without interpreting wording.
* **Port congestion variable** (B2805): needs port call and berth data not modelled here.
