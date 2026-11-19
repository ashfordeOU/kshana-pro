# ELOF v0.1: eLoran Observation & Scenario Format

CONFIDENTIAL, Kshana Pro. Status: **proposal, version 0.1**. Evidence label: **MODELLED**. The
format has been checked only by the internal round-trip and rejection tests of
`crates/kshana-eloran/src/elof/`. No external receiver writes it yet, and no standards body has
reviewed it.

ELOF is a versioned, self-describing exchange format for eLoran data. It does for low-frequency
(LF) navigation what RINEX does for GNSS observations. It carries:

- what a receiver measured (observations);
- what the simulator really emitted and propagated (truth), when the stimulus is simulated;
- the receiver's time solution against a UTC(k) or other reference (timing);
- the decided LORAN Data Channel (LDC) symbols;
- references, by SHA-256, to the scenarios behind all of the above.

The reference implementation is the `kshana_eloran::elof` module: reader, writer, validator and
converters. The CLI is `kshana-pro eloran elof validate|convert`.

## 1. Profiles

| profile | extension | content |
|---|---|---|
| JSON lines | `.elof.jsonl` | line 1: the header record; every following line: one record (`obs`, `truth`, `timing` or `ldc`) |
| compact CSV | `.elof.csv` | line 1 `#ELOF-CSV 0.1`; line 2 `#header <header JSON>`; line 3 the fixed column line; then `obs` rows only |

Every record is one JSON object. It carries `"record": "<kind>"`. Blank lines are ignored.
Numbers are JSON numbers. JSON has no NaN or infinity, so an absent optional field means "not
available". The one exception is `truth.snr_db`, where absence means a noise-free stimulus
(infinite SNR).

## 2. Time

- `epoch_utc` (header) is the UTC instant at which `t_s = 0`. Its form is
  `YYYY-MM-DDTHH:MM:SS[.f]Z`. TAI − UTC comes from the leap-second table.
- Every record has `t_s`, in seconds since `epoch_utc` on the file's `time_scale`. **`t_s` must
  not decrease** through the file (equal values are allowed).
- `time_scale` is one of `utc`, `tai`, `gps`, `receiver` or `scene`. `scene` means the
  simulator's truth timeline.
- `time_reference` names the receiver's reference (`id`, e.g. `UTC(NPL)`) and gives its `kind`:
  `utc_k`, `gnss`, `loran`, `free_running` or `scene`.

### The observable `toa_ns`

`toa_ns` is the arrival time of the group's pulse-0 time origin minus that station's **nominal**
emission of the same group:

    nominal emission = group · GRI   (from the Loran epoch 1958-01-01T00:00:00 TAI)
                     + emission_delay_us of the station

It is measured on the file's time scale and is therefore a pseudo-delay: the propagation delay
plus every clock offset (station steering, receiver clock). `group` is the group number since the
Loran epoch. With the header's station table, a reader can rebuild the absolute arrival time
exactly: `epoch + nominal emission + toa_ns`.

This differs from the receiver-log importers' convention, where the ToA is taken relative to the
chain master's nominal emission. The converters translate between the two.

## 3. Units (normative)

The header repeats this table in `units`. A validator refuses a file whose `units` differ from it
or leave out any entry. Unknown extra keys produce a warning.

| field | unit |
|---|---|
| `t_s` (also `t_emit_s`) | s since `epoch_utc` |
| `toa_ns`, `delay_ns` (also `geom_ns`, `pf_ns`, `sf_ns`) | ns |
| `asf_ns` | ns |
| `ecd_us` | µs (written `us`) |
| `snr_db` | dB in 20 kHz |
| `field_uv_m` | µV/m (written `uV/m`) |
| `bearing_deg` | deg |
| `sky_delay_us` | µs |
| `sky_ratio_db` | dB |
| `time_ref_offset_ns`, `emission_error_ns`, `ref_minus_rx_ns` | ns |
| `lat_deg` (and `lon_deg`) | deg (WGS84) |
| `distance_km` | km |

## 4. Header record

Required fields are in bold.

| field | content |
|---|---|
| **`record`** | `"header"` |
| **`format`** | `"ELOF"` |
| **`version`** | `"0.1"` (major.minor) |
| **`units`** | the table of §3 |
| **`time_scale`** | §2 |
| **`epoch_utc`** | §2 |
| **`source`** | `{kind: simulated / software_receiver / hardware_receiver / field, software, description}` |
| `receiver` | `{id, make, model, firmware, internal_delay_ns?, position? {lat_deg, lon_deg, alt_m}, antenna {kind: e_field / h_loop / crossed_loop / unknown, make, model, cable_delay_ns?}}` |
| **`time_reference`** | `{id, kind, description}` |
| **`stations`** | `[{chain, gri_us, station, role: master / secondary, lat_deg, lon_deg, emission_delay_us, time_ref, ldc_scheme?}]`: every station a record names must be listed here |
| `asf` | `{applied, model, source, grid_sha256?, stations: [{chain, station, asf_ns, sigma_ns?}]}`: whether ASF corrections are already in the observables, and which model or grid supplied them |
| `scenarios` | `[{sha256, name, uri?}]`: the scenarios the data relate to (lower-case SHA-256 hex) |
| `truth_included` | `true` when `truth` records are present |
| `comments` | free text |

## 5. Records

