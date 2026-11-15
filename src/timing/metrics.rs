// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The fleet in the Prometheus text format (roadmap 0.42 T2), for the Grafana dashboard
//! `deploy/grafana/kshana-pro-timing-fleet.json`. Served at `GET /timing/metrics` by the
//! Station console and written by `kshana-pro timing metrics` (for the node exporter's
//! text-file collector). A time beyond the horizon is `+Inf`.

use super::fleet::FleetAssessment;
use super::model::{NoiseSource, Z95};

fn lbl(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', " ")
}

fn val(v: Option<f64>) -> String {
    v.map(|x| format!("{x}")).unwrap_or_else(|| "+Inf".into())
}

/// Status as a number: 0 ok, 1 watch, 2 act, 3 breached.
pub fn status_code(s: &str) -> u8 {
    match s {
        "ok" => 0,
        "watch" => 1,
        "act" => 2,
        _ => 3,
    }
}

pub fn exposition(a: &FleetAssessment) -> String {
    let f = lbl(&a.fleet);
    let mut s = String::new();
    let head = |name: &str, help: &str, s: &mut String| {
        s.push_str(&format!("# HELP {name} {help}\n# TYPE {name} gauge\n"));
    };
    head(
        "kshana_pro_timing_fleet_clocks",
        "Clocks in the fleet.",
        &mut s,
    );
    s.push_str(&format!(
        "kshana_pro_timing_fleet_clocks{{fleet=\"{f}\"}} {}\n",
        a.counts.clocks
    ));
    head(
        "kshana_pro_timing_clock_info",
        "One per clock; its labels describe it.",
        &mut s,
    );
    for c in &a.clocks {
        s.push_str(&format!(
            "kshana_pro_timing_clock_info{{fleet=\"{f}\",clock=\"{}\",site=\"{}\",vendor=\"{}\",model=\"{}\",role=\"{}\",source=\"{}\",record=\"{}\",noise=\"{}\"}} 1\n",
            lbl(&c.id),
            lbl(c.site.as_deref().unwrap_or("")),
            lbl(c.vendor.as_deref().unwrap_or("")),
            lbl(c.model.as_deref().unwrap_or("")),
            lbl(c.role.as_deref().unwrap_or("")),
            lbl(&c.source_format),
            lbl(&c.record),
            match c.noise_source {
                NoiseSource::Measured => "measured",
                NoiseSource::Prior => "prior",
            }
        ));
    }
    head(
        "kshana_pro_timing_status",
        "Clock status: 0 ok, 1 watch, 2 act (warning within 4 h), 3 beyond a limit now.",
        &mut s,
    );
    for c in &a.clocks {
        s.push_str(&format!(
            "kshana_pro_timing_status{{fleet=\"{f}\",clock=\"{}\"}} {}\n",
            lbl(&c.id),
            status_code(&c.status)
        ));
    }
    head(
        "kshana_pro_timing_offset_seconds",
        "Phase at the handover (s): a servo record's present offset, or a free-running record's fitted phase.",
        &mut s,
    );
    for c in &a.clocks {
        s.push_str(&format!(
            "kshana_pro_timing_offset_seconds{{fleet=\"{f}\",clock=\"{}\"}} {:e}\n",
            lbl(&c.id),
            c.holdover.phase_s
        ));
    }
    head(
        "kshana_pro_timing_bound95_seconds",
        "Predicted 95 % bound of the holdover time error at each horizon (s).",
        &mut s,
    );
    for c in &a.clocks {
        for p in &c.predictions {
            s.push_str(&format!(
                "kshana_pro_timing_bound95_seconds{{fleet=\"{f}\",clock=\"{}\",horizon_s=\"{}\"}} {:e}\n",
                lbl(&c.id),
                p.u_s,
                p.mean_s.abs() + Z95 * p.sigma_s
            ));
        }
    }
    for (name, help, pick) in [
        (
            "kshana_pro_timing_breach_warn_seconds",
            "Time until the breach probability of a limit reaches 5 % (s; +Inf beyond the horizon).",
            0,
        ),
        (
            "kshana_pro_timing_breach_median_seconds",
            "Time until the breach probability of a limit reaches 50 % (s; +Inf beyond the horizon).",
            1,
        ),
    ] {
        head(name, help, &mut s);
        for c in &a.clocks {
            for b in &c.breaches {
                s.push_str(&format!(
                    "{name}{{fleet=\"{f}\",clock=\"{}\",limit=\"{}\",limit_s=\"{}\"}} {}\n",
                    lbl(&c.id),
                    lbl(&b.name),
                    b.value_s,
                    val(if pick == 0 { b.t_warn_s } else { b.t_50_s })
                ));
            }
        }
    }
    s
}
