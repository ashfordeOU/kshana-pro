// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Timing appliance security posture (E19 B1902)
//!
//! Research 08b (findings O11, O12, O20): grandmasters and time servers carry their own CVEs
//! (six for one grandmaster line in 2024-2026, one of them upgrade images that are not signed,
//! CVE-2025-47904), and most fixes of one time-server line in 2025 were to bundled third-party
//! libraries (OpenSSH, curl, libxml2). Three tools:
//!
//! * **Exposure report** ([`check`]): every appliance of an inventory against the appliance
//!   advisory table of the time-bomb auditor (the same seed and the same `--advisories` files,
//!   matched by [`super::timebomb::fleet_check`]), the components of the vendor's SBOM against a
//!   component advisory table, the reach of the management interface, the services it exposes,
//!   and whether its firmware is protected by a signature the device checks or by a hash the
//!   operator checked before the upgrade.
//! * **Firmware check** ([`check_firmware`]): the SHA-256 of a firmware image against the
//!   vendor's published digest (a value or a `SHA256SUMS` file), or an Ed25519 detached
//!   signature against the vendor's public key. For an appliance that does not check its own
//!   upgrade images (CVE-2025-47904), this is the operator's out-of-band check.
//! * **SBOM** ([`station_bom`], [`check_bom`]): a CycloneDX 1.5 bill of materials for a
//!   station's firmware from a component list, and a structural check of any CycloneDX JSON
//!   (the Pro release's own `sbom.cdx.json` from `scripts/sbom.py`, or a vendor's).
//!
//! **Label.** The exposure report is COMPUTED from the inventory and the tables as supplied:
//! a match is a prompt to read the advisory, not a finding that the appliance is exploitable.
//! The SBOM check covers the required fields, enumerations and references of the CycloneDX
//! 1.5 JSON schema that this tool writes; it is not a full JSON-schema validation.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::timebomb::{self as tb, version_cmp, AdvisoryTable};

pub const FEATURE: &str = "pro-timing-posture";
pub const REPORT_FORMAT: &str = "kshana-pro-timing-posture/1";
pub const FIRMWARE_FORMAT: &str = "kshana-pro-firmware-check/1";
pub const LABEL: &str = "COMPUTED from the inventory, the vendor SBOMs and the advisory tables as supplied. A match is a prompt to read the vendor's advisory, not a finding that the appliance is exploitable; an appliance with no match may still be affected by an advisory the tables do not carry.";
/// Tag of an appliance advisory whose hazard is an upgrade image the device does not verify.
pub const TAG_UNSIGNED_FIRMWARE: &str = "unsigned-firmware";

/// One appliance of the posture inventory.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Appliance {
    pub id: String,
    #[serde(default)]
    pub site: Option<String>,
    pub vendor: String,
    pub model: String,
    #[serde(default)]
    pub firmware: Option<String>,
    #[serde(default)]
    pub role: Option<String>,
    /// Where the management interface can be reached from: `isolated` (a dedicated
    /// management network), `internal`, `internet` or `unknown` (default).
    #[serde(default)]
    pub management: Option<String>,
    /// Services the appliance exposes (`ssh`, `https`, `http`, `telnet`, `snmpv1`, `snmpv2c`,
    /// `snmpv3`, `ntp`, `ntp-mode6`, `ntp-mode7`, `ptp`, `ftp`, `tftp`, ...).
    #[serde(default)]
    pub services: Vec<String>,
    /// Whether the device checks a signature on its upgrade images: `device-verifies`,
    /// `unsigned` or `unknown` (default).
    #[serde(default)]
    pub upgrade_signing: Option<String>,
    /// A firmware check record (JSON written by `timing posture firmware`) for the image
    /// installed, relative to the inventory.
    #[serde(default)]
    pub firmware_check: Option<String>,
    /// The vendor's SBOM of the installed firmware (CycloneDX JSON), relative to the inventory.
    #[serde(default)]
    pub sbom: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Inventory {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub appliances: Vec<Appliance>,
}

