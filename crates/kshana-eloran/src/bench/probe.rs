// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Find and identify bench hardware without changing anything on it.
//!
//! The probe only *reads*: it sends `*IDN?` to networked instruments, checks that serial
//! ports exist, and lists SDRs with the vendors' own discovery tools (`uhd_find_devices`,
//! `SoapySDRUtil --find`) when they are installed. It never configures an output and never
//! transmits.

use super::catalog::Catalog;
use super::check::{Inventory, InventoryItem};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

/// Probe outcome for one inventory line or address.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProbeRow {
    pub label: String,
    pub target: String,
    /// "found", "mismatch", "unreachable", "not_probed".
    pub state: String,
    pub idn: Option<String>,
    /// Catalogue ids the reply matches.
    pub identified_as: Vec<String>,
    pub detail: String,
}

/// Send `*IDN?` to `host:port` and return the first reply line.
pub fn scpi_idn(host: &str, port: u16, timeout: Duration) -> Result<String, String> {
    let addr = (host, port)
        .to_socket_addrs()
        .map_err(|e| format!("{host}:{port}: {e}"))?
        .next()
        .ok_or_else(|| format!("{host}:{port}: no address"))?;
    let mut s =
        TcpStream::connect_timeout(&addr, timeout).map_err(|e| format!("{host}:{port}: {e}"))?;
    s.set_read_timeout(Some(timeout)).ok();
    s.set_write_timeout(Some(timeout)).ok();
    s.write_all(b"*IDN?\n")
        .map_err(|e| format!("{host}:{port}: {e}"))?;
    let mut line = String::new();
    BufReader::new(s)
        .read_line(&mut line)
        .map_err(|e| format!("{host}:{port}: no reply ({e})"))?;
    let l = line.trim().to_string();
    if l.is_empty() {
        return Err(format!("{host}:{port}: empty reply"));
    }
    Ok(l)
}

/// Run a vendor discovery tool if it is on PATH; `None` when it is not installed.
fn discover(tool: &str, args: &[&str]) -> Option<String> {
    let out = std::process::Command::new(tool).args(args).output().ok()?;
    Some(format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    ))
}

fn probe_item(item: &InventoryItem, cat: &Catalog, timeout: Duration) -> ProbeRow {
    let spec = cat.get(&item.catalog);
    let mut row = ProbeRow {
        label: item.label(),
        target: String::new(),
        state: "not_probed".into(),
        idn: None,
        identified_as: vec![],
        detail: String::new(),
    };
    if let Some(p) = &item.serial {
        row.target = p.clone();
        let ok = std::path::Path::new(p).exists();
        row.state = if ok { "found" } else { "unreachable" }.into();
        row.detail = if ok {
            "serial port present (identity is read when the instrument opens)".into()
        } else {
            "serial port not present: plug in the USB cable or fix the path".into()
        };
        return row;
    }
    let drv = spec
        .and_then(|s| s.kshana_driver.clone())
        .unwrap_or_default();
    if let Some(h) = &item.host {
        let port = item
            .port
            .or(spec.and_then(|s| s.scpi_port))
            .unwrap_or(if drv == "redpitaya" { 5000 } else { 5025 });
        row.target = format!("{h}:{port}");
        match scpi_idn(h, port, timeout) {
            Ok(idn) => {
                row.identified_as = cat
                    .identify(&idn)
                    .into_iter()
                    .map(|s| s.id.clone())
                    .collect();
                let matches = spec.map(|s| s.matches_idn(&idn)).unwrap_or(false);
                let no_patterns = spec.map(|s| s.idn_patterns.is_empty()).unwrap_or(true);
                row.state = if matches || no_patterns {
                    "found"
                } else {
                    "mismatch"
                }
                .into();
                row.detail = if matches {
                    "identity matches the catalogue entry".into()
                } else if no_patterns {
                    "answered; the catalogue entry has no identity pattern to compare".into()
                } else {
                    format!(
                        "answered, but not as {} (identifies as: {})",
                        item.catalog,
                        if row.identified_as.is_empty() {
                            "unknown".into()
                        } else {
                            row.identified_as.join(", ")
                        }
                    )
                };
                row.idn = Some(idn);
            }
            Err(e) => {
                row.state = "unreachable".into();
                row.detail = if drv == "redpitaya" {
                    format!("{e} (start the SCPI server on the board: web interface → Development → SCPI server)")
                } else {
                    e
                };
            }
        }
        return row;
    }
    match drv.as_str() {
        "uhd" => {
            row.target = item.args.clone().unwrap_or_default();
            match discover("uhd_find_devices", &[]) {
                None => row.detail = "uhd_find_devices is not installed: install UHD 4.x to find USRPs".into(),
                Some(out) => {
                    let found = spec.map(|s| s.matches_idn(&out)).unwrap_or(false) || out.contains("serial:");
                    row.state = if found { "found" } else { "unreachable" }.into();
                    row.detail = out.lines().filter(|l| l.contains("serial") || l.contains("product") || l.contains("type")).collect::<Vec<_>>().join("; ");
                    if row.detail.is_empty() {
                        row.detail = "no USRP found".into();
                    }
                }
            }
        }
        "soapy" => {
            row.target = item.args.clone().unwrap_or_default();
            match discover("SoapySDRUtil", &["--find"]) {
                None => row.detail = "SoapySDRUtil is not installed: install SoapySDR 0.8 and the device module".into(),
                Some(out) => {
                    let found = out.contains("driver");
                    row.state = if found { "found" } else { "unreachable" }.into();
                    row.detail = out.lines().filter(|l| l.contains("driver") || l.contains("label") || l.contains("serial")).collect::<Vec<_>>().join("; ");
                    if row.detail.is_empty() {
                        row.detail = "no SoapySDR device found".into();
                    }
                }
            }
        }
        "digilent" => row.detail = "USB device: identity is read when the WaveForms SDK opens it (`bench accept` or `stream --device digilent:0`)".into(),
        _ => row.detail = "no host, serial port or USB driver given: nothing to probe".into(),
    }
    row
}

