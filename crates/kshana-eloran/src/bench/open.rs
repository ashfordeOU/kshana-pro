// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Open any supported output device from one address string or an inventory line, so a
//! bench works the moment it is plugged in.
//!
//! | Address | Driver | Build feature |
//! |---|---|---|
//! | `twin:<catalog-id>` | [`super::spectwin::SpecTwin`] (no hardware) | always |
//! | `file:<base>` | SigMF file (16-bit) | always |
//! | `redpitaya:<host>[:<port>]` | Red Pitaya SCPI + streaming | `redpitaya` |
//! | `uhd:<args>` | Ettus UHD 4.x | `uhd` (Unix) |
//! | `soapy:<args>` | SoapySDR 0.8 | `soapy` (Unix) |
//! | `digilent:<index>` | Digilent WaveForms (Analog Discovery) | `digilent` (Unix) |
//! | `scpi-awg:<host>[:<port>][@<profile.toml or builtin name>]` | SCPI arbitrary waveform generator | always |
//!
//! A driver missing from the build says which feature to rebuild with. Every hardware
//! driver prints the conducted-output policy when it opens.

use super::catalog::{Catalog, HwSpec};
use super::check::InventoryItem;
use super::spectwin::SpecTwin;
use crate::hw::Device;
use std::path::Path;

/// Address string of an inventory line (what [`open_address`] takes).
pub fn address_of(item: &InventoryItem, spec: Option<&HwSpec>) -> Result<String, String> {
    let drv = spec
        .and_then(|s| s.kshana_driver.clone())
        .unwrap_or_else(|| "none".into());
    let host = || {
        item.host.clone().ok_or_else(|| {
            format!(
                "{}: driver {drv} needs `host` in the inventory",
                item.label()
            )
        })
    };
    let with_port = |h: String| match item.port.or(spec.and_then(|s| s.scpi_port)) {
        Some(p) if !h.contains(':') => format!("{h}:{p}"),
        _ => h,
    };
    Ok(match drv.as_str() {
        "redpitaya" => format!("redpitaya:{}", with_port(host()?)),
        "uhd" => format!("uhd:{}", item.args.clone().unwrap_or_default()),
        "soapy" => format!("soapy:{}", item.args.clone().unwrap_or_default()),
        "digilent" => format!("digilent:{}", item.args.clone().unwrap_or_else(|| "0".into())),
        "scpi_awg" => {
            // Use the vendor profile when one is built in for this catalogue entry.
            let prof = spec
                .map(|s| s.id.clone())
                .filter(|id| crate::hw::scpi_awg::ScpiProfile::builtin(id).is_ok());
            let port = item.port.or_else(|| {
                prof.as_deref()
                    .and_then(|id| crate::hw::scpi_awg::ScpiProfile::builtin(id).ok())
                    .map(|p| p.port)
            });
            let h = host()?;
            let addr = match port {
                Some(p) if !h.contains(':') => format!("{h}:{p}"),
                _ => with_port(h),
            };
            match prof {
                Some(id) => format!("scpi-awg:{addr}@{id}"),
                None => format!("scpi-awg:{addr}"),
            }
        }
        other => {
            return Err(format!(
                "{}: no Kshana streaming driver for this model ({other}); use `twin:{}` to rehearse, or `file:` export and the vendor's playback",
                item.label(),
                item.catalog
            ))
        }
    })
}

/// Full scale (V peak) of a catalogue entry's output, when published.
#[cfg_attr(not(feature = "redpitaya"), allow(dead_code))]
fn full_scale_vpeak(spec: Option<&HwSpec>) -> Option<f64> {
    spec.and_then(|s| s.tx.as_ref())
        .and_then(|t| t.max_vpp_into_50ohm)
        .map(|v| v / 2.0)
}

/// Open the device at `addr` (see the module table). `fs_hz` is used by twins; `cat` maps
/// twin ids and provides full-scale values.
pub fn open_address(addr: &str, cat: &Catalog, fs_hz: f64) -> Result<Box<dyn Device>, String> {
    open_address_spec(addr, cat, fs_hz, None)
}

