// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! A generic line-based TCP remote-control adapter (SCPI style) for GNSS simulators.
//!
//! Commands are newline-terminated text lines; a command whose first word ends in `?` is a
//! query and one response line is read back. The actual command set differs between
//! simulator makes, so every action is a list of **command templates** in
//! [`CommandTemplates`] (JSON/TOML-serialisable) that a lab maps to its simulator's
//! remote-control manual. Templates may use the placeholders of
//! [`super::StartEpoch::fields`] (`{date}`, `{time}`, `{epoch_iso}`, `{gps_week}`,
//! `{gps_tow}`, …) and, for trajectories, `{path}`, `{format}` (`nmea`/`csv`) and
//! `{rate_hz}`. The defaults are neutral placeholders, not any vendor's command set.

use super::{
    ArmMode, GnssSimAdapter, GnssSimState, GnssSimStatus, StartEpoch, TrajFormat, TrajectoryFile,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::time::Duration;

/// Command templates of a simulator.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CommandTemplates {
    /// Sent after connecting; the response of the first query is the identity.
    pub connect: Vec<String>,
    pub set_epoch: Vec<String>,
    pub load_trajectory: Vec<String>,
    pub arm_immediate: Vec<String>,
    pub arm_pps: Vec<String>,
    pub arm_trigger: Vec<String>,
    /// Status query (must end in `?`).
    pub status: String,
    /// Substrings of the status response that mean each state (checked in this order:
    /// error, running, armed, stopped, idle).
    pub status_error: String,
    pub status_running: String,
    pub status_armed: String,
    pub status_stopped: String,
    pub status_idle: String,
    pub stop: Vec<String>,
    /// Optional error query sent after every non-query command (e.g. `SYST:ERR?`); a
    /// response not starting with `error_ok_prefix` fails the command.
    #[serde(default)]
    pub error_query: Option<String>,
    #[serde(default)]
    pub error_ok_prefix: String,
}

impl Default for CommandTemplates {
    fn default() -> Self {
        let v = |s: &str| vec![s.to_string()];
        CommandTemplates {
            connect: v("*IDN?"),
            set_epoch: v("SCEN:EPOCH {date},{time}"),
            load_trajectory: v("SCEN:TRAJ:LOAD \"{path}\",{format}"),
            arm_immediate: v("SCEN:START"),
            arm_pps: v("SCEN:ARM PPS"),
            arm_trigger: v("SCEN:ARM TRIG"),
            status: "SCEN:STAT?".into(),
            status_error: "ERR".into(),
            status_running: "RUN".into(),
            status_armed: "ARM".into(),
            status_stopped: "STOP".into(),
            status_idle: "IDLE".into(),
            stop: v("SCEN:STOP"),
            error_query: Some("SYST:ERR?".into()),
            error_ok_prefix: "0".into(),
        }
    }
}

