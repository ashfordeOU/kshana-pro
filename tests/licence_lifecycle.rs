// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
// Roadmap 0.42 C1 and C2: 35-day tokens, seats bound to devices, online renewal while
// paid, the automatic signed licence check, revocation of a whole licence, the 30-day
// renewal window (no Pro use) and clock rollback detection. Keys are generated here.

use std::cell::Cell;
use std::path::PathBuf;

use kshana_pro::license::activation::{State, Store};
use kshana_pro::license::lifecycle::{Delivery, LicenceService, TOKEN_DAYS};
use kshana_pro::license::machine::hash_machine_id;
use kshana_pro::license::online::{self, CheckOutcome, CheckRequest, CheckTransport};
use kshana_pro::license::{public_key_for, Keyring, LicenseClaims, Product, Tier, VerifyOptions};

const SEED: [u8; 32] = [0x5a; 32];
const T0: i64 = 1_800_000_000;
const DAY: i64 = 86_400;

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kpro-lifecycle-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn ring() -> Keyring {
    Keyring::single(public_key_for(&SEED))
}

fn claims(seats: u32) -> LicenseClaims {
    LicenseClaims {
        licensee: "Harbour Pilots Ltd".into(),
        tier: "pro".into(),
        products: vec!["cli".into(), "desktop".into()],
        seats,
        ..Default::default()
    }
}

fn opts_on(device: &str, now: i64) -> VerifyOptions<'static> {
    let d: &'static str = Box::leak(device.to_string().into_boxed_str());
    let f: &'static dyn Fn() -> Option<String> = Box::leak(Box::new(move || Some(d.to_string())));
    VerifyOptions {
        now_unix: now,
        product: Some(Product::Desktop),
        machine: Some(f),
        allow_expired: false,
    }
}

fn claims_of(token: &str) -> LicenseClaims {
    let (p, _) = token.split_once('.').unwrap();
    serde_json::from_slice(&hex::decode(p).unwrap()).unwrap()
}

#[test]
fn tokens_last_35_days_seats_bind_devices_and_renew_while_paid() {
    let dir = tmp("seats");
    let svc = LicenceService::open(SEED, &[], &dir);
    let (a, b) = (hash_machine_id("device-a"), hash_machine_id("device-b"));
    let first = svc
        .open_licence(
            &claims(1),
            Delivery::Online,
            T0 + 365 * DAY,
            "invoice:INV-1",
            "",
            T0,
        )
        .unwrap();
    assert_eq!(first.expires_unix, T0 + TOKEN_DAYS * DAY);
    assert!(claims_of(&first.token).machine.is_none());

    // Seat 1 on device A: a token bound to A.
    let sa = svc.activate(&first.token, &a, T0 + 10).unwrap();
    assert_eq!(claims_of(&sa.token).machine.as_deref(), Some(a.as_str()));
    // Idempotent for the same device; the only seat is taken for device B.
    assert!(svc.activate(&first.token, &a, T0 + 20).is_ok());
    let e = svc
        .activate(&first.token, &b, T0 + 30)
        .unwrap_err()
        .to_string();
    assert!(e.contains("seat"), "{e}");
    // A's token presented from B: refused (bound to another machine).
    assert!(svc.renew(&sa.token, &b, T0 + 40).is_err());
    assert!(svc.activate(&sa.token, &b, T0 + 40).is_err());

    // Renewal on day 30: a fresh 35-day token bound to A.
    let r = svc.renew(&sa.token, &a, T0 + 30 * DAY).unwrap();
    assert_eq!(r.expires_unix, T0 + 30 * DAY + TOKEN_DAYS * DAY);
    assert_eq!(r.root, first.root);
    // A token more than 30 days past its expiry is not renewed.
    assert!(svc.renew(&r.token, &a, T0 + 350 * DAY).is_err());
    // Renewed every 30 days, near the end of the term the token stops at the
    // paid-through date.
    let mut cur = r.clone();
    for d in (60..=330).step_by(30) {
        cur = svc.renew(&cur.token, &a, T0 + d * DAY).unwrap();
    }
    let late = svc.renew(&cur.token, &a, T0 + 350 * DAY).unwrap();
    assert_eq!(late.expires_unix, T0 + 365 * DAY);
    // After the term: refused until paid.
    assert!(svc.renew(&late.token, &a, T0 + 366 * DAY).is_err());
    svc.extend(&first.root, T0 + 730 * DAY).unwrap();
    assert!(svc.renew(&late.token, &a, T0 + 366 * DAY).is_ok());

    // Release the seat: B can take it.
    assert!(svc.release(&first.root, &a).unwrap());
    // (The first token is long expired; the vendor sends a fresh unbound one.)
    assert!(svc.activate(&first.token, &b, T0 + 367 * DAY).is_err());
    let fresh = svc
        .reissue_unbound(&first.root, "manual", T0 + 367 * DAY)
        .unwrap();
    assert!(svc.activate(&fresh.token, &b, T0 + 367 * DAY).is_ok());

    // Revoking the licence revokes every token issued for it.
    let list = svc
        .revoke(&first.root, "refund", "manual", T0 + 368 * DAY)
        .unwrap();
    let parsed = kshana_pro::license::revocation::verify(&ring(), &list).unwrap();
    for t in [&first.token_id, &sa.token_id, &r.token_id, &late.token_id] {
        assert!(parsed.is_revoked(t), "{t} not revoked");
    }
    assert!(svc.renew(&late.token, &a, T0 + 368 * DAY).is_err());
    assert!(svc.activate(&first.token, &a, T0 + 368 * DAY).is_err());
}

