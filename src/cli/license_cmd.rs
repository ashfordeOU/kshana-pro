// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro license ...`: customer commands (verify, activate, status, machine-id,
//! revocations install, refresh) and vendor commands (keygen, pubkey, issue, trial,
//! renew, revoke, revocations build, reminders, register verify). Roadmap F10b–F10d.

use std::process::ExitCode;

use crate::license::activation::{self, CurlFetcher, State, Store};
use crate::license::lifecycle::{Delivery, LicenceService};
use crate::license::register::Register;
use crate::license::{
    self, machine, Entitlement, Keyring, LicenseClaims, Product, Tier, VerifyOptions,
};
use kshana::api::KshanaError;

use super::{flag, has_flag, now_unix, print_entitlement, signing_seed};

const USAGE: &str = "usage: kshana-pro license <command>\n\
  customer:\n\
    verify <token> [--product <p>]          Check a token (exit 1 when invalid)\n\
    activate <token> | --file <path>        Verify and store a token (offline) [--product <p>]\n\
    status [--product <p>] [--json]         The stored licence: state, tier, expiry, warnings\n\
    deactivate                              Remove the stored token\n\
    machine-id                              This machine's hash, for a machine-bound token\n\
    revocations install <file>              Install a signed revocation list (from an update)\n\
    refresh                                 Opt-in online refresh (off unless refresh.toml enables it)\n\
  vendor:\n\
    keygen | pubkey                         New key pair | public key and key ID of $KSHANA_PRO_SIGNING_KEY\n\
    issue --tier <t> --licensee <n> [--days <n>] [--product <p,..>] [--seats <n>] [--stations <n>]\n\
          [--machine <hash>] [--not-before <unix>] [--feature <kind,..>] [--register <file>] [--note <s>] [--v1]\n\
    issue-invoice --licensee <n> --tier <t> --invoice <ref> --data <dir> (--term-days <n> | --until <unix>)\n\
          [--product <p,..>] [--seats <n>] [--stations <n>] [--feature <kind,..>] [--offline]\n\
                                            Licence for an invoiced customer (C7): online 35-day tokens renewed\n\
                                            automatically until the term end, or --offline one token for the term (C2)\n\
    extend-invoice --licence <root id> --until <unix> --invoice <ref> --data <dir>   A new invoice paid: extend the term\n\
    trial --licensee <n> --product <p,..> --register <file>       30-day Pro trial\n\
    renew <token-id> --days <n> --register <file>\n\
    revoke <token-id> --reason <s> --register <file>\n\
    revocations build --register <file> --out <file> [--retire <kid,..>]\n\
    reminders --register <file> [--within <days>] [--json]\n\
    register verify <file>\n\
  The store is $KSHANA_PRO_HOME or the per-user configuration directory; --store <dir> overrides.\n\
  Products: cli, desktop, station, service.";

/// The keyring this build verifies against: the keys compiled in, or in a debug build
/// without any, the demo key.
pub fn keyring() -> Result<Keyring, KshanaError> {
    if let Some(k) = Keyring::embedded() {
        return k;
    }
    #[cfg(debug_assertions)]
    {
        let mut ring = Keyring::single(license::public_key_for(&super::DEMO_SEED));
        // The issuer keys of a registered server-side extension (none in Kshana Pro).
        for k in license::embedded_arksa_issuers()? {
            ring.add_arksa_issuer(k);
        }
        Ok(ring)
    }
    #[cfg(not(debug_assertions))]
    Err(KshanaError::Unsupported(
        "this build has no licence public key (build with KSHANA_PRO_LICENSE_PUBKEY set), \
         so it cannot verify tokens; without --license it runs at Evaluation tier"
            .into(),
    ))
}

fn machine_hash() -> Option<String> {
    machine::this_machine_hash()
}

fn opts(now: i64, product: Product) -> VerifyOptions<'static> {
    VerifyOptions {
        now_unix: now,
        product: Some(product),
        machine: Some(&machine_hash),
        allow_expired: false,
    }
}

