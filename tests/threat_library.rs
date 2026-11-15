// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//! Threat scenario library (roadmap 0.42, E2).

use kshana_pro::threat_library::{self, export};

#[test]
fn explore() {
    for s in threat_library::library().unwrap() {
        match export::run_analogue(&s) {
            Ok(Some(r)) => println!("{} {}/{} {:?}", s.id, r.events_detected, r.events_evaluable, r.events.iter().map(|e| (e.outcome.clone(), e.latency_s, e.first_monitors.clone())).collect::<Vec<_>>()),
            Ok(None) => println!("{} timing only", s.id),
            Err(e) => println!("{} ERR {e}", s.id),
        }
    }
}
