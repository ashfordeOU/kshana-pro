# D14: MCP `assess_receiver_log` reads any local file named in agent-supplied TOML

Status: closed: fixed in kshana-mcp 0.34.1 (2026-10-10; tag v0.34.1, live on every channel 2026-10-10 21:32Z); advisory GHSA-8wpf-hqg5-6xjr published 2026-10-11 (12:48Z). Raised 2026-10-09
by W18 while wiring 0.35 surfaces. Security detail lives here only; public text stays generic.

## What
`kshana-mcp`'s `assess_receiver_log` tool passes the agent's scenario TOML to
`kshana::receiver_trust::scenario::run_toml`. A `[log]` (or navigation) source given as `path = "..."`
is read with `std::fs::read`, so an agent, including one steered by prompt injection, can make the
MCP server read any file the server process can read.

## Affected
kshana-mcp releases that ship the tool: introduced in d4fb01d (2026-10-03); first tag v0.31.0, so
0.31.0, 0.32.0, 0.33.0, 0.33.1 and 0.34.0. The Python binding (in-process, caller's own files) and the
WebAssembly build (no filesystem) are not affected in the same way.

## Impact (first assessment, to be confirmed)
The tool does not return the file's bytes. What can leak: whether a path exists and is readable (error
text), the file's SHA-256 (in the result), and whatever the log parsers extract from a file that happens
to parse as NMEA, UBX, RINEX or Android logs (counts, times, positions). Local server process only; the
Docker image limits the reachable filesystem. Provisional: low severity (CWE-22 / CWE-73).

## Fix
W18: the MCP tool refuses `path` sources (inline content only) and caps input at 4 MiB; a test pins it.
Follow-up: W18 audits every MCP tool for filesystem access derived from agent input.

## Resolved
1. Shipped as a 0.34.1 patch (not 0.35.0): the fix commit f4d473af (W18) plus the release bump, cut from the 0.34.0 line; public PR #74
   merged 2026-10-10, Release run 38073186962 green, 0.34.1 verified live on crates.io (kshana and kshana-mcp), PyPI, npm,
   JetBrains, ghcr (anonymous pull), the MCP registry, the GitHub Release (assets, SHA256SUMS, attestations) and kshana.dev.
   kshana-mcp 0.34.1 requires engine ^0.34.1, so it cannot be built against an engine without the check.
2. Advisory published: GHSA-8wpf-hqg5-6xjr (https://github.com/ashfordeOU/kshana/security/advisories/GHSA-8wpf-hqg5-6xjr),
   2026-10-11. Package kshana-mcp; affected `>= 0.27.0, < 0.34.1`; patched 0.34.1; Medium, CVSS 3.1 4.3
   (`CVSS:3.1/AV:N/AC:L/PR:N/UI:R/S:U/C:L/I:N/A:N`); CWE-73 and CWE-209. A CVE was requested and was not assigned at the time of
   publication.
3. RustSec: entry prepared for the founder to submit to rustsec/advisory-db (crates/kshana-mcp/RUSTSEC-0000-0000.md, aliases the GHSA
   ID, date 2026-10-11); the ID is assigned on merge.

## Scope widened (W18 audit, 2026-10-09, feat/surfaces-0.35 589725fa)
The same class affects the general scenario tools, not only `assess_receiver_log`: `run_scenario`,
`validate_scenario`, `report_scenario`, `animate_scenario`, `list_export_formats`, `export_interop`,
`import_route`, `export_sp3`, `export_omm`, `export_oem`, `export_table_csv`. Scenario kinds with
file-source fields reachable through them: telecom-timing `csv_path`; spectrum recording `meta_path`,
`data_path`; lunar-service `ephemeris_path`; lunar kernel paths (`planetary_kernel_path`,
`earth_orientation_kernel_path`, `moon_orientation_kernel_path`) in lunar-service, lunar-vlbi,
lunar-vlbi-fim, lunar-llr; lunar-llr `data_dir`, `normal_points_dir`, `reflectors_path`,
`stations_path`; realtime-frame-eop `eop_finals2000a`, `eop_finals2000a_later`; receiver-trust
`[log] path`, `[log.nav] path`; campaign members recursively.
Not affected: the IQ tools (opt-in work directory, canonicalised and contained), the bundled-example
tools, and every new 0.35 tool (inline only). No network or process access is reachable from any tool.
Fix: one guard (`surface::reject_file_sources`, `FILE_SOURCE_KEYS`) refuses these fields by name in
every scenario tool, 4 MiB input caps, and a source-scanning test that fails if a scenario type gains
an unlisted path-like field.
Affected versions: to be re-dated per field (the general tools predate receiver-trust, so the range
likely starts before 0.31.0). Impact: being re-assessed per field (whether file values can flow into
tool output) before the severity is set.

## Impact re-assessed (W18 source reading, 2026-10-09)
Every field: the error text reveals whether any path exists and is readable.
File values can reach tool output:
- spectrum `meta_path` / `data_path`: the data file is read as I/Q samples and its averaged power
  spectrum is returned; a short file leaks most of its byte values; meta fields are echoed.
- telecom-timing `csv_path`: two-column numeric rows become the analysed series.
- realtime-frame-eop EOP files, lunar-service `ephemeris_path` (also echoes `# key: value` header
  comments as free text), lunar-llr normal points: numeric values flow into results.
- lunar-llr `reflectors_path` / `stations_path`: parse errors quote the offending line verbatim, so a
  single line of ANY file can be returned; station names and ids appear in output.
- kernel path fields: an arbitrary file leaks its first 8 bytes via the error text.
- receiver-trust `[log]` paths: a log-format file's per-epoch values flow into results.
Revised severity (provisional): MEDIUM. A local MCP server, but the trigger is agent input, so
prompt injection from any content the agent reads can request a file and the agent may relay the
output. CWE-22, CWE-73, CWE-209 (error text quoting file lines).
Recommendation to the founder: a 0.34.1 patch release carrying only the guard (refuse file-source
fields by name in every scenario tool, input caps, the source-scanning test), cut from the v0.34.0
tag, then a GitHub security advisory for kshana-mcp once 0.34.1 is published.
