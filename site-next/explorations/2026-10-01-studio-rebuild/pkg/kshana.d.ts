/* tslint:disable */
/* eslint-disable */
// SPDX-License-Identifier: AGPL-3.0-only
// Typed declarations appended to the wasm-pack `kshana.d.ts` (CAP-34), through
// `typescript_custom_section` in src/wasm.rs. The three closed vocabularies below are
// kept equal to the engine's own lists by tests/typed_api_sync.rs: the scenario kinds
// (`api::list_scenario_kinds`), the error tags (`KshanaError::kind_tag`) and the
// provenance classes (`field_schema::ProvenanceClass::ALL`).

/** Every built-in scenario kind, the value of a scenario's top-level `kind`. */
export type ScenarioKind =
| "clock"
| "inertial"
| "orbit"
| "ephemeris"
| "integrity"
| "lunar-integrity"
| "lunar-time-offset"
| "lunar-vlbi"
| "lunar-joint-od-clock"
| "lunar-frame-realisation"
| "moonlight-service-volume"
| "lunar-differential-pnt"
| "lunar-station-coverage"
| "lunar-interop-export"
| "timetransfer"
| "quantum-anomaly-detect"
| "quantum-gnss-free-nav"
| "quantum-time-transfer"
| "hybrid"
| "fusion"
| "hybrid-ukf"
| "gnss-ins"
| "gnss-sim"
| "jamming"
| "spoof"
| "spoof-detect"
| "sweep"
| "sweep-nd"
| "gravity-map"
| "terrain-nav"
| "terrain-slam"
| "combined-altpnt"
| "pvt"
| "mars-pnt"
| "impairment-eval"
| "quantum-trade"
| "space-weather"
| "oem-interop"
| "launch-window"
| "reentry"
| "eo-coverage"
| "space-packet"
| "attitude-budget"
| "passes"
| "link-budget"
| "lunar-time-budget"
| "hybrid-optical-rf"
| "cislunar-observability"
| "conflict-resilience"
| "lunar-attack-surface"
| "realtime-frame-eop"
| "aperture-duty-cycle"
| "lunar-jamming"
| "ins-trn-coast"
| "lunar-vlbi-fim"
| "tracking-loop"
| "araim-reference-check"
| "lunar-frame-campaign"
| "lunar-llr-datum"
| "lunar-beacon"
| "earth-gnss-lunar"
| "lunar-infrastructure"
| "cislunar-arc-recovery"
| "telecom-timing"
| "slot-timing"
| "spectrum"
| "solar-system"
| "body-pnt"
| "constellation-design"
| "campaign"
| "leo-signal"
| "leo-pass"
| "leo-navmsg"
| "leo-pvt"
| "leo-ppp"
| "ntn-positioning"
| "leo-pnt-chain"
| "contact-schedule"
| "acm-link"
| "aocs"
| "entry-heating"
| "eo-radiometry"
| "ascent-losses"
| "time-transfer-budget"
| "optical-atmosphere"
| "holdover-test"
| "gnss-ssv";

/** The failure category `error_kind` returns (`""` on success). */
export type ErrorKind = "invalid_input" | "non_convergence" | "unsupported" | "io_error";

/** One entry of `scenario_kinds()`. */
export interface KindMeta {
    name: ScenarioKind;
    description: string;
    required_fields: string[];
    optional_fields: string[];
}

/** Where an emitted number came from (`docs/SCHEMA.md`). */
export type ProvenanceClass =
| "input"
| "modelled-input"
| "computed"
| "closed-form"
| "spec"
| "published"
| "constant"
| "modelled"
| "internal-consistency"
| "measured"
| "measured-or-input"
| "derived";

/** One entry of a result document's `units` block. */
export interface FieldUnit {
    unit: string;
    provenance: ProvenanceClass;
    note?: string;
}

/**
 * A parsed result document. Every kind has its own fields (`docs/SCHEMA.md`,
 * `docs/field-units-schema.json`); the ones named here are the shared envelope.
 */
export interface ResultDocument {
    [field: string]: unknown;
    schema_version?: string;
    engine_version?: string;
    units?: Record<string, FieldUnit>;
}

/** What `run_typed` returns: one run's outputs, with the document already parsed. */
export interface RunOutput {
    json: string;
    svg: string;
    summary: string;
    csv: string | null;
    data: ResultDocument;
}

