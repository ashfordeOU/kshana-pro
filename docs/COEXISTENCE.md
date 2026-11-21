# Co-existence and compatibility interference catalogue for civil links

Module `src/coexistence.rs`, command `kshana-pro coexistence` (`src/cli/coexistence_cmd.rs`),
tests in `tests/threat_library.rs` (module `coexistence`). Feature `pro-coexistence-catalogue`
(Pro: the scenario pack; the catalogue itself is free to list).

The threat library (`src/threat_library`) covers GNSS interference events only. This catalogue
covers something different: **co-existence and compatibility** of civil links with lawful
neighbouring services. It contains unintentional interference only, no hostile-interference model,
and it synthesises no waveform.

Every value is cited to the clause or table it was read from (2026-11-21). A row without a read
source says **to confirm**.

| category | values | source |
|---|---|---|
| co-channel | I/N −6 dB (30 MHz–3 GHz), −10 dB (above 3 GHz), −13 dB (indoor FWA vs UWB, 3–6 GHz) | ITU-R F.758-8 § 4.13, Table 5 |
| co-channel | RNSS C-band thresholds: −157.1 / −160.1 dBW narrow-band, −147.1 / −150.1 dB(W/MHz) wideband (tracking / acquisition) | ITU-R M.2031-0 Annex 1, Table 1-1 |
| adjacent-band blocking | GNSS L1: −65 dBm at 1 524, −95 at 1 548, −105 at 1 554 and 1 615, −85 at 1 627 MHz; L5: −75 at 1 154, −85 at 1 310 MHz; 1 MHz AWGN; ΔC/N0 ≤ 1 dB | ETSI EN 303 413 V1.2.1, 4.2.1, Tables 4-2 to 4-4 |
| adjacent-band blocking | 2.4 GHz FHSS receivers, categories 1–3: −34 dBm CW at the listed frequencies; PER/FER ≤ 10 % | ETSI EN 300 328 V2.2.2, 4.3.1.12, Tables 6–8 |
| out-of-band boundary | 2.5 × necessary bandwidth (normal); wideband BU + 1.5 BN, BU by frequency range | ITU-R SM.1539-2 Tables 1, 2 |
| spurious emissions | 43 + 10 log P or 70 dBc (60 dBc space services), whichever is less stringent; reference bandwidths 1 kHz / 10 kHz / 100 kHz / 1 MHz (4 kHz space) | ITU-R SM.329-13 § 4.1, Table 2 |
| spurious emissions | 2.4 GHz FHSS transmitter: −30 dBm/MHz (1–12.75 GHz), −54 dBm/100 kHz in broadcast bands, −36 dBm/100 kHz elsewhere below 1 GHz | ETSI EN 300 328 V2.2.2, 4.3.1.10, Table 4 |
| intermodulation | to confirm | — |

Not transcribed in this edition, so not in the catalogue:

* EN 300 328's non-FHSS clauses (Tables 14–16);
* EN 300 328's out-of-band mask (Figure 1);
* the full SM.329 Table 2 beyond the general and space-service rows.

`coexistence scenarios <request.toml> --out <dir> ...` takes a victim (the catalogue entries
that apply, noise figure and bandwidth) and neighbouring transmitters (power, spurious-limit
entry, coupling loss). It writes a signed scenario pack: per case the frequency relation, the
level at the victim input, the bandwidth, the signal and the pass criterion, with its source and
the derivation where a level is computed:

* an I/N criterion becomes a level: `N = −174 + 10 log10 B + NF`;
* a neighbour's spurious limit at its power, less the coupling loss: SM.329's
  `min(43 + 10 log P, cap)`.

Data and scenarios for a test plan; no signal is generated.