/// Probe every line of the inventory.
pub fn probe_inventory(inv: &Inventory, cat: &Catalog, timeout: Duration) -> Vec<ProbeRow> {
    inv.devices
        .iter()
        .map(|d| probe_item(d, cat, timeout))
        .collect()
}

/// Probe bare hosts on the usual SCPI ports (5025 raw socket, 5000 Red Pitaya, 5024
/// telnet-style) and identify what answers.
pub fn probe_hosts(hosts: &[String], cat: &Catalog, timeout: Duration) -> Vec<ProbeRow> {
    let mut rows = vec![];
    for h in hosts {
        let (host, ports): (&str, Vec<u16>) = match h.rsplit_once(':') {
            Some((a, p)) if p.parse::<u16>().is_ok() => (a, vec![p.parse().unwrap_or(5025)]),
            _ => (h.as_str(), vec![5025, 5000, 5024]),
        };
        let mut last_err = String::new();
        let mut hit = false;
        for p in ports {
            match scpi_idn(host, p, timeout) {
                Ok(idn) => {
                    let ids: Vec<String> = cat
                        .identify(&idn)
                        .into_iter()
                        .map(|s| s.id.clone())
                        .collect();
                    rows.push(ProbeRow {
                        label: host.to_string(),
                        target: format!("{host}:{p}"),
                        state: "found".into(),
                        detail: if ids.is_empty() {
                            "not in the catalogue: add an entry with idn_patterns".into()
                        } else {
                            "identified".into()
                        },
                        idn: Some(idn),
                        identified_as: ids,
                    });
                    hit = true;
                    break;
                }
                Err(e) => last_err = e,
            }
        }
        if !hit {
            rows.push(ProbeRow {
                label: host.to_string(),
                target: host.to_string(),
                state: "unreachable".into(),
                idn: None,
                identified_as: vec![],
                detail: last_err,
            });
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn idn_is_read_and_matched_against_the_catalogue() {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        let t = std::thread::spawn(move || {
            let (mut s, _) = l.accept().unwrap();
            let mut buf = [0u8; 16];
            let _ = std::io::Read::read(&mut s, &mut buf);
            s.write_all(b"Acme Instruments,TIC-9,SN42,1.0\n").unwrap();
        });
        let mut cat = Catalog::default();
        cat.merge_json(r#"[{"id":"acme-tic-9","vendor":"Acme","model":"TIC-9","category":"tic","idn_patterns":["acme","tic-9"],"sources":[{"url":"https://e.org","title":"t"}]}]"#).unwrap();
        let rows = probe_hosts(&[format!("127.0.0.1:{port}")], &cat, Duration::from_secs(2));
        t.join().unwrap();
        assert_eq!(rows[0].state, "found");
        assert_eq!(rows[0].identified_as, vec!["acme-tic-9".to_string()]);
        let rows = probe_hosts(&["127.0.0.1:1".into()], &cat, Duration::from_millis(300));
        assert_eq!(rows[0].state, "unreachable");
    }
}
