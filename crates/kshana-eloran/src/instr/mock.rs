// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! In-process mock TCP instrument (tests and dry runs).
//!
//! Listens on `127.0.0.1` on an ephemeral port, accepts any number of connections (one
//! thread each), reads newline-terminated commands and passes each to a handler: `Some(r)`
//! is sent back as one line, `None` sends nothing (a write command). It is a stand-in for an
//! instrument's raw SCPI socket; it implements no instrument behaviour of its own.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

type Handler = Arc<Mutex<dyn FnMut(&str) -> Option<String> + Send>>;

/// A running mock instrument; stops accepting when dropped.
pub struct MockTcpInstrument {
    port: u16,
    stop: Arc<AtomicBool>,
    received: Arc<Mutex<Vec<String>>>,
}

impl MockTcpInstrument {
    /// Start a mock answering with `handler`.
    pub fn spawn(
        handler: impl FnMut(&str) -> Option<String> + Send + 'static,
    ) -> Result<MockTcpInstrument, String> {
        let l = TcpListener::bind("127.0.0.1:0").map_err(|e| format!("mock bind: {e}"))?;
        let port = l.local_addr().map_err(|e| e.to_string())?.port();
        let stop = Arc::new(AtomicBool::new(false));
        let received = Arc::new(Mutex::new(Vec::new()));
        let h: Handler = Arc::new(Mutex::new(handler));
        let (st, rx) = (stop.clone(), received.clone());
        thread::spawn(move || {
            for conn in l.incoming() {
                if st.load(Ordering::SeqCst) {
                    break;
                }
                let Ok(conn) = conn else { continue };
                let (h, rx) = (h.clone(), rx.clone());
                thread::spawn(move || serve(conn, h, rx));
            }
        });
        Ok(MockTcpInstrument {
            port,
            stop,
            received,
        })
    }

    /// Mock answering from a fixed table (`command → response`); other commands get none.
    pub fn scripted(table: &[(&str, &str)]) -> Result<MockTcpInstrument, String> {
        let t: Vec<(String, String)> = table
            .iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect();
        MockTcpInstrument::spawn(move |c| {
            t.iter()
                .find(|(k, _)| k == c)
                .map(|(_, v)| v.clone())
        })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    /// Every command received so far, in order of arrival.
    pub fn received(&self) -> Vec<String> {
        self.received.lock().map(|v| v.clone()).unwrap_or_default()
    }
}

fn serve(conn: TcpStream, h: Handler, rx: Arc<Mutex<Vec<String>>>) {
    let Ok(mut w) = conn.try_clone() else { return };
    let mut r = BufReader::new(conn);
    let mut line = String::new();
    loop {
        line.clear();
        match r.read_line(&mut line) {
            Ok(0) | Err(_) => return,
            Ok(_) => {}
        }
        let cmd = line.trim_end_matches(['\n', '\r']).to_string();
        if let Ok(mut v) = rx.lock() {
            v.push(cmd.clone());
        }
        let resp = match h.lock() {
            Ok(mut f) => f(&cmd),
            Err(_) => return,
        };
        if let Some(resp) = resp {
            if w.write_all(format!("{resp}\n").as_bytes()).is_err() {
                return;
            }
        }
    }
}

impl Drop for MockTcpInstrument {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // unblock the accept loop
        let _ = TcpStream::connect(("127.0.0.1", self.port));
    }
}

#[cfg(test)]
mod tests {
    use super::super::{ScpiSession, TcpTransport};
    use super::*;
    use std::time::Duration;

    #[test]
    fn scripted_mock_answers_queries_and_records_commands() {
        let m = MockTcpInstrument::scripted(&[("*IDN?", "MOCK,TIC,0,1.0")]).unwrap();
        let t = TcpTransport::connect("127.0.0.1", m.port(), Duration::from_secs(2)).unwrap();
        let mut s = ScpiSession::new("m", Box::new(t), None);
        s.write("*RST").unwrap();
        assert_eq!(s.identify().unwrap(), "MOCK,TIC,0,1.0");
        assert_eq!(m.received(), vec!["*RST", "*IDN?"]);
        assert!(s.transport_description().contains("127.0.0.1"));
    }
}
