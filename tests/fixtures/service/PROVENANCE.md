# Provenance of the job-service test scenarios

These scenario files are unmodified copies of the open Kshana engine's own public
scenario files, used by `tests/service_jobs.rs` to drive the job service and to compare
every job result with a direct run of the same file.

- Source: the open engine repository, directory `scenarios/`
- Engine commit: `da55ae630b16dc93dc81119627b2445396782e3d` (the development pin the
  overlay builds on; engine version string `0.28.0`)
- Copied: 2026-09-30
- No file comes from, or refers to, the engine's optional workshop preset; the test
  `fixtures_never_touch_the_optional_preset` scans every file here for its name in any
  case and fails if it appears.

| File | SHA-256 |
|---|---|
| `campaign-monte-carlo-clock-holdover.toml` | `3504af515f45a4ea2686122978cad662d1ca93c637ffe26a327ea4e282e3a004` |
| `clock-ensemble.toml` | `e61046736632d077c2d14b92a1e5f9a2edaf9ee7b2c98a9b4caafe968279984a` |
| `clock-holdover.toml` | `430c364d5a53a64c80629412de7a77aefdbe62fac9106257423f71b2a82d7f52` |
| `ephemeris.toml` | `64ee89531e8eca33d32d575ed0ea455e42b9a18544e3e9977f4d40b60ce385fb` |
| `integrity-raim.toml` | `0bf9f03f8cfb2062635165c37914e12006bf5e7e0abe12438bacb0490315b4a3` |
| `launch-window.toml` | `56f833c88193a02f8fbe3cf2d5ac3f36a1875ed1f1cc1d098b2bb38d306e3c82` |
| `leo-pnt-mega-shell.toml` | `935a3860dbb7cb70cb64b60089040d642dfdf2cf11b519c47a76d4a96377595f` |
| `link-budget.toml` | `1c75d05b9fc5e9374a896c944d6495cf972cd373fd6922ac1acbef0d3588df7f` |
| `passes.toml` | `4d11beed2b64522bad9d541c1463f2989f33ff5ce3c113380fef8a63355b7267` |
| `space-packet.toml` | `5f5ac347dbcf90d0643d0729f3a01a2dfde8b9952e7873045072c40251d7a638` |
| `timetransfer.toml` | `d2f4d4f6da78a4435c0360b83d24fe0ce622247afbc2e1545b561174c988b6ff` |

The `ci` operation test reuses the Pack C fixtures in `tests/fixtures/pack-c/` (their
provenance is recorded with those files).
