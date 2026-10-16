/* @ts-self-types="./kshana.d.ts" */

/**
 * Run a scenario and return its time series as an animation. `format` is `svg`, `html` or
 * `frames` (empty for `svg`); `fps` 0, `duration_s` NaN and `width` 0 mean the defaults (12,
 * 8 s, 960 px). Returns JSON `{summary, files: {name: text}}`; a kind with no sampled time
 * axis throws. Nothing is uploaded or written.
 * @param {string} toml
 * @param {string} format
 * @param {number} fps
 * @param {number} duration_s
 * @param {number} width
 * @returns {string}
 */
export function animate_scenario(toml, format, fps, duration_s, width) {
    let deferred4_0;
    let deferred4_1;
    try {
        const ptr0 = passStringToWasm0(toml, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(format, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.animate_scenario(ptr0, len0, ptr1, len1, fps, duration_s, width);
        var ptr3 = ret[0];
        var len3 = ret[1];
        if (ret[3]) {
            ptr3 = 0; len3 = 0;
            throw takeFromExternrefTable0(ret[2]);
        }
        deferred4_0 = ptr3;
        deferred4_1 = len3;
        return getStringFromWasm0(ptr3, len3);
    } finally {
        wasm.__wbindgen_free(deferred4_0, deferred4_1, 1);
    }
}

/**
 * Assess a vessel's NMEA 0183 log as a batch run: `session_toml` (`[platform] kind =
 * "vessel"`, no `[log]` needed) and the log text. Returns the result document as JSON, the
 * same as `kshana receiver-trust` writes to `result.json`. Advisory software, not type-approved navigation equipment (IEC 61108, IEC 61162): the operator remains responsible for the navigation of the vessel.
 * @param {string} session_toml
 * @param {string} log
 * @returns {string}
 */
export function assess_vessel_log(session_toml, log) {
    let deferred4_0;
    let deferred4_1;
    try {
        const ptr0 = passStringToWasm0(session_toml, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(log, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.assess_vessel_log(ptr0, len0, ptr1, len1);
        var ptr3 = ret[0];
        var len3 = ret[1];
        if (ret[3]) {
            ptr3 = 0; len3 = 0;
            throw takeFromExternrefTable0(ret[2]);
        }
        deferred4_0 = ptr3;
        deferred4_1 = len3;
        return getStringFromWasm0(ptr3, len3);
    } finally {
        wasm.__wbindgen_free(deferred4_0, deferred4_1, 1);
    }
}

/**
 * Export a scenario's vehicle motion and events for a laboratory GNSS simulator
 * (`docs/TEST-BENCH.md`), in memory: nothing is uploaded or written. `epoch` is the UTC
 * instant of motion time zero, `YYYY-MM-DDTHH:MM:SS` with an optional `Z` (empty for the
 * default 2024-01-01T00:00:00Z). Returns JSON `{files: {suffix: text}, notes, notice}`; keep
 * `notice` with the files. No radio-frequency or baseband signal is written.
 * @param {string} toml
 * @param {string} epoch
 * @returns {string}
 */
export function bench_export(toml, epoch) {
    let deferred4_0;
    let deferred4_1;
    try {
        const ptr0 = passStringToWasm0(toml, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(epoch, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.bench_export(ptr0, len0, ptr1, len1);
        var ptr3 = ret[0];
        var len3 = ret[1];
        if (ret[3]) {
            ptr3 = 0; len3 = 0;
            throw takeFromExternrefTable0(ret[2]);
        }
        deferred4_0 = ptr3;
        deferred4_1 = len3;
        return getStringFromWasm0(ptr3, len3);
    } finally {
        wasm.__wbindgen_free(deferred4_0, deferred4_1, 1);
    }
}

/**
 * Run a scenario and return its SVG chart.
 * @param {string} toml
 * @returns {string}
 */
export function chart_svg(toml) {
    let deferred3_0;
    let deferred3_1;
    try {
        const ptr0 = passStringToWasm0(toml, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.chart_svg(ptr0, len0);
        var ptr2 = ret[0];
        var len2 = ret[1];
        if (ret[3]) {
            ptr2 = 0; len2 = 0;
            throw takeFromExternrefTable0(ret[2]);
        }
        deferred3_0 = ptr2;
        deferred3_1 = len2;
        return getStringFromWasm0(ptr2, len2);
    } finally {
        wasm.__wbindgen_free(deferred3_0, deferred3_1, 1);
    }
}

/**
 * The static public-framework mapping as Markdown, led by the statement every report
 * carries: the tables (`sources` false) or the source documents they cite (`sources` true).
 * @param {boolean} sources
 * @returns {string}
 */
export function compliance_mapping(sources) {
    let deferred1_0;
    let deferred1_1;
    try {
        const ret = wasm.compliance_mapping(sources);
        deferred1_0 = ret[0];
        deferred1_1 = ret[1];
        return getStringFromWasm0(ret[0], ret[1]);
    } finally {
        wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
    }
}

/**
 * Fill the public-framework mapping from result documents: which rows of five resilience
 * frameworks and standards (`docs/compliance/`) the runs support evidence for, which they
 * do not, and the gap each row keeps. `runs_json` is a JSON array of `{label, result,
 * scenario?}` where `result` is the result JSON text and `scenario` the scenario TOML text
 * (it names the scenario kind a result does not); at most 64 runs. Returns JSON
 * `{report, markdown}`. The report carries `statement` verbatim: a status says the runs
 * support evidence for a row's capabilities, not that a framework is met. Nothing is
 * uploaded or read from disk.
 * @param {string} runs_json
 * @returns {string}
 */
export function compliance_report(runs_json) {
    let deferred3_0;
    let deferred3_1;
    try {
        const ptr0 = passStringToWasm0(runs_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.compliance_report(ptr0, len0);
        var ptr2 = ret[0];
        var len2 = ret[1];
        if (ret[3]) {
            ptr2 = 0; len2 = 0;
            throw takeFromExternrefTable0(ret[2]);
        }
        deferred3_0 = ptr2;
        deferred3_1 = len2;
        return getStringFromWasm0(ptr2, len2);
    } finally {
        wasm.__wbindgen_free(deferred3_0, deferred3_1, 1);
    }
}

/**
 * Decode a permalink token back into the scenario TOML; returns an empty string if the
 * token is not valid Base64 or not valid UTF-8.
 * @param {string} token
 * @returns {string}
 */
export function decode_permalink(token) {
    let deferred2_0;
    let deferred2_1;
    try {
        const ptr0 = passStringToWasm0(token, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.decode_permalink(ptr0, len0);
        deferred2_0 = ret[0];
        deferred2_1 = ret[1];
        return getStringFromWasm0(ret[0], ret[1]);
    } finally {
        wasm.__wbindgen_free(deferred2_0, deferred2_1, 1);
    }
}

/**
 * Encode a scenario TOML into a URL-safe permalink token for a `?s=` query parameter.
 * @param {string} toml
 * @returns {string}
 */
export function encode_permalink(toml) {
    let deferred2_0;
    let deferred2_1;
    try {
        const ptr0 = passStringToWasm0(toml, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.encode_permalink(ptr0, len0);
        deferred2_0 = ret[0];
        deferred2_1 = ret[1];
        return getStringFromWasm0(ret[0], ret[1]);
    } finally {
        wasm.__wbindgen_free(deferred2_0, deferred2_1, 1);
    }
}

/**
 * Run a scenario; on failure return the structured error *kind* tag
 * (`invalid_input`, `unsupported`, …) so the caller can branch on the failure
 * category rather than parse the message. Returns an empty string on success.
 * @param {string} toml
 * @returns {ErrorKind | ""}
 */
export function error_kind(toml) {
    let deferred2_0;
    let deferred2_1;
    try {
        const ptr0 = passStringToWasm0(toml, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.error_kind(ptr0, len0);
        deferred2_0 = ret[0];
        deferred2_1 = ret[1];
        return getStringFromWasm0(ret[0], ret[1]);
    } finally {
        wasm.__wbindgen_free(deferred2_0, deferred2_1, 1);
    }
}

/**
 * Bind an RFC 3161 timestamp token to an evidence pack, in memory. `files_json` is the pack's
 * files as JSON (name to text or `{"base64": ...}`); `token_base64` is the `.tsr` file's bytes
 * as base64; `replace` allows replacing an existing token. The pack with the token must still
 * verify or this throws. Returns JSON `{files, notes}`. This does NOT verify the timestamp
 * authority's signature or certificate chain (use `openssl ts -verify`). Nothing is uploaded.
 * @param {string} files_json
 * @param {string} token_base64
 * @param {boolean} replace
 * @returns {string}
 */
export function evidence_attach_timestamp(files_json, token_base64, replace) {
    let deferred4_0;
    let deferred4_1;
    try {
        const ptr0 = passStringToWasm0(files_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(token_base64, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.evidence_attach_timestamp(ptr0, len0, ptr1, len1, replace);
        var ptr3 = ret[0];
        var len3 = ret[1];
        if (ret[3]) {
            ptr3 = 0; len3 = 0;
            throw takeFromExternrefTable0(ret[2]);
        }
        deferred4_0 = ptr3;
        deferred4_1 = len3;
        return getStringFromWasm0(ptr3, len3);
    } finally {
        wasm.__wbindgen_free(deferred4_0, deferred4_1, 1);
    }
}

/**
 * Verify an evidence pack. `files_json` is a JSON object of file name to content: a string,
 * `{"utf8": text}` or `{"base64": bytes}`. `public_key` is the signer's key as 64 hex digits,
 * obtained from the signer by another route (empty for none: the signature then proves only
 * that the pack is intact against the key it names itself). `full_log` is the full log text
 * (empty for none). `require_timestamp` fails a pack without a timestamp token. Returns the
 * report as JSON: `ok`, `failures`, `checks`, `signer_fingerprint`, `signer_pinned`, `notes`,
 * plus `verdict` (`"verified"`, `"intact-signer-not-pinned"` or `"failed"`) and `message`. Nothing is uploaded; packs are made with the
 * command line, Python or the MCP server.
 * @param {string} files_json
 * @param {string} public_key
 * @param {string} full_log
 * @param {boolean} require_timestamp
 * @returns {string}
 */
export function evidence_verify(files_json, public_key, full_log, require_timestamp) {
    let deferred5_0;
    let deferred5_1;
    try {
        const ptr0 = passStringToWasm0(files_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(public_key, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ptr2 = passStringToWasm0(full_log, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len2 = WASM_VECTOR_LEN;
        const ret = wasm.evidence_verify(ptr0, len0, ptr1, len1, ptr2, len2, require_timestamp);
        var ptr4 = ret[0];
        var len4 = ret[1];
        if (ret[3]) {
            ptr4 = 0; len4 = 0;
            throw takeFromExternrefTable0(ret[2]);
        }
        deferred5_0 = ptr4;
        deferred5_1 = len4;
        return getStringFromWasm0(ptr4, len4);
    } finally {
        wasm.__wbindgen_free(deferred5_0, deferred5_1, 1);
    }
}

/**
 * Which interoperability formats apply to a scenario, without running it: a JSON array of
 * `{format, applies, reason, spec_url}` for `czml`, `kml`, `geojson`, `stk` and `sigmf`.
 * @param {string} toml
 * @returns {string}
 */
export function export_formats(toml) {
    let deferred3_0;
    let deferred3_1;
    try {
        const ptr0 = passStringToWasm0(toml, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.export_formats(ptr0, len0);
        var ptr2 = ret[0];
        var len2 = ret[1];
        if (ret[3]) {
            ptr2 = 0; len2 = 0;
            throw takeFromExternrefTable0(ret[2]);
        }
        deferred3_0 = ptr2;
        deferred3_1 = len2;
        return getStringFromWasm0(ptr2, len2);
    } finally {
        wasm.__wbindgen_free(deferred3_0, deferred3_1, 1);
    }
}

/**
 * Export the velocity-carrying state as a **CCSDS OEM 2.0** ephemeris string for
 * flight-dynamics tools (GMAT / Orekit / STK; the CLI `--export-oem` artifact). Pure
 * client-side.
 * @param {string} toml
 * @returns {string}
 */
export function export_oem(toml) {
    let deferred3_0;
    let deferred3_1;
    try {
        const ptr0 = passStringToWasm0(toml, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.export_oem(ptr0, len0);
        var ptr2 = ret[0];
        var len2 = ret[1];
        if (ret[3]) {
            ptr2 = 0; len2 = 0;
            throw takeFromExternrefTable0(ret[2]);
        }
        deferred3_0 = ptr2;
        deferred3_1 = len2;
        return getStringFromWasm0(ptr2, len2);
    } finally {
        wasm.__wbindgen_free(deferred3_0, deferred3_1, 1);
    }
}

/**
 * Export a constellation's mean elements as a **CCSDS OMM** catalogue string (one OMM
 * message per TLE-defined satellite; the CLI `--export-omm` artifact). Pure client-side.
 * @param {string} toml
 * @returns {string}
 */
export function export_omm(toml) {
    let deferred3_0;
    let deferred3_1;
    try {
        const ptr0 = passStringToWasm0(toml, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.export_omm(ptr0, len0);
        var ptr2 = ret[0];
        var len2 = ret[1];
        if (ret[3]) {
            ptr2 = 0; len2 = 0;
            throw takeFromExternrefTable0(ret[2]);
        }
        deferred3_0 = ptr2;
        deferred3_1 = len2;
        return getStringFromWasm0(ptr2, len2);
    } finally {
        wasm.__wbindgen_free(deferred3_0, deferred3_1, 1);
    }
}

/**
 * Export a scenario in one interoperability format (`czml`, `kml`, `geojson`, `stk` or
 * `sigmf`), in memory. Returns JSON `{format, spec_url, files}`; each file is `{suffix, bytes,
 * sha256, encoding, content}` with `encoding` `"utf-8"` (the text) or `"base64"` (a binary
 * file). Times are UTC; the same scenario gives byte-identical files. Nothing is uploaded.
 * @param {string} toml
 * @param {string} format
 * @returns {string}
 */
export function export_scenario(toml, format) {
    let deferred4_0;
    let deferred4_1;
    try {
        const ptr0 = passStringToWasm0(toml, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(format, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.export_scenario(ptr0, len0, ptr1, len1);
        var ptr3 = ret[0];
        var len3 = ret[1];
        if (ret[3]) {
            ptr3 = 0; len3 = 0;
            throw takeFromExternrefTable0(ret[2]);
        }
        deferred4_0 = ptr3;
        deferred4_1 = len3;
        return getStringFromWasm0(ptr3, len3);
    } finally {
        wasm.__wbindgen_free(deferred4_0, deferred4_1, 1);
    }
}

/**
 * Export a propagated constellation scenario as an **SP3-c** precise-ephemeris string
 * (the same artifact the CLI `--export-sp3` flag writes). Pure client-side; nothing is
 * uploaded. Throws a JS error if the scenario cannot produce an SP3 (e.g. a non-orbit kind).
 * @param {string} toml
 * @returns {string}
 */
export function export_sp3(toml) {
    let deferred3_0;
    let deferred3_1;
    try {
        const ptr0 = passStringToWasm0(toml, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.export_sp3(ptr0, len0);
        var ptr2 = ret[0];
        var len2 = ret[1];
        if (ret[3]) {
            ptr2 = 0; len2 = 0;
            throw takeFromExternrefTable0(ret[2]);
        }
        deferred3_0 = ptr2;
        deferred3_1 = len2;
        return getStringFromWasm0(ptr2, len2);
    } finally {
        wasm.__wbindgen_free(deferred3_0, deferred3_1, 1);
    }
}

/**
 * Which fields of a scenario answer its question, best first, for the kinds where that is stated:
 * a JSON object, kind to `{"headline": [dotted path, ...]}`. Display metadata for a front end; the
 * engine never reads it. A kind that is absent has no stated order.
 * @returns {string}
 */
export function field_importance() {
    let deferred1_0;
    let deferred1_1;
    try {
        const ret = wasm.field_importance();
        deferred1_0 = ret[0];
        deferred1_1 = ret[1];
        return getStringFromWasm0(ret[0], ret[1]);
    } finally {
        wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
    }
}

/**
 * The TOML text of one bundled reference scenario, byte for byte the repository's file.
 * @param {string} name
 * @returns {string}
 */
export function get_example(name) {
    let deferred3_0;
    let deferred3_1;
    try {
        const ptr0 = passStringToWasm0(name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.get_example(ptr0, len0);
        var ptr2 = ret[0];
        var len2 = ret[1];
        if (ret[3]) {
            ptr2 = 0; len2 = 0;
            throw takeFromExternrefTable0(ret[2]);
        }
        deferred3_0 = ptr2;
        deferred3_1 = len2;
        return getStringFromWasm0(ptr2, len2);
    } finally {
        wasm.__wbindgen_free(deferred3_0, deferred3_1, 1);
    }
}

/**
 * Write a GeoJSON route into a scenario of a kind that flies a waypoint track and return the
 * new TOML (the command line's `--import-route`).
 * @param {string} toml
 * @param {string} geojson
 * @returns {string}
 */
export function import_route(toml, geojson) {
    let deferred4_0;
    let deferred4_1;
    try {
        const ptr0 = passStringToWasm0(toml, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(geojson, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.import_route(ptr0, len0, ptr1, len1);
        var ptr3 = ret[0];
        var len3 = ret[1];
        if (ret[3]) {
            ptr3 = 0; len3 = 0;
            throw takeFromExternrefTable0(ret[2]);
        }
        deferred4_0 = ptr3;
        deferred4_1 = len3;
        return getStringFromWasm0(ptr3, len3);
    } finally {
        wasm.__wbindgen_free(deferred4_0, deferred4_1, 1);
    }
}

/**
 * Build a GNSS interference map from CSV text (`source` is `"adsb"` or `"ais"`; formats in
 * `docs/INTERFERENCE-MAP.md`). `dataset` is an approved preset or `"custom"`, which also
 * needs `licence`, `licence_url` and `attribution`; pass an empty string for any field
 * that does not apply, and `NaN` for the default 0.5 degree cell. `land_geojson` (AIS only,
 * empty for none) is a land-polygon file. Returns a JSON array of
 * `{file_name, date, cells_published, cells_flagged, geojson}`, one per UTC day. Aggregate
 * only; a degraded cell does not name interference as the cause. Nothing is uploaded.
 * @param {string} source
 * @param {string} csv
 * @param {string} dataset
 * @param {number} cell_deg
 * @param {string} licence
 * @param {string} licence_url
 * @param {string} attribution
 * @param {string} land_geojson
 * @returns {string}
 */
export function interference_map(source, csv, dataset, cell_deg, licence, licence_url, attribution, land_geojson) {
    let deferred9_0;
    let deferred9_1;
    try {
        const ptr0 = passStringToWasm0(source, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(csv, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ptr2 = passStringToWasm0(dataset, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len2 = WASM_VECTOR_LEN;
        const ptr3 = passStringToWasm0(licence, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len3 = WASM_VECTOR_LEN;
        const ptr4 = passStringToWasm0(licence_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len4 = WASM_VECTOR_LEN;
        const ptr5 = passStringToWasm0(attribution, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len5 = WASM_VECTOR_LEN;
        const ptr6 = passStringToWasm0(land_geojson, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len6 = WASM_VECTOR_LEN;
        const ret = wasm.interference_map(ptr0, len0, ptr1, len1, ptr2, len2, cell_deg, ptr3, len3, ptr4, len4, ptr5, len5, ptr6, len6);
        var ptr8 = ret[0];
        var len8 = ret[1];
        if (ret[3]) {
            ptr8 = 0; len8 = 0;
            throw takeFromExternrefTable0(ret[2]);
        }
        deferred9_0 = ptr8;
        deferred9_1 = len8;
        return getStringFromWasm0(ptr8, len8);
    } finally {
        wasm.__wbindgen_free(deferred9_0, deferred9_1, 1);
    }
}

/**
 * A short synthetic scene, acquired and tracked in memory: `ids` are the PRNs (GLONASS L1OF:
 * the frequency channel), `dopplers_hz` one value for all or one each, `cn0_dbhz` NaN for the
 * elevation default. Bounded to 0.25 s, 1 000 000 samples and 8 satellites. Returns JSON
 * `{fs_hz, samples, acquisition, tracking}`; the same request gives the same answer as the
 * native build.
 * @param {number} fs_hz
 * @param {number} duration_s
 * @param {string} signal
 * @param {Float64Array} ids
 * @param {Float64Array} dopplers_hz
 * @param {number} cn0_dbhz
 * @param {number} seed
 * @returns {string}
 */
export function iq_run_short(fs_hz, duration_s, signal, ids, dopplers_hz, cn0_dbhz, seed) {
    let deferred5_0;
    let deferred5_1;
    try {
        const ptr0 = passStringToWasm0(signal, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passArrayF64ToWasm0(ids, wasm.__wbindgen_malloc);
        const len1 = WASM_VECTOR_LEN;
        const ptr2 = passArrayF64ToWasm0(dopplers_hz, wasm.__wbindgen_malloc);
        const len2 = WASM_VECTOR_LEN;
        const ret = wasm.iq_run_short(fs_hz, duration_s, ptr0, len0, ptr1, len1, ptr2, len2, cn0_dbhz, seed);
        var ptr4 = ret[0];
        var len4 = ret[1];
        if (ret[3]) {
            ptr4 = 0; len4 = 0;
            throw takeFromExternrefTable0(ret[2]);
        }
        deferred5_0 = ptr4;
        deferred5_1 = len4;
        return getStringFromWasm0(ptr4, len4);
    } finally {
        wasm.__wbindgen_free(deferred5_0, deferred5_1, 1);
    }
}

/**
 * The signal names `iq_run_short` accepts: a JSON array.
 * @returns {string}
 */
export function iq_signals() {
    let deferred1_0;
    let deferred1_1;
    try {
        const ret = wasm.iq_signals();
        deferred1_0 = ret[0];
        deferred1_1 = ret[1];
        return getStringFromWasm0(ret[0], ret[1]);
    } finally {
        wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
    }
}

/**
 * The modules a scenario kind's verification-matrix rows name where they differ from the kind's
 * own name: a JSON object, kind to array of module names. The Studio's capability map joins a
 * scenario to its matrix rows through it.
 * @returns {string}
 */
export function kind_modules() {
    let deferred1_0;
    let deferred1_1;
    try {
        const ret = wasm.kind_modules();
        deferred1_0 = ret[0];
        deferred1_1 = ret[1];
        return getStringFromWasm0(ret[0], ret[1]);
    } finally {
        wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
    }
}

/**
 * The bundled reference scenarios as JSON `{count, scenarios: [{name, kind, about}]}`; a
 * non-empty `kind` limits the list to one scenario kind.
 * @param {string} kind
 * @returns {string}
 */
export function list_examples(kind) {
    let deferred3_0;
    let deferred3_1;
    try {
        const ptr0 = passStringToWasm0(kind, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.list_examples(ptr0, len0);
        var ptr2 = ret[0];
        var len2 = ret[1];
        if (ret[3]) {
            ptr2 = 0; len2 = 0;
            throw takeFromExternrefTable0(ret[2]);
        }
        deferred3_0 = ptr2;
        deferred3_1 = len2;
        return getStringFromWasm0(ptr2, len2);
    } finally {
        wasm.__wbindgen_free(deferred3_0, deferred3_1, 1);
    }
}

/**
 * List the available scenario kinds and their metadata as a JSON array (name,
 * description, required and optional fields), for programmatic introspection.
 * @returns {string}
 */
export function list_kinds() {
    let deferred1_0;
    let deferred1_1;
    try {
        const ret = wasm.list_kinds();
        deferred1_0 = ret[0];
        deferred1_1 = ret[1];
        return getStringFromWasm0(ret[0], ret[1]);
    } finally {
        wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
    }
}

/**
 * Generate synthetic bridge NMEA 0183 for crew training from a `nmea-scenario` TOML.
 * `seed` replaces the scenario's seed unless it is `NaN` or negative. Returns a JSON object
 * `{nmea, log_json, log_text}`. Text only; never for a vessel's live navigation systems.
 * Advisory, not type-approved navigation equipment.
 * @param {string} toml
 * @param {number} seed
 * @returns {string}
 */
export function nmea_training(toml, seed) {
    let deferred3_0;
    let deferred3_1;
    try {
        const ptr0 = passStringToWasm0(toml, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.nmea_training(ptr0, len0, seed);
        var ptr2 = ret[0];
        var len2 = ret[1];
        if (ret[3]) {
            ptr2 = 0; len2 = 0;
            throw takeFromExternrefTable0(ret[2]);
        }
        deferred3_0 = ptr2;
        deferred3_1 = len2;
        return getStringFromWasm0(ptr2, len2);
    } finally {
        wasm.__wbindgen_free(deferred3_0, deferred3_1, 1);
    }
}

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
 * @param {string} input_base64
 * @param {string} format
 * @param {number | null | undefined} start_gst
 * @param {string} merkle_root_hex
 * @param {string[]} public_keys
 * @param {number | null} [reference_time]
 * @param {number | null} [max_time_error]
 * @returns {string}
 */
export function osnma_verify(input_base64, format, start_gst, merkle_root_hex, public_keys, reference_time, max_time_error) {
    let deferred6_0;
    let deferred6_1;
    try {
        const ptr0 = passStringToWasm0(input_base64, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(format, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ptr2 = passStringToWasm0(merkle_root_hex, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len2 = WASM_VECTOR_LEN;
        const ptr3 = passArrayJsValueToWasm0(public_keys, wasm.__wbindgen_malloc);
        const len3 = WASM_VECTOR_LEN;
        const ret = wasm.osnma_verify(ptr0, len0, ptr1, len1, isLikeNone(start_gst) ? Number.MAX_SAFE_INTEGER : (start_gst) >>> 0, ptr2, len2, ptr3, len3, isLikeNone(reference_time) ? Number.MAX_SAFE_INTEGER : (reference_time) >>> 0, isLikeNone(max_time_error) ? Number.MAX_SAFE_INTEGER : (max_time_error) >>> 0);
        var ptr5 = ret[0];
        var len5 = ret[1];
        if (ret[3]) {
            ptr5 = 0; len5 = 0;
            throw takeFromExternrefTable0(ret[2]);
        }
        deferred6_0 = ptr5;
        deferred6_1 = len5;
        return getStringFromWasm0(ptr5, len5);
    } finally {
        wasm.__wbindgen_free(deferred6_0, deferred6_1, 1);
    }
}

/**
 * Assess a real receiver log described by a `receiver-trust` scenario (TOML text). The
 * browser has no file system, so the log (and any navigation file) must be inline as
 * `text` or `base64`. Returns a JSON object `{json, csv, svg, summary}`.
 * @param {string} toml
 * @returns {string}
 */
export function receiver_trust(toml) {
    let deferred3_0;
    let deferred3_1;
    try {
        const ptr0 = passStringToWasm0(toml, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.receiver_trust(ptr0, len0);
        var ptr2 = ret[0];
        var len2 = ret[1];
        if (ret[3]) {
            ptr2 = 0; len2 = 0;
            throw takeFromExternrefTable0(ret[2]);
        }
        deferred3_0 = ptr2;
        deferred3_1 = len2;
        return getStringFromWasm0(ptr2, len2);
    } finally {
        wasm.__wbindgen_free(deferred3_0, deferred3_1, 1);
    }
}

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
 * @param {string} session_toml
 * @param {string} nmea
 * @returns {string}
 */
export function receiver_trust_replay(session_toml, nmea) {
    let deferred4_0;
    let deferred4_1;
    try {
        const ptr0 = passStringToWasm0(session_toml, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(nmea, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.receiver_trust_replay(ptr0, len0, ptr1, len1);
        var ptr3 = ret[0];
        var len3 = ret[1];
        if (ret[3]) {
            ptr3 = 0; len3 = 0;
            throw takeFromExternrefTable0(ret[2]);
        }
        deferred4_0 = ptr3;
        deferred4_1 = len3;
        return getStringFromWasm0(ptr3, len3);
    } finally {
        wasm.__wbindgen_free(deferred4_0, deferred4_1, 1);
    }
}

/**
 * Share of a route (GeoJSON LineString or `lat,lon` CSV text) through degraded cells of the
 * maps in `maps_json` (a JSON array of map GeoJSON strings, as `interference_map` returns),
 * optionally limited to `date_from`..`date_to` (`YYYY-MM-DD`, empty for no limit). Returns
 * the report as JSON text. Not a forecast; unobserved cells are not evidence of a clear
 * route.
 * @param {string} route
 * @param {string} maps_json
 * @param {string} date_from
 * @param {string} date_to
 * @returns {string}
 */
export function route_exposure(route, maps_json, date_from, date_to) {
    let deferred6_0;
    let deferred6_1;
    try {
        const ptr0 = passStringToWasm0(route, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(maps_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ptr2 = passStringToWasm0(date_from, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len2 = WASM_VECTOR_LEN;
        const ptr3 = passStringToWasm0(date_to, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len3 = WASM_VECTOR_LEN;
        const ret = wasm.route_exposure(ptr0, len0, ptr1, len1, ptr2, len2, ptr3, len3);
        var ptr5 = ret[0];
        var len5 = ret[1];
        if (ret[3]) {
            ptr5 = 0; len5 = 0;
            throw takeFromExternrefTable0(ret[2]);
        }
        deferred6_0 = ptr5;
        deferred6_1 = len5;
        return getStringFromWasm0(ptr5, len5);
    } finally {
        wasm.__wbindgen_free(deferred6_0, deferred6_1, 1);
    }
}

/**
 * Run a scenario given as a TOML string; returns the result document as a JSON
 * string. Throws a JS error if the scenario is invalid.
 * @param {string} toml
 * @returns {string}
 */
export function run(toml) {
    let deferred3_0;
    let deferred3_1;
    try {
        const ptr0 = passStringToWasm0(toml, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.run(ptr0, len0);
        var ptr2 = ret[0];
        var len2 = ret[1];
        if (ret[3]) {
            ptr2 = 0; len2 = 0;
            throw takeFromExternrefTable0(ret[2]);
        }
        deferred3_0 = ptr2;
        deferred3_1 = len2;
        return getStringFromWasm0(ptr2, len2);
    } finally {
        wasm.__wbindgen_free(deferred3_0, deferred3_1, 1);
    }
}

/**
 * Run a scenario ONCE and return every output of that run as a JSON object string:
 * `{"json": <result document string>, "svg": <chart>, "summary": <one line>,
 * "csv": <table or null>}` (the `RunAllOutput` type). `run_typed` returns the same
 * outputs, plus the parsed document, as an object. `run`, `chart_svg`, `summary` and `table_csv` each execute
 * the scenario from scratch, so a page that wants all four paid for four full engine
 * runs — about 12 s instead of 3 s for the heaviest kind in a browser. Throws a JS
 * error if the scenario is invalid.
 * @param {string} toml
 * @returns {string}
 */
export function run_all(toml) {
    let deferred3_0;
    let deferred3_1;
    try {
        const ptr0 = passStringToWasm0(toml, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.run_all(ptr0, len0);
        var ptr2 = ret[0];
        var len2 = ret[1];
        if (ret[3]) {
            ptr2 = 0; len2 = 0;
            throw takeFromExternrefTable0(ret[2]);
        }
        deferred3_0 = ptr2;
        deferred3_1 = len2;
        return getStringFromWasm0(ptr2, len2);
    } finally {
        wasm.__wbindgen_free(deferred3_0, deferred3_1, 1);
    }
}

/**
 * Run a scenario once and return every output as one typed object (`RunOutput` in the
 * declarations): `json`, `svg`, `summary`, `csv` (`null` for a kind that emits no
 * table) and `data`, the result document already parsed. A plain object, not a class,
 * so there is nothing to free. Throws a JS error if the scenario is invalid.
 * @param {string} toml
 * @returns {RunOutput}
 */
export function run_typed(toml) {
    const ptr0 = passStringToWasm0(toml, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len0 = WASM_VECTOR_LEN;
    const ret = wasm.run_typed(ptr0, len0);
    if (ret[2]) {
        throw takeFromExternrefTable0(ret[1]);
    }
    return takeFromExternrefTable0(ret[0]);
}

/**
 * The available scenario kinds and their metadata, already parsed: the typed form of
 * [`list_kinds`], which keeps its JSON-string return for existing callers.
 * @returns {KindMeta[]}
 */
export function scenario_kinds() {
    const ret = wasm.scenario_kinds();
    return ret;
}

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
 * @param {string} request_json
 * @returns {string}
 */
export function sky_geometry(request_json) {
    let deferred3_0;
    let deferred3_1;
    try {
        const ptr0 = passStringToWasm0(request_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.sky_geometry(ptr0, len0);
        var ptr2 = ret[0];
        var len2 = ret[1];
        if (ret[3]) {
            ptr2 = 0; len2 = 0;
            throw takeFromExternrefTable0(ret[2]);
        }
        deferred3_0 = ptr2;
        deferred3_1 = len2;
        return getStringFromWasm0(ptr2, len2);
    } finally {
        wasm.__wbindgen_free(deferred3_0, deferred3_1, 1);
    }
}

/**
 * `n` (1 to 5000) least-squares position errors under independent Gaussian range errors of
 * `sigma_uere_m` on the visible set of `request_json` (as for `sky_geometry`), seeded by
 * `seed` (a non-negative integer; the same request, seed and `n` give the same cloud). Returns
 * a JSON object: `samples_enu_m` (`[[e, n, u], ...]`), `empirical_covariance_enu_m2`,
 * `rms_horizontal_m`, `rms_3d_m`, `predicted_rms_horizontal_m` (sigma x HDOP),
 * `predicted_rms_3d_m`, `fraction_inside_95`, `label`, `statement`, `advisory`. Throws a JS
 * error on an invalid request, a seed that is not a non-negative integer, or `n` out of range.
 * @param {string} request_json
 * @param {number} seed
 * @param {number} n
 * @returns {string}
 */
export function sky_monte_carlo(request_json, seed, n) {
    let deferred3_0;
    let deferred3_1;
    try {
        const ptr0 = passStringToWasm0(request_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.sky_monte_carlo(ptr0, len0, seed, n);
        var ptr2 = ret[0];
        var len2 = ret[1];
        if (ret[3]) {
            ptr2 = 0; len2 = 0;
            throw takeFromExternrefTable0(ret[2]);
        }
        deferred3_0 = ptr2;
        deferred3_1 = len2;
        return getStringFromWasm0(ptr2, len2);
    } finally {
        wasm.__wbindgen_free(deferred3_0, deferred3_1, 1);
    }
}

/**
 * The named skyline presets (`city-street`, `harbour-approach`, `fjord`,
 * `container-terminal`) as a JSON array: `name`, `label` (REPRESENTATIVE), `description`,
 * `provenance`, `table` (the cut every degree of azimuth from north, 360 numbers) and
 * `open_fraction`.
 * @returns {string}
 */
export function skyline_presets() {
    let deferred1_0;
    let deferred1_1;
    try {
        const ret = wasm.skyline_presets();
        deferred1_0 = ret[0];
        deferred1_1 = ret[1];
        return getStringFromWasm0(ret[0], ret[1]);
    } finally {
        wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
    }
}

/**
 * A skyline section (`skyline_json`, the `skyline` object of a `sky_geometry` request) reduced
 * to the normalised table the engine uses: a JSON object with `table` (360 numbers, the cut
 * every degree of azimuth from north), `open_fraction`, `source` and `provenance`. Throws a
 * JS error on an invalid section.
 * @param {string} skyline_json
 * @returns {string}
 */
export function skyline_profile(skyline_json) {
    let deferred3_0;
    let deferred3_1;
    try {
        const ptr0 = passStringToWasm0(skyline_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.skyline_profile(ptr0, len0);
        var ptr2 = ret[0];
        var len2 = ret[1];
        if (ret[3]) {
            ptr2 = 0; len2 = 0;
            throw takeFromExternrefTable0(ret[2]);
        }
        deferred3_0 = ptr2;
        deferred3_1 = len2;
        return getStringFromWasm0(ptr2, len2);
    } finally {
        wasm.__wbindgen_free(deferred3_0, deferred3_1, 1);
    }
}

/**
 * The skyplot of `request_json` (as for `sky_geometry`) as static SVG markup (no script, no
 * external reference; every input string escaped). Throws a JS error on an invalid request.
 * @param {string} request_json
 * @returns {string}
 */
export function skyplot_svg(request_json) {
    let deferred3_0;
    let deferred3_1;
    try {
        const ptr0 = passStringToWasm0(request_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.skyplot_svg(ptr0, len0);
        var ptr2 = ret[0];
        var len2 = ret[1];
        if (ret[3]) {
            ptr2 = 0; len2 = 0;
            throw takeFromExternrefTable0(ret[2]);
        }
        deferred3_0 = ptr2;
        deferred3_1 = len2;
        return getStringFromWasm0(ptr2, len2);
    } finally {
        wasm.__wbindgen_free(deferred3_0, deferred3_1, 1);
    }
}

/**
 * Run a scenario and return its one-line human-readable summary.
 * @param {string} toml
 * @returns {string}
 */
export function summary(toml) {
    let deferred3_0;
    let deferred3_1;
    try {
        const ptr0 = passStringToWasm0(toml, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.summary(ptr0, len0);
        var ptr2 = ret[0];
        var len2 = ret[1];
        if (ret[3]) {
            ptr2 = 0; len2 = 0;
            throw takeFromExternrefTable0(ret[2]);
        }
        deferred3_0 = ptr2;
        deferred3_1 = len2;
        return getStringFromWasm0(ptr2, len2);
    } finally {
        wasm.__wbindgen_free(deferred3_0, deferred3_1, 1);
    }
}

/**
 * Run a scenario and return its reproducibility table as CSV text — the same bytes the
 * CLI writes as `<scenario>.table.csv`. Returns `undefined` for kinds that publish no
 * table (only `realtime-frame-eop`, `lunar-time-budget`, `lunar-jamming`, and
 * `moonlight-service-volume` with an export site configured emit one). Throws a JS error
 * if the scenario is invalid.
 * @param {string} toml
 * @returns {string | undefined}
 */
export function table_csv(toml) {
    const ptr0 = passStringToWasm0(toml, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len0 = WASM_VECTOR_LEN;
    const ret = wasm.table_csv(ptr0, len0);
    if (ret[3]) {
        throw takeFromExternrefTable0(ret[2]);
    }
    let v2;
    if (ret[0] !== 0) {
        v2 = getStringFromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
    }
    return v2;
}

/**
 * Render a trust stream or a batch result in one telemetry format, with the encoders
 * `kshana trust-telemetry` uses (no process, socket or clock). `format` is `"prometheus"`,
 * `"cef"`, `"leef"` or `"otlp"`; `input_kind` is `"result"` (a `receiver-trust` result
 * document) or `"jsonl"` (the live per-epoch stream); `host` names the sender in syslog
 * events (empty for `kshana`). `options_json` is `""` or an object with `expose_position`
 * (bool), `syslog_all` (bool: one event per epoch, not per band change) and `unix_nano`
 * (number, the time of export for `"otlp"`). Syslog lines carry no timestamp. Returns the
 * formatted text.
 * @param {string} input
 * @param {string} format
 * @param {string} host
 * @param {string} input_kind
 * @param {string} options_json
 * @returns {string}
 */
export function trust_telemetry(input, format, host, input_kind, options_json) {
    let deferred7_0;
    let deferred7_1;
    try {
        const ptr0 = passStringToWasm0(input, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(format, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ptr2 = passStringToWasm0(host, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len2 = WASM_VECTOR_LEN;
        const ptr3 = passStringToWasm0(input_kind, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len3 = WASM_VECTOR_LEN;
        const ptr4 = passStringToWasm0(options_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len4 = WASM_VECTOR_LEN;
        const ret = wasm.trust_telemetry(ptr0, len0, ptr1, len1, ptr2, len2, ptr3, len3, ptr4, len4);
        var ptr6 = ret[0];
        var len6 = ret[1];
        if (ret[3]) {
            ptr6 = 0; len6 = 0;
            throw takeFromExternrefTable0(ret[2]);
        }
        deferred7_0 = ptr6;
        deferred7_1 = len6;
        return getStringFromWasm0(ptr6, len6);
    } finally {
        wasm.__wbindgen_free(deferred7_0, deferred7_1, 1);
    }
}

/**
 * Engine version (the crate version).
 * @returns {string}
 */
export function version() {
    let deferred1_0;
    let deferred1_1;
    try {
        const ret = wasm.version();
        deferred1_0 = ret[0];
        deferred1_1 = ret[1];
        return getStringFromWasm0(ret[0], ret[1]);
    } finally {
        wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
    }
}
function __wbg_get_imports() {
    const import0 = {
        __proto__: null,
        __wbg___wbindgen_is_function_754e9f305ff6029e: function(arg0) {
            const ret = typeof(arg0) === 'function';
            return ret;
        },
        __wbg___wbindgen_is_object_56732c2bc353f41d: function(arg0) {
            const val = arg0;
            const ret = typeof(val) === 'object' && val !== null;
            return ret;
        },
        __wbg___wbindgen_is_string_c236cabd84a4d769: function(arg0) {
            const ret = typeof(arg0) === 'string';
            return ret;
        },
        __wbg___wbindgen_is_undefined_67b456be8673d3d7: function(arg0) {
            const ret = arg0 === undefined;
            return ret;
        },
        __wbg___wbindgen_string_get_72bdf95d3ae505b1: function(arg0, arg1) {
            const obj = arg1;
            const ret = typeof(obj) === 'string' ? obj : undefined;
            var ptr1 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            var len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg___wbindgen_throw_1506f2235d1bdba0: function(arg0, arg1) {
            throw new Error(getStringFromWasm0(arg0, arg1));
        },
        __wbg_call_9c758de292015997: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = arg0.call(arg1, arg2);
            return ret;
        }, arguments); },
        __wbg_crypto_38df2bab126b63dc: function(arg0) {
            const ret = arg0.crypto;
            return ret;
        },
        __wbg_getRandomValues_c44a50d8cfdaebeb: function() { return handleError(function (arg0, arg1) {
            arg0.getRandomValues(arg1);
        }, arguments); },
        __wbg_length_4a591ecaa01354d9: function(arg0) {
            const ret = arg0.length;
            return ret;
        },
        __wbg_msCrypto_bd5a034af96bcba6: function(arg0) {
            const ret = arg0.msCrypto;
            return ret;
        },
        __wbg_new_with_length_36a4998e27b014c5: function(arg0) {
            const ret = new Uint8Array(arg0 >>> 0);
            return ret;
        },
        __wbg_node_84ea875411254db1: function(arg0) {
            const ret = arg0.node;
            return ret;
        },
        __wbg_parse_ee08b1d6033e6687: function(arg0, arg1) {
            const ret = JSON.parse(getStringFromWasm0(arg0, arg1));
            return ret;
        },
        __wbg_process_44c7a14e11e9f69e: function(arg0) {
            const ret = arg0.process;
            return ret;
        },
        __wbg_prototypesetcall_3249fc62a0fafa30: function(arg0, arg1, arg2) {
            Uint8Array.prototype.set.call(getArrayU8FromWasm0(arg0, arg1), arg2);
        },
        __wbg_randomFillSync_6c25eac9869eb53c: function() { return handleError(function (arg0, arg1) {
            arg0.randomFillSync(arg1);
        }, arguments); },
        __wbg_require_b4edbdcf3e2a1ef0: function() { return handleError(function () {
            const ret = module.require;
            return ret;
        }, arguments); },
        __wbg_static_accessor_GLOBAL_9d53f2689e622ca1: function() {
            const ret = typeof global === 'undefined' ? null : global;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_static_accessor_GLOBAL_THIS_a1a35cec07001a8a: function() {
            const ret = typeof globalThis === 'undefined' ? null : globalThis;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_static_accessor_SELF_4c59f6c7ea29a144: function() {
            const ret = typeof self === 'undefined' ? null : self;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_static_accessor_WINDOW_e70ae9f2eb052253: function() {
            const ret = typeof window === 'undefined' ? null : window;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_subarray_4aa221f6a4f5ab22: function(arg0, arg1, arg2) {
            const ret = arg0.subarray(arg1 >>> 0, arg2 >>> 0);
            return ret;
        },
        __wbg_versions_276b2795b1c6a219: function(arg0) {
            const ret = arg0.versions;
            return ret;
        },
        __wbindgen_cast_0000000000000001: function(arg0, arg1) {
            // Cast intrinsic for `Ref(Slice(U8)) -> NamedExternref("Uint8Array")`.
            const ret = getArrayU8FromWasm0(arg0, arg1);
            return ret;
        },
        __wbindgen_cast_0000000000000002: function(arg0, arg1) {
            // Cast intrinsic for `Ref(String) -> Externref`.
            const ret = getStringFromWasm0(arg0, arg1);
            return ret;
        },
        __wbindgen_init_externref_table: function() {
            const table = wasm.__wbindgen_externrefs;
            const offset = table.grow(4);
            table.set(0, undefined);
            table.set(offset + 0, undefined);
            table.set(offset + 1, null);
            table.set(offset + 2, true);
            table.set(offset + 3, false);
        },
    };
    return {
        __proto__: null,
        "./kshana_bg.js": import0,
    };
}

function addToExternrefTable0(obj) {
    const idx = wasm.__externref_table_alloc();
    wasm.__wbindgen_externrefs.set(idx, obj);
    return idx;
}

function getArrayU8FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getUint8ArrayMemory0().subarray(ptr / 1, ptr / 1 + len);
}

let cachedDataViewMemory0 = null;
function getDataViewMemory0() {
    if (cachedDataViewMemory0 === null || cachedDataViewMemory0.buffer.detached === true || (cachedDataViewMemory0.buffer.detached === undefined && cachedDataViewMemory0.buffer !== wasm.memory.buffer)) {
        cachedDataViewMemory0 = new DataView(wasm.memory.buffer);
    }
    return cachedDataViewMemory0;
}

let cachedFloat64ArrayMemory0 = null;
function getFloat64ArrayMemory0() {
    if (cachedFloat64ArrayMemory0 === null || cachedFloat64ArrayMemory0.byteLength === 0) {
        cachedFloat64ArrayMemory0 = new Float64Array(wasm.memory.buffer);
    }
    return cachedFloat64ArrayMemory0;
}

function getStringFromWasm0(ptr, len) {
    return decodeText(ptr >>> 0, len);
}

let cachedUint8ArrayMemory0 = null;
function getUint8ArrayMemory0() {
    if (cachedUint8ArrayMemory0 === null || cachedUint8ArrayMemory0.byteLength === 0) {
        cachedUint8ArrayMemory0 = new Uint8Array(wasm.memory.buffer);
    }
    return cachedUint8ArrayMemory0;
}

function handleError(f, args) {
    try {
        return f.apply(this, args);
    } catch (e) {
        const idx = addToExternrefTable0(e);
        wasm.__wbindgen_exn_store(idx);
    }
}

function isLikeNone(x) {
    return x === undefined || x === null;
}

function passArrayF64ToWasm0(arg, malloc) {
    const ptr = malloc(arg.length * 8, 8) >>> 0;
    getFloat64ArrayMemory0().set(arg, ptr / 8);
    WASM_VECTOR_LEN = arg.length;
    return ptr;
}

function passArrayJsValueToWasm0(array, malloc) {
    const ptr = malloc(array.length * 4, 4) >>> 0;
    for (let i = 0; i < array.length; i++) {
        const add = addToExternrefTable0(array[i]);
        getDataViewMemory0().setUint32(ptr + 4 * i, add, true);
    }
    WASM_VECTOR_LEN = array.length;
    return ptr;
}

function passStringToWasm0(arg, malloc, realloc) {
    if (realloc === undefined) {
        const buf = cachedTextEncoder.encode(arg);
        const ptr = malloc(buf.length, 1) >>> 0;
        getUint8ArrayMemory0().subarray(ptr, ptr + buf.length).set(buf);
        WASM_VECTOR_LEN = buf.length;
        return ptr;
    }

    let len = arg.length;
    let ptr = malloc(len, 1) >>> 0;

    const mem = getUint8ArrayMemory0();

    let offset = 0;

    for (; offset < len; offset++) {
        const code = arg.charCodeAt(offset);
        if (code > 0x7F) break;
        mem[ptr + offset] = code;
    }
    if (offset !== len) {
        if (offset !== 0) {
            arg = arg.slice(offset);
        }
        ptr = realloc(ptr, len, len = offset + arg.length * 3, 1) >>> 0;
        const view = getUint8ArrayMemory0().subarray(ptr + offset, ptr + len);
        const ret = cachedTextEncoder.encodeInto(arg, view);

        offset += ret.written;
        ptr = realloc(ptr, len, offset, 1) >>> 0;
    }

    WASM_VECTOR_LEN = offset;
    return ptr;
}

function takeFromExternrefTable0(idx) {
    const value = wasm.__wbindgen_externrefs.get(idx);
    wasm.__externref_table_dealloc(idx);
    return value;
}

let cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
cachedTextDecoder.decode();
const MAX_SAFARI_DECODE_BYTES = 2146435072;
let numBytesDecoded = 0;
function decodeText(ptr, len) {
    numBytesDecoded += len;
    if (numBytesDecoded >= MAX_SAFARI_DECODE_BYTES) {
        cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
        cachedTextDecoder.decode();
        numBytesDecoded = len;
    }
    return cachedTextDecoder.decode(getUint8ArrayMemory0().subarray(ptr, ptr + len));
}

const cachedTextEncoder = new TextEncoder();

if (!('encodeInto' in cachedTextEncoder)) {
    cachedTextEncoder.encodeInto = function (arg, view) {
        const buf = cachedTextEncoder.encode(arg);
        view.set(buf);
        return {
            read: arg.length,
            written: buf.length
        };
    };
}

let WASM_VECTOR_LEN = 0;

let wasmModule, wasmInstance, wasm;
function __wbg_finalize_init(instance, module) {
    wasmInstance = instance;
    wasm = instance.exports;
    wasmModule = module;
    cachedDataViewMemory0 = null;
    cachedFloat64ArrayMemory0 = null;
    cachedUint8ArrayMemory0 = null;
    wasm.__wbindgen_start();
    return wasm;
}

async function __wbg_load(module, imports) {
    if (typeof Response === 'function' && module instanceof Response) {
        if (typeof WebAssembly.instantiateStreaming === 'function') {
            try {
                return await WebAssembly.instantiateStreaming(module, imports);
            } catch (e) {
                const validResponse = module.ok && expectedResponseType(module.type);

                if (validResponse && module.headers.get('Content-Type') !== 'application/wasm') {
                    console.warn("`WebAssembly.instantiateStreaming` failed because your server does not serve Wasm with `application/wasm` MIME type. Falling back to `WebAssembly.instantiate` which is slower. Original error:\n", e);

                } else { throw e; }
            }
        }

        const bytes = await module.arrayBuffer();
        return await WebAssembly.instantiate(bytes, imports);
    } else {
        const instance = await WebAssembly.instantiate(module, imports);

        if (instance instanceof WebAssembly.Instance) {
            return { instance, module };
        } else {
            return instance;
        }
    }

    function expectedResponseType(type) {
        switch (type) {
            case 'basic': case 'cors': case 'default': return true;
        }
        return false;
    }
}

function initSync(module) {
    if (wasm !== undefined) return wasm;


    if (module !== undefined) {
        if (Object.getPrototypeOf(module) === Object.prototype) {
            ({module} = module)
        } else {
            console.warn('using deprecated parameters for `initSync()`; pass a single object instead')
        }
    }

    const imports = __wbg_get_imports();
    if (!(module instanceof WebAssembly.Module)) {
        module = new WebAssembly.Module(module);
    }
    const instance = new WebAssembly.Instance(module, imports);
    return __wbg_finalize_init(instance, module);
}

async function __wbg_init(module_or_path) {
    if (wasm !== undefined) return wasm;


    if (module_or_path !== undefined) {
        if (Object.getPrototypeOf(module_or_path) === Object.prototype) {
            ({module_or_path} = module_or_path)
        } else {
            console.warn('using deprecated parameters for the initialization function; pass a single object instead')
        }
    }

    if (module_or_path === undefined) {
        module_or_path = new URL('kshana_bg.wasm', import.meta.url);
    }
    const imports = __wbg_get_imports();

    if (typeof module_or_path === 'string' || (typeof Request === 'function' && module_or_path instanceof Request) || (typeof URL === 'function' && module_or_path instanceof URL)) {
        module_or_path = fetch(module_or_path);
    }

    const { instance, module } = await __wbg_load(await module_or_path, imports);

    return __wbg_finalize_init(instance, module);
}

export { initSync, __wbg_init as default };
