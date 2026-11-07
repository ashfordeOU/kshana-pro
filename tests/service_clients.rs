// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
// Generated service clients (roadmap A5a): the committed clients are what the generator makes
// from the route table today, and both drive a running server end to end: submit a job, wait,
// read the result and an artefact, and get a typed error for a refused call. Python needs
// `python3`; TypeScript needs Node 22.6 or later (it runs the .ts file with type stripping).
// With KSHANA_REQUIRE_CLIENT_RUNTIMES=1 a missing runtime fails the test instead of skipping.

use std::path::Path;
use std::process::Command;

use kshana_pro::license::{Entitlement, Tier};
use kshana_pro::serve::auth::{new_key, KeysFile, Role};
use kshana_pro::serve::{self, ServeConfig};

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn committed_clients_are_current() {
    for (file, text) in [
        (
            "clients/python/kshana_pro_client.py",
            kshana_pro::serve::clients::python(),
        ),
        (
            "clients/typescript/kshana_pro_client.ts",
            kshana_pro::serve::clients::typescript(),
        ),
    ] {
        let committed = std::fs::read_to_string(root().join(file)).unwrap();
        assert!(
            committed == text,
            "{file} is out of date: run `kshana-pro clients --out clients`"
        );
    }
}

#[test]
fn every_route_has_a_client_method() {
    let py = kshana_pro::serve::clients::python();
    let ts = kshana_pro::serve::clients::typescript();
    for r in kshana_pro::serve::api::ROUTES {
        let id = kshana_pro::serve::openapi::operation_id(r.method, r.path);
        assert!(py.contains(&format!("def {id}(")), "{id}");
        let camel: String = id
            .split('_')
            .enumerate()
            .map(|(i, w)| {
                if i == 0 {
                    w.to_string()
                } else {
                    let mut c = w.chars();
                    c.next()
                        .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                        .unwrap_or_default()
                }
            })
            .collect();
        assert!(ts.contains(&format!("  {camel}(")), "{camel}");
    }
}