#[test]
fn offline_licences_cover_the_term_and_are_not_renewed_online() {
    let dir = tmp("offline");
    let svc = LicenceService::open(SEED, &[], &dir);
    let a = hash_machine_id("air-gapped");
    let i = svc
        .open_licence(
            &claims(2),
            Delivery::Offline,
            T0 + 365 * DAY,
            "invoice:INV-2",
            "",
            T0,
        )
        .unwrap();
    assert_eq!(i.expires_unix, T0 + 365 * DAY);
    let s = svc.activate(&i.token, &a, T0 + 5).unwrap();
    assert_eq!(s.expires_unix, T0 + 365 * DAY);
    let e = svc
        .renew(&s.token, &a, T0 + 100 * DAY)
        .unwrap_err()
        .to_string();
    assert!(e.contains("signed file"), "{e}");
    // A licence must be paid into the future, and name a licensee.
    assert!(svc
        .open_licence(&claims(1), Delivery::Online, T0, "x", "", T0)
        .is_err());
    let mut c = claims(1);
    c.licensee = " ".into();
    assert!(svc
        .open_licence(&c, Delivery::Online, T0 + DAY, "x", "", T0)
        .is_err());
}

/// The licence server in process; `mode` forges or drops answers.
struct InProc<'a> {
    svc: &'a LicenceService,
    now: Cell<i64>,
    mode: Cell<u8>, // 0 normal, 1 offline, 2 forged signature, 3 replay of an old answer
    last: std::cell::RefCell<Option<String>>,
    seen: std::cell::RefCell<Vec<String>>,
}

impl CheckTransport for InProc<'_> {
    fn post(&self, path: &str, body: &str) -> Result<String, String> {
        assert_eq!(path, "/v1/licence/check");
        self.seen.borrow_mut().push(body.to_string());
        let req: CheckRequest = serde_json::from_str(body).map_err(|e| e.to_string())?;
        match self.mode.get() {
            1 => Err("connection refused".into()),
            3 => self.last.borrow().clone().ok_or_else(|| "nothing".into()),
            m => {
                let t = self
                    .svc
                    .check(&req, self.now.get())
                    .map_err(|e| e.to_string())?;
                *self.last.borrow_mut() = Some(t.clone());
                if m == 2 {
                    // Same answer signed by another key.
                    let (p, _) = t.split_once('.').unwrap();
                    use ed25519_dalek::{Signer, SigningKey};
                    let sig = SigningKey::from_bytes(&[7; 32]).sign(&hex::decode(p).unwrap());
                    return Ok(format!("{p}.{}", hex::encode(sig.to_bytes())));
                }
                Ok(t)
            }
        }
    }
}

