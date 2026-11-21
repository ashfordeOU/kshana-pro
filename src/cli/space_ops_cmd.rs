// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro space-ops <ca|identify|disposal|pack|synth>` (research 2026-11-20 gaps, B2205;
//! docs/SPACE-OPS-EVIDENCE.md).

use std::path::Path;
use std::process::ExitCode;

use crate::space_ops as so;
use crate::timing::grid::Section;
use crate::transport::{page, write_pack};

use super::grid_cmd::{gate, signing};

pub const SPACE_OPS_USAGE: &str = "usage: kshana-pro space-ops <command>   (each signed command takes --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev])\n\
    ca <request.toml>                         Collision-avoidance decision record: CDMs, recomputed Pc, decision, timeline, the operator's own policy; signed (Pro)\n\
    identify <ephemeris.csv> <candidates.toml>   Identification evidence: the operator's ephemeris against candidate TLEs on the open SGP4; signed (Pro)\n\
    disposal <request.toml>                   Disposal-plan check: drag lifetime against the plan's own deadline; signed (Pro)\n\
    pack <pack.toml>                          Operator evidence pack: the three records with an index, signed together (Pro)\n\
    synth <dir>                               SYNTHETIC inputs for every command (no licence needed)";

fn f(x: f64) -> String {
    format!("{x:.3e}")
}

fn ca_section(r: &so::CaRecord) -> Vec<Section<'static>> {
    vec![
        (
            format!("{}: conjunction data messages", r.title),
            vec![
                "message",
                "created",
                "TCA",
                "miss (m)",
                "Pc recomputed",
                "Pc reported",
                "before decision",
            ],
            r.cdms
                .iter()
                .map(|c| {
                    vec![
                        c.message_id.clone(),
                        c.creation_utc.clone(),
                        c.tca_utc.clone(),
                        format!("{:.1}", c.miss_m),
                        f(c.pc_computed),
                        c.pc_reported.map(f).unwrap_or_else(|| "-".into()),
                        if c.before_decision { "yes" } else { "no" }.into(),
                    ]
                })
                .collect(),
        ),
        (
            format!("{}: decision against the operator's policy", r.title),
            vec!["item", "value"],
            vec![
                vec![
                    "decision".into(),
                    format!(
                        "{} at {} by {}",
                        r.decision.action, r.decision.utc, r.decision.decided_by
                    ),
                ],
                vec!["rationale".into(), r.decision.rationale.clone()],
                vec![
                    "policy".into(),
                    format!(
                        "Pc threshold {:.1e}, decide {} h before TCA ({})",
                        r.policy.pc_threshold, r.policy.decide_by_h, r.policy.source
                    ),
                ],
                vec![
                    "basis".into(),
                    format!(
                        "{} Pc {}",
                        r.basis_cdm.clone().unwrap_or("-".into()),
                        r.basis_pc.map(f).unwrap_or("-".into())
                    ),
                ],
                vec!["lead time".into(), format!("{:.1} h", r.lead_time_h)],
                vec![
                    "post-manoeuvre Pc".into(),
                    r.post_maneuver_pc.map(f).unwrap_or("-".into()),
                ],
                vec![
                    "points for review".into(),
                    if r.policy_findings.is_empty() {
                        "none".into()
                    } else {
                        r.policy_findings.join("; ")
                    },
                ],
            ],
        ),
    ]
}

fn id_section(r: &so::Identification) -> Section<'static> {
    (
        format!("Identification ({} ephemeris points)", r.points),
        vec!["candidate", "NORAD", "RMS (km)", "max (km)"],
        r.fits
            .iter()
            .map(|x| {
                vec![
                    x.name.clone(),
                    x.norad_id.clone(),
                    format!("{:.3}", x.rms_km),
                    format!("{:.3}", x.max_km),
                ]
            })
            .collect(),
    )
}

fn disposal_section(r: &so::Disposal) -> Section<'static> {
    let q = &r.request;
    (
        "Disposal-plan check".into(),
        vec!["item", "value"],
        vec![
            vec![
                "orbit".into(),
                format!(
                    "{} km circular, {} kg, {} m², Cd {}",
                    q.altitude_km, q.mass_kg, q.area_m2, q.cd
                ),
            ],
            vec![
                "estimated lifetime".into(),
                r.lifetime_years
                    .map(|y| format!("{y:.1} years"))
                    .unwrap_or("beyond 200 years".into()),
            ],
            vec![
                "plan deadline".into(),
                format!("{} years ({})", q.plan_deadline_years, q.plan_source),
            ],
            vec!["model".into(), r.model.into()],
        ],
    )
}

const LABEL: &str = "MODELLED: computed from the operator's own data with open orbit tools; not a regulatory determination";

