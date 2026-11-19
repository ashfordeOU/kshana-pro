// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! The bench kit: everything a lab needs on the day the hardware arrives, generated from
//! the inventory and the catalogue.
//!
//! `write_kit(dir)` writes:
//!
//! | File | What |
//! |---|---|
//! | `ASSESSMENT.txt`, `assessment.json` | readiness check ([`super::check::assess`]) |
//! | `WIRING.md` | cable-by-cable plan: reference, PPS, eLoran path with attenuator, GNSS path, counters |
//! | `PROCEDURE.md` | first-plug procedure, step by step, with the exact commands |
//! | `devices.json` | the address of every device ([`super::open::address_of`]) |
//! | `profiles/<label>.toml` | instrument profiles to open counters and analysers |
//! | `twin-<id>.json` | expected performance of each eLoran source ([`super::spectwin::expected_performance`]) |
//! | `inventory.toml` | the inventory itself |
//!
//! **Instrument command sets are generic placeholders** (IEEE 488.2 / SCPI-style), not a
//! vendor's verified set: confirm each command against the programming manual before the
//! first run (PROCEDURE.md lists them). Nothing here was measured on hardware.

use super::catalog::{Catalog, Category, HwSpec};
use super::check::{assess, Assessment, BenchRole, Inventory, InventoryItem, Status};
use super::open::address_of;
use super::spectwin::expected_performance;
use crate::instr::profile::{InstrumentProfile, Role, TransportConfig};
use std::fmt::Write as _;
use std::path::Path;

/// Standard fixed-attenuator values (dB) for the plan.
const PADS: [f64; 7] = [3.0, 6.0, 10.0, 20.0, 30.0, 40.0, 60.0];

/// Smallest combination of standard pads reaching at least `db` (up to three pads).
pub fn pads_for(db: f64) -> Vec<f64> {
    if db <= 0.0 {
        return vec![];
    }
    let mut best: Option<(f64, Vec<f64>)> = None;
    let mut consider = |v: Vec<f64>| {
        let s: f64 = v.iter().sum();
        if s + 1e-9 >= db {
            let better = match &best {
                None => true,
                Some((bs, bv)) => s < *bs - 1e-9 || ((s - bs).abs() < 1e-9 && v.len() < bv.len()),
            };
            if better {
                best = Some((s, v));
            }
        }
    };
    for a in PADS {
        consider(vec![a]);
        for b in PADS {
            consider(vec![a, b]);
            for c in PADS {
                consider(vec![a, b, c]);
            }
        }
    }
    let mut v = best.map(|b| b.1).unwrap_or_else(|| vec![60.0, 60.0, 60.0]);
    v.sort_by(|a, b| b.partial_cmp(a).unwrap());
    v
}

fn spec<'a>(cat: &'a Catalog, d: &InventoryItem) -> Option<&'a HwSpec> {
    cat.get(&d.catalog)
}

fn model(cat: &Catalog, d: &InventoryItem) -> String {
    spec(cat, d)
        .map(|s| format!("{} {}", s.vendor, s.model))
        .unwrap_or_else(|| d.catalog.clone())
}