/** The object `run_all` encodes as JSON. */
export interface RunAllOutput {
    json: string;
    svg: string;
    summary: string;
    csv: string | null;
}



/**
 * Run a scenario and return its time series as an animation. `format` is `svg`, `html` or
 * `frames` (empty for `svg`); `fps` 0, `duration_s` NaN and `width` 0 mean the defaults (12,
 * 8 s, 960 px). Returns JSON `{summary, files: {name: text}}`; a kind with no sampled time
 * axis throws. Nothing is uploaded or written.
 */
export function animate_scenario(toml: string, format: string, fps: number, duration_s: number, width: number): string;

/**
 * Assess a vessel's NMEA 0183 log as a batch run: `session_toml` (`[platform] kind =
 * "vessel"`, no `[log]` needed) and the log text. Returns the result document as JSON, the
 * same as `kshana receiver-trust` writes to `result.json`. Advisory software, not type-approved navigation equipment (IEC 61108, IEC 61162): the operator remains responsible for the navigation of the vessel.
 */
export function assess_vessel_log(session_toml: string, log: string): string;

/**
 * Export a scenario's vehicle motion and events for a laboratory GNSS simulator
 * (`docs/TEST-BENCH.md`), in memory: nothing is uploaded or written. `epoch` is the UTC
 * instant of motion time zero, `YYYY-MM-DDTHH:MM:SS` with an optional `Z` (empty for the
 * default 2024-01-01T00:00:00Z). Returns JSON `{files: {suffix: text}, notes, notice}`; keep
 * `notice` with the files. No radio-frequency or baseband signal is written.
 */
export function bench_export(toml: string, epoch: string): string;

/**
 * Render kshana-bench result files (a JSON array of result texts) as the static leaderboard
 * page (HTML).
 */
export function bench_leaderboard(results_json: string): string;

/**
 * Score a kshana-bench predictions file against a truth file (JSON texts) and return the
 * result file as JSON text. `threshold` is the declared alarm threshold of tasks a and b.
 */
export function bench_score(truth_json: string, predictions_json: string, threshold?: number | null): string;

/**
 * Run a scenario and return its SVG chart.
 */
export function chart_svg(toml: string): string;

/**
 * The static public-framework mapping as Markdown, led by the statement every report
 * carries: the tables (`sources` false) or the source documents they cite (`sources` true).
 */
export function compliance_mapping(sources: boolean): string;

/**
 * Fill the public-framework mapping from result documents: which rows of six resilience
 * frameworks and standards (`docs/compliance/`) the runs support evidence for, which they
 * do not, and the gap each row keeps. `runs_json` is a JSON array of `{label, result,
 * scenario?}` where `result` is the result JSON text and `scenario` the scenario TOML text
 * (it names the scenario kind a result does not); at most 64 runs. Returns JSON
 * `{report, markdown}`. The report carries `statement` verbatim: a status says the runs
 * support evidence for a row's capabilities, not that a framework is met. Nothing is
 * uploaded or read from disk.
 */
export function compliance_report(runs_json: string): string;

/**
 * Decode a permalink token back into the scenario TOML; returns an empty string if the
 * token is not valid Base64 or not valid UTF-8.
 */
export function decode_permalink(token: string): string;

/**
 * Encode a scenario TOML into a URL-safe permalink token for a `?s=` query parameter.
 */
export function encode_permalink(toml: string): string;

/**
 * Run a scenario; on failure return the structured error *kind* tag
 * (`invalid_input`, `unsupported`, …) so the caller can branch on the failure
 * category rather than parse the message. Returns an empty string on success.
 */
export function error_kind(toml: string): ErrorKind | "";

/**
 * Bind an RFC 3161 timestamp token to an evidence pack, in memory. `files_json` is the pack's
 * files as JSON (name to text or `{"base64": ...}`); `token_base64` is the `.tsr` file's bytes
 * as base64; `replace` allows replacing an existing token. The pack with the token must still
 * verify or this throws. Returns JSON `{files, notes}`. This does NOT verify the timestamp
 * authority's signature or certificate chain (use `openssl ts -verify`). Nothing is uploaded.
 */
export function evidence_attach_timestamp(files_json: string, token_base64: string, replace: boolean): string;

