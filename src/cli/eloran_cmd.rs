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
    let mut cfg = RxConfig::default();
    cfg.output_scale_uv_m = sc.scene.params.output_scale_uv_m;
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