/// The wiring plan (Markdown).
pub fn wiring_md(inv: &Inventory, cat: &Catalog) -> String {
    let req = &inv.requirements;
    let mut s = String::new();
    let _ = writeln!(
        s,
        "# Wiring plan: {}\n",
        if inv.name.is_empty() {
            "bench"
        } else {
            &inv.name
        }
    );
    let _ = writeln!(
        s,
        "> **Conducted use only.** Every output goes to cables, attenuators, couplers or an antenna \
         simulator, never to a radiating antenna. Over-the-air LF or GNSS transmission is not permitted.\n"
    );
    let refs = inv.in_role(BenchRole::Reference);
    let dist = inv.in_role(BenchRole::Distribution);
    let mut n = 0;
    let mut line = |s: &mut String, from: String, to: String, cable: &str, note: &str| {
        n += 1;
        let _ = writeln!(s, "| {n} | {from} | {to} | {cable} | {note} |");
    };
    let _ = writeln!(
        s,
        "## Cables\n\n| # | From | To | Cable | Note |\n|---|---|---|---|---|"
    );
    // Reference distribution.
    if let Some(r) = refs.first() {
        let rname = format!("{} ({})", r.label(), model(cat, r));
        let hub = dist
            .first()
            .map(|d| format!("{} ({})", d.label(), model(cat, d)));
        if let Some(h) = &hub {
            line(
                &mut s,
                format!("{rname} 10 MHz out"),
                format!("{h} 10 MHz in"),
                "50 Ω coax, BNC/SMA",
                "keep short; one reference for the whole bench",
            );
        }
        for (i, d) in inv.devices.iter().enumerate() {
            if matches!(
                d.role,
                BenchRole::Reference | BenchRole::Distribution | BenchRole::Antenna
            ) {
                continue;
            }
            // The same box in a second role (e.g. a Red Pitaya as source and digitiser) is
            // wired once.
            let same_box = inv.devices[..i].iter().any(|e| {
                e.catalog == d.catalog
                    && (d.host.is_none() || e.host == d.host)
                    && e.serial == d.serial
            });
            if same_box {
                continue;
            }
            let Some(sp) = spec(cat, d) else { continue };
            if sp.takes_10mhz() == Some(true) {
                let conn = sp
                    .ref_in
                    .as_ref()
                    .and_then(|r| r.connector.clone())
                    .unwrap_or_else(|| "ref in".into());
                line(
                    &mut s,
                    format!("{} 10 MHz", hub.clone().unwrap_or_else(|| rname.clone())),
                    format!("{} ({}) {conn}", d.label(), model(cat, d)),
                    "50 Ω coax, equal lengths where timing matters",
                    sp.ref_in
                        .as_ref()
                        .and_then(|r| r.level.clone())
                        .as_deref()
                        .unwrap_or("check the input level in the manual"),
                );
            }
            if sp.pps_in == Some(true) {
                line(
                    &mut s,
                    format!("{rname} 1PPS out"),
                    format!("{} ({}) PPS in", d.label(), model(cat, d)),
                    "50 Ω coax; record the length (≈5 ns/m)",
                    "cable delay enters the calibration budget",
                );
            }
        }
        for t in inv.in_role(BenchRole::Tic) {
            line(
                &mut s,
                format!("{rname} 1PPS out"),
                format!("{} channel A (start)", t.label()),
                "50 Ω coax; record the length",
                "TIC start = reference PPS",
            );
        }
    }
    // eLoran path.
    for src in inv.in_role(BenchRole::EloranSource) {
        let sp = spec(cat, src);
        let vpp = sp
            .and_then(|s| s.tx.as_ref())
            .and_then(|t| t.max_vpp_into_50ohm);
        let dut = inv.in_role(BenchRole::EloranRx);
        let target = dut
            .first()
            .map(|d| format!("{} ({}) antenna input", d.label(), model(cat, d)))
            .unwrap_or_else(|| "receiver antenna input / H-field antenna simulator".into());
        let pad_note = match (vpp, req.dut_input_vpp) {
            (Some(v), Some(t)) if v > t => {
                let db = 20.0 * (v / t).log10() + 6.0;
                let p = pads_for(db);
                format!(
                    "attenuate ≥ {db:.1} dB (full scale {v} Vpp → {t} Vpp, +6 dB margin): pads {} dB, 50 Ω, DC–1 MHz rated",
                    p.iter().map(|x| format!("{x:.0}")).collect::<Vec<_>>().join(" + ")
                )
            }
            (Some(v), None) => format!(
                "full scale {v} Vpp into 50 Ω; set the attenuator from the receiver manual's maximum antenna-input level (dut_input_vpp in the inventory gives the pad values)"
            ),
            _ => "full-scale level not published: start with 40 dB and measure".into(),
        };
        line(
            &mut s,
            format!("{} ({}) OUT1", src.label(), model(cat, src)),
            "fixed attenuator(s)".into(),
            "50 Ω coax",
            &pad_note,
        );
        line(
            &mut s,
            "attenuator".into(),
            format!("BNC tee → {target}"),
            "50 Ω coax",
            "terminate in 50 Ω at the far end unless the input is 50 Ω",
        );
        if let Some(dg) = inv.in_role(BenchRole::Digitiser).first() {
            let lim = spec(cat, dg)
                .and_then(|s| s.rx.as_ref())
                .and_then(|r| r.max_input_vpp);
            line(
                &mut s,
                "BNC tee".into(),
                format!("{} ({}) CH1 (loopback capture)", dg.label(), model(cat, dg)),
                "50 Ω coax, short",
                &match lim {
                    Some(l) => format!("input limit {l} Vpp: stay below it"),
                    None => "check the input range before connecting".into(),
                },
            );
        }
        if req.tx_channels >= 2 {
            line(
                &mut s,
                format!("{} OUT2", src.label()),
                "second loop of the antenna simulator (crossed H-field)".into(),
                "50 Ω coax, same length as OUT1",
                "channel-to-channel delay is calibrated by the loopback procedure",
            );
        }
        if let Some(c) = inv.in_role(BenchRole::Spectrum).first() {
            line(&mut s, "second BNC tee (when measuring)".into(), format!("{} ({}) RF in", c.label(), model(cat, c)), "50 Ω coax", "DC block if the analyser input has no DC rating; 100 kHz needs an analyser that starts below 9 kHz");
        }
    }
    // GNSS path.
    for g in inv.in_role(BenchRole::GnssSource) {
        let sp = spec(cat, g);
        let is_sim = sp.map(|s| s.category == Category::GnssSim).unwrap_or(false);
        let note = if is_sim {
            "set the simulator's output to the receiver's expected level (≈ −130 dBm per signal); add a DC block if the receiver feeds antenna bias"
        } else {
            "SDR as a GNSS generator: at least 60 dB of attenuation and a DC block; conducted only, inside a shielded setup"
        };
        let to = inv
            .devices
            .iter()
            .find(|d| {
                matches!(d.role, BenchRole::EloranRx)
                    || spec(cat, d)
                        .map(|s| s.category == Category::GnssTimingRx)
                        .unwrap_or(false)
            })
            .map(|d| format!("{} GNSS antenna input", d.label()))
            .unwrap_or_else(|| "GNSS receiver antenna input".into());
        line(
            &mut s,
            format!("{} ({}) RF out", g.label(), model(cat, g)),
            to,
            "50 Ω coax + DC block + attenuator",
            note,
        );
        if sp.and_then(|s| s.pps_out) == Some(true) {
            if let Some(t) = inv.in_role(BenchRole::Tic).first() {
                line(
                    &mut s,
                    format!("{} 1PPS out", g.label()),
                    format!(
                        "{} channel B (stop) for the eLoran–GNSS sync test",
                        t.label()
                    ),
                    "50 Ω coax; record the length",
                    "",
                );
            }
        }
    }
    for c in inv.in_role(BenchRole::Counter) {
        if let Some(src) = inv.in_role(BenchRole::EloranSource).first() {
            if spec(cat, src)
                .and_then(|s| s.ref_out.as_ref())
                .map(|r| !r.freqs_hz.is_empty())
                == Some(true)
            {
                line(
                    &mut s,
                    format!("{} 10 MHz out", src.label()),
                    format!("{} input", c.label()),
                    "50 Ω coax",
                    "reference-lock test: counter time base on the bench reference",
                );
            }
        }
    }
    let _ = writeln!(s, "\n## Delays to record\n\nWrite every cable length and attenuator part number into the calibration register: the loopback procedure measures the output delay, but cable delays between the reference and each instrument (≈ 5 ns per metre of coax) enter the uncertainty budget.");
    s
}