/**
 * Verify an evidence pack. `files_json` is a JSON object of file name to content: a string,
 * `{"utf8": text}` or `{"base64": bytes}`. `public_key` is the signer's key as 64 hex digits,
 * obtained from the signer by another route (empty for none: the signature then proves only
 * that the pack is intact against the key it names itself). `full_log` is the full log text
 * (empty for none). `require_timestamp` fails a pack without a timestamp token. Returns the
 * report as JSON: `ok`, `failures`, `checks`, `signer_fingerprint`, `signer_pinned`, `notes`,
 * plus `verdict` (`"verified"`, `"intact-signer-not-pinned"` or `"failed"`) and `message`. Nothing is uploaded; packs are made with the
 * command line, Python or the MCP server.
 */
export function evidence_verify(files_json: string, public_key: string, full_log: string, require_timestamp: boolean): string;

/**
 * Which interoperability formats apply to a scenario, without running it: a JSON array of
 * `{format, applies, reason, spec_url}` for `czml`, `kml`, `geojson`, `stk` and `sigmf`.
 */
export function export_formats(toml: string): string;

/**
 * Export the velocity-carrying state as a **CCSDS OEM 2.0** ephemeris string for
 * flight-dynamics tools (GMAT / Orekit / STK; the CLI `--export-oem` artifact). Pure
 * client-side.
 */
export function export_oem(toml: string): string;

/**
 * Export a constellation's mean elements as a **CCSDS OMM** catalogue string (one OMM
 * message per TLE-defined satellite; the CLI `--export-omm` artifact). Pure client-side.
 */
export function export_omm(toml: string): string;

/**
 * Export a scenario in one interoperability format (`czml`, `kml`, `geojson`, `stk` or
 * `sigmf`), in memory. Returns JSON `{format, spec_url, files}`; each file is `{suffix, bytes,
 * sha256, encoding, content}` with `encoding` `"utf-8"` (the text) or `"base64"` (a binary
 * file). Times are UTC; the same scenario gives byte-identical files. Nothing is uploaded.
 */
export function export_scenario(toml: string, format: string): string;

/**
 * Export a propagated constellation scenario as an **SP3-c** precise-ephemeris string
 * (the same artifact the CLI `--export-sp3` flag writes). Pure client-side; nothing is
 * uploaded. Throws a JS error if the scenario cannot produce an SP3 (e.g. a non-orbit kind).
 */
export function export_sp3(toml: string): string;

/**
 * Which fields of a scenario answer its question, best first, for the kinds where that is stated:
 * a JSON object, kind to `{"headline": [dotted path, ...]}`. Display metadata for a front end; the
 * engine never reads it. A kind that is absent has no stated order.
 */
export function field_importance(): string;

/**
 * The TOML text of one bundled reference scenario, byte for byte the repository's file.
 */
export function get_example(name: string): string;

/**
 * Write a GeoJSON route into a scenario of a kind that flies a waypoint track and return the
 * new TOML (the command line's `--import-route`).
 */
export function import_route(toml: string, geojson: string): string;

/**
 * Fit the accelerometer and gyro parameters of the `hybrid` and `fusion` scenarios from a
 * stationary IMU log (`kshana imu-fit`, docs/MACHINE-INPUTS.md). `log` is the CSV text;
 * `format` is `"csv"` or `"euroc"`; `axis` is `"x"`, `"y"` or `"z"`; `expected_mean` is the
 * specific force that axis reads at rest (0 horizontal, about 9.80665 vertical). Returns JSON:
 * `fit`, a ready-to-paste `[accel_classical]` `toml` and the log's `sha256`. MODELLED.
 */
export function imu_fit(log: string, format: string, axis: string, expected_mean: number, id: string): string;

/**
 * Build a GNSS interference map from CSV text (`source` is `"adsb"` or `"ais"`; formats in
 * `docs/INTERFERENCE-MAP.md`). `dataset` is an approved preset or `"custom"`, which also
 * needs `licence`, `licence_url` and `attribution`; pass an empty string for any field
 * that does not apply, and `NaN` for the default 0.5 degree cell. `land_geojson` (AIS only,
 * empty for none) is a land-polygon file. Returns a JSON array of
 * `{file_name, date, cells_published, cells_flagged, geojson}`, one per UTC day. Aggregate
 * only; a degraded cell does not name interference as the cause. Nothing is uploaded.
 */
export function interference_map(source: string, csv: string, dataset: string, cell_deg: number, licence: string, licence_url: string, attribution: string, land_geojson: string): string;