/// The licence status of the store in `dir` (a Desktop app data folder's `licence`), for
/// loading Pro packs from that app data folder.
pub fn status_in(
    dir: &std::path::Path,
    now: i64,
    product: Product,
) -> Result<crate::license::activation::Status, KshanaError> {
    let ring = keyring()?;
    let (st, _) = Store::open(dir).status_guarded(&ring, &[], &opts(now, product));
    Ok(st)
}

/// Verify a token given with `--license` for `product` at `now`.
pub fn verify_token_for(
    token: &str,
    now: i64,
    product: Product,
) -> Result<Entitlement, KshanaError> {
    let ring = keyring()?;
    let lists: Vec<_> = store(&[])
        .and_then(|s| s.revocations(&ring))
        .into_iter()
        .collect();
    let st = activation::evaluate(&ring, Some(token), &lists, &opts(now, product));
    match st.state {
        State::Active | State::Expiring { .. } | State::Grace { .. } => {
            if let Some(w) = &st.warning {
                eprintln!("[licence] {w}");
            }
            Ok(st.entitlement)
        }
        _ => Err(KshanaError::InvalidInput(
            st.warning.unwrap_or_else(|| st.state.as_str().into()),
        )),
    }
}

/// The `stations` claim of the licence in use (`--license <token>`, else the activated
/// one): 0 when there is none or it carries none.
pub fn stations_claim(token: Option<&str>, now: i64, product: Product) -> u32 {
    let Ok(ring) = keyring() else { return 0 };
    let o = opts(now, product);
    match token {
        Some(t) => license::verify_with(&ring, t, &o)
            .map(|l| l.claims.stations)
            .unwrap_or(0),
        None => store(&[])
            .map(|s| s.status(&ring, &[], &o))
            .and_then(|st| st.licence.map(|l| l.claims.stations))
            .unwrap_or(0),
    }
}

/// With no `--license`: the activated token from the store when it is usable, else
/// Evaluation. Prints one line saying which.
pub fn stored_or_evaluation(now: i64, product: Product) -> Entitlement {
    let (Some(store), Ok(ring)) = (store(&[]), keyring()) else {
        eprintln!("[no license — Evaluation tier; use --license <token> or --dev]");
        return Entitlement::evaluation();
    };
    if store.token().is_none() {
        eprintln!("[no license — Evaluation tier; use --license <token>, `kshana-pro license activate`, or --dev]");
        return Entitlement::evaluation();
    }
    let (st, _) = store.status_guarded(&ring, &[], &opts(now, product));
    match &st.state {
        State::Active | State::Expiring { .. } | State::Grace { .. } => {
            eprintln!(
                "[licence: {} tier for {} (activated)]",
                st.entitlement.tier.as_str(),
                st.entitlement.licensee
            );
        }
        _ => eprintln!("[activated licence not usable — Evaluation tier]"),
    }
    if let Some(w) = &st.warning {
        eprintln!("[licence] {w}");
    }
    st.entitlement
}

fn store(args: &[String]) -> Option<Store> {
    flag(args, "--store")
        .map(std::path::PathBuf::from)
        .or_else(Store::default_dir)
        .map(Store::open)
}

fn product_arg(args: &[String], default: Product) -> Result<Product, KshanaError> {
    flag(args, "--product")
        .map(|p| Product::parse(&p))
        .unwrap_or(Ok(default))
}

