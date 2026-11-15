// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Vendor side of Pro pack delivery: the catalogue the release pipeline fills
//! ([`Catalog::build_from_dir`], `kshana-pro packs build`) and issuing one licensee's pack
//! for one device ([`PackIssuer::issue`], the `/v1/packs/fetch` endpoint and `kshana-pro
//! packs issue` for an air-gapped customer).
//!
//! Catalogue layout: `<catalog>/<pack id>/<version>/<platform>.kpc` (the unwatermarked
//! payload container) and `<platform>.json` beside it ([`CatalogEntry`]). The newest
//! version of a pack is the one named in `<catalog>/<pack id>/current`.

use std::path::{Path, PathBuf};

use kshana::api::KshanaError;

use super::format::{self, Container, PackManifest, SealInput, PLATFORMS};
use crate::license::activation::GRACE_DAYS;
use crate::license::lifecycle::LicenceService;
use crate::license::{verify_with, VerifyOptions, KEY_LEN};

fn bad(m: impl Into<String>) -> KshanaError {
    KshanaError::InvalidInput(format!("pro pack: {}", m.into()))
}

/// What the catalogue records beside each payload.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CatalogEntry {
    pub pack_id: String,
    pub version: String,
    pub platform: String,
    /// Kinds and features the payload provides.
    pub provides: Vec<String>,
    /// macOS: the Apple team ID the native payload was signed with (pipeline hook).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub apple_team_id: Option<String>,
    /// SHA-256 of the container file, hex.
    pub container_sha256: String,
    pub files: usize,
}

/// The catalogue directory.
#[derive(Debug, Clone)]
pub struct Catalog {
    dir: PathBuf,
}

fn safe_version(v: &str) -> bool {
    (1..=64).contains(&v.len())
        && v.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-' || b == b'+')
        && !v.starts_with('.')
}

impl Catalog {
    pub fn open(dir: impl Into<PathBuf>) -> Catalog {
        Catalog { dir: dir.into() }
    }

    fn base(&self, pack_id: &str, version: &str, platform: &str) -> Result<PathBuf, KshanaError> {
        if !format::valid_pack_id(pack_id)
            || !safe_version(version)
            || !PLATFORMS.contains(&platform)
        {
            return Err(bad("bad pack id, version or platform"));
        }
        Ok(self.dir.join(pack_id).join(version).join(platform))
    }

    /// Build a payload from every regular file under `src` (relative paths become entry
    /// names; symbolic links are refused), record it, and make it the current version.
    pub fn build_from_dir(
        &self,
        src: &Path,
        pack_id: &str,
        version: &str,
        platform: &str,
        provides: &[String],
        apple_team_id: Option<&str>,
    ) -> Result<CatalogEntry, KshanaError> {
        let base = self.base(pack_id, version, platform)?;
        if platform.starts_with("macos") && apple_team_id.is_none() {
            return Err(bad(
                "a macOS payload names the Apple team ID it is signed with (--apple-team-id)",
            ));
        }
        for p in provides {
            if !p.starts_with(crate::registry::PRO_KIND_PREFIX) {
                return Err(bad("every provided id carries the pro- prefix"));
            }
        }
        let mut c = Container::default();
        collect(src, src, &mut c)?;
        if c.entries.is_empty() {
            return Err(bad(format!("{} holds no file", src.display())));
        }
        if c.get(format::WATERMARK_ENTRY).is_some() {
            return Err(bad("the payload may not carry its own watermark entry"));
        }
        c.entries.sort_by(|a, b| a.0.cmp(&b.0));
        let bytes = c.to_bytes();
        let entry = CatalogEntry {
            pack_id: pack_id.into(),
            version: version.into(),
            platform: platform.into(),
            provides: provides.to_vec(),
            apple_team_id: apple_team_id.map(str::to_string),
            container_sha256: {
                use sha2::{Digest, Sha256};
                hex::encode(Sha256::digest(&bytes))
            },
            files: c.entries.len(),
        };
        let io = |e: std::io::Error| bad(format!("{}: {e}", base.display()));
        if let Some(d) = base.parent() {
            std::fs::create_dir_all(d).map_err(io)?;
        }
        std::fs::write(base.with_extension("kpc"), &bytes).map_err(io)?;
        std::fs::write(
            base.with_extension("json"),
            serde_json::to_string_pretty(&entry).unwrap_or_default(),
        )
        .map_err(io)?;
        std::fs::write(self.dir.join(pack_id).join("current"), version).map_err(io)?;
        Ok(entry)
    }

    /// The current version of `pack_id`.
    pub fn current(&self, pack_id: &str) -> Result<String, KshanaError> {
        if !format::valid_pack_id(pack_id) {
            return Err(bad("bad pack id"));
        }
        let v = std::fs::read_to_string(self.dir.join(pack_id).join("current"))
            .map_err(|_| bad(format!("no pack '{pack_id}' in the catalogue")))?;
        Ok(v.trim().to_string())
    }