/**
 * A short synthetic scene, acquired and tracked in memory: `ids` are the PRNs (GLONASS L1OF:
 * the frequency channel), `dopplers_hz` one value for all or one each, `cn0_dbhz` NaN for the
 * elevation default. Bounded to 0.25 s, 1 000 000 samples and 8 satellites. Returns JSON
 * `{fs_hz, samples, acquisition, tracking}`; the same request gives the same answer as the
 * native build.
 */
export function iq_run_short(fs_hz: number, duration_s: number, signal: string, ids: Float64Array, dopplers_hz: Float64Array, cn0_dbhz: number, seed: number): string;

/**
 * The signal names `iq_run_short` accepts: a JSON array.
 */
export function iq_signals(): string;

/**
 * Read the published Jammertest catalogue and transmission plan as scheduled truth
 * (`kshana jammertest import`). `plans_json` is a JSON array of the plan files' texts,
 * `local_offset_h` the plan's offset from UTC (CEST: 2), `log_start_utc` the log's first epoch
 * (ISO-8601 UTC); `location` and `year` are 0 for "not given". Returns JSON `{events_toml,
 * events, skipped_non_rf, outside_log, unknown_ids}`. Planned slots, not recorded on-air times.
 */
export function jammertest_import(catalog_json: string, plans_json: string, log_start_utc: string, location: number, local_offset_h: number, year: number): string;

/**
 * Score a receiver log against the Jammertest schedule (`kshana jammertest score`). The session
 * needs only `[log] format = "ubx"` (or `rinex`, `android`, `nmea`, `sbf`); `log_base64` is the
 * log's bytes; `log_start_utc` is empty when the log states its own UTC start; `location` and
 * `year` are 0 for "not given". Returns the scorecard as JSON, labelled MODELLED. Signed packs
 * are made with the command line.
 */
export function jammertest_score(session_toml: string, log_base64: string, catalog_json: string, plans_json: string, location: number, local_offset_h: number, log_start_utc: string, year: number, recover_hold_s: number, guard_s: number): string;

/**
 * The modules a scenario kind's verification-matrix rows name where they differ from the kind's
 * own name: a JSON object, kind to array of module names. The Studio's capability map joins a
 * scenario to its matrix rows through it.
 */
export function kind_modules(): string;

/**
 * The bundled reference scenarios as JSON `{count, scenarios: [{name, kind, about}]}`; a
 * non-empty `kind` limits the list to one scenario kind.
 */
export function list_examples(kind: string): string;

/**
 * List the available scenario kinds and their metadata as a JSON array (name,
 * description, required and optional fields), for programmatic introspection.
 */
export function list_kinds(): string;

/**
 * Check this implementation of the LunaNet Augmented Forward Signal against the published
 * tables and worked examples of LSIS-AFS V1.0 (LNIS V5): JSON with `bars` (B1 to B10), `all_met`,
 * `pin` and `status`.
 */
export function lunanet_afs_check_spec(): string;

/**
 * Check an AFS symbol stream given as text of `0` and `1` against section 2.4 of LSIS-AFS V1.0:
 * JSON with the frames found and any problems. Subframes 2 to 4 (LDPC, CRC-24Q) are not
 * verified; the report says so.
 */
export function lunanet_afs_check_symbols(symbols: string, max_sp_errors: number): string;

/**
 * A test stream of `frames` (1 to 100) AFS frames as text of `0` and `1` with placeholder
 * subframes 2 to 4 (not LDPC coded, no message): symbols, not a waveform.
 */
export function lunanet_afs_generate(frames: number, toi0: number): string;

/**
 * Generate synthetic bridge NMEA 0183 for crew training from a `nmea-scenario` TOML.
 * `seed` replaces the scenario's seed unless it is `NaN` or negative. Returns a JSON object
 * `{nmea, log_json, log_text}`. Text only; never for a vessel's live navigation systems.
 * Advisory, not type-approved navigation equipment.
 */
export function nmea_training(toml: string, seed: number): string;