fn list_arg(args: &[String], name: &str) -> Vec<String> {
    flag(args, name)
        .map(|v| {
            v.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn num_arg<T: std::str::FromStr>(args: &[String], name: &str) -> Result<Option<T>, KshanaError> {
    match flag(args, name) {
        None => Ok(None),
        Some(v) => v
            .parse()
            .map(Some)
            .map_err(|_| KshanaError::InvalidInput(format!("{name} needs a number, got '{v}'"))),
    }
}

fn register_arg(args: &[String]) -> Result<Register, KshanaError> {
    flag(args, "--register")
        .map(Register::open)
        .ok_or_else(|| KshanaError::InvalidInput("needs --register <file>".into()))
}

fn finish(r: Result<(), KshanaError>) -> ExitCode {
    match r {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// `kshana-pro license ...`.
pub fn run(args: &[String]) -> ExitCode {
    let sub = args.first().map(String::as_str);
    let rest = if args.is_empty() { args } else { &args[1..] };
    finish(match sub {
        Some("issue") => issue(rest),
        Some("verify") => verify(rest),
        Some("keygen") => keygen(),
        Some("pubkey") => pubkey(),
        Some("machine-id") => match machine::this_machine_hash() {
            Some(h) => {
                println!("{h}");
                Ok(())
            }
            None => Err(KshanaError::Unsupported(
                "this machine's hardware ID could not be read; set KSHANA_MACHINE_ID".into(),
            )),
        },
        Some("activate") => activate(rest),
        Some("status") => status(rest),
        Some("deactivate") => deactivate(rest),
        Some("revocations") => revocations(rest),
        Some("refresh") => refresh(rest),
        Some("trial") => trial(rest),
        Some("issue-invoice") => issue_invoice(rest),
        Some("extend-invoice") => extend_invoice(rest),
        Some("renew") => renew(rest),
        Some("revoke") => revoke(rest),
        Some("reminders") => reminders(rest),
        Some("register") if rest.first().map(String::as_str) == Some("verify") => {
            match rest.get(1) {
                Some(p) => Register::open(p).verify().map(|n| {
                    println!("register chain holds: {n} line(s)");
                }),
                None => Err(KshanaError::InvalidInput(
                    "register verify needs <file>".into(),
                )),
            }
        }
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::FAILURE;
        }
    })
}

fn issue(args: &[String]) -> Result<(), KshanaError> {
    let tier = Tier::parse(&flag(args, "--tier").ok_or_else(|| {
        KshanaError::InvalidInput("license issue needs --tier <evaluation|pro|enterprise>".into())
    })?)?;
    let licensee = flag(args, "--licensee").unwrap_or_else(|| "unnamed".to_string());
    let now = now_unix();
    let days: i64 = num_arg(args, "--days")?.unwrap_or(0);
    let expires_unix = if days > 0 {
        days.checked_mul(86_400)
            .and_then(|s| now.checked_add(s))
            .ok_or_else(|| {
                KshanaError::InvalidInput("--days too large (expiry timestamp overflows)".into())
            })?
    } else {
        0
    };
    let seed = signing_seed()?;
    let v1 = has_flag(args, "--v1");
    let mut products = list_arg(args, "--product");
    if products.is_empty() && !v1 {
        products = Product::ALL
            .iter()
            .map(|p| p.as_str().to_string())
            .collect();
    }
    let claims = LicenseClaims {
        licensee,
        tier: tier.as_str().to_string(),
        expires_unix,
        features: list_arg(args, "--feature"),
        products: if v1 { Vec::new() } else { products },
        seats: num_arg(args, "--seats")?.unwrap_or(0),
        stations: num_arg(args, "--stations")?.unwrap_or(0),
        machine: flag(args, "--machine"),
        not_before_unix: num_arg(args, "--not-before")?.unwrap_or(0),
        ..Default::default()
    };
    let tok = if v1 {
        license::issue(&seed, &claims)?
    } else if let Some(reg) = flag(args, "--register") {
        let note = flag(args, "--note").unwrap_or_default();
        let (t, e) = Register::open(reg).issue(&seed, &claims, "manual", &note, now)?;
        eprintln!("[register: line {} token {}]", e.seq, e.token_id);
        t
    } else {
        let mut c = claims.clone();
        c.issued_unix = now;
        license::issue_v2(&seed, &c)?
    };
    eprintln!(
        "[issued {} license for '{}'{}]",
        tier.as_str(),
        claims.licensee,
        if expires_unix > 0 {
            format!(" (expires in {days}d)")
        } else {
            " (perpetual)".into()
        },
    );
    println!("{tok}");
    Ok(())
}

fn data_service(args: &[String]) -> Result<LicenceService, KshanaError> {
    let dir = flag(args, "--data").ok_or_else(|| {
        KshanaError::InvalidInput("needs --data <dir> (the register and ledger)".into())
    })?;
    Ok(LicenceService::open(
        signing_seed()?,
        &[],
        std::path::Path::new(&dir),
    ))
}

fn issue_invoice(args: &[String]) -> Result<(), KshanaError> {
    let tier = Tier::parse(&flag(args, "--tier").ok_or_else(|| {
        KshanaError::InvalidInput("issue-invoice needs --tier <pro|enterprise>".into())
    })?)?;
    let licensee = flag(args, "--licensee")
        .ok_or_else(|| KshanaError::InvalidInput("issue-invoice needs --licensee".into()))?;
    let invoice = flag(args, "--invoice")
        .ok_or_else(|| KshanaError::InvalidInput("issue-invoice needs --invoice <ref>".into()))?;
    let now = now_unix();
    let until = match (
        num_arg::<i64>(args, "--until")?,
        num_arg::<i64>(args, "--term-days")?,
    ) {
        (Some(u), None) => u,
        (None, Some(d)) if d > 0 && d <= 3660 => now + d * 86_400,
        _ => {
            return Err(KshanaError::InvalidInput(
                "give --term-days <1..3660> or --until <unix>".into(),
            ))
        }
    };
    let mut products = list_arg(args, "--product");
    if products.is_empty() {
        products = vec!["cli".into(), "desktop".into()];
    }
    let claims = LicenseClaims {
        licensee,
        tier: tier.as_str().into(),
        products,
        features: list_arg(args, "--feature"),
        seats: num_arg(args, "--seats")?.unwrap_or(1),
        stations: num_arg(args, "--stations")?.unwrap_or(0),
        ..Default::default()
    };
    let delivery = if has_flag(args, "--offline") {
        Delivery::Offline
    } else {
        Delivery::Online
    };
    let i = data_service(args)?.open_licence(
        &claims,
        delivery,
        until,
        &format!("invoice:{invoice}"),
        &format!("invoice {invoice}"),
        now,
    )?;
    eprintln!(
        "[licence {} for '{}' paid through {until} ({}); first token expires {}]",
        i.root,
        claims.licensee,
        if delivery == Delivery::Offline {
            "offline, one token for the term"
        } else {
            "online, 35-day tokens renewed automatically"
        },
        i.expires_unix
    );
    println!("{}", i.token);
    Ok(())
}

fn extend_invoice(args: &[String]) -> Result<(), KshanaError> {
    let root = flag(args, "--licence").ok_or_else(|| {
        KshanaError::InvalidInput("extend-invoice needs --licence <root id>".into())
    })?;
    let until: i64 = num_arg(args, "--until")?
        .ok_or_else(|| KshanaError::InvalidInput("extend-invoice needs --until <unix>".into()))?;
    let invoice = flag(args, "--invoice")
        .ok_or_else(|| KshanaError::InvalidInput("extend-invoice needs --invoice <ref>".into()))?;
    let p = data_service(args)?.extend(&root, until)?;
    println!("licence {root} paid through {p} (invoice {invoice})");
    Ok(())
}

fn verify(args: &[String]) -> Result<(), KshanaError> {
    let tok = args
        .first()
        .filter(|a| !a.starts_with("--"))
        .ok_or_else(|| KshanaError::InvalidInput("license verify needs a <token>".into()))?;
    let product = product_arg(args, Product::Cli)?;
    let ent = verify_token_for(tok, now_unix(), product)?;
    print_entitlement(&ent);
    Ok(())
}

fn keygen() -> Result<(), KshanaError> {
    // A fresh vendor key pair. The private key goes offline (docs/LICENSING-KEYS.md);
    // the public key is added to KSHANA_PRO_LICENSE_PUBKEY of the next builds.
    use rand::RngCore;
    let mut seed = [0u8; license::KEY_LEN];
    rand::rngs::OsRng.fill_bytes(&mut seed);
    let pk = license::public_key_for(&seed);
    eprintln!("[new key pair: keep the private key offline; it is shown once]");
    println!("KSHANA_PRO_SIGNING_KEY={}", hex::encode(seed));
    println!("KSHANA_PRO_LICENSE_PUBKEY={}", hex::encode(pk));
    println!("KEY_ID={}", license::key_id(&pk));
    Ok(())
}

fn pubkey() -> Result<(), KshanaError> {
    // The public key for KSHANA_PRO_SIGNING_KEY (scripts/release-smoke.sh reads the first
    // line); its key ID on stderr.
    let pk = license::public_key_for(&signing_seed()?);
    println!("{}", hex::encode(pk));
    eprintln!("key id: {}", license::key_id(&pk));
    Ok(())
}

fn need_store(args: &[String]) -> Result<Store, KshanaError> {
    store(args).ok_or_else(|| {
        KshanaError::InvalidInput(
            "no licence store: set KSHANA_PRO_HOME or pass --store <dir>".into(),
        )
    })
}

fn activate(args: &[String]) -> Result<(), KshanaError> {
    let token = match flag(args, "--file") {
        Some(p) => std::fs::read_to_string(&p)
            .map_err(|e| KshanaError::InvalidInput(format!("could not read {p}: {e}")))?,
        None => args
            .first()
            .filter(|a| !a.starts_with("--"))
            .cloned()
            .ok_or_else(|| {
                KshanaError::InvalidInput("license activate needs <token> or --file <path>".into())
            })?,
    };
    let store = need_store(args)?;
    let ring = keyring()?;
    let product = product_arg(args, Product::Cli)?;
    let st = store.activate(&ring, &token, &opts(now_unix(), product))?;
    println!(
        "activated: {} tier for {} ({})",
        st.entitlement.tier.as_str(),
        st.entitlement.licensee,
        store.token_path().display()
    );
    if let Some(w) = st.warning {
        println!("warning: {w}");
    }
    Ok(())
}

fn status(args: &[String]) -> Result<(), KshanaError> {
    let store = need_store(args)?;
    let ring = keyring()?;
    let product = product_arg(args, Product::Cli)?;
    let now = now_unix();
    let (st, _) = store.status_guarded(&ring, &[], &opts(now, product));
    let lic = st.licence.as_ref();
    let revs = store.revocations(&ring).map(|l| l.sequence);
    if has_flag(args, "--json") {
        let v = serde_json::json!({
            "state": st.state.as_str(),
            "tier": st.entitlement.tier.as_str(),
            "licensee": lic.map(|l| l.claims.licensee.clone()),
            "expires_unix": lic.map(|l| l.claims.expires_unix),
            "products": lic.map(activation::products_covered),
            "seats": lic.map(|l| l.claims.seats),
            "stations": lic.map(|l| l.claims.stations),
            "token_id": lic.map(|l| l.token_id.clone()),
            "key_id": lic.map(|l| l.kid.clone()),
            "revocation_list": revs,
            "warning": st.warning,
        });
        println!("{v}");
        return Ok(());
    }
    println!("state:     {}", st.state.as_str());
    println!("tier:      {}", st.entitlement.tier.as_str());
    if let Some(l) = lic {
        println!("licensee:  {}", l.claims.licensee);
        println!(
            "expires:   {}",
            if l.claims.expires_unix == 0 {
                "never (perpetual)".into()
            } else {
                super::utc_from_unix(l.claims.expires_unix)
            }
        );
        println!("products:  {}", activation::products_covered(l).join(", "));
        if l.claims.seats > 0 {
            println!("seats:     {}", l.claims.seats);
        }
        if l.claims.stations > 0 {
            println!("stations:  {}", l.claims.stations);
        }
        println!("token id:  {}", l.token_id);
    }
    println!(
        "revocation list: {}",
        revs.map(|s| format!("sequence {s}"))
            .unwrap_or_else(|| "none installed".into())
    );
    if let Some(w) = st.warning {
        println!("warning:   {w}");
    }
    Ok(())
}

fn deactivate(args: &[String]) -> Result<(), KshanaError> {
    let removed = need_store(args)?.deactivate()?;
    println!(
        "{}",
        if removed {
            "licence removed"
        } else {
            "no licence was stored"
        }
    );
    Ok(())
}

fn revocations(args: &[String]) -> Result<(), KshanaError> {
    match args.first().map(String::as_str) {
        Some("install") => {
            let p = args.get(1).ok_or_else(|| {
                KshanaError::InvalidInput("revocations install needs <file>".into())
            })?;
            let text = std::fs::read_to_string(p)
                .map_err(|e| KshanaError::InvalidInput(format!("could not read {p}: {e}")))?;
            let l = need_store(args)?.install_revocations(&keyring()?, &text)?;
            println!(
                "revocation list sequence {} installed ({} revoked token(s), {} retired key(s))",
                l.sequence,
                l.revoked.len(),
                l.retired_kids.len()
            );
            Ok(())
        }
        Some("build") => {
            let reg = register_arg(args)?;
            let out = flag(args, "--out")
                .ok_or_else(|| KshanaError::InvalidInput("needs --out <file>".into()))?;
            let text =
                reg.revocation_list(&signing_seed()?, &list_arg(args, "--retire"), now_unix())?;
            std::fs::write(&out, format!("{text}\n"))
                .map_err(|e| KshanaError::InvalidInput(format!("could not write {out}: {e}")))?;
            println!("wrote {out}");
            Ok(())
        }
        _ => Err(KshanaError::InvalidInput(
            "license revocations <install <file> | build --register <file> --out <file>>".into(),
        )),
    }
}

fn refresh(args: &[String]) -> Result<(), KshanaError> {
    let store = need_store(args)?;
    let product = product_arg(args, Product::Cli)?;
    let out = activation::refresh(
        &store,
        &keyring()?,
        &CurlFetcher,
        &opts(now_unix(), product),
    )?;
    println!(
        "refresh: revocation list {}; token {}",
        out.revocations
            .map(|s| format!("updated to sequence {s}"))
            .unwrap_or_else(|| "unchanged".into()),
        if out.renewed { "renewed" } else { "unchanged" }
    );
    Ok(())
}

fn trial(args: &[String]) -> Result<(), KshanaError> {
    let licensee = flag(args, "--licensee")
        .ok_or_else(|| KshanaError::InvalidInput("trial needs --licensee <name>".into()))?;
    let products = list_arg(args, "--product");
    let (tok, e) = register_arg(args)?.trial(&signing_seed()?, &licensee, &products, now_unix())?;
    eprintln!(
        "[trial for '{}' until {} — register line {}, token {}]",
        e.licensee,
        super::utc_from_unix(e.expires_unix),
        e.seq,
        e.token_id
    );
    println!("{tok}");
    Ok(())
}

fn renew(args: &[String]) -> Result<(), KshanaError> {
    let id = args
        .first()
        .filter(|a| !a.starts_with("--"))
        .ok_or_else(|| KshanaError::InvalidInput("renew needs <token-id>".into()))?;
    let days = num_arg(args, "--days")?
        .ok_or_else(|| KshanaError::InvalidInput("renew needs --days <n>".into()))?;
    let (tok, e) = register_arg(args)?.renew(&signing_seed()?, id, days, "manual", now_unix())?;
    eprintln!(
        "[renewed {id} as {} until {}]",
        e.token_id,
        super::utc_from_unix(e.expires_unix)
    );
    println!("{tok}");
    Ok(())
}

fn revoke(args: &[String]) -> Result<(), KshanaError> {
    let id = args
        .first()
        .filter(|a| !a.starts_with("--"))
        .ok_or_else(|| KshanaError::InvalidInput("revoke needs <token-id>".into()))?;
    let reason = flag(args, "--reason")
        .ok_or_else(|| KshanaError::InvalidInput("revoke needs --reason <text>".into()))?;
    match register_arg(args)?.revoke(id, &reason, "manual", now_unix())? {
        Some(e) => println!(
            "revoked {id} (register line {}); rebuild the revocation list",
            e.seq
        ),
        None => println!("{id} was already revoked"),
    }
    Ok(())
}

fn reminders(args: &[String]) -> Result<(), KshanaError> {
    let within = num_arg(args, "--within")?.unwrap_or(30);
    let due = register_arg(args)?.reminders(now_unix(), within)?;
    if has_flag(args, "--json") {
        let v: Vec<_> = due
            .iter()
            .map(|e| {
                serde_json::json!({
                    "token_id": e.token_id, "licensee": e.licensee, "tier": e.tier,
                    "expires_unix": e.expires_unix, "products": e.products,
                })
            })
            .collect();
        println!("{}", serde_json::Value::Array(v));
        return Ok(());
    }
    if due.is_empty() {
        println!("no licence expires within {within} day(s)");
    }
    for e in due {
        println!(
            "{}  {}  {}  expires {}",
            e.token_id,
            e.tier,
            e.licensee,
            super::utc_from_unix(e.expires_unix)
        );
    }
    Ok(())
}