/// One advisory on a bundled component (OpenSSH, curl, libxml2, ...).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComponentAdvisory {
    pub id: String,
    /// Component name, case-insensitive (CycloneDX `name`).
    pub component: String,
    /// Versions at and above this are not affected.
    #[serde(default)]
    pub fixed_in: Option<String>,
    /// Versions below this are not affected.
    #[serde(default)]
    pub affected_from: Option<String>,
    pub hazard: String,
    pub source: String,
    pub verification: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComponentTable {
    #[serde(default)]
    pub component_advisory: Vec<ComponentAdvisory>,
}

impl ComponentTable {
    pub fn parse(src: &str) -> Result<ComponentTable, String> {
        let t: ComponentTable =
            toml::from_str(src).map_err(|e| format!("component advisory table: {e}"))?;
        for a in &t.component_advisory {
            if a.id.trim().is_empty() || a.component.trim().is_empty() {
                return Err("component advisory: id and component are required".into());
            }
            if a.fixed_in.is_none() && a.affected_from.is_none() {
                return Err(format!(
                    "component advisory {}: give fixed_in or affected_from",
                    a.id
                ));
            }
        }
        Ok(t)
    }
}

/// One finding of the posture report.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PostureFinding {
    /// `advisory`, `component`, `firmware-integrity`, `management`, `service`.
    pub category: String,
    pub id: String,
    /// `affected`, `review`, `exposed`, `mitigated`, `not-affected`.
    pub status: String,
    /// `act`, `plan`, `watch`, `info`.
    pub priority: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ApplianceResult {
    pub id: String,
    pub site: Option<String>,
    pub vendor: String,
    pub model: String,
    pub firmware: Option<String>,
    pub management: String,
    pub sbom_components: Option<usize>,
    pub findings: Vec<PostureFinding>,
    pub worst: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PostureReport {
    pub format: &'static str,
    pub label: &'static str,
    pub title: Option<String>,
    pub utc: String,
    pub appliance_advisories: usize,
    pub component_advisories: usize,
    pub appliances: Vec<ApplianceResult>,
    pub counts: BTreeMap<String, usize>,
}

fn rank(p: &str) -> u8 {
    match p {
        "act" => 0,
        "plan" => 1,
        "watch" => 2,
        _ => 3,
    }
}

/// Services that are a finding when exposed, with the reason.
const RISKY_SERVICES: [(&str, &str); 8] = [
    ("telnet", "clear-text management login"),
    ("http", "clear-text web management"),
    ("ftp", "clear-text file transfer"),
    ("tftp", "unauthenticated file transfer"),
    ("snmpv1", "community-string SNMP (no authentication)"),
    ("snmpv2c", "community-string SNMP (no authentication)"),
    (
        "ntp-mode6",
        "NTP control queries (ntpq): information disclosure and amplification",
    ),
    (
        "ntp-mode7",
        "NTP private mode (monlist): reflection and amplification",
    ),
];

/// Assess every appliance. `base` resolves the inventory's relative paths.
pub fn check(
    inv: &Inventory,
    table: &AdvisoryTable,
    components: &ComponentTable,
    base: &Path,
    now_unix: i64,
    utc: &str,
) -> Result<PostureReport, String> {
    let mut out = Vec::new();
    let mut ids = BTreeSet::new();
    for a in &inv.appliances {
        if a.id.trim().is_empty() || !ids.insert(a.id.clone()) {
            return Err(format!("appliance id '{}' is empty or repeated", a.id));
        }
        let mgmt = a.management.clone().unwrap_or_else(|| "unknown".into());
        if !["isolated", "internal", "internet", "unknown"].contains(&mgmt.as_str()) {
            return Err(format!(
                "appliance {}: management must be isolated, internal, internet or unknown",
                a.id
            ));
        }
        let signing = a
            .upgrade_signing
            .clone()
            .unwrap_or_else(|| "unknown".into());
        if !["device-verifies", "unsigned", "unknown"].contains(&signing.as_str()) {
            return Err(format!(
                "appliance {}: upgrade_signing must be device-verifies, unsigned or unknown",
                a.id
            ));
        }
        let mut f: Vec<PostureFinding> = Vec::new();
        // Appliance advisories through the time-bomb auditor's matcher.
        let one = tb::Inventory {
            title: None,
            devices: vec![tb::Device {
                id: a.id.clone(),
                vendor: a.vendor.clone(),
                model: a.model.clone(),
                firmware: a.firmware.clone(),
                ..Default::default()
            }],
        };
        let fleet = tb::fleet_check(&one, table, now_unix, utc)?;
        let mut unsigned_advisory: Option<String> = None;
        for x in fleet.devices.into_iter().flat_map(|d| d.findings) {
            if x.source_kind == "epoch hazard" {
                continue;
            }
            let tagged = table
                .advisory
                .iter()
                .any(|t| t.id == x.id && t.tags.iter().any(|g| g == TAG_UNSIGNED_FIRMWARE));
            if tagged && x.status != "not-affected" {
                unsigned_advisory = Some(x.id.clone());
            }
            let mut priority = x.priority.clone();
            if x.status == "review" && mgmt == "internet" {
                priority = "act".into();
            }
            f.push(PostureFinding {
                category: "advisory".into(),
                id: x.id,
                status: x.status,
                priority,
                detail: x.detail,
            });
        }
        // Firmware integrity.
        let record = match &a.firmware_check {
            Some(p) => {
                let path = base.join(p);
                let text = std::fs::read_to_string(&path)
                    .map_err(|e| format!("appliance {}: {}: {e}", a.id, path.display()))?;
                let r: FirmwareCheck = serde_json::from_str(&text)
                    .map_err(|e| format!("appliance {}: {}: {e}", a.id, path.display()))?;
                Some(r)
            }
            None => None,
        };
        let record_ok = record.as_ref().is_some_and(|r| {
            r.passed()
                && match (&r.version, &a.firmware) {
                    (Some(v), Some(fw)) => version_cmp(v, fw).is_eq(),
                    _ => true,
                }
        });
        let needs_out_of_band = signing == "unsigned" || unsigned_advisory.is_some();
        if needs_out_of_band || signing == "unknown" || record.is_some() {
            let why = match (&unsigned_advisory, signing.as_str()) {
                (Some(id), _) => {
                    format!("advisory {id}: the device does not verify its upgrade images")
                }
                (None, "unsigned") => {
                    "the inventory states the device does not verify its upgrade images".into()
                }
                (None, "unknown") => {
                    "whether the device verifies its upgrade images is not stated".into()
                }
                _ => "the device verifies its upgrade images".into(),
            };
            let (status, priority, detail) = match &record {
                Some(r) if record_ok => (
                    "mitigated",
                    "info",
                    format!(
                        "{why}; the installed image was checked out of band: {} ({}), SHA-256 {}",
                        r.verdict, r.method, r.sha256
                    ),
                ),
                Some(r) => (
                    "exposed",
                    "act",
                    format!(
                        "{why}; the firmware check record does not support the installed image: verdict {}{}",
                        r.verdict,
                        match (&r.version, &a.firmware) {
                            (Some(v), Some(fw)) if !version_cmp(v, fw).is_eq() =>
                                format!(", record for version {v} but {fw} installed"),
                            _ => String::new(),
                        }
                    ),
                ),
                None if needs_out_of_band => (
                    "exposed",
                    "act",
                    format!("{why}; no out-of-band check of the installed image is recorded: run `timing posture firmware` against the vendor's published digest or signature before every upgrade"),
                ),
                None => (
                    "review",
                    "plan",
                    format!("{why}; ask the vendor, or record an out-of-band check"),
                ),
            };
            f.push(PostureFinding {
                category: "firmware-integrity".into(),
                id: "upgrade-image".into(),
                status: status.into(),
                priority: priority.into(),
                detail,
            });
        }
        // Management reach and services.
        match mgmt.as_str() {
            "internet" => f.push(PostureFinding {
                category: "management".into(),
                id: "management-reach".into(),
                status: "exposed".into(),
                priority: "act".into(),
                detail: "management interface reachable from the internet".into(),
            }),
            "unknown" => f.push(PostureFinding {
                category: "management".into(),
                id: "management-reach".into(),
                status: "review".into(),
                priority: "watch".into(),
                detail: "reach of the management interface not stated".into(),
            }),
            _ => {}
        }
        for s in &a.services {
            let s = s.trim().to_ascii_lowercase();
            if let Some((_, why)) = RISKY_SERVICES.iter().find(|(n, _)| *n == s) {
                f.push(PostureFinding {
                    category: "service".into(),
                    id: s.clone(),
                    status: "exposed".into(),
                    priority: if mgmt == "internet" { "act" } else { "plan" }.into(),
                    detail: (*why).into(),
                });
            }
        }
        // The vendor SBOM's components.
        let mut n_comp = None;
        if let Some(p) = &a.sbom {
            let path = base.join(p);
            let text = std::fs::read_to_string(&path)
                .map_err(|e| format!("appliance {}: {}: {e}", a.id, path.display()))?;
            let bom: serde_json::Value = serde_json::from_str(&text)
                .map_err(|e| format!("appliance {}: {}: {e}", a.id, path.display()))?;
            let issues = check_bom(&bom);
            if !issues.is_empty() {
                f.push(PostureFinding {
                    category: "component".into(),
                    id: "sbom".into(),
                    status: "review".into(),
                    priority: "watch".into(),
                    detail: format!(
                        "the vendor SBOM has {} structural issue(s): {}",
                        issues.len(),
                        issues.join("; ")
                    ),
                });
            }
            let comps = bom_components(&bom);
            n_comp = Some(comps.len());
            for (name, version) in &comps {
                for c in components
                    .component_advisory
                    .iter()
                    .filter(|c| c.component.eq_ignore_ascii_case(name))
                {
                    let (status, why) = match version {
                        None => ("review", "no version in the SBOM".to_string()),
                        Some(v) if c.affected_from.as_deref().is_some_and(|x| version_cmp(v, x).is_lt()) => (
                            "not-affected",
                            format!("{name} {v} below the first affected {}", c.affected_from.as_deref().unwrap_or("")),
                        ),
                        Some(v) => match &c.fixed_in {
                            Some(fx) if version_cmp(v, fx).is_lt() => ("affected", format!("{name} {v} below the fixed {fx}")),
                            Some(fx) => ("not-affected", format!("{name} {v} at or above the fixed {fx}")),
                            None => ("affected", format!("{name} {v} at or above the first affected; no fixed version in the table")),
                        },
                    };
                    let priority = match status {
                        "affected" => "act",
                        "review" => "plan",
                        _ => "info",
                    };
                    f.push(PostureFinding {
                        category: "component".into(),
                        id: c.id.clone(),
                        status: status.into(),
                        priority: priority.into(),
                        detail: format!(
                            "{}; {why}; source: {}; {}",
                            c.hazard, c.source, c.verification
                        ),
                    });
                }
            }
        }
        f.sort_by(|x, y| {
            rank(&x.priority)
                .cmp(&rank(&y.priority))
                .then(x.category.cmp(&y.category))
                .then(x.id.cmp(&y.id))
        });
        let worst = f
            .first()
            .map(|x| x.priority.clone())
            .unwrap_or_else(|| "info".into());
        out.push(ApplianceResult {
            id: a.id.clone(),
            site: a.site.clone(),
            vendor: a.vendor.clone(),
            model: a.model.clone(),
            firmware: a.firmware.clone(),
            management: mgmt,
            sbom_components: n_comp,
            findings: f,
            worst,
        });
    }
    out.sort_by(|a, b| rank(&a.worst).cmp(&rank(&b.worst)).then(a.id.cmp(&b.id)));
    let mut counts = BTreeMap::new();
    for d in &out {
        *counts.entry(d.worst.clone()).or_insert(0) += 1;
    }
    Ok(PostureReport {
        format: REPORT_FORMAT,
        label: LABEL,
        title: inv.title.clone(),
        utc: utc.into(),
        appliance_advisories: table.advisory.len(),
        component_advisories: components.component_advisory.len(),
        appliances: out,
        counts,
    })
}

/// The report as CSV, one finding per line.
pub fn report_csv(r: &PostureReport) -> String {
    let q = |s: &str| format!("\"{}\"", s.replace('"', "\"\""));
    let mut o = String::from(
        "appliance,site,vendor,model,firmware,management,category,finding,status,priority,detail\n",
    );
    for a in &r.appliances {
        for f in &a.findings {
            o.push_str(&format!(
                "{},{},{},{},{},{},{},{},{},{},{}\n",
                q(&a.id),
                q(a.site.as_deref().unwrap_or("")),
                q(&a.vendor),
                q(&a.model),
                q(a.firmware.as_deref().unwrap_or("")),
                a.management,
                f.category,
                q(&f.id),
                f.status,
                f.priority,
                q(&f.detail)
            ));
        }
    }
    o
}

// ---------------------------------------------------------------- firmware check

/// The vendor reference an image is checked against.
#[derive(Clone, Debug, PartialEq)]
pub enum Reference {
    /// A published SHA-256 (hex).
    Sha256(String),
    /// A `SHA256SUMS`-style file (`<hex>  <name>` per line); the image's file name is looked up.
    Sums(String),
    /// An Ed25519 detached signature over the image (64 bytes: raw, hex or base64) and the
    /// vendor's public key (32 bytes, hex).
    Ed25519 {
        signature: Vec<u8>,
        public_key: String,
    },
}

/// A firmware check record.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FirmwareCheck {
    pub format: String,
    pub image: String,
    pub size: u64,
    pub sha256: String,
    /// `sha256`, `sha256sums` or `ed25519`.
    pub method: String,
    /// The reference digest, or the public key's hex.
    pub reference: String,
    /// `match`, `mismatch`, `signature-valid`, `signature-invalid`, `not-listed`.
    pub verdict: String,
    #[serde(default)]
    pub vendor: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    pub utc: String,
    pub note: String,
}

