// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Payments configuration (`payments.toml`): the one switch, the provider, the
//! editions and their prices. Off by default; prices stay `PLACEHOLDER` until the owner
//! sets them (owner gate 12).

use std::path::{Path, PathBuf};

use kshana::api::KshanaError;

use crate::license::{Product, Tier};

/// The marker for a value the owner has not set yet.
pub const PLACEHOLDER: &str = "PLACEHOLDER";

/// The merchant of record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Paddle,
    #[serde(rename = "lemonsqueezy")]
    LemonSqueezy,
}

impl Provider {
    pub fn as_str(self) -> &'static str {
        match self {
            Provider::Paddle => "paddle",
            Provider::LemonSqueezy => "lemonsqueezy",
        }
    }
    pub fn parse(s: &str) -> Option<Provider> {
        match s {
            "paddle" => Some(Provider::Paddle),
            "lemonsqueezy" | "lemon-squeezy" => Some(Provider::LemonSqueezy),
            _ => None,
        }
    }
}

/// Test (the provider's sandbox) or live.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    #[default]
    Test,
    Live,
}

/// One edition on sale.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct Edition {
    /// Our ID, e.g. `pro-desktop-annual`.
    pub id: String,
    /// What the buyer sees.
    pub name: String,
    pub tier: String,
    pub products: Vec<String>,
    /// Days each paid period gives (365 for annual).
    pub days: i64,
    /// Display price, e.g. `EUR 1,000 / year`. `PLACEHOLDER` until the owner sets it.
    pub price: String,
    /// The provider's price ID (Paddle `pri_…`) or variant ID (Lemon Squeezy).
    /// `PLACEHOLDER` until the provider account exists.
    pub provider_price_id: String,
    /// Stations the licence allows (0: not set).
    #[serde(default)]
    pub stations: u32,
}

/// `[checkout]`: how the paywall opens a checkout.
#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
pub struct Checkout {
    /// Paddle: the client-side token for Paddle.js (`test_…` or `live_…`).
    #[serde(default)]
    pub paddle_client_token: String,
    /// Paddle: the page on the site that opens Paddle.js with the edition's price.
    #[serde(default)]
    pub site_checkout_url: String,
    /// Lemon Squeezy: the store subdomain (`<store>.lemonsqueezy.com`).
    #[serde(default)]
    pub lemonsqueezy_store: String,
}

/// The whole `payments.toml`.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct PaymentsConfig {
    /// The one switch. Off: no checkout is shown and nothing calls out.
    #[serde(default)]
    pub enabled: bool,
    pub provider: Provider,
    #[serde(default)]
    pub mode: Mode,
    /// File holding the provider's webhook signing secret (never the secret itself).
    #[serde(default)]
    pub webhook_secret_file: String,
    /// File holding the payments signing key (64 hex), the service's own key
    /// (docs/LICENSING-KEYS.md section 4).
    #[serde(default)]
    pub signing_key_file: String,
    /// Directory for the register, the webhook state and the outbox.
    #[serde(default)]
    pub data_dir: String,
    /// Paddle: how old a signed timestamp may be, seconds (default 300).
    #[serde(default = "default_tolerance")]
    pub signature_tolerance_secs: i64,
    #[serde(default)]
    pub checkout: Checkout,
    #[serde(default)]
    pub editions: Vec<Edition>,
}

fn default_tolerance() -> i64 {
    300
}

impl PaymentsConfig {
    /// Parse and check a configuration.
    pub fn parse(text: &str) -> Result<PaymentsConfig, KshanaError> {
        let c: PaymentsConfig = toml::from_str(text)
            .map_err(|e| KshanaError::InvalidInput(format!("payments.toml: {e}")))?;
        c.check()?;
        Ok(c)
    }

    /// Read a configuration file.
    pub fn load(path: &Path) -> Result<PaymentsConfig, KshanaError> {
        let text = std::fs::read_to_string(path).map_err(|e| {
            KshanaError::InvalidInput(format!("could not read {}: {e}", path.display()))
        })?;
        PaymentsConfig::parse(&text)
    }

    /// Structural checks, and the live-mode gate: live mode needs every price and
    /// provider ID set (no `PLACEHOLDER` anywhere).
    pub fn check(&self) -> Result<(), KshanaError> {
        let mut ids = std::collections::BTreeSet::new();
        for e in &self.editions {
            if !ids.insert(e.id.as_str()) {
                return Err(KshanaError::InvalidInput(format!(
                    "payments.toml: edition '{}' appears twice",
                    e.id
                )));
            }
            Tier::parse(&e.tier)?;
            if e.products.is_empty() {
                return Err(KshanaError::InvalidInput(format!(
                    "payments.toml: edition '{}' names no product",
                    e.id
                )));
            }
            for p in &e.products {
                Product::parse(p)?;
            }
            if e.days <= 0 || e.days > 3660 {
                return Err(KshanaError::InvalidInput(format!(
                    "payments.toml: edition '{}' days must be 1 to 3660",
                    e.id
                )));
            }
        }
        if self.mode == Mode::Live {
            let open: Vec<String> = self.placeholders();
            if !open.is_empty() {
                return Err(KshanaError::InvalidInput(format!(
                    "payments.toml: live mode needs every price and provider ID set by the owner; still PLACEHOLDER: {}",
                    open.join(", ")
                )));
            }
        }
        Ok(())
    }

    /// Every field still `PLACEHOLDER`.
    pub fn placeholders(&self) -> Vec<String> {
        let mut v = Vec::new();
        for e in &self.editions {
            if e.price.trim() == PLACEHOLDER || e.price.trim().is_empty() {
                v.push(format!("{}.price", e.id));
            }
            if e.provider_price_id.trim() == PLACEHOLDER || e.provider_price_id.trim().is_empty() {
                v.push(format!("{}.provider_price_id", e.id));
            }
        }
        v
    }

    /// The edition whose provider price ID is `price_id`.
    pub fn edition_by_price(&self, price_id: &str) -> Option<&Edition> {
        self.editions
            .iter()
            .find(|e| e.provider_price_id == price_id && price_id != PLACEHOLDER)
    }

    /// The edition with our ID `id`.
    pub fn edition(&self, id: &str) -> Option<&Edition> {
        self.editions.iter().find(|e| e.id == id)
    }

    /// The data directory, relative paths taken from `base`.
    pub fn data_dir(&self, base: &Path) -> PathBuf {
        resolve(base, &self.data_dir)
    }

    /// The webhook secret, read from its file.
    pub fn webhook_secret(&self, base: &Path) -> Result<String, KshanaError> {
        read_secret(&resolve(base, &self.webhook_secret_file), "webhook secret")
    }

    /// The payments signing seed, read from its file.
    pub fn signing_seed(&self, base: &Path) -> Result<[u8; crate::license::KEY_LEN], KshanaError> {
        crate::license::parse_key_hex(&read_secret(
            &resolve(base, &self.signing_key_file),
            "payments signing key",
        )?)
    }
}

fn resolve(base: &Path, p: &str) -> PathBuf {
    let p = PathBuf::from(p);
    if p.is_absolute() {
        p
    } else {
        base.join(p)
    }
}

fn read_secret(path: &Path, what: &str) -> Result<String, KshanaError> {
    let s = std::fs::read_to_string(path).map_err(|e| {
        KshanaError::InvalidInput(format!(
            "could not read the {what} file {}: {e}",
            path.display()
        ))
    })?;
    let s = s.trim().to_string();
    if s.is_empty() {
        return Err(KshanaError::InvalidInput(format!(
            "the {what} file {} is empty",
            path.display()
        )));
    }
    Ok(s)
}
