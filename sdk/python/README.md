# kshana-eloran — Python SDK (proprietary)

Python access to the Kshana Pro eLoran/GNSS simulator. No third-party dependencies; NumPy is
used for recordings when it is installed.

```python
from kshana_eloran import Eloran, ScenarioBuilder, read_sigmf, read_truth

e = Eloran(license="<token>")          # or Eloran(dev=True) with a development build
st = ScenarioBuilder.station
sc = (ScenarioBuilder("2026-11-18T00:00:00Z", 2.0, fs_hz=1_000_000)
      .static_receiver(55.0, 3.0)
      .time_ref("UTC(A)", offset_ns=50.0)
      .ground("smooth_earth")
      .noise(20.0)
      .chain("demo", 79800, [st("M", "master", 56.5, 1.0, time_ref="UTC(A)"),
                             st("X", "secondary", 53.5, 1.5, 13000.0, time_ref="UTC(A)"),
                             st("Y", "secondary", 55.5, 6.0, 31000.0, time_ref="UTC(A)"),
                             st("Z", "secondary", 54.0, 5.5, 52000.0, time_ref="UTC(A)")]))

run = e.run(sc.doc, "out/demo")                      # SigMF + truth + run record
rep = e.rx(sc.doc, "out/demo", "out/rx", truth="out/demo.truth.csv")
mon = e.monitor(sc.doc, "out/monitor")               # out/monitor/monitor.html dashboard
rec = read_sigmf("out/demo"); truth = read_truth("out/demo.truth.csv")
```

| Layer | What |
|---|---|
| `Eloran` | every `kshana-pro eloran` command: `run`, `rx`, `gnss`, `stream`, `monitor`, `gri_*`, `circular_t`, `calibrate_receiver`, `awg`, `accept_software`, `sync_measure`, `asfgrid`, `fidelity`, `import_log`, `conformance_list`, `campaign_run/verify`, `plan`, `twin` |
| `ServiceClient` | a `kshana-pro serve` instance: `example`, `validate`, `run`, `rx`, `monitor`, `gri`, `gri_chain` |
| readers | `read_sigmf` (ri16/rf32 multi-channel), `read_truth`, `read_monitor` |
| builders | `ScenarioBuilder`, `dumps_toml` |

Scenarios may be passed as a path, TOML text or a dictionary. Errors raise `EloranError` /
`ServiceError` with the tool's message. Licensing follows the Kshana Pro tier manifest.

Tests: `python3 -m unittest discover -s tests` (set `KSHANA_PRO_BIN` to a `kshana-pro` binary
for the integration tests).