/// The first-plug procedure (Markdown).
pub fn procedure_md(inv: &Inventory, cat: &Catalog, a: &Assessment) -> String {
    let mut s = String::new();
    let _ = writeln!(
        s,
        "# First-plug procedure: {}\n",
        if inv.name.is_empty() {
            "bench"
        } else {
            &inv.name
        }
    );
    if !a.ready {
        let _ = writeln!(s, "> **The readiness check has FAIL items** (ASSESSMENT.txt). Fix them before buying or wiring.\n");
    }
    let src = inv.in_role(BenchRole::EloranSource).into_iter().next();
    let addr = src.and_then(|d| address_of(d, spec(cat, d)).ok());
    let mut step = 0;
    let mut st = |s: &mut String, t: &str| {
        step += 1;
        let _ = writeln!(s, "{step}. {t}");
    };
    st(&mut s, "Rehearse now, before any hardware: `kshana-pro eloran bench rehearse --inventory inventory.toml --out rehearsal.json`. Every device is replaced by its spec twin; the run must complete with no ERROR verdicts.");
    st(&mut s, "Power the reference and let it settle (a rubidium needs its warm-up; a GPSDO needs sky view and lock). Do not start measurements before the reference reports lock.");
    st(
        &mut s,
        "Wire the bench exactly as WIRING.md, outputs disconnected from the receiver until step 6.",
    );
    st(&mut s, "Find the instruments: `kshana-pro eloran bench probe --inventory inventory.toml`. Each networked instrument must answer `*IDN?` and match its catalogue entry; fix addresses in inventory.toml if not.");
    let reviews: Vec<String> = inv
        .devices
        .iter()
        .filter(|d| {
            matches!(
                d.role,
                BenchRole::Tic | BenchRole::Counter | BenchRole::Spectrum | BenchRole::Power
            )
        })
        .map(|d| format!("`profiles/{}.toml` ({})", d.label(), model(cat, d)))
        .collect();
    if !reviews.is_empty() {
        st(&mut s, &format!(
            "Confirm each instrument profile against its programming manual and record the manual revision in the profile: {}. The commands written by the kit are generic placeholders.",
            reviews.join(", ")
        ));
    }
    if let Some(a) = &addr {
        st(&mut s, &format!("Stream a test scene into the loopback only (receiver disconnected): `kshana-pro eloran stream <scenario.toml> --device {a} --external-ref --external-pps --pps-start`. Check on the scope that pulses arrive once per GRI at the expected level."));
    } else if let Some(d) = src {
        st(&mut s, &format!("{} has no Kshana streaming driver: export with `kshana-pro eloran awg <scenario.toml> --out <base>` and play the file from the vendor's software.", d.label()));
    }
    st(&mut s, "Calibrate the output delay with the loopback capture (calibration pulse train, one pulse per PPS) and store the result in the calibration register.");
    st(&mut s, "Run the acceptance suite: `kshana-pro eloran bench accept --inventory inventory.toml --profiles profiles --out acceptance.json`. Compare with the twin predictions in twin-*.json: a measured value far worse than predicted points at wiring, level or grounding.");
    st(
        &mut s,
        "Connect the receiver (through the attenuator) and run the scenarios of the campaign.",
    );
    let _ = writeln!(s, "\n## Tests this bench can run\n");
    for r in &a.runnable {
        let _ = writeln!(s, "- {r}");
    }
    if !a.not_runnable.is_empty() {
        let _ = writeln!(s, "\n## Tests it cannot run yet\n");
        for (r, why) in &a.not_runnable {
            let _ = writeln!(s, "- {r}: {why}");
        }
    }
    let unknown: Vec<_> = a
        .findings
        .iter()
        .filter(|f| f.status == Status::Unknown)
        .collect();
    if !unknown.is_empty() {
        let _ = writeln!(s, "\n## Check by hand (no published value)\n");
        for f in unknown {
            let _ = writeln!(s, "- {} / {}: {}", f.device, f.check, f.detail);
        }
    }
    s
}

