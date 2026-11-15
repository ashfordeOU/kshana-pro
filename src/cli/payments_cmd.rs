// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro payments ...` (roadmap F10g): `check` a configuration, open a
//! `checkout` (the paywall), `serve` the webhook listener. All of it is inert while
//! `enabled = false`.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::payments::paywall::{self, Paywall};
use crate::payments::webhook::Issuer;
use crate::payments::{server, PaymentsConfig};

use super::flag;

const USAGE: &str = "usage: kshana-pro payments <command> --config <payments.toml>\n\
    check                                       Validate; list what is still PLACEHOLDER\n\
    checkout --edition <id> [--seats <n>] [--licensee <name>] [--email <addr>]   The checkout URL (nothing when payments are off)\n\
    stripe-checkout --edition <id> [--seats <n>] [--licensee <name>] [--email <addr>]   A Stripe checkout session URL (refused while payments.stripe.enabled = false; STRIPE_SECRET_KEY from the environment)\n\
    serve [--addr <host:port>]                  The webhook listener (vendor side; refuses to start when off)";

fn config_path(args: &[String]) -> PathBuf {
    flag(args, "--config")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("commercial/payments.toml"))
}

pub fn run(args: &[String]) -> ExitCode {
    let sub = args.first().map(String::as_str);
    let path = config_path(args);
    let base = path.parent().map(Path::to_path_buf).unwrap_or_default();
    let cfg = match PaymentsConfig::load(&path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };
    match sub {
        Some("check") => {
            println!(
                "payments: {} (provider {}, mode {:?}), {} edition(s)",
                if cfg.enabled { "ON" } else { "off" },
                cfg.provider.as_str(),
                cfg.mode,
                cfg.editions.len()
            );
            let open = cfg.placeholders();
            if open.is_empty() {
                println!("every price and provider ID is set");
            } else {
                println!("still PLACEHOLDER (owner gate 12): {}", open.join(", "));
            }
            ExitCode::SUCCESS
        }
        Some("checkout") => {
            let Some(ed) = flag(args, "--edition") else {
                eprintln!("{USAGE}");
                return ExitCode::FAILURE;
            };
            let seats = flag(args, "--seats")
                .and_then(|s| s.parse().ok())
                .unwrap_or(1);
            match paywall::checkout(
                Some(&cfg),
                &ed,
                seats,
                &flag(args, "--licensee").unwrap_or_default(),
                &flag(args, "--email").unwrap_or_default(),
            ) {
                Ok(Paywall::Off) => {
                    println!("payments are off: no checkout");
                    ExitCode::SUCCESS
                }
                Ok(Paywall::Checkout {
                    url, name, price, ..
                }) => {
                    eprintln!("[{name}: {price}]");
                    println!("{url}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("stripe-checkout") => {
            let Some(ed) = flag(args, "--edition") else {
                eprintln!("{USAGE}");
                return ExitCode::FAILURE;
            };
            let seats = flag(args, "--seats")
                .and_then(|s| s.parse().ok())
                .unwrap_or(1);
            match crate::payments::stripe::create_checkout_session(
                &cfg,
                &crate::payments::stripe_transport::CurlStripe,
                &|k| std::env::var(k).ok(),
                &ed,
                seats,
                &flag(args, "--licensee").unwrap_or_default(),
                &flag(args, "--email").unwrap_or_default(),
            ) {
                Ok(s) => {
                    eprintln!("[Stripe checkout session {}]", s.id);
                    println!("{}", s.url);
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("serve") => {
            let addr = flag(args, "--addr").unwrap_or_else(|| "127.0.0.1:8090".into());
            let issuer = match Issuer::open(cfg, &base) {
                Ok(i) => i,
                Err(e) => {
                    eprintln!("error: {e}");
                    return ExitCode::FAILURE;
                }
            };
            match server::start(issuer, &addr) {
                Ok(l) => {
                    eprintln!("payments webhook listening on http://{}", l.addr);
                    l.wait();
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        _ => {
            eprintln!("{USAGE}");
            ExitCode::FAILURE
        }
    }
}