impl FirmwareCheck {
    /// The image matched its reference.
    pub fn passed(&self) -> bool {
        self.verdict == "match" || self.verdict == "signature-valid"
    }
}

fn decode_sig(raw: &[u8]) -> Option<[u8; 64]> {
    if raw.len() == 64 {
        return raw.try_into().ok();
    }
    let t = std::str::from_utf8(raw).ok()?.trim();
    if let Ok(b) = hex::decode(t) {
        return b.try_into().ok();
    }
    base64_decode(t)?.try_into().ok()
}

fn base64_decode(s: &str) -> Option<Vec<u8>> {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::new();
    let (mut acc, mut bits) = (0u32, 0u32);
    for c in s.bytes().filter(|c| !c.is_ascii_whitespace() && *c != b'=') {
        let v = A.iter().position(|x| *x == c)? as u32;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

/// Check `image` (its bytes, and the file name used in a sums file) against `reference`.
pub fn check_firmware(
    name: &str,
    image: &[u8],
    reference: &Reference,
    utc: &str,
) -> Result<FirmwareCheck, String> {
    let digest = hex::encode(Sha256::digest(image));
    let base = Path::new(name)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(name)
        .to_string();
    let (method, refv, verdict, note) = match reference {
        Reference::Sha256(h) => {
            let h = h.trim().to_ascii_lowercase();
            if h.len() != 64 || hex::decode(&h).is_err() {
                return Err("--sha256 needs 64 hex digits".into());
            }
            let v = if h == digest { "match" } else { "mismatch" };
            ("sha256", h, v, "digest compared with the value given; check that it came from the vendor's own advisory or download page".to_string())
        }
        Reference::Sums(text) => {
            let hit = text.lines().find_map(|l| {
                let mut it = l.split_whitespace();
                let h = it.next()?;
                let f = it.next()?.trim_start_matches('*');
                (Path::new(f).file_name().and_then(|s| s.to_str()) == Some(base.as_str()))
                    .then(|| h.to_ascii_lowercase())
            });
            match hit {
                Some(h) => {
                    let v = if h == digest { "match" } else { "mismatch" };
                    ("sha256sums", h, v, "digest looked up by file name in the sums file; the sums file itself must come from the vendor (and be signed, where the vendor signs it)".to_string())
                }
                None => (
                    "sha256sums",
                    String::new(),
                    "not-listed",
                    format!("{base} is not listed in the sums file"),
                ),
            }
        }
        Reference::Ed25519 {
            signature,
            public_key,
        } => {
            use ed25519_dalek::{Signature, Verifier, VerifyingKey};
            let kb: [u8; 32] = hex::decode(public_key.trim())
                .ok()
                .and_then(|b| b.try_into().ok())
                .ok_or("the public key must be 32 bytes of hex")?;
            let key = VerifyingKey::from_bytes(&kb).map_err(|e| format!("public key: {e}"))?;
            let sig = decode_sig(signature)
                .ok_or("the signature must be 64 bytes (raw, hex or base64)")?;
            let ok = key.verify(image, &Signature::from_bytes(&sig)).is_ok();
            (
                "ed25519",
                public_key.trim().to_ascii_lowercase(),
                if ok { "signature-valid" } else { "signature-invalid" },
                "Ed25519 detached signature over the whole image; the key must be the vendor's, obtained out of band".to_string(),
            )
        }
    };
    Ok(FirmwareCheck {
        format: FIRMWARE_FORMAT.into(),
        image: base,
        size: image.len() as u64,
        sha256: digest,
        method: method.into(),
        reference: refv,
        verdict: verdict.into(),
        vendor: None,
        model: None,
        version: None,
        utc: utc.into(),
        note,
    })
}

// ---------------------------------------------------------------- SBOM

/// One component of a station's firmware.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StationComponent {
    pub name: String,
    #[serde(default)]
    pub version: Option<String>,
    /// CycloneDX component type (default `library`).
    #[serde(default)]
    pub r#type: Option<String>,
    #[serde(default)]
    pub purl: Option<String>,
    #[serde(default)]
    pub supplier: Option<String>,
    /// SPDX licence ids.
    #[serde(default)]
    pub licenses: Vec<String>,
    /// SHA-256 of the component's file (hex), or `file` to hash.
    #[serde(default)]
    pub sha256: Option<String>,
    #[serde(default)]
    pub file: Option<String>,
    /// Names of the components this one depends on.
    #[serde(default)]
    pub depends_on: Vec<String>,
}

/// A station firmware description.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StationSpec {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub supplier: Option<String>,
    #[serde(default)]
    pub components: Vec<StationComponent>,
}

