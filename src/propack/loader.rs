// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! App side of Pro pack delivery (roadmap 0.42 C10).
//!
//! The pack store is one directory inside the app's data directory, named
//! [`PACK_DIR_NAME`]. It holds, per pack, `<id>.kpack` (the pack as downloaded),
//! `<id>.key` (the licence secret, readable by the user only), `<id>.state.json` (which
//! notices were shown) and `unpacked/<id>/` (what the host app unpacks from the payload,
//! for example the native library). Nothing else is ever written or deleted.
//!
//! **Opening** ([`PackStore::open`]) refuses, in this order: a bad signature (not our
//! pack key), another device, a revoked licence, an ended licence (expired, or the
//! licence server said it ended), a payload that does not decrypt with this licence on
//! this device, and a payload whose hash differs from the signed manifest.
//!
//! **Expiry** (owner, 15 November 2026). Pro use ends at the licence's expiry, or when
//! the licence server says the licence ended. Notices come 14, 7 and 1 days before. At
//! the end the pack locks at once (it does not open) but stays on disk for the 30-day
//! renewal window, with a notice every day: a renewal in that window unlocks it again
//! with no download, because the renewed licence (same licensee, same device) carries the
//! new expiry. At the window's end [`PackStore::enforce`] deletes the pack's own files.
//! The free edition, and the verification of signed reports and exports, are not touched
//! by any of this.

use std::path::{Path, PathBuf};

use kshana::api::KshanaError;

use super::format::{self, Container, PackManifest};
use crate::extension::PackExtension;
use crate::license::activation::{State, Status, GRACE_DAYS, WARN_DAYS};
use crate::license::clock::ClockGuard;
use crate::license::revocation::RevocationList;
use crate::license::KEY_LEN;

/// The name of the pack directory inside the app data directory.
pub const PACK_DIR_NAME: &str = "pro-packs";
const DAY: i64 = 86_400;

fn bad(m: impl Into<String>) -> KshanaError {
    KshanaError::InvalidInput(format!("pro pack: {}", m.into()))
}

/// The pack signing public keys this build trusts: `KSHANA_PRO_PACK_PUBKEY` at build
/// time (64 hex, several separated by commas). None set: none trusted, every pack refused.
pub fn embedded_pack_keys() -> Vec<[u8; KEY_LEN]> {
    option_env!("KSHANA_PRO_PACK_PUBKEY")
        .unwrap_or("")
        .split(',')
        .filter_map(|k| crate::license::parse_key_hex(k).ok())
        .collect()
}

/// Proof that a pack passed every check; only this module makes one. The extension hook
/// ([`crate::extension::register_pack`]) takes it.
#[derive(Debug)]
pub struct VerifiedPack {
    pack_id: String,
    provides: Vec<String>,
}

impl VerifiedPack {
    pub fn pack_id(&self) -> &str {
        &self.pack_id
    }
    pub fn provides(&self, id: &str) -> bool {
        self.provides.iter().any(|p| p == id)
    }
}

/// A pack that opened.
#[derive(Debug)]
pub struct OpenedPack {
    pub manifest: PackManifest,
    pub payload: Container,
    pub verified: VerifiedPack,
    /// The phase it opened in (active or warning).
    pub phase: Phase,
}

/// Where a pack stands in time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// More than 14 days of Pro use left.
    Active,
    /// 14 days or fewer left; still opens.
    Expiring { days_left: i64 },
    /// Pro use has ended: locked, kept on disk for the renewal window.
    Locked { window_days_left: i64 },
    /// The renewal window is over: the pack is deleted.
    Gone,
    /// Revoked: locked (deleted at the end of the window like any ended pack).
    Revoked { window_days_left: i64 },
}

impl Phase {
    pub fn opens(self) -> bool {
        matches!(self, Phase::Active | Phase::Expiring { .. })
    }
}

/// The phase at `now` for Pro use ending at `ends`.
pub fn phase_at(ends: i64, now: i64) -> Phase {
    if now < ends {
        let days_left = (ends - now + DAY - 1) / DAY;
        if days_left <= WARN_DAYS[0] {
            Phase::Expiring { days_left }
        } else {
            Phase::Active
        }
    } else if now < ends + GRACE_DAYS * DAY {
        Phase::Locked {
            window_days_left: (ends + GRACE_DAYS * DAY - now + DAY - 1) / DAY,
        }
    } else {
        Phase::Gone
    }
}