/**
 * Check Galileo OSNMA authentication of I/NAV pages, in memory, with the verifier
 * `kshana osnma verify` runs. Advisory: this is analysis and monitoring output and not
 * type-approved navigation equipment. The authentication status reported here is not a
 * navigation integrity service and must not be the sole basis for any navigation or safety
 * decision. `input_base64` is the pages (plain text or the published vector CSV) or a u-blox
 * UBX byte stream; `format` is `"pages"`, `"vector-csv"`, `"ubx"` or `""` (by content);
 * `start_gst` is a vector CSV's start in GST seconds; `merkle_root_hex` is the trusted Merkle
 * root (64 hex digits, or `""`); `public_keys` are trusted keys as `ID:TYPE:HEX`;
 * `reference_time` is the present time in GST seconds from an independent source and
 * `max_time_error` the widest distance of a sub-frame from it (below 30; needs a reference
 * time). Without a reference time freshness is not enforced and the report says so. Returns
 * the JSON `kshana osnma verify --json` prints. Nothing is uploaded.
 */
export function osnma_verify(input_base64: string, format: string, start_gst: number | null | undefined, merkle_root_hex: string, public_keys: string[], reference_time?: number | null, max_time_error?: number | null): string;

/**
 * Assess a real receiver log described by a `receiver-trust` scenario (TOML text). The
 * browser has no file system, so the log (and any navigation file) must be inline as
 * `text` or `base64`. Returns a JSON object `{json, csv, svg, summary}`.
 */
export function receiver_trust(toml: string): string;

/**
 * Score a bounded excerpt of a vessel's NMEA 0183 stream the way `kshana receiver-trust
 * live` scores it (at most 2 MiB and 20,000 epochs; it must hold the calibration window).
 * `session_toml` declares a vessel (`[platform] kind = "vessel"`). Returns a JSON object
 * `{schema, epochs, last_pksht, summary}` (`schema` is `"1.3"`): one report per epoch (`state`, `score`, `advisory`,
 * `deductions` with a `class` each, `alarms`, `geometry`, ...) and counts by state, geometry and
 * class. The geometry is `good`, `degraded` with reasons, or `unknown` without GSV or GSA, with a
 * `label` (`good`, `fair`, `poor`) always reported; the score deducts for geometry only when it
 * is `poor`. The bounded form of the live command: no
 * socket is opened and the gate is not applied (both are command-line only). Advisory software, not type-approved navigation equipment (IEC 61108, IEC 61162): the operator remains responsible for the navigation of the vessel.
 */
export function receiver_trust_replay(session_toml: string, nmea: string): string;

/**
 * Share of a route (GeoJSON LineString or `lat,lon` CSV text) through degraded cells of the
 * maps in `maps_json` (a JSON array of map GeoJSON strings, as `interference_map` returns),
 * optionally limited to `date_from`..`date_to` (`YYYY-MM-DD`, empty for no limit). Returns
 * the report as JSON text. Not a forecast; unobserved cells are not evidence of a clear
 * route.
 */
export function route_exposure(route: string, maps_json: string, date_from: string, date_to: string): string;

/**
 * Run a scenario given as a TOML string; returns the result document as a JSON
 * string. Throws a JS error if the scenario is invalid.
 */
export function run(toml: string): string;

/**
 * Run a scenario ONCE and return every output of that run as a JSON object string:
 * `{"json": <result document string>, "svg": <chart>, "summary": <one line>,
 * "csv": <table or null>}` (the `RunAllOutput` type). `run_typed` returns the same
 * outputs, plus the parsed document, as an object. `run`, `chart_svg`, `summary` and `table_csv` each execute
 * the scenario from scratch, so a page that wants all four paid for four full engine
 * runs — about 12 s instead of 3 s for the heaviest kind in a browser. Throws a JS
 * error if the scenario is invalid.
 */
export function run_all(toml: string): string;

/**
 * Run a scenario once and return every output as one typed object (`RunOutput` in the
 * declarations): `json`, `svg`, `summary`, `csv` (`null` for a kind that emits no
 * table) and `data`, the result document already parsed. A plain object, not a class,
 * so there is nothing to free. Throws a JS error if the scenario is invalid.
 */
export function run_typed(toml: string): RunOutput;

/**
 * The available scenario kinds and their metadata, already parsed: the typed form of
 * [`list_kinds`], which keeps its JSON-string return for existing callers.
 */
export function scenario_kinds(): KindMeta[];