/// The instrument profile for an inventory line, when its role takes one.
pub fn profile_for(item: &InventoryItem, cat: &Catalog) -> Option<InstrumentProfile> {
    let sp = spec(cat, item);
    let ticc = sp.map(|s| s.id.contains("ticc")).unwrap_or(false);
    let role = match item.role {
        BenchRole::Tic if ticc => Role::Ticc,
        BenchRole::Tic => Role::TimeIntervalCounter,
        BenchRole::Counter => Role::FrequencyCounter,
        BenchRole::Spectrum => Role::SpectrumAnalyser,
        BenchRole::Power => Role::PowerMeter,
        BenchRole::Digitiser if sp.map(|s| s.category == Category::Scope).unwrap_or(false) => {
            Role::Oscilloscope
        }
        _ => return None,
    };
    let transport = match (&item.serial, &item.host) {
        (Some(p), _) => TransportConfig::Serial {
            path: p.into(),
            terminator: "\r\n".into(),
        },
        (None, Some(h)) => TransportConfig::Tcp {
            host: h.clone(),
            port: item.port.or(sp.and_then(|s| s.scpi_port)).unwrap_or(5025),
            timeout_ms: 5000,
            terminator: "\n".into(),
        },
        (None, None) if role == Role::Ticc => TransportConfig::Serial {
            path: "/dev/ttyACM0".into(),
            terminator: "\r\n".into(),
        },
        (None, None) => TransportConfig::Tcp {
            host: "192.168.1.100".into(),
            port: sp.and_then(|s| s.scpi_port).unwrap_or(5025),
            timeout_ms: 5000,
            terminator: "\n".into(),
        },
    };
    // The vendor's own commands when built in; otherwise the generic placeholders.
    if let Some(mut p) = InstrumentProfile::builtin(&item.catalog, role, transport.clone()) {
        p.id = item.label();
        return Some(p);
    }
    let mut p = InstrumentProfile::default_for(&item.label(), role, transport);
    if let Some(s) = sp {
        p.model = format!(
            "{} {} (GENERIC placeholder commands: confirm against the manual)",
            s.vendor, s.model
        );
    }
    Some(p)
}

