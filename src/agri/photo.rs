// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! Consistency checks of geotagged photos submitted as evidence of field work.
//!
//! Each JPEG's EXIF block is read (TIFF IFD0, the Exif IFD and the GPS IFD; both byte orders)
//! for: GPS latitude, longitude, altitude, GPS date and time (UTC), DOP and horizontal
//! positioning error, processing method; camera make, model, software; `DateTimeOriginal` and
//! its `OffsetTimeOriginal`.
//!
//! Checks per photo (each `pass`, `flag` or `not_assessed`):
//!
//! * `gps_present`: the photo carries a GPS position;
//! * `inside_parcel`: the position lies in the declared parcel (distance outside reported);
//! * `gps_vs_camera_time`: GPS UTC against camera time: with an offset tag, within
//!   [`PhotoParams::time_tol_s`]; without one, the difference must be a whole number of
//!   quarter hours (a time zone) within that tolerance;
//! * `claimed_window`: the GPS date lies in the claimed work window;
//! * `stated_accuracy`: a stated horizontal error within [`PhotoParams::max_hpe_m`];
//! * `coordinate_precision`: coordinates not rounded to four decimals or fewer (about 10 m);
//! * `software_tag`: a software tag that names neither the camera make nor a firmware
//!   version is reported for review (information, not a finding).
//!
//! Across the set: the same coordinates on photos taken at different times, and travel
//! between consecutive photos faster than [`PhotoParams::max_speed_mps`].
//!
//! **What this is not.** EXIF can be written by anyone with free tools; a photo that passes
//! every check is not shown to be genuine, and a flag is a reason to look, not a finding of
//! fraud. Image content is not analysed.

use std::path::Path;

use serde::Serialize;

use super::geo::{distance_to_boundary, inside, LocalFrame};

/// EXIF fields read from a photo.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Exif {
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub alt_m: Option<f64>,
    /// GPS UTC (unix seconds) from GPSDateStamp + GPSTimeStamp.
    pub gps_utc: Option<i64>,
    pub dop: Option<f64>,
    pub hpe_m: Option<f64>,
    pub processing: Option<String>,
    pub make: Option<String>,
    pub model: Option<String>,
    pub software: Option<String>,
    /// DateTimeOriginal read as if UTC (unix seconds), and the offset tag text.
    pub camera_time: Option<i64>,
    pub offset: Option<String>,
}

struct Tiff<'a> {
    b: &'a [u8],
    le: bool,
}

impl Tiff<'_> {
    fn u16(&self, o: usize) -> Option<u16> {
        let s = self.b.get(o..o + 2)?;
        Some(if self.le {
            u16::from_le_bytes([s[0], s[1]])
        } else {
            u16::from_be_bytes([s[0], s[1]])
        })
    }
    fn u32(&self, o: usize) -> Option<u32> {
        let s = self.b.get(o..o + 4)?;
        let a = [s[0], s[1], s[2], s[3]];
        Some(if self.le {
            u32::from_le_bytes(a)
        } else {
            u32::from_be_bytes(a)
        })
    }
    /// Entries of the IFD at `o`: (tag, type, count, value-or-offset position).
    fn ifd(&self, o: usize) -> Vec<(u16, u16, u32, usize)> {
        let Some(n) = self.u16(o) else { return vec![] };
        (0..n as usize)
            .filter_map(|i| {
                let e = o + 2 + 12 * i;
                let (tag, ty, cnt) = (self.u16(e)?, self.u16(e + 2)?, self.u32(e + 4)?);
                let size = match ty {
                    1 | 2 | 6 | 7 => 1,
                    3 | 8 => 2,
                    4 | 9 | 11 => 4,
                    5 | 10 | 12 => 8,
                    _ => return None,
                } * cnt as usize;
                let pos = if size <= 4 {
                    e + 8
                } else {
                    self.u32(e + 8)? as usize
                };
                Some((tag, ty, cnt, pos))
            })
            .collect()
    }
    fn ascii(&self, pos: usize, cnt: u32) -> Option<String> {
        let s = self.b.get(pos..pos + cnt as usize)?;
        let s = String::from_utf8_lossy(s);
        Some(s.trim_end_matches('\0').trim().to_string())
    }
    fn rationals(&self, pos: usize, cnt: u32) -> Option<Vec<f64>> {
        (0..cnt as usize)
            .map(|i| {
                let (n, d) = (self.u32(pos + 8 * i)?, self.u32(pos + 8 * i + 4)?);
                (d != 0).then(|| n as f64 / d as f64)
            })
            .collect()
    }
}