/**
 * The satellites a site sees under a skyline and their geometry (design note item 2).
 * `request_json` is a JSON (or TOML) object: `site` (`{lat_deg, lon_deg, height_m}`),
 * `time_s`, `systems` (preset names such as `"gps-baseline"`, `"galileo"`, `"beidou-meo"`,
 * `"xona-pulsar"`, or `leo-pvt` `[[system]]` objects) and/or `satellites` (`{id?, system?,
 * az_deg, el_deg}` or `{id?, system?, ecef_m}`), `mask_deg`, `skyline` (`preset`,
 * `street_az_deg`, `width_m`, `height_left_m`, `height_right_m`, `antenna_m`, `length_m`,
 * `table`, `sectors`, `terrain`), `clock` (`"per-system"` default, or `"common"`),
 * `sigma_uere_m` (default 1) and `confidence` (default 0.95). Returns a JSON object: per
 * satellite `id`, `system`, `az_deg`, `el_deg`, `cut_deg`, `visible`, `masked`; `n_visible`,
 * `n_masked`, `open_fraction`; `dop` (`gdop`, `pdop`, `hdop`, `vdop`, `tdop`,
 * `tdop_by_clock`, or null); `states`, `cofactor_enu`, `covariance_enu_m2`; `ellipse_1sigma`
 * and `ellipse` (`semi_major_m`, `semi_minor_m`, `orientation_deg`, ...); `label`
 * (MODELLED), `statement` and `advisory`. Throws a JS error on an invalid request.
 */
export function sky_geometry(request_json: string): string;

/**
 * `n` (1 to 5000) least-squares position errors under independent Gaussian range errors of
 * `sigma_uere_m` on the visible set of `request_json` (as for `sky_geometry`), seeded by
 * `seed` (a non-negative integer; the same request, seed and `n` give the same cloud). Returns
 * a JSON object: `samples_enu_m` (`[[e, n, u], ...]`), `empirical_covariance_enu_m2`,
 * `rms_horizontal_m`, `rms_3d_m`, `predicted_rms_horizontal_m` (sigma x HDOP),
 * `predicted_rms_3d_m`, `fraction_inside_95`, `label`, `statement`, `advisory`. Throws a JS
 * error on an invalid request, a seed that is not a non-negative integer, or `n` out of range.
 */
export function sky_monte_carlo(request_json: string, seed: number, n: number): string;

/**
 * The named skyline presets (`city-street`, `harbour-approach`, `fjord`,
 * `container-terminal`) as a JSON array: `name`, `label` (REPRESENTATIVE), `description`,
 * `provenance`, `table` (the cut every degree of azimuth from north, 360 numbers) and
 * `open_fraction`.
 */
export function skyline_presets(): string;

/**
 * A skyline section (`skyline_json`, the `skyline` object of a `sky_geometry` request) reduced
 * to the normalised table the engine uses: a JSON object with `table` (360 numbers, the cut
 * every degree of azimuth from north), `open_fraction`, `source` and `provenance`. Throws a
 * JS error on an invalid section.
 */
export function skyline_profile(skyline_json: string): string;

/**
 * The skyplot of `request_json` (as for `sky_geometry`) as static SVG markup (no script, no
 * external reference; every input string escaped). Throws a JS error on an invalid request.
 */
export function skyplot_svg(request_json: string): string;

/**
 * The standards packs (`docs/compliance/`): JSON `{packs, notice}`. A pack measures modelled
 * scenarios against bars stated before the run and rates no product.
 */
export function standards_pack_list(): string;

/**
 * Run one standards pack by id. Returns JSON `{report, markdown}`; a test that needs a
 * receiver log is `NEEDS INPUT` and not scored. Nothing is uploaded or read from disk.
 */
export function standards_pack_run(id: string): string;

/**
 * Run a scenario and return its one-line human-readable summary.
 */
export function summary(toml: string): string;

/**
 * Run a scenario and return its reproducibility table as CSV text — the same bytes the
 * CLI writes as `<scenario>.table.csv`. Returns `undefined` for kinds that publish no
 * table (only `realtime-frame-eop`, `lunar-time-budget`, `lunar-jamming`, and
 * `moonlight-service-volume` with an export site configured emit one). Throws a JS error
 * if the scenario is invalid.
 */
export function table_csv(toml: string): string | undefined;

/**
 * Render a trust stream or a batch result in one telemetry format, with the encoders
 * `kshana trust-telemetry` uses (no process, socket or clock). `format` is `"prometheus"`,
 * `"cef"`, `"leef"`, `"otlp"` or `"mqtt"`; `input_kind` is `"result"` (a `receiver-trust` result
 * document) or `"jsonl"` (the live per-epoch stream); `host` names the sender in syslog
 * events (empty for `kshana`). `options_json` is `""` or an object with `expose_position`
 * (bool), `syslog_all` (bool: one event per epoch, not per band change) and `unix_nano`
 * (number, the time of export for `"otlp"`). Syslog lines carry no timestamp. Returns the
 * formatted text.
 */