/// Fill `{key}` placeholders; an unknown placeholder is an error.
pub fn fill(template: &str, fields: &BTreeMap<String, String>) -> Result<String, String> {
    let mut out = String::new();
    let mut rest = template;
    while let Some(a) = rest.find('{') {
        out.push_str(&rest[..a]);
        let b = rest[a..]
            .find('}')
            .ok_or_else(|| format!("unclosed placeholder in {template:?}"))?
            + a;
        let key = &rest[a + 1..b];
        out.push_str(
            fields
                .get(key)
                .ok_or_else(|| format!("unknown placeholder {{{key}}} in {template:?}"))?,
        );
        rest = &rest[b + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

fn is_query(cmd: &str) -> bool {
    cmd.split_whitespace()
        .next()
        .is_some_and(|w| w.ends_with('?'))
}

/// The TCP adapter.
pub struct ScpiTcpAdapter {
    pub addr: String,
    pub templates: CommandTemplates,
    pub timeout: Duration,
    stream: Option<(TcpStream, BufReader<TcpStream>)>,
    /// Every line sent and received (`> cmd`, `< response`).
    pub log: Vec<String>,
}

impl ScpiTcpAdapter {
    pub fn new(addr: &str, templates: CommandTemplates) -> ScpiTcpAdapter {
        ScpiTcpAdapter {
            addr: addr.into(),
            templates,
            timeout: Duration::from_secs(5),
            stream: None,
            log: vec![],
        }
    }

    /// Send one command; returns the response line for a query.
    pub fn send(&mut self, cmd: &str) -> Result<Option<String>, String> {
        let (w, r) = self.stream.as_mut().ok_or("not connected")?;
        w.write_all(format!("{cmd}\n").as_bytes())
            .map_err(|e| format!("{}: {e}", self.addr))?;
        self.log.push(format!("> {cmd}"));
        if !is_query(cmd) {
            return Ok(None);
        }
        let mut line = String::new();
        let k = r
            .read_line(&mut line)
            .map_err(|e| format!("{}: {e}", self.addr))?;
        if k == 0 {
            return Err(format!("{}: connection closed", self.addr));
        }
        let line = line.trim_end_matches(['\r', '\n']).to_string();
        self.log.push(format!("< {line}"));
        Ok(Some(line))
    }

    /// Send a command and, for non-queries, check the error queue.
    fn command(&mut self, cmd: &str) -> Result<Option<String>, String> {
        let r = self.send(cmd)?;
        if r.is_none() {
            if let Some(q) = self.templates.error_query.clone() {
                let e = self.send(&q)?.unwrap_or_default();
                if !e.starts_with(&self.templates.error_ok_prefix) {
                    return Err(format!("simulator rejected {cmd:?}: {e}"));
                }
            }
        }
        Ok(r)
    }

    fn run(
        &mut self,
        templates: &[String],
        fields: &BTreeMap<String, String>,
    ) -> Result<Vec<String>, String> {
        let mut resp = Vec::new();
        for t in templates {
            let c = fill(t, fields)?;
            if let Some(r) = self.command(&c)? {
                resp.push(r);
            }
        }
        Ok(resp)
    }
}

impl GnssSimAdapter for ScpiTcpAdapter {
    fn connect(&mut self) -> Result<String, String> {
        use std::net::ToSocketAddrs;
        let sa = self
            .addr
            .to_socket_addrs()
            .map_err(|e| format!("{}: {e}", self.addr))?
            .next()
            .ok_or_else(|| format!("{}: no address", self.addr))?;
        let s = TcpStream::connect_timeout(&sa, self.timeout)
            .map_err(|e| format!("{}: {e}", self.addr))?;
        s.set_read_timeout(Some(self.timeout))
            .map_err(|e| e.to_string())?;
        s.set_nodelay(true).ok();
        let r = BufReader::new(s.try_clone().map_err(|e| e.to_string())?);
        self.stream = Some((s, r));
        let t = self.templates.connect.clone();
        let resp = self.run(&t, &BTreeMap::new())?;
        Ok(resp.into_iter().next().unwrap_or_else(|| self.addr.clone()))
    }
    fn set_start_epoch(&mut self, epoch: &StartEpoch) -> Result<(), String> {
        let t = self.templates.set_epoch.clone();
        self.run(&t, &epoch.fields()).map(|_| ())
    }
    fn load_trajectory(&mut self, traj: &TrajectoryFile) -> Result<(), String> {
        let mut f = BTreeMap::new();
        f.insert("path".to_string(), traj.path.display().to_string());
        f.insert(
            "format".to_string(),
            match traj.format {
                TrajFormat::Nmea => "nmea",
                TrajFormat::Csv => "csv",
            }
            .to_string(),
        );
        f.insert("rate_hz".to_string(), format!("{}", traj.rate_hz));
        let t = self.templates.load_trajectory.clone();
        self.run(&t, &f).map(|_| ())
    }
    fn arm(&mut self, mode: ArmMode) -> Result<(), String> {
        let t = match mode {
            ArmMode::Immediate => self.templates.arm_immediate.clone(),
            ArmMode::NextPps => self.templates.arm_pps.clone(),
            ArmMode::ExternalTrigger => self.templates.arm_trigger.clone(),
        };
        self.run(&t, &BTreeMap::new()).map(|_| ())
    }
    fn status(&mut self) -> Result<GnssSimStatus, String> {
        let q = self.templates.status.clone();
        let raw = self
            .send(&q)?
            .ok_or("status template must be a query ending in '?'")?;
        let t = &self.templates;
        let has = |p: &str| !p.is_empty() && raw.contains(p);
        let state = if has(&t.status_error) {
            GnssSimState::Error
        } else if has(&t.status_running) {
            GnssSimState::Running
        } else if has(&t.status_armed) {
            GnssSimState::Armed
        } else if has(&t.status_stopped) {
            GnssSimState::Stopped
        } else if has(&t.status_idle) {
            GnssSimState::Idle
        } else {
            GnssSimState::Unknown
        };
        Ok(GnssSimStatus { state, raw })
    }
    fn stop(&mut self) -> Result<(), String> {
        let t = self.templates.stop.clone();
        self.run(&t, &BTreeMap::new()).map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::super::{arm_gnss, EpochScale, SyncTimingModel};
    use super::*;
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};

    /// In-process mock simulator: records commands, answers queries.
    fn mock() -> (String, Arc<Mutex<Vec<String>>>, std::thread::JoinHandle<()>) {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap().to_string();
        let log = Arc::new(Mutex::new(Vec::new()));
        let lg = log.clone();
        let h = std::thread::spawn(move || {
            let (s, _) = l.accept().unwrap();
            let mut w = s.try_clone().unwrap();
            let r = BufReader::new(s);
            let mut state = "IDLE";
            let mut last_err = "0,\"No error\"".to_string();
            for line in r.lines() {
                let Ok(line) = line else { break };
                lg.lock().unwrap().push(line.clone());
                let reply = match line.as_str() {
                    "*IDN?" => Some("MOCK,GNSS-SIM,0001,1.0".to_string()),
                    "SCEN:STAT?" => Some(state.to_string()),
                    "SYST:ERR?" => Some(std::mem::replace(&mut last_err, "0,\"No error\"".into())),
                    "SCEN:ARM PPS" => {
                        state = "ARMED";
                        None
                    }
                    "SCEN:STOP" => {
                        state = "STOPPED";
                        None
                    }
                    l if l.starts_with("BAD") => {
                        last_err = "-113,\"Undefined header\"".into();
                        None
                    }
                    _ => None,
                };
                if let Some(rp) = reply {
                    w.write_all(format!("{rp}\r\n").as_bytes()).unwrap();
                }
            }
        });
        (addr, log, h)
    }

    #[test]
    fn handshake_against_mock_server() {
        let (addr, log, h) = mock();
        let mut a = ScpiTcpAdapter::new(&addr, CommandTemplates::default());
        let model = SyncTimingModel {
            common_10mhz: true,
            common_pps: true,
            eloran_minus_gnss_ns: 0.0,
            eloran_minus_gnss_u_ns: 0.0,
            gnss_arm: ArmMode::NextPps,
            epoch_scale: EpochScale::Gpst,
        };
        let plan = model.plan("2026-11-18T00:00:00Z").unwrap();
        let traj = TrajectoryFile {
            path: "/data/rx.nmea".into(),
            format: TrajFormat::Nmea,
            rate_hz: 10.0,
            samples: 3,
        };
        let hs = arm_gnss(&mut a, &plan, Some(&traj)).unwrap();
        assert_eq!(hs.identity, "MOCK,GNSS-SIM,0001,1.0");
        assert_eq!(hs.status.state, GnssSimState::Armed);
        a.stop().unwrap();
        assert_eq!(a.status().unwrap().state, GnssSimState::Stopped);
        // a rejected command surfaces the error queue
        a.templates.stop = vec!["BAD CMD".into()];
        let e = a.stop().unwrap_err();
        assert!(e.contains("Undefined header"), "{e}");
        drop(a);
        h.join().unwrap();
        let cmds = log.lock().unwrap().clone();
        assert!(
            cmds.contains(&"SCEN:EPOCH 2026-11-18,00:00:18".to_string()),
            "{cmds:?}"
        );
        assert!(cmds.contains(&"SCEN:TRAJ:LOAD \"/data/rx.nmea\",nmea".to_string()));
        assert!(cmds.contains(&"SCEN:ARM PPS".to_string()));
    }

    #[test]
    fn fill_rejects_unknown_placeholders() {
        let mut f = BTreeMap::new();
        f.insert("a".to_string(), "1".to_string());
        assert_eq!(fill("X {a} Y", &f).unwrap(), "X 1 Y");
        assert!(fill("X {b}", &f).is_err());
        assert!(fill("X {a", &f).is_err());
        assert!(is_query("SCEN:STAT? 1"));
        assert!(!is_query("SCEN:STOP"));
    }

    #[test]
    fn templates_round_trip_as_json() {
        let t = CommandTemplates::default();
        let j = serde_json::to_string(&t).unwrap();
        assert_eq!(serde_json::from_str::<CommandTemplates>(&j).unwrap(), t);
    }
}
