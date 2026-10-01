/* tslint:disable */
/* eslint-disable */

/**
 * Run a scenario and return its SVG chart.
 */
export function chart_svg(toml: string): string;

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
export function error_kind(toml: string): string;

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
 * Export a propagated constellation scenario as an **SP3-c** precise-ephemeris string
 * (the same artifact the CLI `--export-sp3` flag writes). Pure client-side; nothing is
 * uploaded. Throws a JS error if the scenario cannot produce an SP3 (e.g. a non-orbit kind).
 */
export function export_sp3(toml: string): string;

/**
 * List the available scenario kinds and their metadata as a JSON array (name,
 * description, required and optional fields), for programmatic introspection.
 */
export function list_kinds(): string;

/**
 * Run a scenario given as a TOML string; returns the result document as a JSON
 * string. Throws a JS error if the scenario is invalid.
 */
export function run(toml: string): string;

/**
 * Run a scenario ONCE and return every output of that run as a JSON object string:
 * `{"json": <result document string>, "svg": <chart>, "summary": <one line>,
 * "csv": <table or null>}`. `run`, `chart_svg`, `summary` and `table_csv` each execute
 * the scenario from scratch, so a page that wants all four paid for four full engine
 * runs — about 12 s instead of 3 s for the heaviest kind in a browser. Throws a JS
 * error if the scenario is invalid.
 */
export function run_all(toml: string): string;

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
 * Engine version (the crate version).
 */
export function version(): string;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly chart_svg: (a: number, b: number) => [number, number, number, number];
    readonly decode_permalink: (a: number, b: number) => [number, number];
    readonly encode_permalink: (a: number, b: number) => [number, number];
    readonly error_kind: (a: number, b: number) => [number, number];
    readonly export_oem: (a: number, b: number) => [number, number, number, number];
    readonly export_omm: (a: number, b: number) => [number, number, number, number];
    readonly export_sp3: (a: number, b: number) => [number, number, number, number];
    readonly list_kinds: () => [number, number];
    readonly run: (a: number, b: number) => [number, number, number, number];
    readonly run_all: (a: number, b: number) => [number, number, number, number];
    readonly summary: (a: number, b: number) => [number, number, number, number];
    readonly table_csv: (a: number, b: number) => [number, number, number, number];
    readonly version: () => [number, number];
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
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
