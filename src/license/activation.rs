// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Offline-first activation and renewal (roadmap F10c).
//!
//! * **Activation** is local: the token is pasted or imported from a file, verified
//!   against the keys compiled into the build, and stored in the licence store. No
//!   network is used, so an air-gapped machine activates the same way (the token
//!   arrives on paper, a USB stick or by e-mail to another machine).
//! * **Revocation** arrives with updates: a signed list ([`super::revocation`]) is
//!   installed from the update or by hand; a revoked token stops at that point.
//! * **Expiry**: warnings 30 and 7 days ahead; after expiry a 14-day grace period
//!   keeps the licence working with a warning; then the build falls back to the
//!   Evaluation tier.
//! * **Online refresh** is opt-in and off by default: only when the store's
//!   `refresh.toml` says `enabled = true` does [`refresh`] fetch a newer revocation
//!   list and a renewed token from the vendor URL. Nothing else in Kshana phones home.
//!
//! The store is a directory: `licence.token`, `revocations.txt`, `refresh.toml`. It
//! is `$KSHANA_PRO_HOME` when set, else the platform's per-user configuration
//! directory (`~/Library/Application Support/Kshana`, `$XDG_CONFIG_HOME/kshana` or
//! `~/.config/kshana`, `%APPDATA%\Kshana`).

use std::path::{Path, PathBuf};

use kshana::api::KshanaError;

use super::revocation::{self, RevocationList};
use super::{verify_with, Entitlement, Keyring, Licence, Product, VerifyOptions};

/// Days of grace after expiry.
pub const GRACE_DAYS: i64 = 14;
/// Days before expiry at which the first and second warnings start.
pub const WARN_DAYS: [i64; 2] = [30, 7];
const DAY: i64 = 86_400;

/// Where a licence stands.
#[derive(Debug, Clone, PartialEq)]
pub enum State {
    /// No token activated: Evaluation tier.
    NoLicence,
    /// Valid, more than 30 days left (or perpetual).
    Active,
    /// Valid, expiring within 30 days.
    Expiring { days_left: i64 },
    /// Expired, within the grace period: still works.
    Grace { days_left: i64 },
    /// Expired beyond the grace period: Evaluation tier.
    Expired,
    /// On the revocation list: Evaluation tier.
    Revoked,
    /// The stored token does not verify here (wrong product, machine, key, not yet
    /// valid, damaged): Evaluation tier.
    Invalid(String),
}

impl State {
    pub fn as_str(&self) -> &'static str {
        match self {
            State::NoLicence => "none",
            State::Active => "active",
            State::Expiring { .. } => "expiring",
            State::Grace { .. } => "grace",
            State::Expired => "expired",
            State::Revoked => "revoked",
            State::Invalid(_) => "invalid",
        }
    }
}

/// The result of evaluating a token.
#[derive(Debug, Clone)]
pub struct Status {
    pub state: State,
    /// The verified licence, when the signature held.
    pub licence: Option<Licence>,
    /// What the build runs at: the licence's entitlement while active, expiring or in
    /// grace, else Evaluation.
    pub entitlement: Entitlement,
    /// A message for the user (expiry warning, grace, revocation, why invalid).
    pub warning: Option<String>,
}

/// Evaluate `token` at `opts.now_unix` against `ring` and the revocation lists (the
/// newest list's retired keys apply). Pure: no file or network access.
pub fn evaluate(
    ring: &Keyring,
    token: Option<&str>,
    lists: &[RevocationList],
    opts: &VerifyOptions<'_>,
) -> Status {
    let eval = |state: State, licence: Option<Licence>, warning: Option<String>| Status {
        state,
        licence,
        entitlement: Entitlement::evaluation(),
        warning,
    };
    let Some(token) = token.map(str::trim).filter(|t| !t.is_empty()) else {
        return eval(State::NoLicence, None, None);
    };
    let mut ring = ring.clone();
    for l in lists {
        l.apply_to(&mut ring);
    }
    let lenient = VerifyOptions {
        now_unix: opts.now_unix,
        product: opts.product,
        machine: opts.machine,
        allow_expired: true,
    };
    let lic = match verify_with(&ring, token, &lenient) {
        Ok(l) => l,
        Err(e) => {
            let msg = e.to_string();
            return eval(State::Invalid(msg.clone()), None, Some(msg));
        }
    };
    if lists.iter().any(|l| l.is_revoked(&lic.token_id)) {
        return eval(
            State::Revoked,
            Some(lic),
            Some("this licence has been revoked; running at Evaluation tier".into()),
        );
    }
    let exp = lic.claims.expires_unix;
    let now = opts.now_unix;
    if exp == 0 || now < exp - WARN_DAYS[0] * DAY {
        return Status {
            state: State::Active,
            entitlement: lic.entitlement.clone(),
            licence: Some(lic),
            warning: None,
        };
    }
    if now < exp {
        let days_left = (exp - now + DAY - 1) / DAY;
        let level = if days_left <= WARN_DAYS[1] {
            "Renew now"
        } else {
            "Renew soon"
        };
        return Status {
            state: State::Expiring { days_left },
            entitlement: lic.entitlement.clone(),
            warning: Some(format!(
                "{level}: the licence expires in {days_left} day(s)"
            )),
            licence: Some(lic),
        };
    }
    let grace_end = exp + GRACE_DAYS * DAY;
    if now < grace_end {
        let days_left = (grace_end - now + DAY - 1) / DAY;
        let mut ent = lic.entitlement.clone();
        ent.expires_unix = grace_end;
        return Status {
            state: State::Grace { days_left },
            entitlement: ent,
            warning: Some(format!(
                "the licence has expired; it keeps working for a {GRACE_DAYS}-day grace period ({days_left} day(s) left). Renew to keep Pro features"
            )),
            licence: Some(lic),
        };
    }
    eval(
        State::Expired,
        Some(lic),
        Some(
            "the licence has expired and the grace period is over; running at Evaluation tier"
                .into(),
        ),
    )
}