    /// Load a payload and its record.
    pub fn load(
        &self,
        pack_id: &str,
        version: &str,
        platform: &str,
    ) -> Result<(Container, CatalogEntry), KshanaError> {
        let base = self.base(pack_id, version, platform)?;
        let entry: CatalogEntry = serde_json::from_str(
            &std::fs::read_to_string(base.with_extension("json"))
                .map_err(|_| bad(format!("no {pack_id} {version} for {platform}")))?,
        )
        .map_err(|e| bad(e.to_string()))?;
        let bytes = std::fs::read(base.with_extension("kpc")).map_err(|e| bad(e.to_string()))?;
        use sha2::{Digest, Sha256};
        if hex::encode(Sha256::digest(&bytes)) != entry.container_sha256 {
            return Err(bad("catalogue payload changed since it was built"));
        }
        Ok((Container::from_bytes(&bytes)?, entry))
    }
}

fn collect(root: &Path, dir: &Path, c: &mut Container) -> Result<(), KshanaError> {
    let mut items: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| bad(format!("{}: {e}", dir.display())))?
        .filter_map(Result::ok)
        .collect();
    items.sort_by_key(|e| e.path());
    for e in items {
        let p = e.path();
        let md = std::fs::symlink_metadata(&p).map_err(|e| bad(e.to_string()))?;
        if md.file_type().is_symlink() {
            return Err(bad(format!("{} is a symbolic link", p.display())));
        }
        if md.is_dir() {
            collect(root, &p, c)?;
        } else if md.is_file() {
            let rel = p
                .strip_prefix(root)
                .map_err(|_| bad("path outside the payload"))?
                .components()
                .map(|c| c.as_os_str().to_string_lossy().to_string())
                .collect::<Vec<_>>()
                .join("/");
            c.put(&rel, std::fs::read(&p).map_err(|e| bad(e.to_string()))?)?;
        }
    }
    Ok(())
}

/// A pack issued for one licensee and device.
#[derive(Debug, Clone)]
pub struct IssuedPack {
    pub file: Vec<u8>,
    pub manifest: PackManifest,
    /// The licence secret the app needs to open the pack (hex); sent with the pack.
    pub licence_secret_hex: String,
}

/// The vendor's pack issuer.
pub struct PackIssuer {
    pub catalog: Catalog,
    master: [u8; 32],
    pack_signer: crate::license::signer::SharedSigner,
}

impl PackIssuer {
    /// `master` is the pack master secret (from the environment, never in a file in the
    /// repository); `pack_seed` the pack signing key.
    pub fn new(catalog: Catalog, master: [u8; 32], pack_seed: [u8; KEY_LEN]) -> PackIssuer {
        PackIssuer::with_signer(catalog, master, std::sync::Arc::new(pack_seed))
    }

    /// A pack issuer signing through any licence signer (a seed or AWS KMS, S7-a).
    pub fn with_signer(
        catalog: Catalog,
        master: [u8; 32],
        pack_signer: crate::license::signer::SharedSigner,
    ) -> PackIssuer {
        PackIssuer {
            catalog,
            master,
            pack_signer,
        }
    }

    /// The watermark key, to trace a leaked payload ([`format::trace`]).
    pub fn watermark_key(&self) -> [u8; 32] {
        format::watermark_key(&self.master)
    }

    /// Issue `pack_id` (the current version unless `version` is given) for `platform` to
    /// the holder of `token` on `device`. The token must verify, be bound to `device` (a
    /// seat activated on it), be unexpired and unrevoked, and its licence must be paid.
    #[allow(clippy::too_many_arguments)]
    pub fn issue(
        &self,
        licences: &LicenceService,
        token: &str,
        device: &str,
        pack_id: &str,
        platform: &str,
        version: Option<&str>,
        now: i64,
    ) -> Result<IssuedPack, KshanaError> {
        let dev = device.to_string();
        let here = move || Some(dev.clone());
        let lic = verify_with(
            licences.keyring(),
            token,
            &VerifyOptions {
                now_unix: now,
                product: Some(crate::license::Product::Desktop),
                machine: Some(&here),
                allow_expired: false,
            },
        )?;
        if lic.claims.machine.as_deref() != Some(device) {
            return Err(bad(
                "the token is not bound to this device; activate a seat on it first",
            ));
        }
        if lic.claims.expires_unix == 0 {
            return Err(bad("a pack is issued only for a licence with an expiry"));
        }
        if licences.register.is_revoked(&lic.token_id)? {
            return Err(bad("this licence has been revoked"));
        }
        if let Some(root) = licences.ledger.root_of(&lic.token_id)? {
            let rec = licences.ledger.load()?.remove(&root);
            if rec.is_some_and(|r| r.revoked || r.paid_through_unix <= now) {
                return Err(bad("this licence has ended"));
            }
        }
        let version = match version {
            Some(v) => v.to_string(),
            None => self.catalog.current(pack_id)?,
        };
        let (base, entry) = self.catalog.load(pack_id, &version, platform)?;
        let (file, manifest) = format::seal(
            &base,
            &SealInput {
                pack_id,
                version: &version,
                platform,
                licensee: &lic.claims.licensee,
                token_id: &lic.token_id,
                device,
                issued_unix: now,
                expires_unix: lic.claims.expires_unix,
                grace_days: GRACE_DAYS,
                provides: entry.provides.clone(),
                apple_team_id: entry.apple_team_id.clone(),
            },
            &self.master,
            self.pack_signer.as_ref(),
        )?;
        Ok(IssuedPack {
            file,
            manifest,
            licence_secret_hex: hex::encode(format::licence_secret(&self.master, &lic.token_id)),
        })
    }
}