fn runtime(cmd: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(cmd).args(args).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn missing(what: &str) {
    if std::env::var("KSHANA_REQUIRE_CLIENT_RUNTIMES").as_deref() == Ok("1") {
        panic!("{what} is required (KSHANA_REQUIRE_CLIENT_RUNTIMES=1)");
    }
    eprintln!("skipped: {what} not available");
}

const PY_SCRIPT: &str = r#"
import sys
from kshana_pro_client import KshanaProClient, KshanaProError
base, key, viewer, scenario = sys.argv[1], sys.argv[2], sys.argv[3], open(sys.argv[4]).read()
c = KshanaProClient(base, token=key)
assert c.get_health().strip() == "ok"
job = c.post_jobs({"op": "run", "request": {"scenario": scenario}})
s = c.wait_job(job["job_id"], poll_s=0.05)
assert s["state"] == "succeeded", s
res = c.get_jobs_id_result(job["job_id"])
assert isinstance(res, dict) and res, res
art = s["artefacts"][0]["name"]
assert len(c.get_jobs_id_artefacts_name(job["job_id"], art)) > 0
assert c.get_jobs(limit=5)["total"] == 1
try:
    KshanaProClient(base, token=viewer).post_jobs({"op": "run", "request": {"scenario": scenario}})
    raise SystemExit("a reader key must not submit")
except KshanaProError as e:
    assert e.status == 403, e
try:
    KshanaProClient(base).get_jobs()
    raise SystemExit("no key must be refused")
except KshanaProError as e:
    assert e.status == 401 and "authorization required" in e.body["error"], e.body
print("python client ok", job["job_id"])
"#;

const TS_SCRIPT: &str = r#"
import { readFileSync } from "node:fs";
import { KshanaProClient, KshanaProError } from "CLIENT";
const [base, key, viewer, scenarioPath] = process.argv.slice(2);
const scenario = readFileSync(scenarioPath, "utf8");
const c = new KshanaProClient(base, key);
if ((await c.getHealth()).trim() !== "ok") throw new Error("health");
const job = await c.postJobs({ op: "run", request: { scenario } });
const s = await c.waitJob(job.job_id, 50);
if (s.state !== "succeeded") throw new Error(JSON.stringify(s));
const res = await c.getJobsIdResult(job.job_id);
if (typeof res !== "object") throw new Error("result");
const listed = await c.getJobs({ limit: 5 });
if (listed.total !== 1) throw new Error("list " + JSON.stringify(listed));
try {
  await new KshanaProClient(base, viewer).postJobs({ op: "run", request: { scenario } });
  throw new Error("a reader key must not submit");
} catch (e) {
  if (!(e instanceof KshanaProError) || e.status !== 403) throw e;
}
console.log("typescript client ok", job.job_id);
"#;

#[test]
fn both_clients_drive_a_running_server() {
    let (key, e1) = new_key("client-test", "lab", Role::Submitter);
    let (viewer, e2) = new_key("viewer", "lab", Role::Reader);
    let data = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "service_clients-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let scenario = root().join("tests/fixtures/service/link-budget.toml");
    let base_cfg = |dir: &Path| {
        let mut cfg = ServeConfig::new(
            "127.0.0.1:0",
            Entitlement {
                licensee: "test".into(),
                tier: Tier::Enterprise,
                expires_unix: 0,
                features: vec![],
            },
        );
        cfg.data_dir = Some(dir.to_path_buf());
        cfg.keys = Some(KeysFile {
            keys: vec![e1.clone(), e2.clone()],
        });
        cfg
    };

    // Python, on its own server (a fresh data directory, so the job list starts empty).
    if runtime("python3", &["--version"]).is_some() {
        let dir = data.join("py");
        std::fs::create_dir_all(&dir).unwrap();
        let srv = serve::start(base_cfg(&dir)).unwrap();
        let script = dir.join("drive.py");
        // `-I` (isolated) ignores PYTHONPATH, so the script puts the client on sys.path itself.
        let shim = format!(
            "import sys\nsys.path.insert(0, {:?})\n{PY_SCRIPT}",
            root().join("clients/python").display().to_string()
        );
        std::fs::write(&script, shim).unwrap();
        let out = Command::new("python3")
            .args(["-I", "-B"])
            .arg(&script)
            .arg(format!("http://{}", srv.local_addr()))
            .args([&key, &viewer])
            .arg(&scenario)
            .output()
            .unwrap();
        srv.stop();
        let text = String::from_utf8_lossy(&out.stdout).to_string()
            + &String::from_utf8_lossy(&out.stderr);
        assert!(out.status.success(), "{text}");
        assert!(text.contains("python client ok"), "{text}");
    } else {
        missing("python3");
    }

    // TypeScript, run by Node with type stripping (22.6 or later).
    let node_ok = runtime("node", &["--version"]).is_some_and(|v| {
        let mut it = v
            .trim_start_matches('v')
            .split('.')
            .map(|x| x.parse::<u32>().unwrap_or(0));
        let (maj, min) = (it.next().unwrap_or(0), it.next().unwrap_or(0));
        maj > 22 || (maj == 22 && min >= 6)
    });
    if node_ok {
        let dir = data.join("ts");
        std::fs::create_dir_all(&dir).unwrap();
        let srv = serve::start(base_cfg(&dir)).unwrap();
        let script = dir.join("drive.mts");
        // A `.mts` copy is always an ES module. The committed `.ts` is read as CommonJS
        // when the nearest package.json above it lacks "type": "module" (seen on the CI
        // runner), and then its exports are not found.
        let client = dir.join("kshana_pro_client.mts");
        std::fs::copy(
            root().join("clients/typescript/kshana_pro_client.ts"),
            &client,
        )
        .unwrap();
        std::fs::write(
            &script,
            TS_SCRIPT.replace("CLIENT", &client.display().to_string()),
        )
        .unwrap();
        let out = Command::new("node")
            .args(["--experimental-strip-types", "--no-warnings"])
            .arg(&script)
            .arg(format!("http://{}", srv.local_addr()))
            .args([&key, &viewer])
            .arg(&scenario)
            .output()
            .unwrap();
        srv.stop();
        let text = String::from_utf8_lossy(&out.stdout).to_string()
            + &String::from_utf8_lossy(&out.stderr);
        assert!(
            out.status.success() && text.contains("typescript client ok"),
            "{text}"
        );
    } else {
        missing("node 22.6+");
    }
    let _ = std::fs::remove_dir_all(&data);
}
