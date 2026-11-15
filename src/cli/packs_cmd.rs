// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro packs ...` (roadmap 0.42 C10): build a pack payload into the catalogue
//! (release pipeline), issue a pack for an air-gapped customer, serve the licence and
//! pack endpoints (vendor side); install a pack and show its state (app side).

use std::path::PathBuf;
use std::process::ExitCode;

use kshana::api::KshanaError;

use super::{flag, now_unix, signing_seed};
use crate::license::lifecycle::LicenceService;
use crate::license::{self, KEY_LEN};
use crate::propack::issuer::{Catalog, PackIssuer};
use crate::propack::loader::{self, LoadContext, PackStore};
use crate::propack::{format, server};

const USAGE: &str = "usage: kshana-pro packs <command>\n\
  vendor (release pipeline and licence server):\n\
    build --from <dir> --catalog <dir> --pack-id <id> --version <v> --platform <p> --provides <kind,..> [--apple-team-id <id>]\n\
    issue --catalog <dir> --data <dir> --token <device token> --device <hash> --pack-id <id> --platform <p> [--version <v>] --out <file>\n\
    serve --catalog <dir> --data <dir> [--addr <host:port>]   /v1/licence/activate, /v1/licence/check, /v1/revocations, /v1/packs/fetch\n\
    trace <decrypted payload file>          Who a leaked payload was issued to (checks the watermark)\n\
  app side:\n\
    install <file> --secret <hex> --app-data <dir>\n\
    status --app-data <dir> [--device <hash>]\n\
  Keys from the environment only: KSHANA_PRO_SIGNING_KEY (licences), KSHANA_PRO_PACK_SIGNING_KEY,\n\
  KSHANA_PRO_PACK_MASTER_SECRET (64 hex each). Platforms: macos-aarch64, macos-x86_64, windows-x86_64,\n\
  windows-aarch64, linux-x86_64, linux-aarch64, any.";

fn env_key(name: &str) -> Result<[u8; KEY_LEN], KshanaError> {
    let v = std::env::var(name)
        .map_err(|_| KshanaError::InvalidInput(format!("set {name} (64 hex characters)")))?;
    license::parse_key_hex(&v)
}

fn need(args: &[String], name: &str) -> Result<String, KshanaError> {
    flag(args, name).ok_or_else(|| KshanaError::InvalidInput(format!("needs {name}")))
}

fn issuer(args: &[String]) -> Result<(LicenceService, PackIssuer), KshanaError> {
    let data = PathBuf::from(need(args, "--data")?);
    let licences = LicenceService::open(signing_seed()?, &[], &data);
    let packs = PackIssuer::new(
        Catalog::open(need(args, "--catalog")?),
        env_key("KSHANA_PRO_PACK_MASTER_SECRET")?,
        env_key("KSHANA_PRO_PACK_SIGNING_KEY")?,
    );
    Ok((licences, packs))
}

pub fn run(args: &[String]) -> ExitCode {
    let sub = args.first().map(String::as_str);
    let rest = if args.is_empty() { args } else { &args[1..] };
    let r = match sub {
        Some("build") => build(rest),
        Some("issue") => issue(rest),
        Some("serve") => serve(rest),
        Some("trace") => trace(rest),
        Some("install") => install(rest),
        Some("status") => status(rest),
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::FAILURE;
        }
    };
    match r {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn build(args: &[String]) -> Result<(), KshanaError> {
    let provides: Vec<String> = need(args, "--provides")?
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let e = Catalog::open(need(args, "--catalog")?).build_from_dir(
        &PathBuf::from(need(args, "--from")?),
        &need(args, "--pack-id")?,
        &need(args, "--version")?,
        &need(args, "--platform")?,
        &provides,
        flag(args, "--apple-team-id").as_deref(),
    )?;
    println!("{}", serde_json::to_string_pretty(&e).unwrap_or_default());
    Ok(())
}

fn issue(args: &[String]) -> Result<(), KshanaError> {
    let (licences, packs) = issuer(args)?;
    let p = packs.issue(
        &licences,
        &need(args, "--token")?,
        &need(args, "--device")?,
        &need(args, "--pack-id")?,
        &need(args, "--platform")?,
        flag(args, "--version").as_deref(),
        now_unix(),
    )?;
    let out = need(args, "--out")?;
    std::fs::write(&out, &p.file).map_err(|e| KshanaError::InvalidInput(format!("{out}: {e}")))?;
    eprintln!(
        "[pack {} {} for {} written to {out}; give the customer the file and the secret below]",
        p.manifest.pack_id, p.manifest.version, p.manifest.licensee
    );
    println!("{}", p.licence_secret_hex);
    Ok(())
}

fn serve(args: &[String]) -> Result<(), KshanaError> {
    let (licences, packs) = issuer(args)?;
    let addr = flag(args, "--addr").unwrap_or_else(|| "127.0.0.1:8091".into());
    let l = server::start(server::Service { licences, packs }, &addr)
        .map_err(|e| KshanaError::InvalidInput(e.to_string()))?;
    eprintln!("licence and pack service listening on http://{}", l.addr);
    l.wait();
    Ok(())
}

fn trace(args: &[String]) -> Result<(), KshanaError> {
    let path = args
        .first()
        .ok_or_else(|| KshanaError::InvalidInput("trace needs a payload file".into()))?;
    let bytes = std::fs::read(path).map_err(|e| KshanaError::InvalidInput(e.to_string()))?;
    let key = format::watermark_key(&env_key("KSHANA_PRO_PACK_MASTER_SECRET")?);
    let w = format::trace(&bytes, &key)?;
    println!("{}", serde_json::to_string_pretty(&w).unwrap_or_default());
    Ok(())
}

fn install(args: &[String]) -> Result<(), KshanaError> {
    let file = args
        .first()
        .filter(|a| !a.starts_with("--"))
        .ok_or_else(|| KshanaError::InvalidInput("install needs a pack file".into()))?;
    let bytes = std::fs::read(file).map_err(|e| KshanaError::InvalidInput(e.to_string()))?;
    let store = PackStore::in_app_data(&PathBuf::from(need(args, "--app-data")?));
    let m = store.install(
        &bytes,
        &need(args, "--secret")?,
        &loader::embedded_pack_keys(),
    )?;
    println!("installed {} {} ({})", m.pack_id, m.version, m.platform);
    Ok(())
}

fn status(args: &[String]) -> Result<(), KshanaError> {
    let store = PackStore::in_app_data(&PathBuf::from(need(args, "--app-data")?));
    let device = flag(args, "--device")
        .or_else(license::machine::this_machine_hash)
        .unwrap_or_default();
    let keys = loader::embedded_pack_keys();
    let ctx = LoadContext {
        keys: &keys,
        device: &device,
        licence: None,
        revocations: &[],
        now: now_unix(),
    };
    for id in store.installed() {
        match store.phase(&id, &ctx) {
            Ok((m, p)) => println!("{id} {} {}: {p:?}", m.version, m.platform),
            Err(e) => println!("{id}: {e}"),
        }
    }
    Ok(())
}
