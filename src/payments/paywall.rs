// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The paywall: what an "Upgrade" button (Desktop, the site's pricing page,
//! `kshana-pro payments checkout`) opens. With payments off it returns
//! [`Paywall::Off`], and callers show nothing. It builds a URL only: no network call is
//! made here or anywhere in [`super`] except by the provider calling the webhook.

use kshana::api::KshanaError;

use super::config::{PaymentsConfig, Provider, PLACEHOLDER};

/// What the paywall offers.
#[derive(Debug, Clone, PartialEq)]
pub enum Paywall {
    /// Payments are off (or no configuration): show no checkout, no prices.
    Off,
    /// A checkout to open in the browser.
    Checkout {
        provider: Provider,
        edition: String,
        name: String,
        price: String,
        url: String,
    },
}

/// Percent-encode a query value (RFC 3986 unreserved characters kept).
pub fn encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// The checkout for `edition_id` and `seats`, carrying the licensee and e-mail as
/// custom data so the webhook can name the licence.
pub fn checkout(
    cfg: Option<&PaymentsConfig>,
    edition_id: &str,
    seats: u32,
    licensee: &str,
    email: &str,
) -> Result<Paywall, KshanaError> {
    let Some(cfg) = cfg.filter(|c| c.enabled) else {
        return Ok(Paywall::Off);
    };
    let ed = cfg.edition(edition_id).ok_or_else(|| {
        KshanaError::InvalidInput(format!("no edition '{edition_id}' in payments.toml"))
    })?;
    if ed.provider_price_id.trim() == PLACEHOLDER || ed.provider_price_id.trim().is_empty() {
        return Err(KshanaError::Unsupported(format!(
            "edition '{edition_id}' is not on sale yet (provider price ID is PLACEHOLDER)"
        )));
    }
    let seats = seats.clamp(1, 10_000);
    let url = match cfg.provider {
        Provider::LemonSqueezy => {
            let store = cfg.checkout.lemonsqueezy_store.trim();
            if store.is_empty() || store == PLACEHOLDER {
                return Err(KshanaError::Unsupported(
                    "checkout.lemonsqueezy_store is not set".into(),
                ));
            }
            format!(
                "https://{}.lemonsqueezy.com/checkout/buy/{}?quantity={seats}&checkout%5Bemail%5D={}&checkout%5Bcustom%5D%5Blicensee%5D={}",
                encode(store),
                encode(&ed.provider_price_id),
                encode(email),
                encode(licensee)
            )
        }
        Provider::Paddle => {
            let page = cfg.checkout.site_checkout_url.trim();
            if !page.starts_with("https://") {
                return Err(KshanaError::Unsupported(
                    "checkout.site_checkout_url (an https:// page that opens Paddle.js) is not set"
                        .into(),
                ));
            }
            format!(
                "{page}?price={}&quantity={seats}&licensee={}&email={}",
                encode(&ed.provider_price_id),
                encode(licensee),
                encode(email)
            )
        }
    };
    Ok(Paywall::Checkout {
        provider: cfg.provider,
        edition: ed.id.clone(),
        name: ed.name.clone(),
        price: ed.price.clone(),
        url,
    })
}
