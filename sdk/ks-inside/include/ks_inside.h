/*
 * PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
 *
 * KS-Inside: the Kshana Pro embedded SDK for OEMs (C ABI).
 * Documentation: docs/KS-INSIDE.md. Rust side: src/ks_inside/ffi.rs.
 *
 * Use:
 *   1. ks_inside_open() with the OEM licence token (product "sdk", grant "pro-ks-inside"),
 *      the unit's serial number and a writable meter-log path (one log per process).
 *   2. ks_inside_call() a method by name with its text input; the answer is JSON text.
 *      Every call to a known method is written to the meter log before it returns; when the
 *      log cannot be written the call fails with KS_ERR_METER and no answer is returned.
 *   3. Free every returned string with ks_inside_free_string(), close with ks_inside_close().
 *
 * Strings are NUL-terminated UTF-8. A handle may be shared between threads. No function
 * lets a panic or exception cross this boundary (KS_ERR_INTERNAL instead).
 */
#ifndef KS_INSIDE_H
#define KS_INSIDE_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define KS_INSIDE_ABI_VERSION 1

/* Status codes. On any code but KS_OK the output string holds {"error": ..., "code": n}. */
#define KS_OK 0
#define KS_ERR_ARGUMENT 1 /* null pointer, not UTF-8, empty unit ID, input too long */
#define KS_ERR_LICENCE 2  /* the licence does not open the SDK (open only) */
#define KS_ERR_REFUSED 3  /* the licence does not allow this call (metered) */
#define KS_ERR_METHOD 4   /* no such method in this ABI version (not metered) */
#define KS_ERR_INPUT 5    /* invalid input or the method failed (metered) */
#define KS_ERR_METER 6    /* the meter log could not be read or written: no answer */
#define KS_ERR_INTERNAL 7 /* an internal error was contained */

/* Methods of ABI version 1 (input -> JSON answer):
 *   "scenario.run"             a Pro scenario as TOML text (kind = "pro-...")
 *   "timing.drift"             {"series": [[unix_s, time_error_s], ...], "detector": {...},
 *                               "budget_s": n, "name": s}
 *   "timing.timebomb-analyse"  {"vector": {...}, "capture_format": "nmea"|"csv"|"ubx",
 *                               "capture": text | "capture_hex": hex,
 *                               "start_index": k, "replay_start_lab": s}
 */

typedef struct KsInsideHandle ks_inside;

/* The ABI version of the library (compare with KS_INSIDE_ABI_VERSION). */
uint32_t ks_inside_abi_version(void);

/* Open. revocations may be NULL (else the text of one signed revocation list). On success
 * *handle is set; on failure *handle is NULL and *error (when error is not NULL) holds a
 * JSON error to free. */
int32_t ks_inside_open(const char *token, const char *unit_id, const char *meter_log_path,
                       const char *revocations, ks_inside **handle, char **error);

/* Call a method. *output always receives a string to free (answer or error) unless output
 * is NULL, which returns KS_ERR_ARGUMENT. */
int32_t ks_inside_call(const ks_inside *handle, const char *method, const char *input,
                       char **output);

/* The instance's description as JSON (not metered); NULL for a NULL handle. Free it. */
char *ks_inside_info(const ks_inside *handle);

/* Free a string returned by the library. NULL is ignored. */
void ks_inside_free_string(char *s);

/* Close a handle. NULL is ignored. */
void ks_inside_close(ks_inside *handle);

#ifdef __cplusplus
}
#endif

#endif /* KS_INSIDE_H */
