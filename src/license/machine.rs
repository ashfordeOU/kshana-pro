// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Machine binding for licence tokens (roadmap F10b).
//!
//! A token may name one machine by a hash of that machine's hardware ID. The raw ID
//! never leaves the machine: the customer runs `kshana-pro license machine-id`, which
//! prints only the hash, and sends that to the vendor (by e-mail or on paper for an
//! air-gapped site). The hash is salted with a fixed Kshana prefix so it cannot be
//! matched against the same ID used by other software.
//!
//! Hardware ID sources: Linux `/etc/machine-id` (or `/var/lib/dbus/machine-id`),
//! macOS `IOPlatformUUID` from `ioreg`, Windows `MachineGuid` from the registry. The
//! `KSHANA_MACHINE_ID` environment variable overrides the source (containers, where
//! the ID is set by the operator, and tests).

use sha2::{Digest, Sha256};

/// Hash a raw hardware ID into the 64-hex form a token carries.
pub fn hash_machine_id(raw: &str) -> String {
    let mut h = Sha256::new();
    h.update(b"kshana-machine-v1:");
    h.update(raw.trim().to_ascii_lowercase().as_bytes());
    hex::encode(h.finalize())
}

/// The raw hardware ID of this machine, when it can be read.
pub fn raw_machine_id() -> Option<String> {
    if let Ok(v) = std::env::var("KSHANA_MACHINE_ID") {
        if !v.trim().is_empty() {
            return Some(v.trim().to_string());
        }
    }
    platform_id().filter(|s| !s.trim().is_empty())
}

/// The machine hash of this machine.
pub fn this_machine_hash() -> Option<String> {
    raw_machine_id().map(|r| hash_machine_id(&r))
}

#[cfg(target_os = "linux")]
fn platform_id() -> Option<String> {
    ["/etc/machine-id", "/var/lib/dbus/machine-id"]
        .iter()
        .find_map(|p| std::fs::read_to_string(p).ok())
        .map(|s| s.trim().to_string())
}

#[cfg(target_os = "macos")]
fn platform_id() -> Option<String> {
    let out = std::process::Command::new("/usr/sbin/ioreg")
        .args(["-rd1", "-c", "IOPlatformExpertDevice"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    text.lines()
        .find(|l| l.contains("IOPlatformUUID"))
        .and_then(|l| l.split('"').nth(3))
        .map(str::to_string)
}

#[cfg(target_os = "windows")]
fn platform_id() -> Option<String> {
    let out = std::process::Command::new("reg")
        .args([
            "query",
            r"HKLM\SOFTWARE\Microsoft\Cryptography",
            "/v",
            "MachineGuid",
        ])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    text.lines()
        .find(|l| l.contains("MachineGuid"))
        .and_then(|l| l.split_whitespace().last())
        .map(str::to_string)
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn platform_id() -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_is_stable_salted_and_case_insensitive() {
        let a = hash_machine_id("ABCDEF-1234");
        assert_eq!(a, hash_machine_id(" abcdef-1234\n"));
        assert_eq!(a.len(), 64);
        // Salted: not the bare SHA-256 of the ID.
        assert_ne!(a, hex::encode(Sha256::digest(b"abcdef-1234")));
        assert_ne!(a, hash_machine_id("abcdef-1235"));
    }
}