#[test]
fn automatic_check_renews_while_active_and_locks_when_ended_or_offline_too_long() {
    let dir = tmp("auto");
    let svc = LicenceService::open(SEED, &[], &dir.join("vendor"));
    let a = hash_machine_id("laptop");
    let first = svc
        .open_licence(
            &claims(1),
            Delivery::Online,
            T0 + 90 * DAY,
            "stripe:evt_1",
            "",
            T0,
        )
        .unwrap();
    let seat = svc.activate(&first.token, &a, T0 + 60).unwrap();
    let store = Store::open(dir.join("app"));
    store
        .activate(&ring(), &seat.token, &opts_on(&a, T0 + 60))
        .unwrap();
    let t = InProc {
        svc: &svc,
        now: Cell::new(T0 + 60),
        mode: Cell::new(0),
        last: Default::default(),
        seen: Default::default(),
    };

    // At start: confirmed (the token is fresh); the request carries nothing personal.
    let out = online::check_if_due(
        &store,
        &ring(),
        &t,
        &a,
        "0.42.0",
        Some("1.0.0"),
        T0 + 60,
        true,
    )
    .unwrap()
    .unwrap();
    assert_eq!(out, CheckOutcome::Unchanged);
    let sent: serde_json::Value = serde_json::from_str(&t.seen.borrow()[0]).unwrap();
    let mut keys: Vec<&str> = sent
        .as_object()
        .unwrap()
        .keys()
        .map(|k| k.as_str())
        .collect();
    keys.sort();
    assert_eq!(
        keys,
        [
            "app_version",
            "device",
            "licence_id",
            "nonce",
            "pack_version"
        ]
    );
    assert!(!t.seen.borrow()[0].contains("Harbour"));
    // Not due again within the day.
    assert!(
        online::check_if_due(&store, &ring(), &t, &a, "0.42.0", None, T0 + 3600, false)
            .unwrap()
            .is_none()
    );

    // Day 20, subscription active: a fresh 35-day token arrives silently.
    let now = T0 + 20 * DAY;
    t.now.set(now);
    let out = online::check_if_due(&store, &ring(), &t, &a, "0.42.0", None, now, false)
        .unwrap()
        .unwrap();
    assert_eq!(
        out,
        CheckOutcome::Renewed {
            expires_unix: now + 35 * DAY
        }
    );
    let st = store.status(&ring(), &[], &opts_on(&a, now));
    assert_eq!(st.entitlement.tier, Tier::Pro);

    // A forged answer, and a replayed old answer: refused, nothing changes.
    t.mode.set(2);
    assert!(matches!(
        online::check_now(&store, &ring(), &t, &a, "0.42.0", None, now + 10).unwrap(),
        CheckOutcome::Offline(m) if m.contains("refused")
    ));
    t.mode.set(3);
    assert!(matches!(
        online::check_now(&store, &ring(), &t, &a, "0.42.0", None, now + 20).unwrap(),
        CheckOutcome::Offline(m) if m.contains("does not answer")
    ));

    // Offline: the stored token keeps Pro working until its own expiry ...
    t.mode.set(1);
    let exp = now + 35 * DAY;
    assert!(matches!(
        online::check_now(&store, &ring(), &t, &a, "0.42.0", None, exp - DAY).unwrap(),
        CheckOutcome::Offline(_)
    ));
    let st = store.status(&ring(), &[], &opts_on(&a, exp - DAY));
    assert_eq!(st.entitlement.tier, Tier::Pro);
    // ... then Pro locks (renewal window, no Pro use).
    let st = store.status(&ring(), &[], &opts_on(&a, exp + 1));
    assert!(matches!(st.state, State::Grace { .. }));
    assert_eq!(st.entitlement.tier, Tier::Evaluation);

    // Back online inside the window while still paid: Pro comes back.
    t.mode.set(0);
    t.now.set(exp + 2);
    let out = online::check_now(&store, &ring(), &t, &a, "0.42.0", None, exp + 2).unwrap();
    assert!(matches!(out, CheckOutcome::Renewed { .. }), "{out:?}");
    assert_eq!(
        store
            .status(&ring(), &[], &opts_on(&a, exp + 3))
            .entitlement
            .tier,
        Tier::Pro
    );

    // The subscription ends (paid through day 90): the server says it has ended; Pro is
    // locked and the 30-day renewal window runs from the end.
    let end = T0 + 91 * DAY;
    t.now.set(end);
    let out = online::check_now(&store, &ring(), &t, &a, "0.42.0", None, end).unwrap();
    assert!(
        matches!(out, CheckOutcome::Ended { ended_unix, .. } if ended_unix == T0 + 90 * DAY),
        "{out:?}"
    );
    let st = store.status(&ring(), &[], &opts_on(&a, end));
    assert!(matches!(st.state, State::Grace { .. }), "{:?}", st.state);
    assert_eq!(st.entitlement.tier, Tier::Evaluation);
    assert_eq!(st.pro_ends_unix, T0 + 90 * DAY);
    // After the 30-day window: expired.
    let st = store.status(&ring(), &[], &opts_on(&a, T0 + 121 * DAY));
    assert_eq!(st.state, State::Expired);

    // Revoked (refund) while the stored token still has weeks left: the server says it has
    // ended, and Pro locks at once.
    svc.extend(&first.root, T0 + 400 * DAY).unwrap();
    t.now.set(end + 10);
    let out = online::check_now(&store, &ring(), &t, &a, "0.42.0", None, end + 10).unwrap();
    assert!(matches!(out, CheckOutcome::Renewed { .. }), "{out:?}");
    svc.revoke(&first.root, "refund", "stripe:evt_9", end + 20)
        .unwrap();
    t.now.set(end + 30);
    let out = online::check_now(&store, &ring(), &t, &a, "0.42.0", None, end + 30).unwrap();
    assert!(
        matches!(out, CheckOutcome::Ended { ref reason, .. } if reason.contains("revoked")),
        "{out:?}"
    );
    assert_eq!(
        store
            .status(&ring(), &[], &opts_on(&a, end + 40))
            .entitlement
            .tier,
        Tier::Evaluation
    );
}