pub const COMPONENT_TYPES: [&str; 12] = [
    "application",
    "framework",
    "library",
    "container",
    "platform",
    "operating-system",
    "device",
    "device-driver",
    "firmware",
    "file",
    "machine-learning-model",
    "data",
];
const HASH_ALGS: [(&str, usize); 12] = [
    ("MD5", 32),
    ("SHA-1", 40),
    ("SHA-256", 64),
    ("SHA-384", 96),
    ("SHA-512", 128),
    ("SHA3-256", 64),
    ("SHA3-384", 96),
    ("SHA3-512", 128),
    ("BLAKE2b-256", 64),
    ("BLAKE2b-384", 96),
    ("BLAKE2b-512", 128),
    ("BLAKE3", 64),
];

fn bom_ref(name: &str, version: Option<&str>) -> String {
    format!("{}@{}", name, version.unwrap_or("unversioned"))
}

/// A urn:uuid made from a digest (version 5 layout, so the serial is reproducible).
fn uuid_from(d: &[u8]) -> String {
    let mut b = [0u8; 16];
    b.copy_from_slice(&d[..16]);
    b[6] = (b[6] & 0x0f) | 0x50;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h = hex::encode(b);
    format!(
        "urn:uuid:{}-{}-{}-{}-{}",
        &h[0..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..32]
    )
}

