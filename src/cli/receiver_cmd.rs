// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! Receiver scorecard commands of the research backlog epic E13, hung under the existing
//! commands:
//!
//! * `kshana-pro scorecard flags <request.toml> ...` and `scorecard flags-synth <dir>` (B1301,
//!   gate `pro-receiver-scorecard`);
//! * `kshana-pro scorecard units <request.toml> ...` and `scorecard units-synth <dir>` (B1302,
//!   same gate);
//! * `kshana-pro exposure scheme <request.toml> ...` (B1303, gate `pro-insurer-scorecard`);
//! * `kshana-pro timing recovery <request.toml> ...` and `timing recovery-synth <dir>` (B1304,
//!   gate `pro-timing-scorecard`).
//!
//! Producing a signed result needs the licence feature of its gate; the `-synth` commands
//! (synthetic fixtures to try the commands) and verifying a signed directory
//! (`kshana-pro dossier verify`) need none.

use std::path::Path;
use std::process::ExitCode;

use super::evidence_cmd::{producing, sign};

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::FAILURE
}

fn file_arg(args: &[String]) -> Option<&String> {
    args.get(1).filter(|a| !a.starts_with("--"))
}

pub const FLAGS_USAGE: &str = "usage: kshana-pro scorecard flags <request.toml> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]\n\
        Spoof-, jamming- and integrity-flag efficacy per model and firmware from logs under test-kit scenarios: detection and false-alarm rates, CW vs broadband, preconditions, flag normal while wrong; signed (Pro)\n\
    kshana-pro scorecard flags-synth <dir>\n\
        Write synthetic flag logs and a request to try it (free)";

pub const UNITS_USAGE: &str = "usage: kshana-pro scorecard units <request.toml> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]\n\
        Low-cost receiver, board and handset scorecards per model and firmware, unit-to-unit variability and firmware regression alerts; signed (Pro)\n\
    kshana-pro scorecard units-synth <dir>\n\
        Write synthetic unit logs and a request (two firmware versions) to try it (free)";

pub const SCHEME_USAGE: &str = "usage: kshana-pro exposure scheme <request.toml> --out <dir> --signer <name> --key-file <file> [--trust <hex>[,...]] [--utc <iso>] [--license <token>] [--dev]\n\
        Discount-scheme device scorecard: each stated claim (detection, no misleading output, recovery, A-PNT holdover) checked against signed scorecards, with the publication rule; signed (Pro)";

pub const RECOVERY_USAGE: &str = "usage: kshana-pro timing recovery <request.toml> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]\n\
        Timing-module attack recovery from time-error logs: recovery time, residual offset, holdover exit step, multi-receiver comparison; signed (Pro, T4)\n\
    kshana-pro timing recovery-synth <dir>\n\
        Write three synthetic time-error logs and a request to try it (free)";