/// Read EXIF from JPEG bytes. `None` when the file has no EXIF block.
pub fn read_exif(jpeg: &[u8]) -> Option<Exif> {
    if jpeg.get(0..2)? != [0xFF, 0xD8] {
        return None;
    }
    let mut i = 2;
    let tiff = loop {
        if jpeg.get(i)? != &0xFF {
            return None;
        }
        let marker = *jpeg.get(i + 1)?;
        let len = u16::from_be_bytes([*jpeg.get(i + 2)?, *jpeg.get(i + 3)?]) as usize;
        if marker == 0xE1 && jpeg.get(i + 4..i + 10)? == b"Exif\0\0" {
            break jpeg.get(i + 10..i + 2 + len)?;
        }
        if marker == 0xDA || marker == 0xD9 {
            return None;
        }
        i += 2 + len;
    };
    let le = match tiff.get(0..2)? {
        b"II" => true,
        b"MM" => false,
        _ => return None,
    };
    let t = Tiff { b: tiff, le };
    let ifd0 = t.u32(4)? as usize;
    let mut x = Exif::default();
    let mut exif_ifd = None;
    let mut gps_ifd = None;
    for (tag, _, cnt, pos) in t.ifd(ifd0) {
        match tag {
            0x010F => x.make = t.ascii(pos, cnt),
            0x0110 => x.model = t.ascii(pos, cnt),
            0x0131 => x.software = t.ascii(pos, cnt),
            0x8769 => exif_ifd = t.u32(pos).map(|v| v as usize),
            0x8825 => gps_ifd = t.u32(pos).map(|v| v as usize),
            _ => {}
        }
    }
    if let Some(o) = exif_ifd {
        for (tag, _, cnt, pos) in t.ifd(o) {
            match tag {
                0x9003 => {
                    x.camera_time = t.ascii(pos, cnt).and_then(|s| {
                        let s = s.replacen(':', "-", 2);
                        super::parse_utc(&s).ok()
                    })
                }
                0x9011 => x.offset = t.ascii(pos, cnt),
                _ => {}
            }
        }
    }
    if let Some(o) = gps_ifd {
        let (mut lr, mut la, mut or, mut lo) = (None, None, None, None);
        let (mut ar, mut al, mut ts, mut ds) = (0u8, None, None, None);
        for (tag, _, cnt, pos) in t.ifd(o) {
            match tag {
                1 => lr = t.ascii(pos, cnt),
                2 => la = t.rationals(pos, cnt),
                3 => or = t.ascii(pos, cnt),
                4 => lo = t.rationals(pos, cnt),
                5 => ar = *tiff.get(pos).unwrap_or(&0),
                6 => al = t.rationals(pos, 1).and_then(|v| v.first().copied()),
                7 => ts = t.rationals(pos, cnt),
                11 => x.dop = t.rationals(pos, 1).and_then(|v| v.first().copied()),
                27 => {
                    x.processing = t.ascii(pos, cnt).map(|s| {
                        s.trim_start_matches("ASCII\0\0\0")
                            .trim_matches('\0')
                            .to_string()
                    })
                }
                29 => ds = t.ascii(pos, cnt),
                31 => x.hpe_m = t.rationals(pos, 1).and_then(|v| v.first().copied()),
                _ => {}
            }
        }
        let dms = |v: &Vec<f64>| {
            v.first().copied().unwrap_or(0.0)
                + v.get(1).copied().unwrap_or(0.0) / 60.0
                + v.get(2).copied().unwrap_or(0.0) / 3600.0
        };
        if let (Some(v), Some(r)) = (&la, &lr) {
            x.lat = Some(if r == "S" { -dms(v) } else { dms(v) });
        }
        if let (Some(v), Some(r)) = (&lo, &or) {
            x.lon = Some(if r == "W" { -dms(v) } else { dms(v) });
        }
        x.alt_m = al.map(|a| if ar == 1 { -a } else { a });
        if let (Some(d), Some(tv)) = (ds, ts) {
            if let Ok(day) = super::parse_utc(&d.replacen(':', "-", 2)) {
                let sec = tv.first().copied().unwrap_or(0.0) * 3600.0
                    + tv.get(1).copied().unwrap_or(0.0) * 60.0
                    + tv.get(2).copied().unwrap_or(0.0);
                x.gps_utc = Some(day + sec.round() as i64);
            }
        }
    }
    Some(x)
}

