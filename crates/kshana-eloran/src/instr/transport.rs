// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Line-oriented transports: raw TCP socket (SCPI "port 5025" style) and serial device
//! files.
//!
//! Every message is one text line. On send the configured terminator (default `"\n"`) is
//! appended; on receive a line is read up to `'\n'` and a trailing `'\r'` is removed.
//!
//! The raw-socket protocol is the LXI/IVI "SCPI-RAW" convention (TCP port 5025 on most
//! LXI instruments): plain SCPI text over a TCP stream, no VXI-11 (ONC RPC) and no HiSLIP
//! framing. Instruments that only speak VXI-11 or HiSLIP are not supported here.

use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// A bidirectional line transport.
pub trait Transport: Send {
    /// Send one line (the terminator is appended).
    fn send(&mut self, line: &str) -> Result<(), String>;
    /// Receive one line (terminator removed). An end of stream is an error.
    fn recv_line(&mut self) -> Result<String, String>;
    /// Human-readable description (address or path).
    fn describe(&self) -> String;
}

/// Raw SCPI over TCP.
pub struct TcpTransport {
    addr: String,
    writer: TcpStream,
    reader: BufReader<TcpStream>,
    terminator: String,
}

impl TcpTransport {
    /// Connect to `host:port` with a connect and read timeout.
    pub fn connect(host: &str, port: u16, timeout: Duration) -> Result<TcpTransport, String> {
        let addr = format!("{host}:{port}");
        let sa = addr
            .to_socket_addrs()
            .map_err(|e| format!("{addr}: {e}"))?
            .next()
            .ok_or_else(|| format!("{addr}: no address"))?;
        let s = TcpStream::connect_timeout(&sa, timeout).map_err(|e| format!("{addr}: {e}"))?;
        s.set_read_timeout(Some(timeout))
            .map_err(|e| format!("{addr}: {e}"))?;
        s.set_write_timeout(Some(timeout))
            .map_err(|e| format!("{addr}: {e}"))?;
        let _ = s.set_nodelay(true);
        let r = s.try_clone().map_err(|e| format!("{addr}: {e}"))?;
        Ok(TcpTransport {
            addr,
            writer: s,
            reader: BufReader::new(r),
            terminator: "\n".into(),
        })
    }

    /// Change the send terminator (e.g. `"\r\n"`).
    pub fn with_terminator(mut self, t: &str) -> TcpTransport {
        self.terminator = t.to_string();
        self
    }
}

fn read_line<R: BufRead>(r: &mut R, what: &str) -> Result<String, String> {
    let mut buf = Vec::new();
    let n = r
        .read_until(b'\n', &mut buf)
        .map_err(|e| format!("{what}: read: {e}"))?;
    if n == 0 {
        return Err(format!("{what}: end of stream"));
    }
    while matches!(buf.last(), Some(b'\n') | Some(b'\r')) {
        buf.pop();
    }
    String::from_utf8(buf).map_err(|_| format!("{what}: response is not UTF-8"))
}

impl Transport for TcpTransport {
    fn send(&mut self, line: &str) -> Result<(), String> {
        let mut b = line.as_bytes().to_vec();
        b.extend_from_slice(self.terminator.as_bytes());
        self.writer
            .write_all(&b)
            .and_then(|_| self.writer.flush())
            .map_err(|e| format!("{}: write: {e}", self.addr))
    }
    fn recv_line(&mut self) -> Result<String, String> {
        read_line(&mut self.reader, &self.addr)
    }
    fn describe(&self) -> String {
        format!("tcp://{}", self.addr)
    }
}

/// Serial port as a device file (e.g. `/dev/ttyUSB0`, `/dev/ttyACM0`, `COM3`).
///
/// **No line discipline is configured here** (no termios dependency): set the baud rate,
/// framing and raw mode before opening, e.g. on Linux
/// `stty -F /dev/ttyUSB0 115200 cs8 -cstopb -parenb raw -echo` (the TAPR TICC uses
/// 115200 8N1). Reads block according to those settings (`VMIN`/`VTIME`); there is no read
/// timeout at this level. [`SerialTransport::open_split`] takes separate read and write
/// paths (named pipes, PTYs, or files in tests).
pub struct SerialTransport {
    path: PathBuf,
    reader: BufReader<Box<dyn Read + Send>>,
    writer: Box<dyn Write + Send>,
    terminator: String,
}

impl SerialTransport {
    /// Open a device file read/write.
    pub fn open(path: &Path) -> Result<SerialTransport, String> {
        let f = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        let w = f
            .try_clone()
            .map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(SerialTransport {
            path: path.to_path_buf(),
            reader: BufReader::new(Box::new(f)),
            writer: Box::new(w),
            terminator: "\r\n".into(),
        })
    }

    /// Read from `rx_path`, write to `tx_path` (appending).
    pub fn open_split(rx_path: &Path, tx_path: &Path) -> Result<SerialTransport, String> {
        let r = File::open(rx_path).map_err(|e| format!("{}: {e}", rx_path.display()))?;
        let w = OpenOptions::new()
            .append(true)
            .create(true)
            .open(tx_path)
            .map_err(|e| format!("{}: {e}", tx_path.display()))?;
        Ok(SerialTransport {
            path: rx_path.to_path_buf(),
            reader: BufReader::new(Box::new(r)),
            writer: Box::new(w),
            terminator: "\r\n".into(),
        })
    }

    pub fn with_terminator(mut self, t: &str) -> SerialTransport {
        self.terminator = t.to_string();
        self
    }
}

impl Transport for SerialTransport {
    fn send(&mut self, line: &str) -> Result<(), String> {
        let mut b = line.as_bytes().to_vec();
        b.extend_from_slice(self.terminator.as_bytes());
        self.writer
            .write_all(&b)
            .and_then(|_| self.writer.flush())
            .map_err(|e| format!("{}: write: {e}", self.path.display()))
    }
    fn recv_line(&mut self) -> Result<String, String> {
        read_line(&mut self.reader, &self.path.display().to_string())
    }
    fn describe(&self) -> String {
        format!("serial://{}", self.path.display())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serial_split_reads_lines_and_writes_with_terminator() {
        let d = std::env::temp_dir().join(format!("kshana-serial-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let rx = d.join("rx");
        let tx = d.join("tx");
        std::fs::write(&rx, "line one\r\nline two\n").unwrap();
        let _ = std::fs::remove_file(&tx);
        let mut t = SerialTransport::open_split(&rx, &tx).unwrap();
        assert_eq!(t.recv_line().unwrap(), "line one");
        assert_eq!(t.recv_line().unwrap(), "line two");
        assert!(t.recv_line().unwrap_err().contains("end of stream"));
        t.send("*IDN?").unwrap();
        assert_eq!(std::fs::read_to_string(&tx).unwrap(), "*IDN?\r\n");
        assert!(t.describe().starts_with("serial://"));
        std::fs::remove_dir_all(&d).ok();
    }
}
