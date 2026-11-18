// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro eloran ...`: the eLoran/GNSS signal simulator (crate `kshana-eloran`).
//!
//! * `run` (feature `pro-eloran`, Enterprise): run a scenario, write the multi-channel
//!   SigMF recording, the truth table and the run record (scenario SHA-256, statistics).
//! * `validate` and `example` need no licence.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use kshana_eloran::io::{Format, SigmfWriter};
use kshana_eloran::scene::{truth_csv, NullSink, SampleSink};
use kshana_eloran::Scenario;

use super::scorecard_cmd::entitlement;
use super::{flag, now_unix};

/// Licence feature of the simulator.
pub const FEATURE: &str = "pro-eloran";

pub const USAGE: &str = "usage: kshana-pro eloran <command>\n\
    run <scenario.toml> --out <base> [--format ri16|rf32|none] [--license <token>] [--dev]\n\
        Generate the eLoran signal of a scenario: <base>.sigmf-data/.sigmf-meta (one channel per\n\
        [[outputs]] antenna element), <base>.truth.csv (time of arrival, delay parts, ECD, field,\n\
        SNR, time-reference offset per station per group) and <base>.run.json (Enterprise)\n\
    gnss <scenario.toml> --nav <rinex-3 nav> --out <base> [--fs <hz>] [--no-galileo] [--no-gps] [--cn0 <dB-Hz>] [--threads <n>] [--license <token>] [--dev]\n\
        Generate the GNSS half of a hybrid scene in software: GPS L1 C/A and Galileo E1-B complex baseband\n\
        (<base>.gnss.sigmf-data/-meta, ci16_le at 1575.42 MHz) and per-satellite truth, on the scenario's\n\
        trajectory and start instant (Enterprise)\n\
    rx <scenario.toml> --recording <base> --out <dir> [--truth <truth.csv>] [--per-loop] [--no-cri-blanking] [--license <token>] [--dev]\n\
        Software eLoran receiver on a recording (the scenario gives chains, start and antennas): per-station\n\
        ToA, ECD, SNR; time solution at the scenario's receiver position; position solution; with --truth,\n\
        error statistics against the simulator truth (Enterprise)\n\
    gri <list|show|snippet> [<gri|id>] [--region <r>] [--active-on <YYYY-MM-DD>] [--search <text>]\n\
        The preloaded worldwide GRI/chain database with sources and confidence (no licence needed)\n\
    calibrate receiver --tic <readings.csv> --inputs <inputs.json> [--column <n>] [--unit s|ns] --out <report.json>\n\
        Receiver internal delay from time-interval-counter readings with a GUM uncertainty budget (Enterprise)\n\
    stream <scenario.toml> --device <file:<base> | soapy:<args>> [--pps-start] [--external-ref] [--external-pps] [--gain-db <dB>] [--cal <calibration.json>] [--license <token>] [--dev]\n\
        Stream the scenario in real time to an LF output device: a paced SigMF file device, or SoapySDR\n\
        hardware (build with feature eloran-soapy). Conducted use only: cables or antenna simulators,\n\
        never over the air (Enterprise, feature pro-eloran-hw)\n\
    awg <scenario.toml> --out <base> [--bits <n>] [--max-samples <n>] [--license <token>] [--dev]\n\
        Compile a periodic scenario into a repeating AWG segment (SigMF ri16 + report) (Enterprise)\n\
    accept software --out <report.json> [--license <token>] [--dev]\n\
        Run the software-only acceptance tests (resolution, SNR, skywave ranges) (Enterprise)\n\
    sync measure --tic <readings.csv> --out <report.json> [--unit s|ns] [--column <n>] [--target-ns <x>] [--license <token>] [--dev]\n\
        eLoran-vs-GNSS sync analysis: offset, TDEV/MTIE, drift, alarms, correction (Enterprise)\n\
    timeref circular-t <circular-t.txt> --labs <A,B> --start <utc> --duration <s>\n\
        Print [[time_refs]] models fitted to BIPM Circular T values (free)\n\
    asfgrid <scenario.toml> --box <lat1,lat2,lon1,lon2> --res <deg> --out <base> [--license <token>] [--dev]\n\
        ASF grid per station with the scenario's ground model: <base>.asf.csv and .asf.geojson (Enterprise)\n\
    fidelity <recording-base> --meta <capture.json> --scenario <scenario.toml> --out <report.json> [--license <token>] [--dev]\n\
        Compare a recording with its simulation at signal, observable and solution level (Enterprise)\n\
    import <log> --profile <csv-profile.toml> --out <observations.json> [--license <token>] [--dev]\n\
        Import a receiver log into common observation records (Enterprise)\n\
    conformance list\n\
        List the conformance test library (free)\n\
    campaign <run <plan.toml> --out <dir> | verify <dir>> [--license <token>] [--dev]\n\
        Run a pre-registered validation campaign (Enterprise); verify its manifest (free)\n\
    plan <plan.toml> --out <base> [--license <token>] [--dev]\n\
        Network planner: coverage, accuracy, availability, site selection, CRI risk (Enterprise)\n\
    twin <twin.toml> --out <result.json> [--license <token>] [--dev]\n\
        National-network digital twin: clocks, monitors, alarms, steering, KPIs (Enterprise)\n\
    validate <scenario.toml>\n\
        Check a scenario and print its SHA-256 (no licence needed)\n\
    example\n\
        Print an example scenario (two chains, two UTC(k), crossed loops, moving receiver)";