/// A synthetic JPEG (SOI, an APP1 EXIF block, EOI; no image data) carrying `x`. Big-endian
/// TIFF. For examples and tests.
pub fn synth_jpeg(x: &Exif) -> Vec<u8> {
    // Build IFDs in a TIFF buffer: header, IFD0, Exif IFD, GPS IFD, then data.
    struct E {
        tag: u16,
        ty: u16,
        cnt: u32,
        data: Vec<u8>,
    }
    let ascii = |tag, s: &str| {
        let mut d = s.as_bytes().to_vec();
        d.push(0);
        E {
            tag,
            ty: 2,
            cnt: d.len() as u32,
            data: d,
        }
    };
    let rat = |tag, v: &[f64]| {
        let mut d = Vec::new();
        for x in v {
            let den = 1_000_000u32;
            d.extend_from_slice(&((x * den as f64).round() as u32).to_be_bytes());
            d.extend_from_slice(&den.to_be_bytes());
        }
        E {
            tag,
            ty: 5,
            cnt: v.len() as u32,
            data: d,
        }
    };
    let dms = |v: f64| {
        let a = v.abs();
        let d = a.floor();
        let m = ((a - d) * 60.0).floor();
        [d, m, (a - d - m / 60.0) * 3600.0]
    };
    let mut ifd0 = Vec::new();
    if let Some(s) = &x.make {
        ifd0.push(ascii(0x010F, s));
    }
    if let Some(s) = &x.model {
        ifd0.push(ascii(0x0110, s));
    }
    if let Some(s) = &x.software {
        ifd0.push(ascii(0x0131, s));
    }
    let mut exif = Vec::new();
    if let Some(t) = x.camera_time {
        let s = super::utc_text(t);
        exif.push(ascii(
            0x9003,
            &format!("{}:{}:{} {}", &s[0..4], &s[5..7], &s[8..10], &s[11..19]),
        ));
    }
    if let Some(o) = &x.offset {
        exif.push(ascii(0x9011, o));
    }
    let mut gps = Vec::new();
    if let (Some(la), Some(lo)) = (x.lat, x.lon) {
        gps.push(ascii(1, if la < 0.0 { "S" } else { "N" }));
        gps.push(rat(2, &dms(la)));
        gps.push(ascii(3, if lo < 0.0 { "W" } else { "E" }));
        gps.push(rat(4, &dms(lo)));
    }
    if let Some(a) = x.alt_m {
        gps.push(E {
            tag: 5,
            ty: 1,
            cnt: 1,
            data: vec![u8::from(a < 0.0)],
        });
        gps.push(rat(6, &[a.abs()]));
    }
    if let Some(t) = x.gps_utc {
        let s = t.rem_euclid(86_400) as f64;
        gps.push(rat(
            7,
            &[
                (s / 3600.0).floor(),
                ((s % 3600.0) / 60.0).floor(),
                s % 60.0,
            ],
        ));
        let u = super::utc_text(t);
        gps.push(ascii(
            29,
            &format!("{}:{}:{}", &u[0..4], &u[5..7], &u[8..10]),
        ));
    }
    if let Some(d) = x.dop {
        gps.push(rat(11, &[d]));
    }
    if let Some(h) = x.hpe_m {
        gps.push(rat(31, &[h]));
    }
    // Layout.
    let has_exif = !exif.is_empty();
    let has_gps = !gps.is_empty();
    let n0 = ifd0.len() + usize::from(has_exif) + usize::from(has_gps);
    let size = |n: usize| 2 + 12 * n + 4;
    let off0 = 8usize;
    let off_exif = off0 + size(n0);
    let off_gps = off_exif + if has_exif { size(exif.len()) } else { 0 };
    let mut data_off = off_gps + if has_gps { size(gps.len()) } else { 0 };
    let mut tiff = b"MM\0\x2a".to_vec();
    tiff.extend_from_slice(&(off0 as u32).to_be_bytes());
    let mut tail = Vec::new();
    let mut write_ifd = |tiff: &mut Vec<u8>, mut es: Vec<E>, extra: Vec<(u16, u32)>| {
        for (tag, off) in extra {
            es.push(E {
                tag,
                ty: 4,
                cnt: 1,
                data: off.to_be_bytes().to_vec(),
            });
        }
        es.sort_by_key(|e| e.tag);
        tiff.extend_from_slice(&(es.len() as u16).to_be_bytes());
        for e in es {
            tiff.extend_from_slice(&e.tag.to_be_bytes());
            tiff.extend_from_slice(&e.ty.to_be_bytes());
            tiff.extend_from_slice(&e.cnt.to_be_bytes());
            if e.data.len() <= 4 {
                let mut v = e.data.clone();
                v.resize(4, 0);
                tiff.extend_from_slice(&v);
            } else {
                tiff.extend_from_slice(&(data_off as u32).to_be_bytes());
                data_off += e.data.len();
                tail.extend_from_slice(&e.data);
            }
        }
        tiff.extend_from_slice(&0u32.to_be_bytes());
    };
    let mut extra = Vec::new();
    if has_exif {
        extra.push((0x8769, off_exif as u32));
    }
    if has_gps {
        extra.push((0x8825, off_gps as u32));
    }
    write_ifd(&mut tiff, ifd0, extra);
    if has_exif {
        write_ifd(&mut tiff, exif, vec![]);
    }
    if has_gps {
        write_ifd(&mut tiff, gps, vec![]);
    }
    tiff.extend_from_slice(&tail);
    let mut j = vec![0xFF, 0xD8, 0xFF, 0xE1];
    j.extend_from_slice(&((tiff.len() + 8) as u16).to_be_bytes());
    j.extend_from_slice(b"Exif\0\0");
    j.extend_from_slice(&tiff);
    j.extend_from_slice(&[0xFF, 0xD9]);
    j
}

