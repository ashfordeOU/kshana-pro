// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro lunar <time|rates|w0|circular|holdover|offset|lunex>` (W1119 family D,
//! [`crate::lunar_pro`]). The computing commands need the Pro feature `pro-timing-holdover`
//! (lunar time keeping and holdover is timing assurance); decoding a broadcast offset, writing
//! the conformance vectors or the synthetic example, and validating a LUNEX file are free.

use std::path::Path;
use std::process::ExitCode;

use crate::lunar_pro::{self, circular, holdover, lunex, offset_codec, time};

use super::scorecard_cmd::entitlement;
use super::{flag, has_flag, now_unix};

pub const USAGE: &str = "usage: kshana-pro lunar <command>   (MODELLED; see docs/LUNAR-PRO.md)\n\
    time --from <scale> --to <scale> (--at <YYYY-MM-DDTHH:MM:SS> | --jd <jd>) [--rate-source lte440|first-principles] [--w0 <candidate>] [--kernel <file.bsp>... [--kernel-start <iso>] [--kernel-days <n>]] [--json]\n\
        Convert an instant between UTC, TAI, TT, TCG, TDB, TCB, TCL and TL with its GUM budget (Pro)\n\
    rates [--rate-source ...] [--w0 <candidate>] [--json]   Secular rates of TCL and TL against TT, TCG and TCB, with uncertainty (Pro)\n\
    w0 [--reference <candidate>] [--rate-source ...] [--json]   ns/day consequence of each candidate lunar W0_L (Pro)\n\
    circular --comparisons <csv> (--earth-csv <csv> | --circular-t <file>...) --month <YYYY-MM> --out <dir> [--option tl|tcl] [--issue <n>] [--window-days <d>] [--w0 <candidate>]\n\
        \"Circular T for the Moon\": [TL - TL(k)] and TL(k) - UTC with uncertainty, monthly table (Pro)\n\
    circular example --out <dir>   Write SYNTHETIC comparisons and Earth series to try the chain (free)\n\
    holdover (--clock <name> | --adev <tau:adev,...> | --h <h0,h-1,h-2>) --link-ns <sigma> --interval <s> [--n <per contact>] [--cal-ns <u>] [--strategy phase-only|phase-and-frequency] [--budget-ns <B>] [--k <k>] [--json]\n\
        Earth-Moon TWTT holdover: time error between contacts, required contact interval (Pro)\n\
    offset fit --option tl|tcl --centre <YYYY-MM-DDTHH:MM:SS> [--validity <0-8>] [--iod <n>] [--reference <0-15>] [--w0 <candidate>] [--json]   Fit a KLTO-1 broadcast offset to the D1 model (Pro; a proposal pending LNIS AD5, not a standard)\n\
    offset decode <hex>   Decode a KLTO-1 message (free)\n\
    offset vectors --out <file.json>   Write the KLTO-1 conformance vectors (free)\n\
    lunex convert --csv <in.csv> --out <file.lnx> [--time-tag <GPS|GAL|UTC|TAI|TT|TCL|TL>] [--platform <p>] [--frame <f>] [--marker <m>] [--interval <s>] [--w0 <m2/s2>] [--realisation <code>]\n\
        LuGRE-style CSV to LUNEX (Pro; a prototype proposal for the IGS RINEX Committee, not a standard)\n\
    lunex validate <file.lnx> [--json]   Validate a LUNEX file (free)\n\
  W0 candidates: kshana-equatorial-j2 (default), point-mass-mean-radius, turyshev-selenoid, turyshev-south-pole, conventional:<m2/s2>\n\
  Licence: --license <token> | an activated licence | --dev (debug builds); feature pro-timing-holdover";

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::FAILURE
}

fn gate(args: &[String]) -> Result<(), String> {
    let now = now_unix();
    let ent = entitlement(args, now)?;
    crate::license::check(&ent, lunar_pro::FEATURE, now).map_err(|e| e.to_string())
}