/// The licence store directory.
#[derive(Debug, Clone)]
pub struct Store {
    dir: PathBuf,
}

/// Opt-in online refresh settings (`refresh.toml` in the store).
#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
pub struct RefreshConfig {
    /// Off unless set to `true` by the user.
    #[serde(default)]
    pub enabled: bool,
    /// Vendor base URL; `<url>/revocations.txt` and `<url>/token/<token-id>` are read.
    #[serde(default)]
    pub url: String,
}

impl Store {
    /// The default store: `$KSHANA_PRO_HOME`, else the per-user configuration directory.
    pub fn default_dir() -> Option<PathBuf> {
        if let Ok(h) = std::env::var("KSHANA_PRO_HOME") {
            if !h.trim().is_empty() {
                return Some(PathBuf::from(h));
            }
        }
        let env = |k: &str| {
            std::env::var_os(k)
                .filter(|v| !v.is_empty())
                .map(PathBuf::from)
        };
        if cfg!(target_os = "windows") {
            return env("APPDATA").map(|p| p.join("Kshana"));
        }
        if cfg!(target_os = "macos") {
            return env("HOME").map(|p| p.join("Library/Application Support/Kshana"));
        }
        env("XDG_CONFIG_HOME")
            .map(|p| p.join("kshana"))
            .or_else(|| env("HOME").map(|p| p.join(".config/kshana")))
    }

    pub fn open(dir: impl Into<PathBuf>) -> Store {
        Store { dir: dir.into() }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }
    pub fn token_path(&self) -> PathBuf {
        self.dir.join("licence.token")
    }
    pub fn revocations_path(&self) -> PathBuf {
        self.dir.join("revocations.txt")
    }
    pub fn refresh_path(&self) -> PathBuf {
        self.dir.join("refresh.toml")
    }

    /// The activated token, if any.
    pub fn token(&self) -> Option<String> {
        std::fs::read_to_string(self.token_path())
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    /// Verify `token` and store it. Refuses a token that does not verify, is revoked by
    /// the stored list, or has expired beyond the grace period.
    pub fn activate(
        &self,
        ring: &Keyring,
        token: &str,
        opts: &VerifyOptions<'_>,
    ) -> Result<Status, KshanaError> {
        let lists = self.revocations(ring).into_iter().collect::<Vec<_>>();
        let st = evaluate(ring, Some(token), &lists, opts);
        match &st.state {
            State::Active | State::Expiring { .. } | State::Grace { .. } => {
                write_private(&self.token_path(), token.trim())?;
                Ok(st)
            }
            other => Err(KshanaError::InvalidInput(format!(
                "not activated: {}",
                st.warning.clone().unwrap_or_else(|| other.as_str().into())
            ))),
        }
    }

    /// Remove the activated token.
    pub fn deactivate(&self) -> Result<bool, KshanaError> {
        match std::fs::remove_file(self.token_path()) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(KshanaError::InvalidInput(format!(
                "could not remove the licence: {e}"
            ))),
        }
    }

    /// The stored revocation list, when present and valid.
    pub fn revocations(&self, ring: &Keyring) -> Option<RevocationList> {
        let text = std::fs::read_to_string(self.revocations_path()).ok()?;
        revocation::verify(ring, &text).ok()
    }

    /// Verify and install a signed revocation list. A list older than the stored one
    /// (lower `sequence`) is refused, so an old list cannot be replayed. Installing the
    /// same sequence again is a no-op.
    pub fn install_revocations(
        &self,
        ring: &Keyring,
        text: &str,
    ) -> Result<RevocationList, KshanaError> {
        let list = revocation::verify(ring, text)?;
        if let Some(cur) = self.revocations(ring) {
            if list.sequence < cur.sequence {
                return Err(KshanaError::InvalidInput(format!(
                    "revocation list {} is older than the installed list {}",
                    list.sequence, cur.sequence
                )));
            }
            if list.sequence == cur.sequence {
                return Ok(cur);
            }
        }
        write_private(&self.revocations_path(), text.trim())?;
        Ok(list)
    }