/// Check parameters.
#[derive(Clone, Debug, Serialize)]
pub struct PhotoParams {
    pub time_tol_s: f64,
    pub max_hpe_m: f64,
    pub max_speed_mps: f64,
    /// Claimed work window (unix seconds, inclusive), when given.
    pub window: Option<(i64, i64)>,
}

impl Default for PhotoParams {
    fn default() -> Self {
        PhotoParams {
            time_tol_s: 120.0,
            max_hpe_m: 30.0,
            max_speed_mps: 50.0,
            window: None,
        }
    }
}

/// One check result.
#[derive(Clone, Debug, Serialize)]
pub struct Check {
    pub id: &'static str,
    /// `pass`, `flag`, `info` or `not_assessed`.
    pub result: &'static str,
    pub detail: String,
}

/// One photo's checks.
#[derive(Clone, Debug, Serialize)]
pub struct PhotoResult {
    pub file: String,
    pub sha256: String,
    pub exif: Option<Exif>,
    pub checks: Vec<Check>,
    pub flags: usize,
}

/// The report.
#[derive(Clone, Debug, Serialize)]
pub struct PhotoReport {
    pub label: &'static str,
    pub params: PhotoParams,
    pub photos: Vec<PhotoResult>,
    pub set_checks: Vec<Check>,
    pub flagged_photos: usize,
    pub caveat: &'static str,
}

const CAVEAT: &str = "EXIF can be written by anyone; passing every check does not show a photo is genuine, and a flag is a reason to look, not a finding.";

fn chk(id: &'static str, ok: Option<bool>, detail: String) -> Check {
    Check {
        id,
        result: match ok {
            Some(true) => "pass",
            Some(false) => "flag",
            None => "not_assessed",
        },
        detail,
    }
}

fn offset_s(o: &str) -> Option<i64> {
    let sign = if o.starts_with('-') { -1 } else { 1 };
    let (h, m) = o.trim_start_matches(['+', '-']).split_once(':')?;
    Some(sign * (h.parse::<i64>().ok()? * 3600 + m.parse::<i64>().ok()? * 60))
}

fn decimals(v: f64) -> usize {
    for d in 0..=8 {
        let f = 10f64.powi(d as i32);
        if ((v * f).round() - v * f).abs() < 1e-6 {
            return d;
        }
    }
    9
}

