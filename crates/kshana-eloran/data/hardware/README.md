# Hardware catalogue (`catalog.json`)

PROPRIETARY — Kshana Pro. Embedded in the crate with `include_str!` (see `src/bench/catalog.rs`).

One JSON array, one object per device: signal sources (SDRs, AWGs, USB instruments), GNSS
simulators, references, time-interval and frequency counters, scopes, eLoran receivers and
antennas. Schema: `bench::catalog::HwSpec`. Units: Hz, seconds, ohms, Vpp.

**Every value comes from the vendor's datasheet, product page or manual and is cited in
`sources`. A value no source gave is `null`; nothing is estimated.** `confidence` says where
the entry came from (`datasheet`, `manual`, `vendor-web`, `secondary`). Where a vendor site
refused automated access, the vendor's own datasheet hosted by a distributor or university was
used and the entry says so. Inferences (for example DC coupling of an AWG from its DC-offset
specification) are stated in `notes`.

Two fields are Kshana's own mapping, not specifications:

* `kshana_driver`: the driver that runs the device (`redpitaya`, `uhd`, `soapy`, `digilent`,
  `scpi_awg`, `gnss_sim:<adapter>`, `tic:ticc`), or null.
* `idn_patterns`: substrings of the instrument's `*IDN?` reply (or the UHD/SoapySDR device
  string) that `bench probe` uses to recognise it; conventions, confirmed on the first probe.

`scpi_port` is null throughout: no cited datasheet states a port number (the probe tries 5025,
then 5000 for Red Pitaya, 5024).

Collected 18 Nov 2026. Not catalogued for lack of a reliable source: Leo Bodnar Mini GPSDO,
LabSat 4, Safran BroadSim hardware. A lab adds or overrides entries with its own file
(`--catalog lab.json`); an entry with the same `id` replaces the built-in one, so a measured
value can replace a datasheet value.
