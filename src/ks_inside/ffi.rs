// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The KS-Inside C ABI (header `sdk/ks-inside/include/ks_inside.h`, ABI version
//! [`super::ABI_VERSION`]).
//!
//! Rules every function keeps: no panic crosses the boundary (each body runs under
//! `catch_unwind`; a panic is reported as [`KS_ERR_INTERNAL`]); every pointer is checked
//! for null; strings are NUL-terminated UTF-8; every string the library returns is freed
//! with [`ks_inside_free_string`] and every handle with [`ks_inside_close`]. A handle may be
//! used from several threads (calls are serialised on the meter log).
//!
//! The shared or static library is built from this crate with
//! `cargo rustc --release --lib --crate-type cdylib` (or `staticlib`); see docs/KS-INSIDE.md.

use std::ffi::{c_char, CStr, CString};
use std::panic::{catch_unwind, AssertUnwindSafe};

use super::{CallError, Config, KsInside};

pub const KS_OK: i32 = 0;
pub const KS_ERR_ARGUMENT: i32 = 1;
pub const KS_ERR_LICENCE: i32 = 2;
pub const KS_ERR_REFUSED: i32 = 3;
pub const KS_ERR_METHOD: i32 = 4;
pub const KS_ERR_INPUT: i32 = 5;
pub const KS_ERR_METER: i32 = 6;
pub const KS_ERR_INTERNAL: i32 = 7;

/// The opaque handle of the C ABI.
pub struct KsInsideHandle(KsInside);

fn now() -> i64 {
    crate::cli::now_unix()
}

/// A string for the caller, or null when it cannot be represented (interior NUL).
fn out_string(s: &str) -> *mut c_char {
    CString::new(s.replace('\0', " "))
        .map(CString::into_raw)
        .unwrap_or(std::ptr::null_mut())
}

fn error_json(e: &CallError) -> String {
    serde_json::json!({ "error": e.message(), "code": e.code() }).to_string()
}

/// Read a C string argument. Null → `Err`; not UTF-8 → `Err`.
///
/// # Safety
/// `p` is null or points to a NUL-terminated string valid for the call.
unsafe fn arg<'a>(p: *const c_char, name: &str) -> Result<&'a str, CallError> {
    if p.is_null() {
        return Err(CallError::Argument(format!("{name} is null")));
    }
    CStr::from_ptr(p)
        .to_str()
        .map_err(|_| CallError::Argument(format!("{name} is not UTF-8")))
}

/// Write `s` to `*out` when `out` is not null.
///
/// # Safety
/// `out` is null or a valid pointer to write one pointer to.
unsafe fn put(out: *mut *mut c_char, s: &str) {
    if !out.is_null() {
        *out = out_string(s);
    }
}

/// The ABI version of this library.
#[no_mangle]
pub extern "C" fn ks_inside_abi_version() -> u32 {
    super::ABI_VERSION
}

/// Open the SDK. On success `*handle` is set and [`KS_OK`] returned; on failure `*handle` is
/// null and, when `error` is not null, `*error` holds a JSON error (free it).
///
/// `revocations` may be null: the text of one signed revocation list shipped with the
/// firmware.
///
/// # Safety
/// The string arguments are null or NUL-terminated; `handle` is a valid pointer; `error` is
/// null or a valid pointer.
#[no_mangle]
pub unsafe extern "C" fn ks_inside_open(
    token: *const c_char,
    unit_id: *const c_char,
    meter_log_path: *const c_char,
    revocations: *const c_char,
    handle: *mut *mut KsInsideHandle,
    error: *mut *mut c_char,
) -> i32 {
    let r = catch_unwind(AssertUnwindSafe(|| -> Result<KsInside, CallError> {
        let cfg = Config {
            token: arg(token, "token")?.to_string(),
            unit_id: arg(unit_id, "unit_id")?.to_string(),
            meter_log: arg(meter_log_path, "meter_log_path")?.into(),
            revocations: if revocations.is_null() {
                Vec::new()
            } else {
                vec![arg(revocations, "revocations")?.to_string()]
            },
        };
        KsInside::open(&cfg, now())
    }));
    if handle.is_null() {
        put(
            error,
            &error_json(&CallError::Argument("handle is null".into())),
        );
        return KS_ERR_ARGUMENT;
    }
    *handle = std::ptr::null_mut();
    match r {
        Ok(Ok(k)) => {
            *handle = Box::into_raw(Box::new(KsInsideHandle(k)));
            KS_OK
        }
        Ok(Err(e)) => {
            put(error, &error_json(&e));
            e.code()
        }
        Err(_) => {
            put(error, r#"{"error":"internal error","code":7}"#);
            KS_ERR_INTERNAL
        }
    }
}

/// Call `method` with `input`. On [`KS_OK`] `*output` holds the JSON answer; on any other
/// code it holds a JSON error. Free it with [`ks_inside_free_string`].
///
/// # Safety
/// `handle` is null or came from [`ks_inside_open`] and was not closed; the strings are null
/// or NUL-terminated; `output` is a valid pointer.
#[no_mangle]
pub unsafe extern "C" fn ks_inside_call(
    handle: *const KsInsideHandle,
    method: *const c_char,
    input: *const c_char,
    output: *mut *mut c_char,
) -> i32 {
    if output.is_null() {
        return KS_ERR_ARGUMENT;
    }
    *output = std::ptr::null_mut();
    let r = catch_unwind(AssertUnwindSafe(|| -> Result<String, CallError> {
        let h = handle
            .as_ref()
            .ok_or_else(|| CallError::Argument("handle is null".into()))?;
        let method = arg(method, "method")?;
        let input = arg(input, "input")?;
        h.0.call(method, input, now())
    }));
    match r {
        Ok(Ok(s)) => {
            *output = out_string(&s);
            KS_OK
        }
        Ok(Err(e)) => {
            *output = out_string(&error_json(&e));
            e.code()
        }
        Err(_) => {
            *output = out_string(r#"{"error":"internal error","code":7}"#);
            KS_ERR_INTERNAL
        }
    }
}

/// The instance's description as JSON (not metered); null for a null handle. Free it.
///
/// # Safety
/// `handle` is null or came from [`ks_inside_open`] and was not closed.
#[no_mangle]
pub unsafe extern "C" fn ks_inside_info(handle: *const KsInsideHandle) -> *mut c_char {
    match handle.as_ref() {
        None => std::ptr::null_mut(),
        Some(h) => catch_unwind(AssertUnwindSafe(|| out_string(&h.0.info().to_string())))
            .unwrap_or(std::ptr::null_mut()),
    }
}

/// Free a string the library returned. Null is ignored.
///
/// # Safety
/// `s` is null or a string returned by this library and not yet freed.
#[no_mangle]
pub unsafe extern "C" fn ks_inside_free_string(s: *mut c_char) {
    if !s.is_null() {
        drop(CString::from_raw(s));
    }
}

/// Close a handle. Null is ignored.
///
/// # Safety
/// `handle` is null or came from [`ks_inside_open`] and was not closed.
#[no_mangle]
pub unsafe extern "C" fn ks_inside_close(handle: *mut KsInsideHandle) {
    if !handle.is_null() {
        drop(Box::from_raw(handle));
    }
}