/// Check photos (file name, bytes) against a parcel boundary (`[lat, lon]` points).
pub fn check(photos: &[(String, Vec<u8>)], parcel: &[[f64; 2]], p: &PhotoParams) -> PhotoReport {
    use sha2::{Digest, Sha256};
    let f = LocalFrame::new(parcel[0][0], parcel[0][1], 0.0);
    let poly: Vec<[f64; 2]> = parcel.iter().map(|q| f.en(q[0], q[1])).collect();
    let mut out = Vec::new();
    for (name, bytes) in photos {
        let exif = read_exif(bytes);
        let mut c = Vec::new();
        let x = exif.clone().unwrap_or_default();
        let pos = x.lat.zip(x.lon);
        c.push(chk(
            "gps_present",
            Some(pos.is_some()),
            match pos {
                Some((a, b)) => format!("{a:.7}, {b:.7}"),
                None if exif.is_none() => "no EXIF block".into(),
                None => "no GPS position in EXIF".into(),
            },
        ));
        c.push(match pos {
            Some((a, b)) => {
                let q = f.en(a, b);
                let ins = inside(q, &poly);
                chk(
                    "inside_parcel",
                    Some(ins),
                    if ins {
                        "inside the declared parcel".into()
                    } else {
                        format!(
                            "{:.0} m outside the declared parcel",
                            distance_to_boundary(q, &poly)
                        )
                    },
                )
            }
            None => chk("inside_parcel", None, "no position".into()),
        });
        c.push(match (x.gps_utc, x.camera_time) {
            (Some(g), Some(cam)) => match x.offset.as_deref().and_then(offset_s) {
                Some(off) => {
                    let d = (cam - off - g) as f64;
                    chk(
                        "gps_vs_camera_time",
                        Some(d.abs() <= p.time_tol_s),
                        format!("camera minus GPS {d:.0} s with offset {}", x.offset.clone().unwrap_or_default()),
                    )
                }
                None => {
                    let d = (cam - g) as f64;
                    let q = (d / 900.0).round() * 900.0;
                    chk(
                        "gps_vs_camera_time",
                        Some((d - q).abs() <= p.time_tol_s && q.abs() <= 14.0 * 3600.0),
                        format!("camera minus GPS {d:.0} s (no offset tag; nearest time zone {:+.2} h)", q / 3600.0),
                    )
                }
            },
            _ => chk("gps_vs_camera_time", None, "GPS time or camera time missing".into()),
        });
        c.push(match (p.window, x.gps_utc.or(x.camera_time)) {
            (Some((a, b)), Some(t)) => chk(
                "claimed_window",
                Some(t >= a && t <= b),
                format!("taken {}", super::utc_text(t)),
            ),
            (Some(_), None) => chk("claimed_window", None, "no time in EXIF".into()),
            (None, _) => chk("claimed_window", None, "no window given".into()),
        });
        c.push(match x.hpe_m {
            Some(h) => chk(
                "stated_accuracy",
                Some(h <= p.max_hpe_m),
                format!("stated horizontal error {h:.1} m"),
            ),
            None => chk("stated_accuracy", None, "no stated horizontal error".into()),
        });
        c.push(match pos {
            Some((a, b)) => {
                let d = decimals(a).min(decimals(b));
                chk(
                    "coordinate_precision",
                    Some(d > 4),
                    format!("{d} decimal places"),
                )
            }
            None => chk("coordinate_precision", None, "no position".into()),
        });
        if let Some(sw) = &x.software {
            let make = x.make.clone().unwrap_or_default().to_ascii_lowercase();
            let looks_firmware = sw.chars().next().is_some_and(|ch| ch.is_ascii_digit())
                || (!make.is_empty() && sw.to_ascii_lowercase().contains(&make));
            if !looks_firmware {
                c.push(Check {
                    id: "software_tag",
                    result: "info",
                    detail: format!("software tag '{sw}': review whether the file was edited"),
                });
            }
        }
        let flags = c.iter().filter(|k| k.result == "flag").count();
        out.push(PhotoResult {
            file: name.clone(),
            sha256: hex::encode(Sha256::digest(bytes)),
            exif,
            checks: c,
            flags,
        });
    }
    // Set checks.
    let mut set = Vec::new();
    let mut timed: Vec<(i64, f64, f64, &str)> = out
        .iter()
        .filter_map(|r| {
            let e = r.exif.as_ref()?;
            Some((
                e.gps_utc.or(e.camera_time)?,
                e.lat?,
                e.lon?,
                r.file.as_str(),
            ))
        })
        .collect();
    timed.sort_by_key(|t| t.0);
    let mut dups = Vec::new();
    for (i, a) in timed.iter().enumerate() {
        for b in &timed[i + 1..] {
            if (a.1 - b.1).abs() < 1e-7 && (a.2 - b.2).abs() < 1e-7 && (b.0 - a.0).abs() > 60 {
                dups.push(format!("{} and {}", a.3, b.3));
            }
        }
    }
    set.push(chk(
        "repeated_coordinates",
        (timed.len() >= 2).then_some(dups.is_empty()),
        if dups.is_empty() {
            "no identical coordinates on photos taken more than a minute apart".into()
        } else {
            format!(
                "identical coordinates at different times: {}",
                dups.join("; ")
            )
        },
    ));
    let mut fast = Vec::new();
    for w in timed.windows(2) {
        let d = kshana::geodesic::distance_m(w[0].1, w[0].2, w[1].1, w[1].2);
        let dt = (w[1].0 - w[0].0).max(1) as f64;
        if d / dt > p.max_speed_mps {
            fast.push(format!(
                "{} -> {}: {:.0} m in {:.0} s",
                w[0].3, w[1].3, d, dt
            ));
        }
    }
    set.push(chk(
        "travel_speed",
        (timed.len() >= 2).then_some(fast.is_empty()),
        if fast.is_empty() {
            format!(
                "no travel faster than {} m/s between consecutive photos",
                p.max_speed_mps
            )
        } else {
            fast.join("; ")
        },
    ));
    let flagged = out.iter().filter(|r| r.flags > 0).count();
    PhotoReport {
        label: super::LABEL,
        params: p.clone(),
        photos: out,
        set_checks: set,
        flagged_photos: flagged,
        caveat: CAVEAT,
    }
}

