# D14: MCP `assess_receiver_log` reads any local file named in agent-supplied TOML

Status: OPEN, SCOPE WIDENED 2026-10-09 (fix written on public branch feat/surfaces-0.35, ships in 0.35.0). Raised 2026-10-09
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

## Open decisions (founder)
1. Ship the fix in 0.35.0 (default) or a 0.34.1 patch.
2. Whether to publish a GitHub security advisory for kshana-mcp (low severity) once the fix is released.

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
