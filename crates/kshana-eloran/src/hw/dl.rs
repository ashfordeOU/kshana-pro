// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Minimal `dlopen` helper shared by the run-time-loaded drivers (`uhd`, `digilent`).
//! The vendor library is the user's own installation; the crate has no link dependency.

use std::ffi::{c_void, CString};

/// An open shared library, closed on drop.
pub struct DynLib {
    handle: *mut c_void,
    pub name: String,
}

// SAFETY: a dlopen handle is process-global and may be used from any thread.
unsafe impl Send for DynLib {}

impl DynLib {
    /// Open the first library of `names` that loads; `env` names an environment variable
    /// whose value, when set, is tried first (a full path to the library).
    pub fn open(names: &[&str], env: Option<&str>) -> Result<DynLib, String> {
        let mut list: Vec<String> = vec![];
        if let Some(e) = env.and_then(|e| std::env::var(e).ok()) {
            list.push(e);
        }
        list.extend(names.iter().map(|s| s.to_string()));
        for name in &list {
            let c = CString::new(name.as_str()).map_err(|e| e.to_string())?;
            // SAFETY: dlopen with a valid C string; a null result is handled.
            let h = unsafe { libc::dlopen(c.as_ptr(), libc::RTLD_NOW) };
            if !h.is_null() {
                return Ok(DynLib {
                    handle: h,
                    name: name.clone(),
                });
            }
        }
        Err(format!("library not found (tried {})", list.join(", ")))
    }

    /// Look up a symbol as a function pointer of type `T`.
    ///
    /// # Safety
    /// `T` must be an `extern "C" fn` pointer type matching the symbol's C signature.
    pub unsafe fn sym<T: Copy>(&self, name: &str) -> Result<T, String> {
        let c = CString::new(name).map_err(|e| e.to_string())?;
        // SAFETY: dlsym on an open handle.
        let p = unsafe { libc::dlsym(self.handle, c.as_ptr()) };
        if p.is_null() {
            return Err(format!("{}: symbol {name} missing", self.name));
        }
        debug_assert_eq!(std::mem::size_of::<T>(), std::mem::size_of::<*mut c_void>());
        // SAFETY: the caller guarantees `T` is a function pointer type of pointer size.
        Ok(unsafe { std::mem::transmute_copy::<*mut c_void, T>(&p) })
    }
}

impl Drop for DynLib {
    fn drop(&mut self) {
        // SAFETY: the handle came from dlopen and is closed once.
        unsafe { libc::dlclose(self.handle) };
    }
}