    /// The licence status: the stored token against the stored list and any `extra`
    /// lists (for example the one bundled with the installed app).
    pub fn status(
        &self,
        ring: &Keyring,
        extra: &[RevocationList],
        opts: &VerifyOptions<'_>,
    ) -> Status {
        let mut lists: Vec<RevocationList> = extra.to_vec();
        lists.extend(self.revocations(ring));
        evaluate(ring, self.token().as_deref(), &lists, opts)
    }

    /// The refresh settings (off when the file is absent).
    pub fn refresh_config(&self) -> RefreshConfig {
        std::fs::read_to_string(self.refresh_path())
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default()
    }
}

/// Fetches a URL; the only network access in licensing. Kept behind a trait so the
/// refresh is tested without a network and so the off switch is provable.
pub trait Fetcher {
    /// `Ok(None)` when the resource does not exist (HTTP 404).
    fn get(&self, url: &str) -> Result<Option<String>, String>;
}

/// Fetch with the system `curl` (present on macOS, Windows 10 and later and most
/// Linux systems), HTTPS only.
pub struct CurlFetcher;

impl Fetcher for CurlFetcher {
    fn get(&self, url: &str) -> Result<Option<String>, String> {
        if !url.starts_with("https://") {
            return Err("online refresh uses https:// URLs only".into());
        }
        let out = std::process::Command::new("curl")
            .args([
                "-sS",
                "--proto",
                "=https",
                "--max-time",
                "30",
                "-w",
                "\n%{http_code}",
                url,
            ])
            .output()
            .map_err(|e| format!("could not run curl: {e}"))?;
        if !out.status.success() {
            return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
        }
        let text = String::from_utf8_lossy(&out.stdout).to_string();
        let (body, code) = text.rsplit_once('\n').unwrap_or(("", text.as_str()));
        match code.trim() {
            "200" => Ok(Some(body.to_string())),
            "404" => Ok(None),
            c => Err(format!("HTTP {c} from {url}")),
        }
    }
}

/// What a refresh did.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RefreshOutcome {
    /// Sequence of the revocation list installed, when a newer one arrived.
    pub revocations: Option<u64>,
    /// Whether a renewed token replaced the stored one.
    pub renewed: bool,
}

/// Opt-in online refresh: fetch a newer revocation list and a renewed token. Refused,
/// with no call to `fetcher`, unless the store's `refresh.toml` enables it.
pub fn refresh(
    store: &Store,
    ring: &Keyring,
    fetcher: &dyn Fetcher,
    opts: &VerifyOptions<'_>,
) -> Result<RefreshOutcome, KshanaError> {
    let cfg = store.refresh_config();
    if !cfg.enabled || cfg.url.trim().is_empty() {
        return Err(KshanaError::Unsupported(format!(
            "online refresh is off (set enabled = true and url in {})",
            store.refresh_path().display()
        )));
    }
    let base = cfg.url.trim().trim_end_matches('/');
    let mut out = RefreshOutcome::default();
    let before = store.revocations(ring).map(|l| l.sequence);
    if let Some(text) = fetcher
        .get(&format!("{base}/revocations.txt"))
        .map_err(KshanaError::InvalidInput)?
    {
        let l = store.install_revocations(ring, &text)?;
        if Some(l.sequence) != before {
            out.revocations = Some(l.sequence);
        }
    }
    let current = store.status(ring, &[], opts);
    if let Some(lic) = &current.licence {
        if let Some(text) = fetcher
            .get(&format!("{base}/token/{}", lic.token_id))
            .map_err(KshanaError::InvalidInput)?
        {
            let text = text.trim();
            if text != store.token().unwrap_or_default() {
                // Check before writing: only a valid token for the same licensee counts
                // as a renewal; anything else leaves the stored token as it was.
                let lists: Vec<_> = store.revocations(ring).into_iter().collect();
                let st = evaluate(ring, Some(text), &lists, opts);
                if st.licence.as_ref().map(|l| &l.claims.licensee) != Some(&lic.claims.licensee) {
                    return Err(KshanaError::InvalidInput(
                        "refresh returned a token for another licensee or one that does not verify; kept the stored token".into(),
                    ));
                }
                store.activate(ring, text, opts)?;
                out.renewed = true;
            }
        }
    }
    Ok(out)
}

/// Write a file readable by its owner only, atomically (temporary file and rename).
fn write_private(path: &Path, text: &str) -> Result<(), KshanaError> {
    let err = |e: std::io::Error| {
        KshanaError::InvalidInput(format!("could not write {}: {e}", path.display()))
    };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(err)?;
    }
    let tmp = path.with_extension(format!("tmp{}", std::process::id()));
    {
        use std::io::Write;
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut f = opts.open(&tmp).map_err(err)?;
        f.write_all(text.as_bytes()).map_err(err)?;
        f.write_all(b"\n").map_err(err)?;
        f.sync_all().map_err(err)?;
    }
    std::fs::rename(&tmp, path).map_err(err)
}

/// The products a status reports as covered.
pub fn products_covered(lic: &Licence) -> Vec<&'static str> {
    Product::ALL
        .iter()
        .filter(|p| {
            lic.claims.products.is_empty() || lic.claims.products.iter().any(|c| c == p.as_str())
        })
        .map(|p| p.as_str())
        .collect()
}