### `obs`: one station observation (one tracking window or one logged epoch)

| field | req. | content |
|---|---|---|
| `t_s` | yes | time tag |
| `chain`, `station` | yes | declared in the header |
| `group` | yes | the group the observable refers to |
| `channel` | | loop channel when loops are processed separately |
| `toa_ns` | yes | §2; the validator requires \|`toa_ns`\| < one GRI |
| `ecd_us` | | envelope-to-cycle difference, within ±10 µs |
| `snr_db` | | −60 to 120 dB |
| `field_uv_m` | | ≥ 0 |
| `phase_code` | | `A` or `B`; must match the parity of `group` (even groups are A) |
| `bearing_deg` | | crossed-loop bearing, [0, 360) |
| `flags` | | from `cycle_slip`, `cycle_warning`, `snr_warning`, `blink`, `invalid`, `no_fix` (unknown flags produce a warning) |

### `truth`: simulator truth of one station's group (only with `truth_included`)

`t_s` is the true arrival. `t_emit_s` is the true emission. The other fields are `chain`,
`station`, `group`, `toa_ns`, `delay_ns` = `geom_ns` + `pf_ns` + `sf_ns` + `asf_ns`, `ecd_us`,
`field_uv_m`, `snr_db?`, `sky_delay_us?`, `sky_ratio_db?`, `time_ref_offset_ns`,
`emission_error_ns`, `rx_lat_deg`, `rx_lon_deg`, `rx_heading_deg`, `bearing_deg`, `distance_km`
and `ldc_symbol?` (the LDC symbol transmitted in this group). The relation
`toa_ns = delay_ns + emission_error_ns` holds.

### `timing`: receiver time solution

The fields are `t_s`, `time_ref` (e.g. `UTC(A)`), `ref_minus_rx_ns` (the reference minus the
receiver clock), `std_ns?` and `n?`. With `chain` and `station`, the record is a per-station
estimate; without them, it is the estimate for that time reference. `chain` and `station` must
appear together or not at all.

### `ldc`: one decided data-channel symbol

The fields are `t_s`, `chain`, `station`, `group`, `scheme` (it must match the station's
`ldc_scheme` when that is declared), `symbol` and `missing` (true when the pulse timing was not
measured and the symbol is a placeholder).

## 6. Validation

`elof::validate` reports errors (the file is invalid) and warnings. It checks:

1. **version**: format `ELOF`, same major version, minor ≤ the reader's. A newer version is
   refused before the rest of the header is read;
2. **units**: as in §3;
3. **required fields**: header fields and record fields (a missing field is a parse error that
   names the field);
4. **header consistency**: station ids are unique, GRI is 40 000–99 990 µs in 10 µs steps,
   positions are in range, the emission delay is inside the GRI, SHA-256 digests are well
   formed, and ASF stations are declared;
5. **records**: stations are declared; values are finite and in range; `toa_ns` is within one
   GRI (this catches µs/ns mix-ups); the phase code matches the group parity; truth records
   appear only when declared;
6. **monotonic time**: `t_s` never decreases.

## 7. Compact CSV profile

```
#ELOF-CSV 0.1
#header {"record":"header","format":"ELOF","version":"0.1",...}
t_s,chain,station,group,channel,toa_ns,ecd_us,snr_db,field_uv_m,phase_code,bearing_deg,flags
0.0123,conf-A,M,27238737305,,765443.818,0.0,24.5,812.0,B,,
```

The column line is fixed. Empty cells mean absent values. Flags are joined with `|`. Ids may not
contain `,`, `|`, `"` or line breaks. Numbers are written in the shortest form that reads back as
the same IEEE-754 double, so a CSV round trip of `obs` records is exact. `truth`, `timing` and
`ldc` records have no CSV form. The writer reports how many it left out, and the header is
written with `truth_included: false`.

## 8. Converters (reference implementation)

| from | to |
|---|---|
| scenario | header (stations, epoch, scenario SHA-256, static position, antenna kind, ASF model name, LDC schemes) |
| simulator truth (in memory, or the `*.truth.csv` of `eloran run`) | `truth` records, including transmitted LDC symbols |
| software-receiver measurements, time solution, LDC | `obs`, `timing` and `ldc` records |
| `obs` / `timing` / `truth` records | engine measurements, time solution and truth records (ToA errors, clock errors and scoring are then computed exactly as for the built-in receiver) |
| imported receiver logs (`eloran import` observation JSON) | `obs` records (group recovered from the time tag), and back |

Round trips are exact for every stored field except the absolute `toa_s` of the engine records.
That value is rebuilt from the nominal emission plus `toa_ns` and agrees within 1 ps over a day.

## 9. Versioning

- A change that makes old readers misread data increments the **major** version.
- Optional fields or record kinds that old readers can safely ignore increment the **minor**
  version. A 0.1 reader refuses 0.2 files rather than silently dropping fields.

## 10. Limits of v0.1

- There is no binary or HDF5 profile yet. JSON lines are verbose (about 300 bytes per `obs`
  record).
- There are no per-pulse records. Pulse-level timing for LDC research stays in the engine.
- Positions in `obs` are not carried, because ELOF v0.1 has no navigation-solution record. Use
  the importer records for fixes.
- Field mappings from vendor logs (UrsaNav, reelektronika and others) are not yet written as
  converters. They go through the generic CSV importer first.