const EXAMPLE: &str = include_str!("../../crates/kshana-eloran/scenarios/synthetic-two-chain.toml");

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::FAILURE
}

pub fn run(args: &[String]) -> ExitCode {
    match args.first().map(String::as_str) {
        Some("run") => run_cmd(args),
        Some("validate") => validate_cmd(args),
        Some("gnss") => gnss_cmd(args),
        Some("rx") => rx_cmd(args),
        Some("stream") => stream_cmd(args),
        Some("awg") => awg_cmd(args),
        Some("accept") => accept_cmd(args),
        Some("sync") => sync_cmd(args),
        Some("timeref") => timeref_cmd(args),
        Some("asfgrid") => asfgrid_cmd(args),
        Some("fidelity") => fidelity_cmd(args),
        Some("import") => import_cmd(args),
        Some("conformance") => conformance_cmd(args),
        Some("campaign") => campaign_cmd(args),
        Some("plan") => plan_cmd(args),
        Some("twin") => twin_cmd(args),
        Some("gri") => gri_cmd(args),
        Some("calibrate") => calibrate_cmd(args),
        Some("example") => {
            print!("{EXAMPLE}");
            ExitCode::SUCCESS
        }
        _ => fail(USAGE),
    }
}

fn load(path: &str) -> Result<Scenario, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    Scenario::from_toml(&text).map_err(|e| format!("{path}: {e}"))
}