/// What the app knows about the licence, for the pack checks.
pub struct LoadContext<'a> {
    /// The pack signing public keys ([`embedded_pack_keys`]).
    pub keys: &'a [[u8; KEY_LEN]],
    /// This device's machine hash.
    pub device: &'a str,
    /// The licence status ([`crate::license::activation::Store::status_guarded`]); a
    /// renewed licence for the same licensee and device extends the pack, an ended one
    /// locks it.
    pub licence: Option<&'a Status>,
    /// Installed revocation lists.
    pub revocations: &'a [RevocationList],
    /// The clock (the store's clock guard is applied on top).
    pub now: i64,
}

/// A notice for the user.
#[derive(Debug, Clone, PartialEq)]
pub struct Notice {
    pub pack_id: String,
    pub phase: Phase,
    pub message: String,
}

/// What the host app does with a verified payload: unpack what it needs into
/// `unpack_dir` (inside the pack store) and return the kinds the pack provides. On macOS
/// the host loads only a native library signed with the manifest's Apple team ID (library
/// validation); see docs/PRO-PACKS.md.
pub trait Activator {
    fn activate(&self, pack: &OpenedPack, unpack_dir: &Path) -> Result<PackExtension, KshanaError>;
}

/// The per-pack notice memory.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct NoticeState {
    /// The end the notices below were for (a renewal resets them).
    ends_unix: i64,
    /// The smallest warning threshold already shown (14, 7, 1), or 0 for none.
    last_threshold: i64,
    /// The day number (Unix day) of the last daily notice in the window.
    last_daily: i64,
}

/// The pack store.
#[derive(Debug, Clone)]
pub struct PackStore {
    dir: PathBuf,
}