export function trust_telemetry(input: string, format: string, host: string, input_kind: string, options_json: string): string;

/**
 * Engine version (the crate version).
 */
export function version(): string;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly animate_scenario: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => [number, number, number, number];
    readonly assess_vessel_log: (a: number, b: number, c: number, d: number) => [number, number, number, number];
    readonly bench_export: (a: number, b: number, c: number, d: number) => [number, number, number, number];
    readonly bench_leaderboard: (a: number, b: number) => [number, number, number, number];
    readonly bench_score: (a: number, b: number, c: number, d: number, e: number, f: number) => [number, number, number, number];
    readonly chart_svg: (a: number, b: number) => [number, number, number, number];
    readonly compliance_mapping: (a: number) => [number, number];
    readonly compliance_report: (a: number, b: number) => [number, number, number, number];
    readonly decode_permalink: (a: number, b: number) => [number, number];
    readonly encode_permalink: (a: number, b: number) => [number, number];
    readonly error_kind: (a: number, b: number) => [number, number];
    readonly evidence_attach_timestamp: (a: number, b: number, c: number, d: number, e: number) => [number, number, number, number];
    readonly evidence_verify: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => [number, number, number, number];
    readonly export_formats: (a: number, b: number) => [number, number, number, number];
    readonly export_oem: (a: number, b: number) => [number, number, number, number];
    readonly export_omm: (a: number, b: number) => [number, number, number, number];
    readonly export_scenario: (a: number, b: number, c: number, d: number) => [number, number, number, number];
    readonly export_sp3: (a: number, b: number) => [number, number, number, number];
    readonly field_importance: () => [number, number];
    readonly get_example: (a: number, b: number) => [number, number, number, number];
    readonly import_route: (a: number, b: number, c: number, d: number) => [number, number, number, number];
    readonly imu_fit: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => [number, number, number, number];
    readonly interference_map: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number, m: number, n: number, o: number) => [number, number, number, number];
    readonly iq_run_short: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number) => [number, number, number, number];
    readonly iq_signals: () => [number, number];
    readonly jammertest_import: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => [number, number, number, number];
    readonly jammertest_score: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number, m: number, n: number, o: number) => [number, number, number, number];
    readonly kind_modules: () => [number, number];
    readonly list_examples: (a: number, b: number) => [number, number, number, number];
    readonly list_kinds: () => [number, number];
    readonly lunanet_afs_check_spec: () => [number, number];
    readonly lunanet_afs_check_symbols: (a: number, b: number, c: number) => [number, number, number, number];
    readonly lunanet_afs_generate: (a: number, b: number) => [number, number, number, number];
    readonly nmea_training: (a: number, b: number, c: number) => [number, number, number, number];
    readonly osnma_verify: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number) => [number, number, number, number];
    readonly receiver_trust: (a: number, b: number) => [number, number, number, number];
    readonly receiver_trust_replay: (a: number, b: number, c: number, d: number) => [number, number, number, number];
    readonly route_exposure: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => [number, number, number, number];
    readonly run: (a: number, b: number) => [number, number, number, number];
    readonly run_all: (a: number, b: number) => [number, number, number, number];
    readonly run_typed: (a: number, b: number) => [number, number, number];
    readonly sky_geometry: (a: number, b: number) => [number, number, number, number];
    readonly sky_monte_carlo: (a: number, b: number, c: number, d: number) => [number, number, number, number];
    readonly skyline_presets: () => [number, number];
    readonly skyline_profile: (a: number, b: number) => [number, number, number, number];
    readonly skyplot_svg: (a: number, b: number) => [number, number, number, number];
    readonly standards_pack_list: () => [number, number];
    readonly standards_pack_run: (a: number, b: number) => [number, number, number, number];
    readonly summary: (a: number, b: number) => [number, number, number, number];
    readonly table_csv: (a: number, b: number) => [number, number, number, number];
    readonly trust_telemetry: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number) => [number, number, number, number];
    readonly version: () => [number, number];
    readonly scenario_kinds: () => any;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
