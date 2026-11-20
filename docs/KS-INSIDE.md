# KS-Inside: the embedded OEM SDK (`ks_inside.h`, `kshana-pro ks-inside`)

Research backlog E39, item B3901 (sources 09b H46–H48). Source: `src/ks_inside/` (Rust facade
and C ABI), `src/license/meter.rs` (meter log), `src/cli/ks_inside_cmd.rs`. Header:
`sdk/ks-inside/include/ks_inside.h`; example: `sdk/ks-inside/examples/minimal.c`. Tests:
`tests/oem_licensing.rs`.

KS-Inside lets a receiver, simulator or timing-appliance maker call a small set of Kshana Pro
functions from inside their own product, through a stable C ABI, under an OEM licence, with
every call metered per unit. It is a facade over existing Pro functions: results carry the
labels of the function they come from (MODELLED, COMPUTED) and the SDK adds no claim of its own.
Nothing here is published, uploaded or released as a binary by this repository.

## Licence

`ks_inside_open` (Rust: `KsInside::open`) needs a token that:

1. verifies under the keys compiled into the build and the revocation list passed to it (the
   OEM ships the vendor's signed list with its firmware updates);
2. names the **`sdk` product explicitly**. `sdk` is never part of a default token
   (`Product::DEFAULT`), so no existing customer licence opens the SDK by accident;
3. carries the **explicit grant `pro-ks-inside`**, and is at the **Enterprise** tier the tier
   manifest gives that feature (`commercial/tier-manifest.json`). The tier alone is not enough;
4. if machine-bound, names this machine (as everywhere else in Kshana Pro).

Each call is checked again: the licence's expiry, `pro-ks-inside`, and the Pro kind or feature
the method needs. A licence that expires while the device runs stops at the next call.
Programme licences (docs/PROGRAMME-LICENCES.md) can never carry `pro-ks-inside`.

Issuing an OEM licence (vendor):

```sh
kshana-pro license issue --tier enterprise --licensee "Example OEM" --days 365 \
    --product sdk --feature pro-ks-inside --register register.jsonl
```

The commercial terms (per-unit fee, floor, audit rights) are the owner's decision and live in the
OEM agreement, not in this repository.

## Methods (ABI version 1)

| Method | Needs | Input | Answer |
|---|---|---|---|
| `scenario.run` | the scenario's `pro-` kind | a Pro scenario as TOML text | the kind's JSON result |
| `timing.drift` | `pro-grid-timing` | `{"series": [[unix_s, te_s], ...], "detector": {...}, "budget_s": n}` | drift report (MODELLED) |
| `timing.timebomb-analyse` | `pro-timing-timebomb` | `{"vector": {...}, "capture_format": "nmea"\|"csv"\|"ubx", "capture": text \| "capture_hex": hex}` | fault analysis (COMPUTED) |

All three are pure computations: no file or network access from inside a call. `scenario.run`
refuses open kinds (they run in the open engine). `kshana-pro ks-inside methods [--json]` lists
the methods of the build.

## C ABI

```c
uint32_t ks_inside_abi_version(void);
int32_t  ks_inside_open(const char *token, const char *unit_id, const char *meter_log_path,
                        const char *revocations, ks_inside **handle, char **error);
int32_t  ks_inside_call(const ks_inside *handle, const char *method, const char *input,
                        char **output);
char    *ks_inside_info(const ks_inside *handle);
void     ks_inside_free_string(char *s);
void     ks_inside_close(ks_inside *handle);
```

Status codes: `KS_OK` 0, `KS_ERR_ARGUMENT` 1, `KS_ERR_LICENCE` 2, `KS_ERR_REFUSED` 3,
`KS_ERR_METHOD` 4, `KS_ERR_INPUT` 5, `KS_ERR_METER` 6, `KS_ERR_INTERNAL` 7. On any code but
`KS_OK` the output string is `{"error": "...", "code": n}`. Pointers are checked for null, strings
must be UTF-8, no panic crosses the boundary, and a handle may be shared between threads. The ABI
version rises only on an incompatible change; a test (`header_matches_the_abi`) keeps the header
and the Rust side in step.

Building the library from this crate (for an OEM integration build; not part of CI or of any
release):

```sh
cargo rustc --release --lib --crate-type cdylib     # libkshana_pro.so / .dylib / .dll
cargo rustc --release --lib --crate-type staticlib  # libkshana_pro.a
cc -I sdk/ks-inside/include sdk/ks-inside/examples/minimal.c -L target/release -lkshana_pro
```

A build without `KSHANA_PRO_LICENSE_PUBKEY` set verifies no token in release mode; the debug
demo key exists only in debug builds.

## Metering (per call, per unit)

Every call to a known method, allowed or refused, appends a line to the meter log **before** the
answer is returned. When the line cannot be written the call fails with `KS_ERR_METER` and the
answer is dropped: there is no unmetered use. An unknown method is refused without a line (no Pro
function ran).

One line (`kshana-pro-meter/1`, JSON Lines):

```json
{"format":"kshana-pro-meter/1","seq":0,"unix":1800000000,"licence":"kt_…","unit":"<sha256>",
 "method":"timing.drift","feature":"pro-grid-timing","outcome":"ok",
 "input_sha256":"…","prev":"0000…"}
```

* `unit` is SHA-256 over a fixed prefix, the licence ID and the unit ID: the vendor can check a
  given serial during an audit, but the log does not list serials in clear.
* No input values are stored, only their SHA-256.
* `prev` is the SHA-256 of the line before, so an edited, reordered or deleted line breaks the
  chain. A log with a broken chain is refused at open: start a new file and report the old one.
* One log has one writer: give each process its own file.

Reports:

```sh
kshana-pro ks-inside meter verify meter.jsonl
kshana-pro ks-inside meter summary meter.jsonl --json   # per month: active units, ok/refused/error, per method
```

The summary carries the hash of the last line (`head`), so the vendor can check that a later
report continues the same chain.

**Limits, stated plainly.** The meter log is tamper-evident, not tamper-proof: whoever controls
the device can delete the whole file. It records use; it does not enforce a unit count. Audit
rights in the OEM agreement and the per-unit report are what make it a royalty basis.

`kshana-pro ks-inside call <method> <input-file> --token-file <f> --unit <id> --meter-log <f>`
makes one metered call exactly as the C ABI does, for integration tests on a workstation.

## Not built

* **DSP/FPGA reference integration** (09b H48, Tensilica/Xilinx class). Not built and not
  tested on hardware. On a Zynq-class part the library would run on the application processor
  under Linux, with the FPGA fabric supplying time-error or capture data to the calls; a bare-metal
  DSP port would need a `no_std` subset of the methods, which this crate is not.
* **Signed meter reports.** The summary is not signed; a signed report (OEM key) is a follow-up.
* **A pre-built binary SDK or package.** Deliberately none: building and shipping a binary is a
  release decision.
* **Clock-rollback guard inside the SDK.** The CLI's licence store keeps one (`clock.json`); the
  SDK takes the system clock at each call.
