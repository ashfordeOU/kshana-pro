// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro area picture` (roadmap 0.42 F1, F2): needs `pro-area-console`; a console with
//! more than one tenant needs an Enterprise licence.

use std::path::Path;
use std::process::ExitCode;

use crate::area::{self, Inputs};

use super::scorecard_cmd::entitlement;
use super::{flag, now_unix};

pub const USAGE: &str = "usage: kshana-pro area picture <console.toml> [--json <out.json>] [--out <page.html>] [--alerts <alerts.csv>] [--tenant <id>] [--license <token>] [--dev]\n\
    Fuse Stations, receivers and customer-supplied feeds into the area trust picture with event timelines\n\
    and wide-area events (Pro); detections and areas only, never emitter positions. --alerts writes one\n\
    raw alert per event for `kshana-pro alerts`.";

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::FAILURE
}

pub fn run(args: &[String]) -> ExitCode {
    if args.first().map(String::as_str) != Some("picture") {
        return fail(USAGE);
    }
    let Some(path) = args.get(1).filter(|a| !a.starts_with("--")) else {
        return fail(USAGE);
    };
    let now = now_unix();
    let ent = match entitlement(args, now) {
        Ok(e) => e,
        Err(e) => return fail(e),
    };
    if let Err(e) = crate::license::check(&ent, area::FEATURE, now) {
        return fail(e);
    }
    let inputs = match Inputs::load(Path::new(path)) {
        Ok(i) => i,
        Err(e) => return fail(e),
    };
    let tenants = inputs.spec.tenants();
    if tenants.len() > 1 && !area::multi_tenant_allowed(&ent) {
        return fail(format!(
            "this console has {} tenants; more than one tenant in one console needs an Enterprise licence",
            tenants.len()
        ));
    }
    let mut p = area::picture(&inputs);
    if let Some(t) = flag(args, "--tenant") {
        if !tenants.contains(&t) {
            return fail(format!("no tenant {t:?} in this console"));
        }
        p = p.for_tenant(&t);
    }
    let write = |f: Option<String>, body: &str| -> Result<(), String> {
        match f {
            Some(f) => std::fs::write(&f, body).map_err(|e| format!("{f}: {e}")),
            None => Ok(()),
        }
    };
    let res = write(flag(args, "--json"), &p.to_json())
        .and_then(|_| write(flag(args, "--out"), &area::page(&p, "")))
        .and_then(|_| {
            write(
                flag(args, "--alerts"),
                &crate::alerts::raw_log(&crate::alerts::from_picture(&p)),
            )
        });
    if let Err(e) = res {
        return fail(e);
    }
    println!(
        "{}: {} areas, {} events, {} wide-area; emitter positions {} ({} withheld)",
        p.title,
        p.areas.len(),
        p.events().len(),
        p.wide_area.len(),
        p.positions.emitter_positions,
        p.positions.withheld
    );
    for a in &p.areas {
        println!(
            "  {} [{}] now {}: {} events",
            a.id,
            a.tenant,
            a.current.as_str(),
            a.events.len()
        );
    }
    ExitCode::SUCCESS
}