/// Write the kit into `dir`. Returns the files written.
pub fn write_kit(
    inv: &Inventory,
    inv_text: &str,
    cat: &Catalog,
    dir: &Path,
    fs_hz: f64,
) -> Result<Vec<String>, String> {
    let mk = |p: &Path| std::fs::create_dir_all(p).map_err(|e| format!("{}: {e}", p.display()));
    mk(dir)?;
    let mut written = vec![];
    let mut put = |name: &str, body: &str| -> Result<(), String> {
        let p = dir.join(name);
        if let Some(d) = p.parent() {
            std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
        }
        std::fs::write(&p, body).map_err(|e| format!("{}: {e}", p.display()))?;
        written.push(name.to_string());
        Ok(())
    };
    let a = assess(inv, cat);
    put("inventory.toml", inv_text)?;
    put("ASSESSMENT.txt", &a.to_text())?;
    put("assessment.json", &a.to_json())?;
    put("WIRING.md", &wiring_md(inv, cat))?;
    put("PROCEDURE.md", &procedure_md(inv, cat, &a))?;
    let devices: Vec<serde_json::Value> = inv
        .devices
        .iter()
        .map(|d| {
            serde_json::json!({
                "label": d.label(),
                "role": d.role,
                "catalog": d.catalog,
                "address": address_of(d, cat.get(&d.catalog)).ok(),
                "rehearsal_address": format!("twin:{}", d.catalog),
            })
        })
        .collect();
    put(
        "devices.json",
        &serde_json::to_string_pretty(&devices).unwrap_or_default(),
    )?;
    for d in &inv.devices {
        if let Some(p) = profile_for(d, cat) {
            put(&format!("profiles/{}.toml", d.label()), &p.to_toml())?;
        }
    }
    for d in inv.in_role(BenchRole::EloranSource) {
        if let Some(sp) = cat.get(&d.catalog) {
            let body = match expected_performance(sp, fs_hz, &[0.0, -20.0, -40.0, -60.0], 10) {
                Ok(r) => r.to_json(),
                Err(e) => {
                    serde_json::to_string_pretty(&serde_json::json!({"device": sp.id, "error": e}))
                        .unwrap_or_default()
                }
            };
            put(&format!("twin-{}.json", sp.id), &body)?;
        }
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pads_reach_the_target_with_few_parts() {
        assert_eq!(pads_for(0.0), Vec::<f64>::new());
        assert_eq!(pads_for(10.0), vec![10.0]);
        assert_eq!(pads_for(8.0), vec![6.0, 3.0]);
        assert_eq!(pads_for(26.0), vec![20.0, 6.0]);
        assert_eq!(pads_for(45.0), vec![40.0, 6.0]);
    }
}
