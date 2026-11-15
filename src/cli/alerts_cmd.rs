// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro alerts <route|raw>` (roadmap 0.42 F5). `route` needs `pro-alert-policy`; `raw`
//! needs no licence: the free edition keeps raw alerts.

use std::path::Path;
use std::process::ExitCode;

use crate::alerts::{self, Policy};

use super::scorecard_cmd::entitlement;
use super::{flag, now_unix};

pub const USAGE: &str = "usage: kshana-pro alerts <command>\n\
    route <policy.toml> --alerts <alerts.csv> [--acks <acks.csv>] [--until <unix>] [--out <dir>] [--license <token>] [--dev]\n\
        Plan who is told what, when and through which I5 output: routes, quiet hours, escalation,\n\
        de-duplication (Pro); writes alert-plan.json and alert-notifications.csv\n\
    raw <alerts.csv>\n\
        Every alert, unchanged, in time order (no licence needed)";

/// The plan's file name.
pub const PLAN_FILE: &str = "alert-plan.json";
/// The notifications table's file name.
pub const NOTIFICATIONS_FILE: &str = "alert-notifications.csv";

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::FAILURE
}

fn read(p: &str) -> Result<String, String> {
    std::fs::read_to_string(p).map_err(|e| format!("{p}: {e}"))
}

pub fn run(args: &[String]) -> ExitCode {
    match args.first().map(String::as_str) {
        Some("raw") => {
            let Some(p) = args.get(1) else {
                return fail(USAGE);
            };
            match read(p).and_then(|t| alerts::parse_alerts(&t)) {
                Ok(a) => {
                    print!("{}", alerts::raw_log(&a));
                    ExitCode::SUCCESS
                }
                Err(e) => fail(e),
            }
        }
        Some("route") => route(args),
        _ => fail(USAGE),
    }
}

fn route(args: &[String]) -> ExitCode {
    let Some(policy_path) = args.get(1).filter(|a| !a.starts_with("--")) else {
        return fail(USAGE);
    };
    let Some(alerts_path) = flag(args, "--alerts") else {
        return fail(USAGE);
    };
    let now = now_unix();
    let ent = match entitlement(args, now) {
        Ok(e) => e,
        Err(e) => return fail(e),
    };
    if let Err(e) = crate::license::check(&ent, alerts::FEATURE, now) {
        return fail(format!(
            "{e}\nraw alerts stay free: kshana-pro alerts raw {alerts_path}"
        ));
    }
    let policy = match read(policy_path).and_then(|t| Policy::parse(&t)) {
        Ok(p) => p,
        Err(e) => return fail(e),
    };
    let list = match read(&alerts_path).and_then(|t| alerts::parse_alerts(&t)) {
        Ok(a) => a,
        Err(e) => return fail(e),
    };
    let acks = match flag(args, "--acks") {
        Some(p) => match read(&p).and_then(|t| alerts::parse_acks(&t)) {
            Ok(a) => a,
            Err(e) => return fail(e),
        },
        None => Vec::new(),
    };
    let until = match flag(args, "--until").map(|u| u.parse::<i64>()) {
        Some(Ok(u)) => u,
        Some(Err(_)) => return fail("--until is unix seconds"),
        None => list.iter().map(|a| a.t_unix).max().unwrap_or(0) + 7 * 86_400,
    };
    let plan = alerts::evaluate(&policy, &list, &acks, until);
    debug_assert!(plan.accounts_for(&list));
    if let Some(out) = flag(args, "--out") {
        let dir = Path::new(&out);
        let body = serde_json::to_string_pretty(&plan).expect("the plan is plain data") + "\n";
        if let Err(e) = std::fs::create_dir_all(dir)
            .and_then(|_| std::fs::write(dir.join(PLAN_FILE), body))
            .and_then(|_| std::fs::write(dir.join(NOTIFICATIONS_FILE), plan.notifications_csv()))
        {
            return fail(format!("{out}: {e}"));
        }
    }
    println!(
        "{} raw alerts: {} notifications, {} folded, {} held for quiet hours, {} unrouted (all kept in the raw log)",
        plan.raw_alerts,
        plan.notifications.len(),
        plan.folded.len(),
        plan.held.len(),
        plan.unrouted.len()
    );
    for n in &plan.notifications {
        let to: Vec<String> = n
            .deliveries
            .iter()
            .map(|(r, o)| format!("{r}:{o}"))
            .collect();
        println!(
            "  {} {} {} {} -> {} [{}]",
            n.t_unix,
            n.route,
            n.reason,
            n.severity.as_str(),
            to.join(" "),
            n.alerts.join(" ")
        );
    }
    ExitCode::SUCCESS
}