/// Read every `.jpg`/`.jpeg` in the given files or directories (sorted by name).
pub fn collect(paths: &[std::path::PathBuf]) -> Result<Vec<(String, Vec<u8>)>, String> {
    let mut files = Vec::new();
    for p in paths {
        if p.is_dir() {
            for e in std::fs::read_dir(p).map_err(|e| format!("{}: {e}", p.display()))? {
                let q = e.map_err(|e| e.to_string())?.path();
                if q.extension()
                    .and_then(|x| x.to_str())
                    .is_some_and(|x| matches!(x.to_ascii_lowercase().as_str(), "jpg" | "jpeg"))
                {
                    files.push(q);
                }
            }
        } else {
            files.push(p.clone());
        }
    }
    files.sort();
    files
        .iter()
        .map(|q: &std::path::PathBuf| {
            let b = std::fs::read(q).map_err(|e| format!("{}: {e}", q.display()))?;
            let n = Path::new(q)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            Ok((n, b))
        })
        .collect()
}

/// A Markdown summary.
pub fn markdown(r: &PhotoReport) -> String {
    let mut m = format!(
        "# Geotagged photo checks ({})\n\n{} photos, {} with at least one flag.\n\n> {}\n\n| Photo | Flags | Checks |\n|---|---|---|\n",
        r.label,
        r.photos.len(),
        r.flagged_photos,
        r.caveat
    );
    for p in &r.photos {
        let s: Vec<String> = p
            .checks
            .iter()
            .filter(|c| c.result != "pass")
            .map(|c| format!("{} {}: {}", c.id, c.result, c.detail))
            .collect();
        m.push_str(&format!(
            "| {} | {} | {} |\n",
            p.file,
            p.flags,
            s.join("; ")
        ));
    }
    m.push_str("\nSet checks:\n\n");
    for c in &r.set_checks {
        m.push_str(&format!("- {} {}: {}\n", c.id, c.result, c.detail));
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exif_round_trip() {
        let x = Exif {
            lat: Some(58.3781234),
            lon: Some(26.7290123),
            alt_m: Some(-12.5),
            gps_utc: Some(1_715_000_000),
            dop: Some(1.2),
            hpe_m: Some(4.0),
            processing: None,
            make: Some("Cam".into()),
            model: Some("X1".into()),
            software: Some("1.0.3".into()),
            camera_time: Some(1_715_000_000 + 3 * 3600),
            offset: Some("+03:00".into()),
        };
        let j = synth_jpeg(&x);
        let r = read_exif(&j).unwrap();
        assert!((r.lat.unwrap() - 58.3781234).abs() < 1e-7);
        assert!((r.lon.unwrap() - 26.7290123).abs() < 1e-7);
        assert!((r.alt_m.unwrap() + 12.5).abs() < 1e-6);
        assert_eq!(r.gps_utc, x.gps_utc);
        assert_eq!(r.camera_time, x.camera_time);
        assert_eq!(r.offset, x.offset);
        assert_eq!(r.make, x.make);
        assert_eq!(r.hpe_m, Some(4.0));
        assert!(read_exif(b"not a jpeg").is_none());
    }
}