#[test]
fn a_clock_set_back_is_detected_and_does_not_extend_the_licence() {
    let dir = tmp("clock");
    let svc = LicenceService::open(SEED, &[], &dir.join("vendor"));
    let a = hash_machine_id("desk");
    let first = svc
        .open_licence(
            &claims(1),
            Delivery::Online,
            T0 + 365 * DAY,
            "invoice:INV-3",
            "",
            T0,
        )
        .unwrap();
    let seat = svc.activate(&first.token, &a, T0).unwrap();
    let store = Store::open(dir.join("app"));
    store
        .activate(&ring(), &seat.token, &opts_on(&a, T0))
        .unwrap();
    let exp = T0 + 35 * DAY;
    // Seen after expiry: locked.
    let (st, c) = store.status_guarded(&ring(), &[], &opts_on(&a, exp + DAY));
    assert!(!c.rolled_back);
    assert_eq!(st.entitlement.tier, Tier::Evaluation);
    // The clock is set back ten days: detected, the later time is used, still locked.
    let (st, c) = store.status_guarded(&ring(), &[], &opts_on(&a, exp - 9 * DAY));
    assert!(c.rolled_back);
    assert_eq!(c.effective_unix, exp + DAY);
    assert_eq!(st.entitlement.tier, Tier::Evaluation);
    assert!(st.warning.unwrap().contains("clock"));
    // A small step back (daylight saving) is not a rollback.
    let (_, c) = store.status_guarded(&ring(), &[], &opts_on(&a, exp + DAY - 3600));
    assert!(!c.rolled_back);
}
