// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! # kshana-eloran: the Kshana Pro eLoran/GNSS signal simulation engine
//!
//! A software-defined LORAN/eLoran signal simulator built for hybrid eLoran/GNSS testing:
//!
//! * [`signal`]: data-driven pulse signal definitions (LORAN-C per COMDTINST M16562.4A and
//!   any emerging definition), phase codes, ECD;
//! * [`chain`]: chains, stations, faults and the emission schedule on the Loran epoch;
//! * [`timebase`]: one truth timeline and any number of UTC(k) / GNSS time references;
//! * [`prop`]: groundwave and skywave propagation behind one trait;
//! * [`antenna`]: E-field and crossed-loop H-field output channels;
//! * [`interf`]: interference and noise;
//! * [`ldc`]: LORAN Data Channel schemes;
//! * [`scene`]: the synthesis engine (continuous-time delays, picosecond resolution) and
//!   the truth record;
//! * [`io`]: SigMF real-sample files for record and replay;
//! * [`rx`]: the software receiver;
//! * [`scenario`]: TOML scenarios;
//! * [`instr`]: bench instrument automation (TCP/serial, role drivers, TAPR TICC, evidence log);
//! * [`accept`]: calibration wizards and the hardware acceptance suite with RVM output;
//! * [`calreg`]: the hash-chained calibration register.
//!
//! Evidence labels follow the Kshana convention: a model is VALIDATED only where an
//! independent external reference backs it (see `docs/VALIDATION.md` of this crate).

pub mod accept;
pub mod antenna;
pub mod calreg;
pub mod chain;
pub mod dloran;
pub mod geo;
pub mod gridb;
pub mod hw;
pub mod instr;
pub mod interf;
pub mod io;
pub mod ldc;
pub mod plan;
pub mod prop;
pub mod replay;
pub mod rt;
pub mod rx;
pub mod threat;
pub mod twin;
pub mod scenario;
pub mod scene;
pub mod signal;
pub mod sync;
pub mod time;
pub mod timebase;
pub mod timeref;
pub mod traj;
pub mod valid;

pub use scenario::Scenario;
pub use scene::{MemSink, Scene, TruthRecord};

/// Run a scenario held in memory; returns samples, truth and statistics.
pub fn run_to_memory(
    src: &str,
) -> Result<(scene::MemSink, Vec<TruthRecord>, scene::RunStats), String> {
    let sc = Scenario::from_toml(src)?;
    let mut scene = sc.build()?;
    let mut sink = MemSink::default();
    let mut truth = Vec::new();
    let stats = scene.run(&mut sink, &mut truth)?;
    Ok((sink, truth, stats))
}