/// [`open_address`] with the catalogue entry of the device (for its full scale).
pub fn open_address_spec(
    addr: &str,
    cat: &Catalog,
    fs_hz: f64,
    spec_hint: Option<&HwSpec>,
) -> Result<Box<dyn Device>, String> {
    let (kind, rest) = addr
        .split_once(':')
        .ok_or_else(|| format!("device address {addr:?}: expected <kind>:<target>"))?;
    #[allow(unused_variables)]
    let missing = |f: &str| -> String {
        format!("this build has no {kind} driver: rebuild kshana-pro with --features eloran-{f}")
    };
    let _ = spec_hint;
    match kind {
        "twin" => {
            let s = cat.get(rest).ok_or_else(|| {
                let sug = cat.suggest(rest);
                format!(
                    "twin:{rest}: not in the catalogue{}",
                    if sug.is_empty() { String::new() } else { format!(" (did you mean: {})", sug.join(", ")) }
                )
            })?;
            Ok(Box::new(SpecTwin::from_spec(s, fs_hz)?))
        }
        "file" => Ok(Box::new(crate::hw::file::FileDevice::tx(
            Path::new(rest),
            crate::io::Format::Ri16,
        ))),
        "redpitaya" => {
            #[cfg(feature = "redpitaya")]
            {
                use crate::hw::redpitaya::{RedPitaya, RedPitayaSettings};
                let mut st = RedPitayaSettings::default();
                let host = match rest.rsplit_once(':') {
                    Some((h, p)) if p.parse::<u16>().is_ok() => {
                        st.scpi_port = p.parse().unwrap_or(5000);
                        h.to_string()
                    }
                    _ => rest.to_string(),
                };
                let s = spec_hint.or_else(|| cat.get("redpitaya-stemlab-125-14"));
                if let Some(v) = full_scale_vpeak(s) {
                    st.out_full_scale_v = v;
                }
                Ok(Box::new(RedPitaya::connect(&host, st)?))
            }
            #[cfg(not(feature = "redpitaya"))]
            {
                Err(missing("redpitaya"))
            }
        }
        "uhd" => {
            #[cfg(all(feature = "uhd", unix))]
            {
                Ok(Box::new(crate::hw::uhd::UhdDevice::open(rest)?))
            }
            #[cfg(not(all(feature = "uhd", unix)))]
            {
                Err(missing("uhd"))
            }
        }
        "soapy" => {
            #[cfg(all(feature = "soapy", unix))]
            {
                Ok(Box::new(crate::hw::soapy::SoapyDevice::open(
                    rest,
                    crate::hw::soapy::SoapyFormat::Cf32,
                )?))
            }
            #[cfg(not(all(feature = "soapy", unix)))]
            {
                Err(missing("soapy"))
            }
        }
        "digilent" => {
            #[cfg(all(feature = "digilent", unix))]
            {
                let idx = rest
                    .parse::<i32>()
                    .map_err(|_| format!("digilent:{rest}: expected a device index"))?;
                Ok(Box::new(crate::hw::digilent::DigilentDevice::open(idx)?))
            }
            #[cfg(not(all(feature = "digilent", unix)))]
            {
                Err(missing("digilent"))
            }
        }
        "scpi-awg" => {
            use crate::hw::scpi_awg::{ScpiAwg, ScpiAwgSettings, ScpiProfile};
            let (host, prof) = match rest.split_once('@') {
                Some((h, p)) => (h, p),
                None => (rest, "generic-binary"),
            };
            let profile = if prof.ends_with(".toml") {
                ScpiProfile::load(Path::new(prof))?
            } else {
                ScpiProfile::builtin(prof)?
            };
            Ok(Box::new(ScpiAwg::connect(
                host,
                profile,
                ScpiAwgSettings::default(),
            )?))
        }
        _ => Err(format!(
            "device address {addr:?}: kind must be twin, file, redpitaya, uhd, soapy, digilent or scpi-awg"
        )),
    }
}

/// Open an inventory line: its driver, or its spec twin when `rehearse` is set.
pub fn open_item(
    item: &InventoryItem,
    cat: &Catalog,
    fs_hz: f64,
    rehearse: bool,
) -> Result<(String, Box<dyn Device>), String> {
    let addr = if rehearse {
        format!("twin:{}", item.catalog)
    } else {
        address_of(item, cat.get(&item.catalog))?
    };
    let d = open_address_spec(&addr, cat, fs_hz, cat.get(&item.catalog))?;
    Ok((addr, d))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bench::check::BenchRole;

    fn cat() -> Catalog {
        let mut c = Catalog::default();
        c.merge_json(
            r#"[{"id":"lf","vendor":"V","model":"LF","category":"sdr","kshana_driver":"redpitaya","scpi_port":5000,
                 "tx":{"channels":2,"dac_bits":14,"max_sample_rate_hz":125e6,"freq_min_hz":0,"freq_max_hz":5e7,"coupling":"dc","max_vpp_into_50ohm":2},
                 "sources":[{"url":"https://e.org","title":"t"}]},
                {"id":"rf","vendor":"V","model":"RF","category":"sdr","kshana_driver":"uhd",
                 "tx":{"freq_min_hz":7e7,"freq_max_hz":6e9,"coupling":"ac"},
                 "sources":[{"url":"https://e.org","title":"t"}]}]"#,
        )
        .unwrap();
        c
    }

    fn item(cat_id: &str, host: Option<&str>) -> InventoryItem {
        InventoryItem {
            catalog: cat_id.into(),
            role: BenchRole::EloranSource,
            name: None,
            host: host.map(Into::into),
            port: None,
            serial: None,
            args: Some("type=b200".into()),
        }
    }

    #[test]
    fn addresses_follow_the_catalogue_driver() {
        let c = cat();
        assert_eq!(
            address_of(&item("lf", Some("10.0.0.5")), c.get("lf")).unwrap(),
            "redpitaya:10.0.0.5:5000"
        );
        assert_eq!(
            address_of(&item("rf", None), c.get("rf")).unwrap(),
            "uhd:type=b200"
        );
        assert!(address_of(&item("lf", None), c.get("lf"))
            .unwrap_err()
            .contains("needs `host`"));
    }

    #[test]
    fn twins_open_and_rf_only_twins_are_refused() {
        let c = cat();
        let (addr, d) = open_item(&item("lf", None), &c, 2e6, true).unwrap();
        assert_eq!(addr, "twin:lf");
        assert!(d.name().contains("spec twin"));
        let e = open_address("twin:rf", &c, 2e6).err().unwrap();
        assert!(e.contains("cannot output 90–110 kHz"), "{e}");
        let e = open_address("twin:lff", &c, 2e6).err().unwrap();
        assert!(e.contains("did you mean: lf"), "{e}");
        assert!(open_address("nope:1", &c, 2e6).is_err());
    }
}