/// A CycloneDX 1.5 BOM of a station's firmware. `base` resolves component files.
pub fn station_bom(
    spec: &StationSpec,
    base: &Path,
    utc: &str,
) -> Result<serde_json::Value, String> {
    use serde_json::json;
    let mut names = BTreeMap::new();
    let mut comps = Vec::new();
    for c in &spec.components {
        let r = bom_ref(&c.name, c.version.as_deref());
        if names.insert(c.name.clone(), r.clone()).is_some() {
            return Err(format!("component '{}' is listed twice", c.name));
        }
        let ty = c.r#type.clone().unwrap_or_else(|| "library".into());
        if !COMPONENT_TYPES.contains(&ty.as_str()) {
            return Err(format!("component {}: unknown type '{ty}'", c.name));
        }
        let mut o = json!({ "type": ty, "bom-ref": r, "name": c.name });
        if let Some(v) = &c.version {
            o["version"] = json!(v);
        }
        if let Some(p) = &c.purl {
            if !p.starts_with("pkg:") {
                return Err(format!("component {}: a purl starts with pkg:", c.name));
            }
            o["purl"] = json!(p);
        }
        if let Some(s) = &c.supplier {
            o["supplier"] = json!({ "name": s });
        }
        if !c.licenses.is_empty() {
            o["licenses"] = json!(c
                .licenses
                .iter()
                .map(|l| json!({ "license": { "id": l } }))
                .collect::<Vec<_>>());
        }
        let digest = match (&c.sha256, &c.file) {
            (Some(h), _) => Some(h.trim().to_ascii_lowercase()),
            (None, Some(f)) => {
                let p = base.join(f);
                let b = std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?;
                Some(hex::encode(Sha256::digest(&b)))
            }
            _ => None,
        };
        if let Some(h) = digest {
            if h.len() != 64 || hex::decode(&h).is_err() {
                return Err(format!(
                    "component {}: sha256 must be 64 hex digits",
                    c.name
                ));
            }
            o["hashes"] = json!([{ "alg": "SHA-256", "content": h }]);
        }
        comps.push(o);
    }
    let mut deps = Vec::new();
    let root = bom_ref(&spec.name, Some(&spec.version));
    deps.push(json!({
        "ref": root,
        "dependsOn": spec.components.iter().map(|c| bom_ref(&c.name, c.version.as_deref())).collect::<Vec<_>>()
    }));
    for c in &spec.components {
        let mut on = Vec::new();
        for d in &c.depends_on {
            on.push(
                names
                    .get(d)
                    .cloned()
                    .ok_or_else(|| format!("component {}: depends on unknown '{d}'", c.name))?,
            );
        }
        deps.push(json!({ "ref": bom_ref(&c.name, c.version.as_deref()), "dependsOn": on }));
    }
    let mut meta = json!({
        "timestamp": utc,
        "tools": { "components": [{ "type": "application", "name": "kshana-pro", "version": env!("CARGO_PKG_VERSION") }] },
        "component": { "type": "firmware", "bom-ref": root, "name": spec.name, "version": spec.version },
    });
    if let Some(s) = &spec.supplier {
        meta["supplier"] = json!({ "name": s });
    }
    let mut bom = json!({
        "bomFormat": "CycloneDX",
        "specVersion": "1.5",
        "version": 1,
        "metadata": meta,
        "components": comps,
        "dependencies": deps,
    });
    let canon = serde_json::to_vec(&bom).expect("serialises");
    bom["serialNumber"] = json!(uuid_from(&Sha256::digest(&canon)));
    Ok(bom)
}