fn print_json<T: serde::Serialize>(v: &T) -> ExitCode {
    match serde_json::to_string_pretty(v) {
        Ok(s) => {
            println!("{s}");
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}

pub fn run(args: &[String]) -> ExitCode {
    let r = match args.first().map(String::as_str) {
        Some("time") => gated(args, time_cmd),
        Some("rates") => gated(args, rates_cmd),
        Some("w0") => gated(args, w0_cmd),
        Some("circular") if args.get(1).map(String::as_str) == Some("example") => {
            circular_example(args)
        }
        Some("circular") => gated(args, circular_cmd),
        Some("holdover") => gated(args, holdover_cmd),
        Some("offset") => match args.get(1).map(String::as_str) {
            Some("fit") => gated(args, offset_fit),
            Some("decode") => offset_decode(args),
            Some("vectors") => offset_vectors(args),
            _ => Err(USAGE.to_string()),
        },
        Some("lunex") => match args.get(1).map(String::as_str) {
            Some("convert") => gated(args, lunex_convert),
            Some("validate") => return lunex_validate(args),
            _ => Err(USAGE.to_string()),
        },
        _ => Err(USAGE.to_string()),
    };
    match r {
        Ok(code) => code,
        Err(e) => fail(e),
    }
}

fn gated(
    args: &[String],
    f: fn(&[String]) -> Result<ExitCode, String>,
) -> Result<ExitCode, String> {
    gate(args)?;
    f(args)
}

fn model(args: &[String]) -> Result<time::Model, String> {
    let rs =
        time::RateSource::parse(&flag(args, "--rate-source").unwrap_or_else(|| "lte440".into()))?;
    let w0 = time::W0Candidate::parse(
        &flag(args, "--w0").unwrap_or_else(|| "kshana-equatorial-j2".into()),
    )?;
    let mut m = time::Model::new(rs, w0);
    let kernels: Vec<&String> = args
        .iter()
        .enumerate()
        .filter(|(i, a)| *a == "--kernel" && *i + 1 < args.len())
        .map(|(i, _)| &args[i + 1])
        .collect();
    if !kernels.is_empty() {
        if rs != time::RateSource::FirstPrinciples {
            return Err("--kernel needs --rate-source first-principles".into());
        }
        let start = time::parse_iso(
            &flag(args, "--kernel-start").unwrap_or_else(|| "2025-01-01T00:00:00".into()),
        )?;
        let days = flag(args, "--kernel-days")
            .map(|s| s.parse::<usize>().map_err(|_| "--kernel-days: bad count"))
            .transpose()?
            .unwrap_or(365);
        // The kernel averages are compared with the analytic average over the same window.
        let a =
            time::orbit_averages_with(start.to_f64(), days, "open engine analytic series", |jd| {
                let t = (jd - kshana::timescales::JD_J2000) / 36_525.0;
                Ok((
                    kshana::ephem::moon_position(t),
                    kshana::lunar_time::moon_geocentric_velocity_m_s(t),
                    kshana::ephem::sun_position(t),
                ))
            })?;
        m.averages = Some(a);
        for k in kernels {
            let ke = kshana::ephem_provider::KernelEphemeris::open(Path::new(k))?;
            m.ephemeris_alternatives
                .push(time::orbit_averages_kernel(&ke, start.to_f64(), days)?);
        }
    }
    Ok(m)
}

fn num(args: &[String], name: &str) -> Result<Option<f64>, String> {
    flag(args, name)
        .map(|s| {
            s.parse::<f64>()
                .map_err(|_| format!("{name}: bad number '{s}'"))
        })
        .transpose()
}

fn time_cmd(args: &[String]) -> Result<ExitCode, String> {
    let from = time::Scale::parse(&flag(args, "--from").ok_or(USAGE)?)?;
    let to = time::Scale::parse(&flag(args, "--to").ok_or(USAGE)?)?;
    let jd = match (flag(args, "--at"), num(args, "--jd")?) {
        (Some(s), _) => time::parse_iso(&s)?,
        (None, Some(j)) => kshana::timescales::TwoPartJd::from_f64(j),
        _ => return Err(USAGE.into()),
    };
    let m = model(args)?;
    let c = m.convert(from, jd, to);
    if has_flag(args, "--json") {
        return Ok(print_json(
            &serde_json::json!({ "label": lunar_pro::LABEL, "conversion": c }),
        ));
    }
    let out = kshana::timescales::civil_from_jd(c.jd_out.to_f64());
    println!("{}", lunar_pro::LABEL);
    println!(
        "{} -> {}: {} - {} = {:.12} s  (u_c {:.3} ns, U(k=2) {:.3} ns)",
        from.as_str(),
        to.as_str(),
        to.as_str(),
        from.as_str(),
        c.offset.value(),
        c.offset.u_c() * 1e9,
        c.offset.expanded(2.0) * 1e9
    );
    println!(
        "result: JD {} + {:.15} ({:04}-{:02}-{:02} {:02}:{:02}:{:09.6} {})",
        c.jd_out.day,
        c.jd_out.frac,
        out.year,
        out.month,
        out.day,
        out.hour,
        out.minute,
        out.second,
        to.as_str()
    );
    println!("budget:");
    for t in &c.offset.terms {
        println!(
            "  {:<44} {:>22.12} s  u {:>12.4} ns  [{:?}] {}",
            t.name,
            t.value,
            t.u.abs() * 1e9,
            t.kind,
            t.source
        );
    }
    for n in &c.notes {
        println!("note: {n}");
    }
    Ok(ExitCode::SUCCESS)
}

fn rates_cmd(args: &[String]) -> Result<ExitCode, String> {
    use time::Scale::*;
    let m = model(args)?;
    let rows: Vec<serde_json::Value> = [
        (Tcl, Tt),
        (Tl, Tt),
        (Tcl, Tl),
        (Tcg, Tcl),
        (Tcl, Tcb),
        (Tl, Tcb),
    ]
    .iter()
    .map(|&(a, b)| {
        let r = m.secular_rate(a, b);
        serde_json::json!({
            "rate": format!("d({}-{})/dt", a.as_str(), b.as_str()),
            "value": r.value(),
            "us_per_day": r.value() * 86_400e6,
            "u_ns_per_day": r.u_c() * 86_400e9,
            "budget": r,
        })
    })
    .collect();
    if has_flag(args, "--json") {
        return Ok(print_json(
            &serde_json::json!({ "label": lunar_pro::LABEL, "w0": m.w0, "rates": rows }),
        ));
    }
    println!(
        "{}\nW0_L: {} ({:.4} m2/s2)",
        lunar_pro::LABEL,
        m.w0.name,
        m.w0.w0_m2_s2
    );
    for r in &rows {
        println!(
            "  {:<14} {:>16.9e}  = {:>12.6} us/day  u {:.4} ns/day",
            r["rate"].as_str().unwrap_or(""),
            r["value"].as_f64().unwrap_or(f64::NAN),
            r["us_per_day"].as_f64().unwrap_or(f64::NAN),
            r["u_ns_per_day"].as_f64().unwrap_or(f64::NAN)
        );
    }
    Ok(ExitCode::SUCCESS)
}

fn w0_cmd(args: &[String]) -> Result<ExitCode, String> {
    let rs =
        time::RateSource::parse(&flag(args, "--rate-source").unwrap_or_else(|| "lte440".into()))?;
    let reference = time::W0Candidate::parse(
        &flag(args, "--reference").unwrap_or_else(|| "kshana-equatorial-j2".into()),
    )?;
    let rows = time::w0_sensitivity(rs, &reference, &time::W0Candidate::all());
    if has_flag(args, "--json") {
        return Ok(print_json(&serde_json::json!({
            "label": lunar_pro::LABEL,
            "d_rate_d_w0_ns_per_day_per_m2s2": time::d_rate_d_w0_ns_day(),
            "reference": reference,
            "rows": rows,
        })));
    }
    println!("{}", lunar_pro::LABEL);
    println!(
        "slope: {:.6e} ns/day per m2/s2 ({:.6e} ns/day per metre of lunar height); reference {}",
        time::d_rate_d_w0_ns_day(),
        -time::d_rate_d_w0_ns_day() * time::moon_surface_gravity(),
        reference.name
    );
    println!(
        "  {:<24} {:>16} {:>14} {:>14} {:>12} {:>12} {:>10}",
        "candidate", "W0_L m2/s2", "TCL-TL ns/d", "TL-TT us/d", "vs ref ns/d", "height m", "u ns/d"
    );
    for r in rows {
        println!(
            "  {:<24} {:>16.4} {:>14.4} {:>14.6} {:>12.4} {:>12.1} {:>10.4}",
            r.name,
            r.w0_m2_s2,
            r.tcl_minus_tl_ns_day,
            r.tl_minus_tt_us_day,
            r.delta_vs_ref_ns_day,
            r.height_equiv_m,
            r.u_ns_day
        );
    }
    Ok(ExitCode::SUCCESS)
}

fn write(dir: &Path, name: &str, body: &str) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let p = dir.join(name);
    std::fs::write(&p, body).map_err(|e| format!("{}: {e}", p.display()))
}

fn circular_cmd(args: &[String]) -> Result<ExitCode, String> {
    let cpath = flag(args, "--comparisons").ok_or(USAGE)?;
    let out = flag(args, "--out").ok_or(USAGE)?;
    let month = flag(args, "--month").ok_or(USAGE)?;
    let (y, m) = month.split_once('-').ok_or("--month YYYY-MM")?;
    let year: i32 = y.parse().map_err(|_| "--month YYYY-MM")?;
    let month: u32 = m.parse().map_err(|_| "--month YYYY-MM")?;
    let read = |p: &str| std::fs::read_to_string(p).map_err(|e| format!("{p}: {e}"));
    let comparisons = circular::parse_comparisons(&read(&cpath)?)?;
    let earth = match flag(args, "--earth-csv") {
        Some(p) => circular::EarthSeries::from_csv(&read(&p)?)?,
        None => {
            let files: Vec<String> = args
                .iter()
                .enumerate()
                .filter(|(i, a)| *a == "--circular-t" && *i + 1 < args.len())
                .map(|(i, _)| read(&args[i + 1]))
                .collect::<Result<_, _>>()?;
            if files.is_empty() {
                return Err("give --earth-csv or --circular-t".into());
            }
            circular::EarthSeries::from_circular_t(&files)?
        }
    };
    let cfg = circular::Config {
        option: time::LunarOption::parse(&flag(args, "--option").unwrap_or_else(|| "tl".into()))?,
        issue: flag(args, "--issue")
            .map(|s| s.parse().map_err(|_| "--issue: bad number"))
            .transpose()?
            .unwrap_or(1),
        year,
        month,
        window_days: num(args, "--window-days")?.unwrap_or(2.5),
    };
    let c = circular::compute(&model(args)?, &cfg, &comparisons, &earth)?;
    let dir = Path::new(&out);
    write(dir, "circular.txt", &c.to_text())?;
    write(
        dir,
        "circular.json",
        &serde_json::to_string_pretty(&c).map_err(|e| e.to_string())?,
    )?;
    println!("{}", c.to_text());
    println!("wrote {}/circular.txt and circular.json", dir.display());
    Ok(ExitCode::SUCCESS)
}

fn circular_example(args: &[String]) -> Result<ExitCode, String> {
    let out = flag(args, "--out").ok_or(USAGE)?;
    let m = time::Model::default_model();
    let plan = circular::SyntheticPlan::example();
    let (cmp, earth, _) = circular::synthetic(&m, time::LunarOption::Tl, &plan)?;
    let dir = Path::new(&out);
    write(
        dir,
        "comparisons.csv",
        &format!(
            "# SYNTHETIC lunar clock comparisons (no lunar clock data exists)\n{}",
            circular::comparisons_csv(&cmp)
        ),
    )?;
    write(
        dir,
        "earth.csv",
        &format!(
            "# SYNTHETIC Earth laboratory [UTC-UTC(k)] series\n{}",
            circular::earth_csv(&earth)
        ),
    )?;
    write(
        dir,
        "truth.json",
        &serde_json::to_string_pretty(&plan).map_err(|e| e.to_string())?,
    )?;
    println!(
        "wrote SYNTHETIC {0}/comparisons.csv, {0}/earth.csv and {0}/truth.json; run:\n  kshana-pro lunar circular --comparisons {0}/comparisons.csv --earth-csv {0}/earth.csv --month {1}-{2:02} --out {0}/out",
        dir.display(),
        plan.year,
        plan.month
    );
    Ok(ExitCode::SUCCESS)
}

fn holdover_cmd(args: &[String]) -> Result<ExitCode, String> {
    let clock = if let Some(n) = flag(args, "--clock") {
        holdover::ClockNoise::named(&n)?
    } else if let Some(s) = flag(args, "--adev") {
        let mut taus = Vec::new();
        let mut adevs = Vec::new();
        for p in s.split(',') {
            let (t, a) = p.split_once(':').ok_or("--adev tau:adev,...")?;
            taus.push(t.trim().parse::<f64>().map_err(|_| "--adev: bad tau")?);
            adevs.push(a.trim().parse::<f64>().map_err(|_| "--adev: bad adev")?);
        }
        holdover::ClockNoise::from_adev_points(&taus, &adevs)?
    } else if let Some(s) = flag(args, "--h") {
        let v: Vec<f64> = s
            .split(',')
            .map(|x| x.trim().parse::<f64>())
            .collect::<Result<_, _>>()
            .map_err(|_| "--h h0,h-1,h-2")?;
        if v.len() != 3 || v.iter().any(|x| *x < 0.0) {
            return Err("--h needs three non-negative coefficients h0,h-1,h-2".into());
        }
        holdover::ClockNoise {
            h_0: v[0],
            h_m1: v[1],
            h_m2: v[2],
        }
    } else {
        return Err(USAGE.into());
    };
    let link = holdover::Link {
        sigma_s: num(args, "--link-ns")?.ok_or(USAGE)? * 1e-9,
        n_per_contact: flag(args, "--n")
            .map(|s| s.parse().map_err(|_| "--n: bad count"))
            .transpose()?
            .unwrap_or(1),
        calibration_u_s: num(args, "--cal-ns")?.unwrap_or(0.0) * 1e-9,
    };
    let strategy = holdover::Strategy::parse(
        &flag(args, "--strategy").unwrap_or_else(|| "phase-and-frequency".into()),
    )?;
    let h = holdover::Holdover {
        clock,
        link,
        strategy,
    };
    let interval = num(args, "--interval")?.ok_or(USAGE)?;
    let k = num(args, "--k")?.unwrap_or(2.0);
    let report = h.report(interval, k, 25);
    let required = num(args, "--budget-ns")?.map(|b| (b, h.required_interval(b * 1e-9, k, 1e9)));
    if has_flag(args, "--json") {
        return Ok(print_json(&serde_json::json!({
            "clock": clock,
            "link": link,
            "strategy": strategy,
            "report": report,
            "budget_ns": required.map(|r| r.0),
            "required_interval_s": required.and_then(|r| r.1),
        })));
    }
    println!("{}", lunar_pro::LABEL);
    println!(
        "contact every {interval} s ({strategy:?}): worst time error k*sigma(T) = {:.3} ns (k = {k}); rms over the cycle {:.3} ns",
        report.max_time_error_s * 1e9,
        report.rms_time_error_s * 1e9
    );
    let c = report.at_interval;
    println!(
        "  at T: link {:.3} ns, white FM {:.3} ns, flicker FM {:.3} ns, random-walk FM {:.3} ns, calibration {:.3} ns (1 sigma each)",
        c.link.sqrt() * 1e9,
        c.white_fm.sqrt() * 1e9,
        c.flicker_fm.sqrt() * 1e9,
        c.random_walk_fm.sqrt() * 1e9,
        c.calibration.sqrt() * 1e9
    );
    if let Some((b, r)) = required {
        match r {
            Some(t) => println!("  required contact interval for {b} ns at k = {k}: {t:.1} s ({:.3} h)", t / 3600.0),
            None => println!("  a {b} ns budget at k = {k} cannot be met at any cadence: the link or calibration alone exceeds it"),
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn offset_fit(args: &[String]) -> Result<ExitCode, String> {
    let option = time::LunarOption::parse(&flag(args, "--option").ok_or(USAGE)?)?;
    let centre = time::parse_iso(&flag(args, "--centre").ok_or(USAGE)?)?;
    let vi = flag(args, "--validity")
        .map(|s| s.parse::<usize>().map_err(|_| "--validity 0-8"))
        .transpose()?
        .unwrap_or(5);
    let iod = flag(args, "--iod")
        .map(|s| s.parse::<u8>().map_err(|_| "--iod 0-255"))
        .transpose()?
        .unwrap_or(0);
    let reference = flag(args, "--reference")
        .map(|s| s.parse::<u8>().map_err(|_| "--reference 0-15"))
        .transpose()?
        .unwrap_or(0);
    let f = offset_codec::fit(
        &model(args)?,
        offset_codec::BroadcastScale::from_option(option),
        centre,
        vi,
        iod,
        reference,
    )?;
    let hex = offset_codec::to_hex(&f.message.encode()?);
    if has_flag(args, "--json") {
        return Ok(print_json(
            &serde_json::json!({ "status": offset_codec::STATUS, "fit": f, "hex": hex }),
        ));
    }
    println!("{}", offset_codec::STATUS);
    println!("hex: {hex}");
    let p = f.physical;
    println!(
        "{:?} - UTC = {:.12} s + {:.6e}*dt + {:.6e}*dt^2, t_ot week {} + {} s, validity {} s",
        p.scale, p.offset_s, p.rate, p.rate_change, p.wn_ot, p.t_ot_s, p.validity_s
    );
    println!(
        "max residual vs the D1 model {:.3} ns; model U(k=2) {:.3} ns; class {} (bound {:?} ns)",
        f.max_residual_s * 1e9,
        f.model_u_k2_s * 1e9,
        f.message.uclass,
        p.u_bound_ns
    );
    Ok(ExitCode::SUCCESS)
}

fn offset_decode(args: &[String]) -> Result<ExitCode, String> {
    let h = args.get(2).ok_or(USAGE)?;
    let b = hex::decode(h.trim()).map_err(|e| format!("bad hex: {e}"))?;
    let m = offset_codec::Message::decode(&b)?;
    Ok(print_json(
        &serde_json::json!({ "status": offset_codec::STATUS, "message": m, "physical": m.physical() }),
    ))
}

fn offset_vectors(args: &[String]) -> Result<ExitCode, String> {
    let out = flag(args, "--out").ok_or(USAGE)?;
    let v = offset_codec::conformance_vectors()?;
    std::fs::write(
        &out,
        serde_json::to_string_pretty(&v).map_err(|e| e.to_string())? + "\n",
    )
    .map_err(|e| format!("{out}: {e}"))?;
    println!(
        "wrote {} KLTO-1 conformance vectors to {out} ({})",
        v.vectors.len(),
        offset_codec::STATUS
    );
    Ok(ExitCode::SUCCESS)
}

fn lunex_convert(args: &[String]) -> Result<ExitCode, String> {
    let input = flag(args, "--csv").ok_or(USAGE)?;
    let out = flag(args, "--out").ok_or(USAGE)?;
    let text = std::fs::read_to_string(&input).map_err(|e| format!("{input}: {e}"))?;
    let tag = flag(args, "--time-tag").unwrap_or_else(|| "GPS".into());
    let lunar_scale = if lunex::LUNAR_TAGS.contains(&tag.as_str()) {
        Some((
            tag.clone(),
            num(args, "--w0")?,
            flag(args, "--realisation").unwrap_or_default(),
        ))
    } else {
        None
    };
    let d = lunex::CsvOptions::default();
    let opt = lunex::CsvOptions {
        marker: flag(args, "--marker").unwrap_or(d.marker),
        platform: flag(args, "--platform").unwrap_or(d.platform),
        frame: flag(args, "--frame").unwrap_or(d.frame),
        time_tag: tag,
        lunar_scale,
        interval_s: num(args, "--interval")?,
        date: {
            let c = kshana::timescales::civil_from_jd(now_unix() as f64 / 86_400.0 + 2_440_587.5);
            format!(
                "{:04}{:02}{:02} {:02}{:02}{:02} UTC",
                c.year, c.month, c.day, c.hour, c.minute, c.second as u32
            )
        },
        ..d
    };
    let l = lunex::from_csv(&text, &opt)?;
    let body = l.to_text();
    let v = lunex::validate(&body);
    std::fs::write(&out, &body).map_err(|e| format!("{out}: {e}"))?;
    println!("wrote {out}: {} epochs ({})", l.epochs.len(), lunex::STATUS);
    for f in &v.findings {
        println!(
            "  {} {}: {}",
            if f.error { "error" } else { "warning" },
            f.code,
            f.message
        );
    }
    Ok(if v.valid {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

fn lunex_validate(args: &[String]) -> ExitCode {
    let Some(p) = args.get(2) else {
        return fail(USAGE);
    };
    let text = match std::fs::read_to_string(p) {
        Ok(t) => t,
        Err(e) => return fail(format!("{p}: {e}")),
    };
    let v = lunex::validate(&text);
    if has_flag(args, "--json") {
        print_json(&v);
    } else {
        println!(
            "{p}: {} ({} epochs) — {}",
            if v.valid { "VALID" } else { "INVALID" },
            v.epochs,
            v.status
        );
        for f in &v.findings {
            println!(
                "  {} {} (line {}): {}",
                if f.error { "error" } else { "warning" },
                f.code,
                f.line,
                f.message
            );
        }
    }
    if v.valid {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
