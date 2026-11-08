// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//! The service settings file (`serve.conf`) read by `kshana-pro serve --config <file>`.
//!
//! The same `KEY=VALUE` file systemd reads as its `EnvironmentFile` on Linux, so one
//! format serves both: on Windows the service has no environment file, and the Windows
//! service passes `--config` instead. Recognised keys:
//!
//! | Key | Becomes |
//! |---|---|
//! | `KSHANA_PRO_ADDR` | `--addr` |
//! | `KSHANA_PRO_DATA_DIR` | `--data-dir` |
//! | `KSHANA_PRO_HOME` | the licence store directory (the `KSHANA_PRO_HOME` environment variable) |
//! | `KSHANA_PRO_SERVE_ARGS` | further `serve` options, split on spaces (double quotes keep a value with spaces whole) |
//!
//! A relative path in `KSHANA_PRO_DATA_DIR` or `KSHANA_PRO_HOME` is taken from the
//! settings file's own directory, so a Windows install can say `data` and `licence`
//! wherever `%ProgramData%` is. Options given on the command line win over the file.
//! Blank lines and lines starting with `#` are ignored; any other key is refused, so a
//! misspelt setting is not silently dropped.

use std::path::{Path, PathBuf};

/// What a settings file adds to a `serve` command line.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Settings {
    /// Options appended after the command line's own (so the command line wins).
    pub args: Vec<String>,
    /// The licence store directory, when the file sets one.
    pub licence_home: Option<PathBuf>,
}

/// Read the settings in `text`, a file in directory `dir`, for a command line `cli`
/// (the `serve` arguments already given).
pub fn parse(text: &str, dir: &Path, cli: &[String]) -> Result<Settings, String> {
    let mut out = Settings::default();
    let given = |name: &str| cli.iter().any(|a| a == name);
    let resolve = |v: &str| {
        let p = PathBuf::from(v);
        if p.is_absolute() {
            p
        } else {
            dir.join(p)
        }
    };
    for (n, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (k, v) = line
            .split_once('=')
            .ok_or_else(|| format!("line {}: not KEY=VALUE", n + 1))?;
        let (k, v) = (k.trim(), unquote(v.trim()));
        match k {
            "KSHANA_PRO_ADDR" if !given("--addr") => {
                out.args.extend(["--addr".into(), v.to_string()]);
            }
            "KSHANA_PRO_DATA_DIR" if !given("--data-dir") => {
                out.args.extend([
                    "--data-dir".into(),
                    resolve(v).to_string_lossy().into_owned(),
                ]);
            }
            "KSHANA_PRO_HOME" => out.licence_home = Some(resolve(v)),
            "KSHANA_PRO_SERVE_ARGS" => out
                .args
                .extend(split_args(v).map_err(|e| format!("line {}: {e}", n + 1))?),
            "KSHANA_PRO_ADDR" | "KSHANA_PRO_DATA_DIR" => {}
            other => return Err(format!("line {}: unknown setting {other}", n + 1)),
        }
    }
    Ok(out)
}

/// A value written as `"..."` is the text between the quotes.
fn unquote(v: &str) -> &str {
    v.strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or(v)
}

/// Split options on spaces; a double-quoted part stays one option.
fn split_args(s: &str) -> Result<Vec<String>, String> {
    let (mut out, mut cur, mut quoted, mut any) = (Vec::new(), String::new(), false, false);
    for c in s.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                any = true;
            }
            c if c.is_whitespace() && !quoted => {
                if any {
                    out.push(std::mem::take(&mut cur));
                    any = false;
                }
            }
            c => {
                cur.push(c);
                any = true;
            }
        }
    }
    if quoted {
        return Err("an opening quote has no closing quote".into());
    }
    if any {
        out.push(cur);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn the_linux_file_and_a_windows_file_read_the_same_way() {
        let dir = Path::new("/etc/kshana-pro");
        let linux = include_str!("../../deploy/linux/serve.conf");
        let st = parse(linux, dir, &s(&["--data-dir", "/var/lib/kshana-pro/data"])).unwrap();
        assert_eq!(st.args, s(&["--addr", "127.0.0.1:8088", "--workers", "2"]));
        assert_eq!(st.licence_home, None);

        let win = include_str!("../../deploy/windows/serve.conf");
        let base = Path::new("/programdata/kshana-pro");
        let st = parse(win, base, &[]).unwrap();
        assert_eq!(
            st.args,
            s(&[
                "--addr",
                "127.0.0.1:8088",
                "--data-dir",
                &base.join("data").to_string_lossy(),
                "--workers",
                "2"
            ])
        );
        assert_eq!(st.licence_home, Some(base.join("licence")));
    }

    #[test]
    fn the_command_line_wins_quotes_keep_spaces_and_unknown_keys_are_refused() {
        let dir = Path::new("/x");
        let st = parse(
            "KSHANA_PRO_ADDR=0.0.0.0:1\nKSHANA_PRO_SERVE_ARGS=--tls-cert \"/a b/c.pem\" --workers 3\n",
            dir,
            &s(&["--addr", "127.0.0.1:9"]),
        )
        .unwrap();
        assert_eq!(st.args, s(&["--tls-cert", "/a b/c.pem", "--workers", "3"]));
        let st = parse("KSHANA_PRO_HOME=\"/abs/licence\"\n", dir, &[]).unwrap();
        assert_eq!(st.licence_home, Some(PathBuf::from("/abs/licence")));
        assert!(parse("KSHANA_PRO_ADRR=1\n", dir, &[])
            .unwrap_err()
            .contains("unknown setting KSHANA_PRO_ADRR"));
        assert!(parse("no equals\n", dir, &[]).is_err());
        assert!(parse("KSHANA_PRO_SERVE_ARGS=--a \"b\n", dir, &[]).is_err());
    }
}