fn validate_cmd(args: &[String]) -> ExitCode {
    let Some(path) = args.get(1) else {
        return fail(USAGE);
    };
    match load(path).and_then(|s| s.build().map(|_| s)) {
        Ok(s) => {
            println!("ok {} sha256={}", path, s.hash());
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}

fn run_cmd(args: &[String]) -> ExitCode {
    let Some(path) = args.get(1).filter(|a| !a.starts_with("--")) else {
        return fail(USAGE);
    };
    let Some(out) = flag(args, "--out") else {
        return fail(USAGE);
    };
    let now = now_unix();
    let ent = match entitlement(args, now) {
        Ok(e) => e,
        Err(e) => return fail(e),
    };
    if let Err(e) = crate::license::check(&ent, FEATURE, now) {
        return fail(e);
    }
    let sc = match load(path) {
        Ok(s) => s,
        Err(e) => return fail(e),
    };
    let fmt_s = flag(args, "--format").unwrap_or_else(|| sc.file.format.clone());
    let mut scene = match sc.build() {
        Ok(s) => s,
        Err(e) => return fail(format!("{path}: {e}")),
    };
    let base = PathBuf::from(&out);
    if let Some(dir) = base.parent().filter(|d| !d.as_os_str().is_empty()) {
        if let Err(e) = std::fs::create_dir_all(dir) {
            return fail(format!("{}: {e}", dir.display()));
        }
    }
    let hash = sc.hash();
    let extra = serde_json::json!({
        "scenario_sha256": hash,
        "output_scale_uv_m": sc.scene.params.output_scale_uv_m,
        "outputs": sc.outputs,
        "start_utc": sc.scene.start_utc,
        "engine": format!("kshana-eloran {}", env!("CARGO_PKG_VERSION")),
    });
    let mut sink: Box<dyn SampleSink> = if fmt_s == "none" {
        Box::new(NullSink::default())
    } else {
        let fmt = match Format::parse(&fmt_s) {
            Ok(f) => f,
            Err(e) => return fail(e),
        };
        match SigmfWriter::create(
            &base,
            fmt,
            sc.outputs.len(),
            sc.scene.params.fs_hz,
            extra.clone(),
        ) {
            Ok(w) => Box::new(w),
            Err(e) => return fail(e),
        }
    };
    let mut truth = Vec::new();
    let stats = match scene.run(sink.as_mut(), &mut truth) {
        Ok(s) => s,
        Err(e) => return fail(e),
    };
    let truth_path = with_suffix(&base, "truth.csv");
    if let Err(e) = std::fs::write(&truth_path, truth_csv(&truth)) {
        return fail(format!("{}: {e}", truth_path.display()));
    }
    let record = serde_json::json!({
        "format": "kshana-eloran-run",
        "version": 1,
        "scenario": path,
        "scenario_sha256": hash,
        "licensee": ent.licensee,
        "stats": stats,
        "evidence_label": "MODELLED",
        "files": {
            "samples": if fmt_s == "none" { serde_json::Value::Null } else { serde_json::json!(base.with_extension("sigmf-data")) },
            "truth": truth_path,
        },
    });
    let run_path = with_suffix(&base, "run.json");
    if let Err(e) = std::fs::write(&run_path, serde_json::to_string_pretty(&record).unwrap()) {
        return fail(format!("{}: {e}", run_path.display()));
    }
    println!(
        "eloran: {} s, {} channel(s), {} pulses, {} truth rows, peak {:.3} FS, clipped {}, {:.1}x real time; sha256={}",
        sc.scene.params.duration_s,
        stats.channels,
        stats.pulses,
        stats.truth_records,
        stats.peak_abs,
        stats.clipped_samples,
        stats.realtime_factor,
        &hash[..16]
    );
    ExitCode::SUCCESS
}

fn with_suffix(base: &Path, suffix: &str) -> PathBuf {
    let mut s = base.as_os_str().to_os_string();
    s.push(".");
    s.push(suffix);
    PathBuf::from(s)
}

fn gnss_cmd(args: &[String]) -> ExitCode {
    let Some(path) = args.get(1).filter(|a| !a.starts_with("--")) else {
        return fail(USAGE);
    };
    let (Some(nav), Some(out)) = (flag(args, "--nav"), flag(args, "--out")) else {
        return fail(USAGE);
    };
    let now = now_unix();
    let ent = match entitlement(args, now) {
        Ok(e) => e,
        Err(e) => return fail(e),
    };
    if let Err(e) = crate::license::check(&ent, FEATURE, now) {
        return fail(e);
    }
    let sc = match load(path).and_then(|s| s.build().map(|_| s)) {
        Ok(s) => s,
        Err(e) => return fail(e),
    };
    let nav_text = match std::fs::read_to_string(&nav) {
        Ok(t) => t,
        Err(e) => return fail(format!("{nav}: {e}")),
    };
    let mut opts = crate::eloran_gnss::GnssOptions::default();
    if let Some(v) = flag(args, "--fs") {
        match v.parse() {
            Ok(x) => opts.fs_hz = x,
            Err(_) => return fail("--fs needs a number"),
        }
    }
    if let Some(v) = flag(args, "--cn0") {
        match v.parse() {
            Ok(x) => opts.cn0_dbhz = Some(x),
            Err(_) => return fail("--cn0 needs a number"),
        }
    }
    if let Some(v) = flag(args, "--threads") {
        match v.parse() {
            Ok(x) => opts.threads = x,
            Err(_) => return fail("--threads needs a whole number"),
        }
    }
    opts.gps = !args.iter().any(|a| a == "--no-gps");
    opts.galileo = !args.iter().any(|a| a == "--no-galileo");
    let base = PathBuf::from(&out);
    if let Some(dir) = base.parent().filter(|d| !d.as_os_str().is_empty()) {
        if let Err(e) = std::fs::create_dir_all(dir) {
            return fail(format!("{}: {e}", dir.display()));
        }
    }
    match crate::eloran_gnss::generate(&sc, &nav_text, &opts, &base) {
        Ok(r) => {
            println!(
                "gnss: GPS week {} TOW {:.3} s, {} satellite(s), {} samples, {} truth rows -> {}",
                r.gps_week,
                r.start_tow_s,
                r.satellites.len(),
                r.samples,
                r.truth_records,
                r.data.display()
            );
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}

fn licensed(args: &[String]) -> Result<String, ExitCode> {
    let now = now_unix();
    let ent = entitlement(args, now).map_err(fail)?;
    crate::license::check(&ent, FEATURE, now).map_err(fail)?;
    Ok(ent.licensee)
}

fn rx_cmd(args: &[String]) -> ExitCode {
    use kshana_eloran::rx::{self, solve, truth as rxt, Antenna, LoopMode, RxConfig, RxInput};
    let Some(path) = args.get(1).filter(|a| !a.starts_with("--")) else {
        return fail(USAGE);
    };
    let (Some(rec), Some(out)) = (flag(args, "--recording"), flag(args, "--out")) else {
        return fail(USAGE);
    };
    if let Err(c) = licensed(args) {
        return c;
    }
    let sc = match load(path).and_then(|s| s.build().map(|_| s)) {
        Ok(s) => s,
        Err(e) => return fail(e),
    };
    let start = match kshana_eloran::time::AbsTime::from_utc_iso(&sc.scene.start_utc) {
        Ok(t) => t,
        Err(e) => return fail(e),
    };
    let recording = match kshana_eloran::io::read_sigmf(Path::new(&rec), None) {
        Ok(r) => r,
        Err(e) => return fail(e),
    };
    let chains: Vec<_> = sc.chains.iter().map(|c| c.chain.clone()).collect();
    let mut cfg = RxConfig {
        output_scale_uv_m: sc.scene.params.output_scale_uv_m,
        ..Default::default()
    };
    let loops: Vec<usize> = sc
        .outputs
        .iter()
        .enumerate()
        .filter(|(_, o)| matches!(o, kshana_eloran::antenna::OutputChannel::HLoop { .. }))
        .map(|(i, _)| i)
        .collect();
    cfg.antenna = if loops.len() >= 2 {
        let mode = if args.iter().any(|a| a == "--per-loop") {
            LoopMode::PerLoop
        } else {
            LoopMode::BeamSteer
        };
        Antenna::CrossedLoop {
            a: loops[0],
            b: loops[1],
            mode,
        }
    } else {
        Antenna::EField { channel: 0 }
    };
    if args.iter().any(|a| a == "--no-cri-blanking") {
        cfg.cri.blank = false;
    }
    cfg.ldc = match sc.ldc_configs() {
        Ok(l) => l,
        Err(e) => return fail(e),
    };
    let input = RxInput::from_recording(&recording, start);
    let (_rx, output) = match rx::process(&input, &chains, cfg.clone()) {
        Ok(r) => r,
        Err(e) => return fail(e),
    };
    let dir = PathBuf::from(&out);
    if let Err(e) = std::fs::create_dir_all(&dir) {
        return fail(format!("{}: {e}", dir.display()));
    }
    let mut csv = String::from("chain,station,channel,group,toa_s,envelope_toa_s,ecd_us,snr_db,field_uv_m,cycle_slip,bearing_deg,pulses\n");
    for m in &output.measurements {
        csv.push_str(&format!(
            "{},{},{},{},{:.12},{:.12},{:.4},{:.2},{:.4},{},{},{}\n",
            m.chain,
            m.station,
            m.channel.map(|c| c.to_string()).unwrap_or_default(),
            m.group,
            m.toa_s,
            m.envelope_toa_s,
            m.ecd_us,
            m.snr_db,
            m.field_uv_m,
            m.cycle_slip,
            m.bearing_deg.map(|b| format!("{b:.3}")).unwrap_or_default(),
            m.pulses
        ));
    }
    if let Err(e) = std::fs::write(dir.join("measurements.csv"), csv) {
        return fail(e);
    }
    let corr = cfg.corrections.clone();
    let rx_pos = sc.receiver.state(0.0).pos;
    let ts = solve::time_solution(&output.measurements, &chains, start, rx_pos, &corr);
    let pos = solve::position_solution(
        &output.measurements,
        &chains,
        start,
        &corr,
        &solve::PositionConfig::default(),
    );
    let mut report = serde_json::json!({
        "format": "kshana-eloran-rx",
        "version": 1,
        "scenario_sha256": sc.hash(),
        "recording": rec,
        "stations_tracked": output.tracks.len(),
        "measurements": output.measurements.len(),
        "time_solution": ts.stations.iter().map(|s| serde_json::json!({
            "chain": s.chain, "station": s.station, "time_ref": s.time_ref,
            "ref_minus_rx_ns": s.ref_minus_rx_ns, "std_ns": s.std_ns, "n": s.n })).collect::<Vec<_>>(),
        "time_by_reference": ts.by_time_ref.iter().map(|(r, v, n)| serde_json::json!({"time_ref": r, "ref_minus_rx_ns": v, "stations": n})).collect::<Vec<_>>(),
        "position_solution": match &pos {
            Ok(p) => serde_json::json!({
                "lat_deg": p.position.lat_deg, "lon_deg": p.position.lon_deg, "hdop": p.hdop,
                "clock_bias_ns": p.clock_bias_ns, "residuals_ns": p.residuals_ns, "converged": p.converged }),
            Err(e) => serde_json::json!({ "error": e }),
        },
        "evidence_label": "MODELLED",
    });
    if !output.ldc.is_empty() {
        report["ldc"] = serde_json::json!(output
            .ldc
            .iter()
            .map(|l| serde_json::json!({
                "chain": l.chain,
                "station": l.station,
                "scheme": l.scheme,
                "first_frame": l.first_frame,
                "symbols": l.symbols.len(),
                "groups_missing": l.groups_missing,
                "frames_ok": l.report.frames_ok,
                "frames_failed": l.report.frames_failed,
                "symbols_corrected": l.report.symbols_corrected,
                "crc_failures": l.report.crc_failures,
                "messages": l.messages().map(|m| serde_json::to_value(m).unwrap_or_default()).collect::<Vec<_>>(),
                "auth": l.auth.iter().map(|a| serde_json::json!({"message": a.message, "verified": a.verified})).collect::<Vec<_>>(),
            }))
            .collect::<Vec<_>>());
    }
    if let Some(tp) = flag(args, "--truth") {
        let truth = match std::fs::read_to_string(&tp)
            .map_err(|e| format!("{tp}: {e}"))
            .and_then(|t| kshana_eloran::scene::truth_from_csv(&t))
        {
            Ok(t) => t,
            Err(e) => return fail(e),
        };
        let errs = rxt::toa_errors(&output.measurements, &truth);
        report["truth_comparison"] = serde_json::json!({
            "stations": errs.iter().map(|e| serde_json::json!({
                "chain": e.chain, "station": e.station, "channel": e.channel, "n": e.n,
                "toa_mean_ns": e.toa_mean_ns, "toa_std_ns": e.toa_std_ns, "toa_rms_ns": e.toa_rms_ns,
                "toa_max_abs_ns": e.toa_max_abs_ns, "ecd_rms_err_us": e.ecd_rms_err_us,
                "cycle_errors": e.cycle_errors })).collect::<Vec<_>>(),
            "position_error_m": pos.as_ref().ok().and_then(|p| rxt::position_error_m(p.position, &truth)),
        });
    }
    if let Err(e) = std::fs::write(
        dir.join("rx-report.json"),
        serde_json::to_string_pretty(&report).unwrap(),
    ) {
        return fail(e);
    }
    println!(
        "rx: {} station track(s), {} measurement(s){} -> {}",
        output.tracks.len(),
        output.measurements.len(),
        match &pos {
            Ok(p) => format!(
                ", fix {:.6} {:.6} HDOP {:.2}",
                p.position.lat_deg, p.position.lon_deg, p.hdop
            ),
            Err(_) => String::new(),
        },
        dir.display()
    );
    ExitCode::SUCCESS
}

fn gri_cmd(args: &[String]) -> ExitCode {
    use kshana_eloran::gridb::{self, GriDb};
    let db = match GriDb::embedded() {
        Ok(d) => d,
        Err(e) => return fail(e),
    };
    let target = args.get(2).filter(|a| !a.starts_with("--")).cloned();
    let find = |t: &str| -> Vec<&gridb::DbChain> {
        if let Some(c) = db.get(t) {
            return vec![c];
        }
        match t.parse::<u32>() {
            Ok(g) => db.by_gri(g),
            Err(_) => db.search(t),
        }
    };
    match args.get(1).map(String::as_str) {
        Some("list") => {
            let mut v: Vec<&gridb::DbChain> = db.chains.iter().collect();
            if let Some(r) = flag(args, "--region") {
                let keep: Vec<String> = db.by_region(&r).iter().map(|c| c.id.clone()).collect();
                v.retain(|c| keep.contains(&c.id));
            }
            if let Some(d) = flag(args, "--active-on") {
                v.retain(|c| c.active_on(&d));
            }
            if let Some(q) = flag(args, "--search") {
                let keep: Vec<String> = db.search(&q).iter().map(|c| c.id.clone()).collect();
                v.retain(|c| keep.contains(&c.id));
            }
            for c in v {
                println!(
                    "{:<22} GRI {:>5}  {:<28} {:<10} {}",
                    c.id,
                    c.gri_us / 10,
                    c.name,
                    if c.is_complete() {
                        "complete"
                    } else {
                        "incomplete"
                    },
                    c.region
                );
            }
            ExitCode::SUCCESS
        }
        Some("show") => {
            let Some(t) = target else { return fail(USAGE) };
            let found = find(&t);
            if found.is_empty() {
                return fail(format!("no chain matches {t}"));
            }
            for c in found {
                println!("{}", serde_json::to_string_pretty(c).unwrap());
            }
            ExitCode::SUCCESS
        }
        Some("snippet") => {
            let Some(t) = target else { return fail(USAGE) };
            let found = find(&t);
            let Some(c) = found.first() else {
                return fail(format!("no chain matches {t}"));
            };
            match gridb::scenario_snippet_for(c, &Default::default()) {
                Ok(s) => {
                    print!("{s}");
                    ExitCode::SUCCESS
                }
                Err(e) => fail(e),
            }
        }
        _ => fail(USAGE),
    }
}

fn calibrate_cmd(args: &[String]) -> ExitCode {
    use kshana_eloran::hw::calib::{parse_tic_csv, receiver_delay, ReceiverDelayInputs, TicUnit};
    if args.get(1).map(String::as_str) != Some("receiver") {
        return fail(USAGE);
    }
    let (Some(tic), Some(inputs), Some(out)) = (
        flag(args, "--tic"),
        flag(args, "--inputs"),
        flag(args, "--out"),
    ) else {
        return fail(USAGE);
    };
    if let Err(c) = licensed(args) {
        return c;
    }
    let unit = match flag(args, "--unit").as_deref() {
        None | Some("s") => TicUnit::Seconds,
        Some("ns") => TicUnit::Nanoseconds,
        Some(u) => return fail(format!("--unit {u}: s or ns")),
    };
    let column = match flag(args, "--column").map(|c| c.parse::<usize>()) {
        None => None,
        Some(Ok(c)) => Some(c),
        Some(Err(_)) => return fail("--column needs a whole number"),
    };
    let text = match std::fs::read_to_string(&tic) {
        Ok(t) => t,
        Err(e) => return fail(format!("{tic}: {e}")),
    };
    let readings = match parse_tic_csv(&text, column, unit) {
        Ok(r) => r,
        Err(e) => return fail(e),
    };
    let inp: ReceiverDelayInputs = match std::fs::read_to_string(&inputs)
        .map_err(|e| format!("{inputs}: {e}"))
        .and_then(|t| serde_json::from_str(&t).map_err(|e| format!("{inputs}: {e}")))
    {
        Ok(i) => i,
        Err(e) => return fail(e),
    };
    match receiver_delay(&readings, &inp) {
        Ok(r) => {
            if let Err(e) = std::fs::write(&out, serde_json::to_string_pretty(&r).unwrap()) {
                return fail(format!("{out}: {e}"));
            }
            println!(
                "receiver delay {:.2} ns (n = {}, Type A {:.2} ns) -> {}",
                r.receiver_delay_ns, r.n_readings, r.u_a_ns, out
            );
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}

/// Licence feature for driving LF hardware.
pub const HW_FEATURE: &str = "pro-eloran-hw";

fn stream_cmd(args: &[String]) -> ExitCode {
    use kshana_eloran::hw::{self, Device, DeviceConfig, PpsSource, RefSource, StartTime};
    let Some(path) = args.get(1).filter(|a| !a.starts_with("--")) else {
        return fail(USAGE);
    };
    let Some(dev_spec) = flag(args, "--device") else {
        return fail(USAGE);
    };
    let now = now_unix();
    let ent = match entitlement(args, now) {
        Ok(e) => e,
        Err(e) => return fail(e),
    };
    for f in [FEATURE, HW_FEATURE] {
        if let Err(e) = crate::license::check(&ent, f, now) {
            return fail(e);
        }
    }
    let sc = match load(path) {
        Ok(s) => s,
        Err(e) => return fail(e),
    };
    let scene = match sc.build() {
        Ok(s) => s,
        Err(e) => return fail(format!("{path}: {e}")),
    };
    let mut cfg = DeviceConfig::tx(sc.scene.params.fs_hz, sc.outputs.len());
    if args.iter().any(|a| a == "--external-ref") {
        cfg.clock_source = RefSource::External;
    }
    if args.iter().any(|a| a == "--external-pps") {
        cfg.time_source = PpsSource::External;
    }
    if let Some(g) = flag(args, "--gain-db") {
        match g.parse::<f64>() {
            Ok(v) => cfg.tx_gain_db = vec![v; sc.outputs.len()],
            Err(_) => return fail("--gain-db needs a number"),
        }
    }
    if flag(args, "--cal").is_some() {
        return fail("--cal: apply the calibration file with `kshana_eloran::hw::calib::CalibratedSink` in an integration; the command line does not apply it yet");
    }
    let mut device: Box<dyn Device> = if let Some(base) = dev_spec.strip_prefix("file:") {
        Box::new(hw::file::FileDevice::tx(
            Path::new(base),
            kshana_eloran::io::Format::Ri16,
        ))
    } else if let Some(_a) = dev_spec.strip_prefix("soapy:") {
        #[cfg(feature = "eloran-soapy")]
        {
            match hw::soapy::SoapyDevice::open(_a, hw::soapy::SoapyFormat::Cf32) {
                Ok(d) => Box::new(d),
                Err(e) => return fail(e),
            }
        }
        #[cfg(not(feature = "eloran-soapy"))]
        {
            return fail(
                "this build has no SoapySDR support (rebuild with --features eloran-soapy)",
            );
        }
    } else {
        return fail("--device must be file:<base> or soapy:<args>");
    };
    eprintln!("{}", hw::OUTPUT_POLICY);
    if let Err(e) = device.configure(&cfg) {
        return fail(e);
    }
    let at = if args.iter().any(|a| a == "--pps-start") {
        StartTime::NextPps
    } else {
        StartTime::Now
    };
    let session = match hw::stream_scene(device, scene, at, 16) {
        Ok(s) => s,
        Err(e) => return fail(e),
    };
    match session.wait() {
        Ok(r) => {
            let (stats, truth) = r.produced;
            println!(
                "stream: {} on {} — {} samples/channel, {} truth rows, FIFO underruns {}, device underruns {}",
                path,
                r.device.name(),
                r.session.samples_written,
                truth.len(),
                r.session.fifo_underruns,
                r.device_status.underruns
            );
            let _ = stats;
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}

fn read(path: &str) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))
}

fn write_json<T: serde::Serialize>(path: &str, v: &T) -> Result<(), String> {
    let p = Path::new(path);
    if let Some(d) = p.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
    }
    std::fs::write(
        p,
        serde_json::to_string_pretty(v).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("{path}: {e}"))
}

fn done(r: Result<String, String>) -> ExitCode {
    match r {
        Ok(msg) => {
            println!("{msg}");
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}

fn num_flag<T: std::str::FromStr>(args: &[String], name: &str) -> Result<Option<T>, String> {
    match flag(args, name) {
        None => Ok(None),
        Some(v) => v
            .parse::<T>()
            .map(Some)
            .map_err(|_| format!("{name}: not a valid number")),
    }
}

fn awg_cmd(args: &[String]) -> ExitCode {
    let Some(path) = args.get(1).filter(|a| !a.starts_with("--")).cloned() else {
        return fail(USAGE);
    };
    let Some(out) = flag(args, "--out") else {
        return fail(USAGE);
    };
    if let Err(c) = licensed(args) {
        return c;
    }
    done((|| {
        let sc = load(&path)?;
        let mut opt = kshana_eloran::hw::awg::AwgOptions::default();
        if let Some(b) = num_flag::<u32>(args, "--bits")? {
            opt.bits = b;
        }
        if let Some(m) = num_flag::<u64>(args, "--max-samples")? {
            opt.max_samples = m;
        }
        let seg = kshana_eloran::hw::awg::compile(&sc, &opt)?;
        let n_ch = seg.codes.len();
        let base = PathBuf::from(&out);
        let mut w = SigmfWriter::create(
            &base,
            kshana_eloran::io::Format::Rf32,
            n_ch,
            sc.scene.params.fs_hz,
            serde_json::json!({"awg_segment": true, "scenario_sha256": sc.hash()}),
        )?;
        let chans: Vec<Vec<f64>> = (0..n_ch).map(|c| seg.normalised(c)).collect();
        w.write_block(&chans)?;
        w.finish()?;
        write_json(&format!("{out}.awg.json"), &seg.report)?;
        Ok(format!(
            "awg: {} channel(s), {} samples per channel -> {}.sigmf-data",
            n_ch,
            chans.first().map(|c| c.len()).unwrap_or(0),
            out
        ))
    })())
}

fn accept_cmd(args: &[String]) -> ExitCode {
    use kshana_eloran::accept::suite::{default_suite, run_suite, Bench};
    if args.get(1).map(String::as_str) != Some("software") {
        return fail(USAGE);
    }
    let Some(out) = flag(args, "--out") else {
        return fail(USAGE);
    };
    if let Err(c) = licensed(args) {
        return c;
    }
    done((|| {
        let cases: Vec<_> = default_suite()
            .into_iter()
            .filter(|c| c.test.software_only())
            .collect();
        let rep = run_suite(&cases, None, &mut Bench::default());
        write_json(&out, &rep)?;
        let pass = rep
            .results
            .iter()
            .filter(|r| format!("{:?}", r.verdict) == "Pass")
            .count();
        Ok(format!(
            "accept: {pass}/{} software tests passed -> {out}",
            rep.results.len()
        ))
    })())
}

fn sync_cmd(args: &[String]) -> ExitCode {
    use kshana_eloran::hw::calib::TicUnit;
    use kshana_eloran::sync::measure::{analyse, MeasureConfig, TicSeries};
    if args.get(1).map(String::as_str) != Some("measure") {
        return fail(USAGE);
    }
    let (Some(tic), Some(out)) = (flag(args, "--tic"), flag(args, "--out")) else {
        return fail(USAGE);
    };
    if let Err(c) = licensed(args) {
        return c;
    }
    done((|| {
        let unit = match flag(args, "--unit").as_deref() {
            None | Some("ns") => TicUnit::Nanoseconds,
            Some("s") => TicUnit::Seconds,
            Some(u) => return Err(format!("--unit {u}: s or ns")),
        };
        let series = TicSeries::from_csv(&read(&tic)?, num_flag::<usize>(args, "--column")?, unit)?;
        let mut cfg = MeasureConfig::default();
        if let Some(t) = num_flag::<f64>(args, "--target-ns")? {
            cfg.target_ns = t;
        }
        let rep = analyse(&series, &cfg)?;
        write_json(&out, &rep)?;
        Ok(format!(
            "sync: {} readings analysed -> {out}",
            series.readings_ns.len()
        ))
    })())
}

fn timeref_cmd(args: &[String]) -> ExitCode {
    use kshana_eloran::sync::UtcStamp;
    use kshana_eloran::timeref::circular_t::{CircularT, ModelMode};
    if args.get(1).map(String::as_str) != Some("circular-t") {
        return fail(USAGE);
    }
    let Some(file) = args.get(2).filter(|a| !a.starts_with("--")).cloned() else {
        return fail(USAGE);
    };
    let (Some(labs), Some(start)) = (flag(args, "--labs"), flag(args, "--start")) else {
        return fail(USAGE);
    };
    done((|| {
        let ct = CircularT::parse(&read(&file)?)?;
        let dur = num_flag::<f64>(args, "--duration")?.unwrap_or(86_400.0);
        let labs: Vec<&str> = labs
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();
        let models =
            ct.time_ref_models(&labs, UtcStamp::parse(&start)?, dur, ModelMode::LinearFit)?;
        let mut s = String::new();
        for m in &models {
            s.push_str(&format!(
                "# {} from BIPM Circular T ({} values, fit RMS {:.2} ns)\n",
                m.lab, m.points_used, m.fit_rms_ns
            ));
            s.push_str("[[time_refs]]\n");
            s.push_str(&toml::to_string(&m.model).map_err(|e| e.to_string())?);
            s.push('\n');
        }
        Ok(s)
    })())
}

fn asfgrid_cmd(args: &[String]) -> ExitCode {
    use kshana_eloran::prop::asfgrid::{generate, GridSpec};
    let Some(path) = args.get(1).filter(|a| !a.starts_with("--")).cloned() else {
        return fail(USAGE);
    };
    let (Some(bx), Some(out)) = (flag(args, "--box"), flag(args, "--out")) else {
        return fail(USAGE);
    };
    if let Err(c) = licensed(args) {
        return c;
    }
    done((|| {
        let sc = load(&path)?;
        let v: Vec<f64> = bx
            .split(',')
            .map(|x| x.trim().parse::<f64>().map_err(|_| format!("--box: {x}")))
            .collect::<Result<_, _>>()?;
        if v.len() != 4 {
            return Err("--box needs lat1,lat2,lon1,lon2".into());
        }
        let res = num_flag::<f64>(args, "--res")?.unwrap_or(0.1);
        let spec: GridSpec = serde_json::from_value(serde_json::json!({
            "lat_min": v[0].min(v[1]), "lat_max": v[0].max(v[1]),
            "lon_min": v[2].min(v[3]), "lon_max": v[2].max(v[3]), "res_deg": res
        }))
        .map_err(|e| format!("grid: {e}"))?;
        let ctx = sc.build_context()?;
        let model = kshana_eloran::prop::build(&sc.propagation, &ctx)?;
        let stations: Vec<_> = sc
            .chains
            .iter()
            .flat_map(|c| c.chain.stations.clone())
            .collect();
        let grid = generate(
            model.as_ref(),
            &stations,
            &spec,
            kshana_eloran::prop::ground::K_LORAN,
        )?;
        std::fs::write(format!("{out}.asf.csv"), grid.to_csv()).map_err(|e| e.to_string())?;
        std::fs::write(format!("{out}.asf.geojson"), grid.to_geojson_points())
            .map_err(|e| e.to_string())?;
        Ok(format!(
            "asfgrid: {} station(s) -> {out}.asf.csv, {out}.asf.geojson",
            stations.len()
        ))
    })())
}

fn fidelity_cmd(args: &[String]) -> ExitCode {
    use kshana_eloran::valid::fidelity::{compare, CaptureMeta, FidelityConfig, ReferenceSource};
    let Some(rec) = args.get(1).filter(|a| !a.starts_with("--")).cloned() else {
        return fail(USAGE);
    };
    let (Some(meta), Some(scen), Some(out)) = (
        flag(args, "--meta"),
        flag(args, "--scenario"),
        flag(args, "--out"),
    ) else {
        return fail(USAGE);
    };
    if let Err(c) = licensed(args) {
        return c;
    }
    done((|| {
        let recording = kshana_eloran::io::read_sigmf(Path::new(&rec), None)?;
        let meta: CaptureMeta =
            serde_json::from_str(&read(&meta)?).map_err(|e| format!("capture metadata: {e}"))?;
        let sc = load(&scen)?;
        let rep = compare(
            &recording,
            &meta,
            &ReferenceSource::Scenario(Box::new(sc)),
            &FidelityConfig::default(),
        )?;
        write_json(&out, &rep)?;
        Ok(format!("fidelity: report -> {out}"))
    })())
}

fn import_cmd(args: &[String]) -> ExitCode {
    use kshana_eloran::field::importers::CsvProfile;
    let Some(log) = args.get(1).filter(|a| !a.starts_with("--")).cloned() else {
        return fail(USAGE);
    };
    let (Some(prof), Some(out)) = (flag(args, "--profile"), flag(args, "--out")) else {
        return fail(USAGE);
    };
    if let Err(c) = licensed(args) {
        return c;
    }
    done((|| {
        let p = CsvProfile::from_toml(&read(&prof)?)?;
        let (obs, rep) = p.import(&read(&log)?, &log)?;
        write_json(
            &out,
            &serde_json::json!({ "observations": obs, "report": rep }),
        )?;
        Ok(format!(
            "import: {} observation record(s) -> {out}",
            obs.len()
        ))
    })())
}

fn conformance_cmd(args: &[String]) -> ExitCode {
    use kshana_eloran::campaign::conformance::{library_dir, load_library};
    if args.get(1).map(String::as_str) != Some("list") {
        return fail(USAGE);
    }
    done((|| {
        let lib = load_library(&library_dir())?;
        let mut s = String::new();
        for f in &lib {
            s.push_str(&format!(
                "{:<28} {:<24} {}\n",
                f.case.id, f.case.category, f.case.title
            ));
        }
        s.push_str(&format!(
            "{} case(s); limits are illustrative until confirmed against the cited standards",
            lib.len()
        ));
        Ok(s)
    })())
}

fn campaign_cmd(args: &[String]) -> ExitCode {
    use kshana_eloran::campaign::runner::{run_campaign, verify_campaign};
    match args.get(1).map(String::as_str) {
        Some("verify") => {
            let Some(dir) = args.get(2) else {
                return fail(USAGE);
            };
            done(verify_campaign(Path::new(dir)).map(|m| {
                format!(
                    "campaign {} verified: {} file(s), {} pass, {} fail, {} not evaluated",
                    m.name,
                    m.files.len(),
                    m.pass,
                    m.fail,
                    m.not_evaluated
                )
            }))
        }
        Some("run") => {
            let Some(plan) = args.get(2).filter(|a| !a.starts_with("--")).cloned() else {
                return fail(USAGE);
            };
            let Some(out) = flag(args, "--out") else {
                return fail(USAGE);
            };
            if let Err(c) = licensed(args) {
                return c;
            }
            done((|| {
                let src = read(&plan)?;
                let dir = Path::new(&plan)
                    .parent()
                    .map(Path::to_path_buf)
                    .unwrap_or_default();
                let s = run_campaign(&src, &dir, Path::new(&out))?;
                Ok(format!(
                    "campaign {}: {} pass, {} fail, {} not evaluated -> {out}",
                    s.name, s.pass, s.fail, s.not_evaluated
                ))
            })())
        }
        _ => fail(USAGE),
    }
}

/// Planner configuration file.
#[derive(serde::Deserialize)]
struct PlanFile {
    candidates: Vec<kshana_eloran::plan::Candidate>,
    area: kshana_eloran::plan::PlanArea,
    #[serde(default)]
    settings: Option<kshana_eloran::plan::PlanSettings>,
    #[serde(default)]
    propagation: kshana_eloran::prop::PropConfig,
    /// Number of sites to select (default: all candidates).
    #[serde(default)]
    select: Option<usize>,
}

fn plan_cmd(args: &[String]) -> ExitCode {
    use kshana_eloran::plan::{coverage_csv, coverage_geojson, Planner};
    let Some(path) = args.get(1).filter(|a| !a.starts_with("--")).cloned() else {
        return fail(USAGE);
    };
    let Some(out) = flag(args, "--out") else {
        return fail(USAGE);
    };
    if let Err(c) = licensed(args) {
        return c;
    }
    done((|| {
        let pf: PlanFile = toml::from_str(&read(&path)?).map_err(|e| format!("{path}: {e}"))?;
        let ctx = kshana_eloran::prop::BuildContext {
            start: kshana_eloran::time::AbsTime::from_utc_iso("2026-06-21T12:00:00Z")?,
            duration_s: 1.0,
            seed: 0,
        };
        let model = kshana_eloran::prop::build(&pf.propagation, &ctx)?;
        let n = pf.candidates.len();
        let planner = Planner::new(
            pf.candidates,
            pf.area,
            pf.settings.unwrap_or_default(),
            model.as_ref(),
        )?;
        let cov = match pf.select {
            Some(m) if m < n => planner.select(m, 5000),
            _ => planner.evaluate(&(0..n).collect::<Vec<_>>()),
        };
        std::fs::write(format!("{out}.coverage.csv"), coverage_csv(&cov))
            .map_err(|e| e.to_string())?;
        std::fs::write(format!("{out}.coverage.geojson"), coverage_geojson(&cov))
            .map_err(|e| e.to_string())?;
        write_json(
            &format!("{out}.plan.json"),
            &serde_json::json!({"sites": cov.sites, "coverage": cov.coverage, "cost": cov.cost}),
        )?;
        Ok(format!(
            "plan: sites {:?}, coverage {:.1} %, cost {} -> {out}.*",
            cov.sites,
            cov.coverage * 100.0,
            cov.cost
        ))
    })())
}

fn twin_cmd(args: &[String]) -> ExitCode {
    let Some(path) = args.get(1).filter(|a| !a.starts_with("--")).cloned() else {
        return fail(USAGE);
    };
    let Some(out) = flag(args, "--out") else {
        return fail(USAGE);
    };
    if let Err(c) = licensed(args) {
        return c;
    }
    done((|| {
        let cfg: kshana_eloran::twin::TwinConfig =
            toml::from_str(&read(&path)?).map_err(|e| format!("{path}: {e}"))?;
        let r = kshana_eloran::twin::run(&cfg)?;
        write_json(&out, &r)?;
        let k: Vec<String> = r
            .kpis
            .iter()
            .map(|k| {
                format!(
                    "{}: max {:.1} ns, avail {:.4}, alarms {}",
                    k.id, k.max_abs_error_ns, k.availability, k.alarms
                )
            })
            .collect();
        Ok(format!("twin: {} -> {out}", k.join("; ")))
    })())
}