fn finish<T: serde::Serialize>(
    args: &[String],
    cmd: &str,
    rep: &T,
    (title, verdict, intro): (&str, &str, &[String]),
    secs: &[Section],
    inputs: &[(String, String)],
) -> Result<String, String> {
    let s = signing(args, cmd)?;
    let out = Path::new(&s.out);
    let html = page(title, LABEL, verdict, intro, secs);
    let (first_name, first) = inputs.first().cloned().unwrap_or_default();
    write_pack(out, rep, &html, &first_name, &first)?;
    for (n, t) in inputs.iter().skip(1) {
        std::fs::write(out.join("inputs").join(n), t)
            .map_err(|e| format!("{}: {e}", out.display()))?;
    }
    crate::signed_dossier::sign(out, &s.seed, &s.signer, &s.utc).map_err(|e| e.to_string())?;
    Ok(format!(
        "[space-ops] {verdict}\n[space-ops] signed evidence in {}",
        s.out
    ))
}

fn read(p: &str) -> Result<String, String> {
    std::fs::read_to_string(p).map_err(|e| format!("{p}: {e}"))
}

/// `kshana-pro space-ops ...`.
pub fn run(args: &[String]) -> ExitCode {
    let pos: Vec<&String> = args
        .iter()
        .skip(1)
        .take_while(|a| !a.starts_with("--"))
        .collect();
    let r: Result<String, String> = match (args.first().map(String::as_str), pos.as_slice()) {
        (Some("synth"), [d]) => (|| {
            std::fs::create_dir_all(d.as_str()).map_err(|e| format!("{d}: {e}"))?;
            for (n, t) in so::synth()? {
                std::fs::write(Path::new(d.as_str()).join(n), t)
                    .map_err(|e| format!("{d}: {e}"))?;
            }
            Ok(format!(
                "[space-ops] SYNTHETIC inputs in {d} (pack.toml ties them together)"
            ))
        })(),
        (Some("ca"), [p]) => (|| {
            gate(args, so::FEATURE)?;
            let s = read(p)?;
            let q: so::CaRequest = crate::transport::parse_toml(&s, p)?;
            let r = so::ca_record(&q)?;
            finish(
                args,
                "space-ops ca",
                &r,
                (
                    "Collision-avoidance decision record",
                    &r.verdict,
                    &[r.method.to_string()],
                ),
                &ca_section(&r),
                &[("request.toml".into(), s)],
            )
        })(),
        (Some("identify"), [e, c]) => (|| {
            gate(args, so::FEATURE)?;
            let (es, cs) = (read(e)?, read(c)?);
            let cands: so::Candidates = crate::transport::parse_toml(&cs, c)?;
            let r = so::identify(&es, &cands.candidate)?;
            finish(
                args,
                "space-ops identify",
                &r,
                ("Identification evidence", &r.verdict, &[]),
                &[id_section(&r)],
                &[("ephemeris.csv".into(), es), ("candidates.toml".into(), cs)],
            )
        })(),
        (Some("disposal"), [p]) => (|| {
            gate(args, so::FEATURE)?;
            let s = read(p)?;
            let q: so::DisposalRequest = crate::transport::parse_toml(&s, p)?;
            let r = so::disposal(&q)?;
            finish(
                args,
                "space-ops disposal",
                &r,
                ("Disposal-plan check", &r.verdict, &[]),
                &[disposal_section(&r)],
                &[("request.toml".into(), s)],
            )
        })(),
        (Some("pack"), [p]) => (|| {
            gate(args, so::FEATURE)?;
            let s = read(p)?;
            let q: so::PackRequest = crate::transport::parse_toml(&s, p)?;
            let base = Path::new(p.as_str()).parent().unwrap_or(Path::new("."));
            let (r, mut inputs) = so::pack(&q, base)?;
            let mut secs = Vec::new();
            for c in &r.ca_records {
                secs.extend(ca_section(c));
            }
            if let Some(i) = &r.identification {
                secs.push(id_section(i));
            }
            if let Some(d) = &r.disposal {
                secs.push(disposal_section(d));
            }
            secs.push((
                "Open points".into(),
                vec!["point"],
                r.open_points.iter().map(|x| vec![x.clone()]).collect(),
            ));
            inputs.insert(0, ("pack.toml".into(), s));
            let intro = vec![format!("Operator: {}. {}", r.operator, r.framework_note)];
            finish(
                args,
                "space-ops pack",
                &r,
                (&r.title, &r.verdict, &intro),
                &secs,
                &inputs,
            )
        })(),
        _ => Err(SPACE_OPS_USAGE.into()),
    };
    match r {
        Ok(m) => {
            eprintln!("{m}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