/// The (name, version) of every component, nested ones included.
pub fn bom_components(bom: &serde_json::Value) -> Vec<(String, Option<String>)> {
    fn walk(v: &serde_json::Value, out: &mut Vec<(String, Option<String>)>) {
        for c in v.as_array().into_iter().flatten() {
            if let Some(n) = c.get("name").and_then(|x| x.as_str()) {
                out.push((
                    n.to_string(),
                    c.get("version")
                        .and_then(|x| x.as_str())
                        .map(str::to_string),
                ));
            }
            if let Some(sub) = c.get("components") {
                walk(sub, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(&bom["components"], &mut out);
    out
}

/// Structural check of a CycloneDX JSON BOM (1.4 to 1.6): required fields, enumerations,
/// hash lengths, unique `bom-ref`s and dependency references. Empty when no issue is found.
pub fn check_bom(bom: &serde_json::Value) -> Vec<String> {
    let mut e = Vec::new();
    if bom.get("bomFormat").and_then(|x| x.as_str()) != Some("CycloneDX") {
        e.push("bomFormat must be \"CycloneDX\"".to_string());
    }
    match bom.get("specVersion").and_then(|x| x.as_str()) {
        Some("1.4" | "1.5" | "1.6") => {}
        other => e.push(format!("specVersion {other:?} is not 1.4, 1.5 or 1.6")),
    }
    if let Some(v) = bom.get("version") {
        if v.as_u64().is_none_or(|n| n < 1) {
            e.push("version must be an integer of at least 1".into());
        }
    }
    if let Some(s) = bom.get("serialNumber") {
        let ok = s.as_str().is_some_and(|s| {
            let u = s.strip_prefix("urn:uuid:").unwrap_or("");
            let parts: Vec<&str> = u.split('-').collect();
            parts.len() == 5
                && parts
                    .iter()
                    .zip([8, 4, 4, 4, 12])
                    .all(|(p, n)| p.len() == n && p.chars().all(|c| c.is_ascii_hexdigit()))
        });
        if !ok {
            e.push("serialNumber must be urn:uuid:<RFC 4122 UUID>".into());
        }
    }
    let mut refs = BTreeSet::new();
    fn comp(c: &serde_json::Value, at: &str, e: &mut Vec<String>, refs: &mut BTreeSet<String>) {
        match c.get("type").and_then(|x| x.as_str()) {
            Some(t) if COMPONENT_TYPES.contains(&t) || t == "cryptographic-asset" => {}
            other => e.push(format!(
                "{at}: type {other:?} is not a CycloneDX component type"
            )),
        }
        if c.get("name")
            .and_then(|x| x.as_str())
            .is_none_or(str::is_empty)
        {
            e.push(format!("{at}: name is required"));
        }
        if let Some(r) = c.get("bom-ref") {
            match r.as_str() {
                Some(r) if !r.is_empty() => {
                    if !refs.insert(r.to_string()) {
                        e.push(format!("{at}: bom-ref '{r}' is not unique"));
                    }
                }
                _ => e.push(format!("{at}: bom-ref must be a non-empty string")),
            }
        }
        if let Some(p) = c.get("purl") {
            if !p
                .as_str()
                .is_some_and(|p| p.starts_with("pkg:") && p[4..].contains('/'))
            {
                e.push(format!("{at}: purl must be pkg:<type>/<name>"));
            }
        }
        for (j, h) in c
            .get("hashes")
            .and_then(|x| x.as_array())
            .into_iter()
            .flatten()
            .enumerate()
        {
            let alg = h.get("alg").and_then(|x| x.as_str()).unwrap_or("");
            let content = h.get("content").and_then(|x| x.as_str()).unwrap_or("");
            match HASH_ALGS.iter().find(|(a, _)| *a == alg) {
                None => e.push(format!("{at}.hashes[{j}]: unknown alg '{alg}'")),
                Some((_, n)) => {
                    if content.len() != *n || !content.chars().all(|c| c.is_ascii_hexdigit()) {
                        e.push(format!(
                            "{at}.hashes[{j}]: {alg} content must be {n} hex digits"
                        ));
                    }
                }
            }
        }
        for (j, s) in c
            .get("components")
            .and_then(|x| x.as_array())
            .into_iter()
            .flatten()
            .enumerate()
        {
            comp(s, &format!("{at}.components[{j}]"), e, refs);
        }
    }
    if let Some(m) = bom.get("metadata").and_then(|m| m.get("component")) {
        comp(m, "metadata.component", &mut e, &mut refs);
    }
    match bom.get("components") {
        None => {}
        Some(serde_json::Value::Array(a)) => {
            for (i, c) in a.iter().enumerate() {
                comp(c, &format!("components[{i}]"), &mut e, &mut refs);
            }
        }
        Some(_) => e.push("components must be an array".into()),
    }
    for (i, d) in bom
        .get("dependencies")
        .and_then(|x| x.as_array())
        .into_iter()
        .flatten()
        .enumerate()
    {
        match d.get("ref").and_then(|x| x.as_str()) {
            Some(r) if refs.contains(r) => {}
            Some(r) => e.push(format!("dependencies[{i}]: ref '{r}' names no component")),
            None => e.push(format!("dependencies[{i}]: ref is required")),
        }
        for x in d
            .get("dependsOn")
            .and_then(|x| x.as_array())
            .into_iter()
            .flatten()
        {
            if !x.as_str().is_some_and(|x| refs.contains(x)) {
                e.push(format!(
                    "dependencies[{i}]: dependsOn {x} names no component"
                ));
            }
        }
    }
    e
}