/// `scorecard flags`.
pub fn flags(args: &[String]) -> ExitCode {
    use crate::scorecards::flags as f;
    let Some(file) = file_arg(args) else {
        return fail(FLAGS_USAGE);
    };
    let r = producing(args, crate::scorecards::KIND).and_then(|(out, seed, signer, utc)| {
        let (req, src) = f::Request::from_file(Path::new(file))?;
        let runs = f::load_runs(&req)?;
        let s = f::score_loaded(&req, &src, &runs)?;
        f::write_dir(Path::new(&out), &s, &src, &runs)?;
        sign(&out, &seed, &signer, &utc)?;
        Ok((s, out))
    });
    match r {
        Ok((s, out)) => {
            eprintln!(
                "[flags] {} run(s), {} indicator row(s), {} finding(s); signed in {out}",
                s.runs.len(),
                s.table.len(),
                s.findings.len()
            );
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}

fn write_all(dir: &Path, files: &[(String, String)]) -> Result<(), String> {
    for (name, text) in files {
        let p = dir.join(name);
        if let Some(d) = p.parent() {
            std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
        }
        std::fs::write(&p, text).map_err(|e| format!("{}: {e}", p.display()))?;
    }
    Ok(())
}

/// The synthetic flag-efficacy campaign: three models, the receiver-flags pack.
pub fn flags_synth_files() -> Result<Vec<(String, String)>, String> {
    use crate::scorecards::flags::{load_scenario, synth, Unit};
    let mut files = vec![];
    let mut runs = String::new();
    let units = [
        ("Model-G", synth::Behaviour::Good),
        ("Model-S", synth::Behaviour::SilentSpoof),
        ("Model-M", synth::Behaviour::MultiOnly),
        ("Model-C", synth::Behaviour::CwConfused),
    ];
    let scen = [
        "flags-clean",
        "flags-jam-cw",
        "flags-jam-broadband",
        "flags-spoof-simplistic",
        "flags-spoof-drag",
    ];
    for (model, b) in units {
        let unit = Unit {
            model: model.into(),
            firmware: Some("1.0".into()),
            serial: None,
            class: Some("module".into()),
        };
        for sc in scen {
            let spec = load_scenario(&format!("receiver-flags/{sc}"), None)?;
            for (k, pre) in [
                (1, [("constellations", "multi"), ("start", "genuine-first")]),
                (
                    2,
                    [("constellations", "single"), ("start", "genuine-first")],
                ),
                (3, [("constellations", "multi"), ("start", "genuine-first")]),
            ] {
                let l = synth::log(&spec, &unit, b, &pre, 3.0);
                let name = format!("logs/{model}-{sc}-{k}.json");
                runs.push_str(&format!(
                    "\n[[run]]\nid = \"{model}-{sc}-{k}\"\nlog = \"{name}\"\n"
                ));
                files.push((name, l.to_json()));
            }
        }
    }
    let mut req = format!(
        "format = \"{}\"\ntitle = \"Synthetic flag-efficacy campaign (MODELLED fixtures)\"\n\n[bars]\nfixed_utc = \"2026-11-20T09:00:00Z\"\ndetect_within_s = 30\n",
        crate::scorecards::flags::REQUEST_FORMAT
    );
    for (model, _) in units {
        req.push_str(&format!(
            "\n[[indicator]]\nmodel = \"{model}\"\nname = \"spoof\"\nmeaning = \"spoofing\"\nraise_at = 2\ndocumented_in = \"Synthetic interface description, flags section\"\npreconditions = [\"start=genuine-first\"]\n\n[[indicator]]\nmodel = \"{model}\"\nname = \"jam\"\nmeaning = \"jamming\"\n\n[[indicator]]\nmodel = \"{model}\"\nname = \"cw\"\nmeaning = \"jamming_cw\"\n\n[[indicator]]\nmodel = \"{model}\"\nname = \"integrity\"\nmeaning = \"integrity\"\n"
        ));
    }
    req.push_str(&runs);
    files.push(("request.toml".into(), req));
    Ok(files)
}

/// `scorecard flags-synth`.
pub fn flags_synth(args: &[String]) -> ExitCode {
    let Some(dir) = file_arg(args) else {
        return fail(FLAGS_USAGE);
    };
    match flags_synth_files().and_then(|f| write_all(Path::new(dir), &f)) {
        Ok(()) => {
            eprintln!("[flags] synthetic campaign in {dir} (request.toml, logs/)");
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}

/// `scorecard units`.
pub fn units(args: &[String]) -> ExitCode {
    use crate::scorecards::units as u;
    let Some(file) = file_arg(args) else {
        return fail(UNITS_USAGE);
    };
    let r = producing(args, crate::scorecards::KIND).and_then(|(out, seed, signer, utc)| {
        let (req, src) = u::Request::from_file(Path::new(file))?;
        let s = u::score(&req, &src)?;
        u::write_dir(Path::new(&out), &s, &src)?;
        sign(&out, &seed, &signer, &utc)?;
        Ok((s, out))
    });
    match r {
        Ok((s, out)) => {
            let regs: usize = s.regressions.iter().map(|x| x.regressions).sum();
            eprintln!(
                "[units] {} unit(s), {} model/firmware card(s), {} variability note(s), {} firmware regression(s); signed in {out}",
                s.units.len(),
                s.models.len(),
                s.variability.len(),
                regs
            );
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}

/// The synthetic unit campaign: one board model, three units on firmware 1.0 that recover in
/// 10 s and three on 1.1 that recover in 60 s.
pub fn units_synth_files() -> Result<Vec<(String, String)>, String> {
    use crate::pics::log::{Device, DeviceLog, Epoch, FORMAT};
    use crate::scorecards::flags::load_scenario;
    let spec = load_scenario("receiver-flags/flags-jam-broadband", None)?;
    let (on, off) = spec.threat_window(None).ok_or("no threat")?;
    let mut files = vec![];
    let mut req = format!(
        "format = \"{}\"\ntitle = \"Synthetic board campaign, firmware 1.0 against 1.1 (MODELLED fixtures)\"\n\n[bars]\nfixed_utc = \"2026-11-20T09:00:00Z\"\n\n[[compare]]\nmodel = \"Board-X\"\nfrom = \"1.0\"\nto = \"1.1\"\n",
        crate::scorecards::units::REQUEST_FORMAT
    );
    let tr = spec.trajectory.at(0.0);
    for (i, (fw, rec)) in [
        ("1.0", 10.0),
        ("1.0", 11.0),
        ("1.0", 12.0),
        ("1.1", 58.0),
        ("1.1", 60.0),
        ("1.1", 62.0),
    ]
    .into_iter()
    .enumerate()
    {
        let id = format!("bx-{}", i + 1);
        let epochs = (0..=spec.scene.duration_s as usize)
            .map(|k| {
                let t = k as f64;
                let jammed = t >= on + 2.0 && t < off + rec;
                Epoch {
                    t_s: t,
                    fix_valid: Some(!jammed),
                    lat_deg: Some(tr.lat_deg),
                    lon_deg: Some(tr.lon_deg),
                    jamming: Some(jammed && t < off),
                    ..Epoch::default()
                }
            })
            .collect();
        let log = DeviceLog {
            format: FORMAT.into(),
            device: Device {
                name: "Board-X".into(),
                firmware: Some(fw.into()),
                serial: Some(id.clone()),
            },
            scenario: spec.id.clone(),
            truth: None,
            source: Some("synthetic".into()),
            epochs,
        };
        files.push((format!("logs/{id}.json"), log.to_json()));
        req.push_str(&format!(
            "\n[[unit]]\nid = \"{id}\"\nmodel = \"Board-X\"\nclass = \"board\"\nfirmware = \"{fw}\"\nserial = \"{id}\"\n\n[[run]]\nunit = \"{id}\"\nscenario = \"receiver-flags/flags-jam-broadband\"\nlog = \"logs/{id}.json\"\n"
        ));
    }
    files.push(("request.toml".into(), req));
    Ok(files)
}

/// `scorecard units-synth`.
pub fn units_synth(args: &[String]) -> ExitCode {
    let Some(dir) = file_arg(args) else {
        return fail(UNITS_USAGE);
    };
    match units_synth_files().and_then(|f| write_all(Path::new(dir), &f)) {
        Ok(()) => {
            eprintln!("[units] synthetic campaign in {dir} (request.toml, logs/)");
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}

/// `exposure scheme`.
pub fn scheme(args: &[String]) -> ExitCode {
    use crate::exposure::scheme as s;
    let Some(file) = file_arg(args) else {
        return fail(SCHEME_USAGE);
    };
    let r = producing(args, crate::exposure::FEATURE).and_then(|(out, seed, signer, utc)| {
        let trust = super::forensics_cmd::keys(args, "--trust")?;
        let (req, src) = s::Request::from_file(Path::new(file))?;
        let card = s::check(&req, &src, &trust)?;
        s::write_dir(Path::new(&out), &card, &src)?;
        sign(&out, &seed, &signer, &utc)?;
        Ok((card, out))
    });
    match r {
        Ok((c, out)) => {
            eprintln!(
                "[scheme] {} supported, {} not supported, {} insufficient evidence; publishable: {}; signed in {out}",
                c.supported,
                c.not_supported,
                c.insufficient,
                if c.publishable { "yes" } else { "no" }
            );
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}

/// `timing recovery`.
pub fn recovery(args: &[String]) -> ExitCode {
    use crate::timing::recovery as rc;
    let Some(file) = file_arg(args) else {
        return fail(RECOVERY_USAGE);
    };
    let r = producing(args, crate::timing::scorecard::KIND).and_then(|(out, seed, signer, utc)| {
        let (req, src) = rc::Request::from_file(Path::new(file))?;
        let rep = rc::score(&req, &src)?;
        rc::write_dir(Path::new(&out), &rep, &src)?;
        sign(&out, &seed, &signer, &utc)?;
        Ok((rep, out))
    });
    match r {
        Ok((rep, out)) => {
            eprintln!(
                "[recovery] {} configuration(s); ranking {}; signed in {out}",
                rep.receivers.len(),
                rep.ranking.join(" > ")
            );
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}

/// Three synthetic modules under a 10 ns/s push from 600 s to 900 s, and a request with the
/// median ensemble.
pub fn recovery_synth_files() -> Vec<(String, String)> {
    use crate::timing::recovery::synth;
    let logs = [
        (
            "fast",
            synth::push_and_slew(5.0, 600.0, 900.0, 10.0, 100.0, 2400.0, None),
        ),
        (
            "slow",
            synth::push_and_slew(-5.0, 600.0, 900.0, 10.0, 5.0, 2400.0, None),
        ),
        (
            "holdover",
            synth::push_and_slew(0.0, 600.0, 900.0, 10.0, 1.0, 2400.0, Some((960.0, 250.0))),
        ),
    ];
    let mut req = format!(
        "format = \"{}\"\ntitle = \"Synthetic timing modules under a time push (MODELLED fixtures)\"\nattack_scenario = \"receiver-flags/flags-spoof-time-push\"\nensemble_median = true\n\n[attack]\nstart_s = 600\nend_s = 900\n\n[bars]\nfixed_utc = \"2026-11-20T09:00:00Z\"\n",
        crate::timing::recovery::REQUEST_FORMAT
    );
    let mut files = vec![];
    for (id, s) in &logs {
        req.push_str(&format!(
            "\n[[receiver]]\nid = \"{id}\"\nmodel = \"Module-T\"\nlog = {{ format = \"csv\", path = \"{id}.csv\" }}\n"
        ));
        files.push((format!("{id}.csv"), synth::csv(s)));
    }
    files.push(("request.toml".into(), req));
    files
}

/// `timing recovery-synth`.
pub fn recovery_synth(args: &[String]) -> ExitCode {
    let Some(dir) = file_arg(args) else {
        return fail(RECOVERY_USAGE);
    };
    match write_all(Path::new(dir), &recovery_synth_files()) {
        Ok(()) => {
            eprintln!("[recovery] synthetic logs and request in {dir}");
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}