impl PackStore {
    /// The store inside the app data directory `app_data`.
    pub fn in_app_data(app_data: &Path) -> PackStore {
        PackStore {
            dir: app_data.join(PACK_DIR_NAME),
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn file(&self, id: &str, ext: &str) -> Result<PathBuf, KshanaError> {
        if !format::valid_pack_id(id) {
            return Err(bad("bad pack id"));
        }
        Ok(self.dir.join(format!("{id}.{ext}")))
    }

    /// Where the host unpacks `id`.
    pub fn unpack_dir(&self, id: &str) -> Result<PathBuf, KshanaError> {
        if !format::valid_pack_id(id) {
            return Err(bad("bad pack id"));
        }
        Ok(self.dir.join("unpacked").join(id))
    }

    /// Install a downloaded pack and its licence secret, after checking the signature.
    /// Replaces an older copy of the same pack.
    pub fn install(
        &self,
        file: &[u8],
        licence_secret_hex: &str,
        keys: &[[u8; KEY_LEN]],
    ) -> Result<PackManifest, KshanaError> {
        let sp = format::verify_signed(file, keys)?;
        let secret = hex::decode(licence_secret_hex.trim())
            .ok()
            .filter(|b| b.len() == 32)
            .ok_or_else(|| bad("licence secret is not 32 hex bytes"))?;
        let id = sp.manifest.pack_id.clone();
        std::fs::create_dir_all(&self.dir).map_err(|e| bad(e.to_string()))?;
        write_private(&self.file(&id, "kpack")?, file)?;
        write_private(&self.file(&id, "key")?, hex::encode(secret).as_bytes())?;
        Ok(sp.manifest)
    }

    /// The installed pack ids.
    pub fn installed(&self) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(&self.dir)
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .filter_map(|e| {
                let n = e.file_name().to_string_lossy().to_string();
                let id = n.strip_suffix(".kpack")?.to_string();
                format::valid_pack_id(&id).then_some(id)
            })
            .collect();
        v.sort();
        v
    }

    /// The time to use: the clock, or the latest time seen if it was set back.
    fn now(&self, now: i64) -> i64 {
        ClockGuard::in_dir(&self.dir).check(now).effective_unix
    }

    /// The end of Pro use for `m` given the licence, and whether it is revoked.
    fn ends(&self, m: &PackManifest, ctx: &LoadContext<'_>) -> (i64, bool) {
        let mut revoked = ctx.revocations.iter().any(|l| l.is_revoked(&m.token_id));
        let mut ends = m.expires_unix;
        if let Some(st) = ctx.licence {
            if let Some(lic) = &st.licence {
                let same = format::licensee_id(&lic.claims.licensee) == m.licensee_id
                    && lic.claims.machine.as_deref().is_none_or(|d| d == m.device);
                if same {
                    match st.state {
                        State::Active | State::Expiring { .. } => {
                            ends = ends.max(st.pro_ends_unix);
                        }
                        State::Revoked => revoked = true,
                        State::Grace { .. } | State::Expired => {
                            // Ended (expired, or the server said so): lock now, even when
                            // the pack's own date is later.
                            ends = ends.min(st.pro_ends_unix.max(1));
                        }
                        _ => {}
                    }
                }
            }
        }
        (ends, revoked)
    }

    /// The phase of an installed pack.
    pub fn phase(
        &self,
        id: &str,
        ctx: &LoadContext<'_>,
    ) -> Result<(PackManifest, Phase), KshanaError> {
        let file = std::fs::read(self.file(id, "kpack")?).map_err(|_| bad("not installed"))?;
        let sp = format::verify_signed(&file, ctx.keys)?;
        let now = self.now(ctx.now);
        let (ends, revoked) = self.ends(&sp.manifest, ctx);
        let p = phase_at(ends, now);
        let p = match (revoked, p) {
            (true, Phase::Gone) => Phase::Gone,
            (true, Phase::Locked { window_days_left }) => Phase::Revoked { window_days_left },
            (true, _) => Phase::Revoked {
                window_days_left: GRACE_DAYS,
            },
            (false, p) => p,
        };
        Ok((sp.manifest, p))
    }

    /// Verify and decrypt an installed pack.
    pub fn open(&self, id: &str, ctx: &LoadContext<'_>) -> Result<OpenedPack, KshanaError> {
        let file = std::fs::read(self.file(id, "kpack")?).map_err(|_| bad("not installed"))?;
        let sp = format::verify_signed(&file, ctx.keys)?;
        if sp.manifest.pack_id != id {
            return Err(bad("the file holds another pack"));
        }
        if !sp.manifest.device.eq_ignore_ascii_case(ctx.device) {
            return Err(bad("this pack was issued for another device"));
        }
        let (_, phase) = self.phase(id, ctx)?;
        match phase {
            Phase::Revoked { .. } => return Err(bad("the licence has been revoked; Pro is locked")),
            Phase::Locked { window_days_left } => {
                return Err(bad(format!(
                    "the licence has ended and Pro is locked; renew within {window_days_left} day(s) to unlock it without a new download"
                )))
            }
            Phase::Gone => return Err(bad("the renewal window is over; the pack is removed")),
            _ => {}
        }
        let secret: [u8; 32] = std::fs::read_to_string(self.file(id, "key")?)
            .ok()
            .and_then(|t| hex::decode(t.trim()).ok())
            .and_then(|b| b.try_into().ok())
            .ok_or_else(|| bad("the licence secret is missing"))?;
        let plain = format::open_sealed(&file, &sp, &secret, ctx.device)?;
        let payload = Container::from_bytes(&plain)?;
        Ok(OpenedPack {
            verified: VerifiedPack {
                pack_id: sp.manifest.pack_id.clone(),
                provides: sp.manifest.provides.clone(),
            },
            manifest: sp.manifest,
            payload,
            phase,
        })
    }

    /// Open a pack and load it through the extension hook.
    pub fn load(
        &self,
        id: &str,
        ctx: &LoadContext<'_>,
        activator: &dyn Activator,
    ) -> Result<OpenedPack, KshanaError> {
        let opened = self.open(id, ctx)?;
        let dir = self.unpack_dir(id)?;
        std::fs::create_dir_all(&dir).map_err(|e| bad(e.to_string()))?;
        let ext = activator.activate(&opened, &dir)?;
        crate::extension::register_pack(ext, &opened.verified)?;
        Ok(opened)
    }

    /// Run the expiry rules over every installed pack: return the notices due now (14, 7
    /// and 1 days before the end; daily in the renewal window) and delete each pack whose
    /// renewal window is over. Call at start and about daily.
    pub fn enforce(&self, ctx: &LoadContext<'_>) -> Vec<Notice> {
        let now = self.now(ctx.now);
        let mut out = Vec::new();
        for id in self.installed() {
            let Ok((m, phase)) = self.phase(&id, ctx) else {
                continue;
            };
            let (ends, _) = self.ends(&m, ctx);
            if phase == Phase::Gone {
                let removed = self.delete(&id).is_ok();
                out.push(Notice {
                    pack_id: id.clone(),
                    phase,
                    message: if removed {
                        format!("the renewal window for {id} is over; the Pro pack was removed. Your files, reports and exports are untouched and stay verifiable in the free edition")
                    } else {
                        format!("the renewal window for {id} is over; the Pro pack could not be removed")
                    },
                });
                continue;
            }
            let mut st = self.notice_state(&id);
            if st.ends_unix != ends {
                st = NoticeState {
                    ends_unix: ends,
                    ..Default::default()
                };
            }
            let msg = match phase {
                Phase::Expiring { days_left } => {
                    let t = WARN_DAYS
                        .iter()
                        .copied()
                        .filter(|t| days_left <= *t)
                        .min()
                        .unwrap_or(WARN_DAYS[0]);
                    (st.last_threshold == 0 || t < st.last_threshold).then(|| {
                        st.last_threshold = t;
                        format!("Kshana Pro ends in {days_left} day(s); renew to keep it")
                    })
                }
                Phase::Locked { window_days_left } | Phase::Revoked { window_days_left } => {
                    let day = now.div_euclid(DAY);
                    (st.last_daily != day).then(|| {
                        st.last_daily = day;
                        format!("Kshana Pro is locked; renew within {window_days_left} day(s) or the Pro pack is removed (the free edition keeps working)")
                    })
                }
                _ => None,
            };
            if let Some(message) = msg {
                let _ = self.save_notice_state(&id, &st);
                out.push(Notice {
                    pack_id: id,
                    phase,
                    message,
                });
            }
        }
        out
    }

    fn notice_state(&self, id: &str) -> NoticeState {
        self.file(id, "state.json")
            .ok()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    fn save_notice_state(&self, id: &str, st: &NoticeState) -> Result<(), KshanaError> {
        write_private(
            &self.file(id, "state.json")?,
            serde_json::to_string(st).unwrap_or_default().as_bytes(),
        )
    }

    /// Delete one pack's files: `<id>.kpack`, `<id>.key`, `<id>.state.json` and
    /// `unpacked/<id>/`, all inside the pack store. Refuses to act when the store is not a
    /// directory named [`PACK_DIR_NAME`] or is a symbolic link; never follows a link.
    pub fn delete(&self, id: &str) -> Result<(), KshanaError> {
        if self.dir.file_name().and_then(|n| n.to_str()) != Some(PACK_DIR_NAME) {
            return Err(bad("refusing to delete outside the pack store"));
        }
        let md = std::fs::symlink_metadata(&self.dir).map_err(|e| bad(e.to_string()))?;
        if !md.is_dir() || md.file_type().is_symlink() {
            return Err(bad("the pack store is not a plain directory"));
        }
        for ext in ["kpack", "key", "state.json"] {
            let p = self.file(id, ext)?;
            match std::fs::symlink_metadata(&p) {
                Ok(m) if m.is_file() || m.file_type().is_symlink() => {
                    std::fs::remove_file(&p).map_err(|e| bad(e.to_string()))?
                }
                _ => {}
            }
        }
        let u = self.unpack_dir(id)?;
        match std::fs::symlink_metadata(&u) {
            Ok(m) if m.file_type().is_symlink() => {
                std::fs::remove_file(&u).map_err(|e| bad(e.to_string()))?
            }
            // remove_dir_all does not follow symbolic links inside the tree.
            Ok(m) if m.is_dir() => std::fs::remove_dir_all(&u).map_err(|e| bad(e.to_string()))?,
            _ => {}
        }
        Ok(())
    }
}

fn write_private(path: &Path, data: &[u8]) -> Result<(), KshanaError> {
    let tmp = path.with_extension(format!("tmp{}", std::process::id()));
    std::fs::write(&tmp, data).map_err(|e| bad(format!("{}: {e}", tmp.display())))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
    }
    std::fs::rename(&tmp, path).map_err(|e| bad(format!("{}: {e}", path.display())))
}
