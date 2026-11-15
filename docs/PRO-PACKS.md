# Pro packs and the licence lifecycle (roadmap 0.42 C10, C1, C2)

PRIVATE. This page belongs to this private repository only.

Owner decisions of 15 November 2026 (ROADMAP-0.42 section 1d, decision 10, and the two
corrections sent the same day):

* **No App Store.** One free, notarised Kshana Desktop app for macOS, plus Windows and Linux
  builds, direct download. After purchase the app downloads the **Pro pack** from our hosting.
* The pack is **signed by our key** (and, on macOS, its native code by our Apple team ID),
  **encrypted per licence and device**, and **watermarked per licensee**. The same pack system
  serves all three platforms.
* **Subscriptions renew by themselves.** The app checks the licence with our server at start
  and about daily, and silently fetches a fresh 35-day token while the subscription is active.
* **At expiry Pro locks at once.** The 30-day grace period is only a window to renew in: no
  Pro use. During it the locked pack stays on disk, so a renewal unlocks it again with no
  download; the app warns daily. At the window's end the pack is **deleted automatically**
  (only our pack files, inside our app data directory).
* The free edition, and the verification of the customer's signed reports and exports, are
  never touched by any of this.
* Secret methods stay hosted only (C3); Core never ships; the Arksa-exclusive code is server
  side only. A pack carries only `pro` edition code.

## 1. Pieces

| Piece | Code | Where it runs |
|---|---|---|
| Pack format, keys, sealing, watermark | `src/propack/format.rs` | both |
| Catalogue and pack issuer | `src/propack/issuer.rs` | our servers, release pipeline |
| Licence and pack endpoints | `src/propack/server.rs` | our servers |
| Loader: install, verify, decrypt, load, notices, lock, delete | `src/propack/loader.rs` | the app |
| Extension hook for packs | `extension::register_pack` | the app |
| Licence lifecycle: ledger, seats, renewal, revocation | `src/license/lifecycle.rs` | our servers |
| Automatic signed licence check | `src/license/online.rs` | both (server answers, app asks) |
| Clock rollback detection | `src/license/clock.rs` | the app |
| Command line | `kshana-pro packs ...`, `kshana-pro license issue-invoice / extend-invoice` | both |

The loader names no Pro method and depends only on the licence and format modules, so it can
be split into a small crate for the open Desktop app without carrying anything private.

## 2. The pack file

```
"KSPPACK1" | u32 manifest length | signed manifest | sealed payload
```

* **Signed manifest** (`<JSON hex>.<Ed25519 signature hex>`): format, pack id, version,
  platform, licensee and licensee id, the token it was issued for, the device's machine hash,
  issue time, licence expiry, grace days, SHA-256 and length of the plaintext payload, key
  salt, nonce, the kinds the pack provides, watermark id, the Apple team ID (macOS), the
  signing key id. The manifest bytes are also the associated data of the encryption, so a
  payload cannot be moved under another manifest.
* **Encryption:** ChaCha20-Poly1305 and HKDF-SHA256 from `ring` (already in the dependency
  tree through rustls; no new crate). The server holds one pack master secret. The licence
  secret is HKDF(master, token id); it is sent with the pack. The payload key is
  HKDF(licence secret, a fresh random salt per pack, device hash, pack id, version,
  platform). A pack copied to another machine does not decrypt; a fresh salt per build means
  a nonce is never reused under one key.
* **Payload:** a container of named files (`KPC1`: names checked, no `..`, no absolute
  paths). The builder adds the watermark entry `.kshana/watermark.json`: licensee id, token,
  pack, version, time, and an HMAC mark under a key derived from the master secret. A leaked
  decrypted payload is traced with `kshana-pro packs trace <file>`. Native libraries are not
  modified by the watermark (a changed Mach-O would break its code signature), so the mark
  is a separate entry; the per-licence encryption already makes every pack file unique.
  Marking inside native code would need a per-licensee build and signing; not done.

## 3. Release pipeline and hosting

```
# macOS only: sign every Mach-O of the payload with our Developer ID (existing secrets)
APPLE_TEAM_ID=... scripts/packs/sign-macos-payload.sh payload/
kshana-pro packs build --from payload/ --catalog /srv/kshana-packs/catalog \
  --pack-id pro-core --version 1.0.0 --platform macos-aarch64 \
  --provides pro-twin,pro-trade-study --apple-team-id "$APPLE_TEAM_ID"
```

* `packs build` writes `<catalog>/<pack id>/<version>/<platform>.kpc` (the unwatermarked
  container) and `.json` (its record with the container hash), and marks the version current.
  A macOS payload without `--apple-team-id` is refused.
* **macOS library validation:** the notarised Desktop app runs with the hardened runtime and
  loads only native code signed by the same team. The signing step is a pipeline hook,
  `scripts/packs/sign-macos-payload.sh`, using the secrets the Desktop Pro release already
  has (`APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_TEAM_ID`); no secret is added.
  The manifest records the team ID and the host refuses a library signed by any other team.
* **Windows and Linux** use the same pack format; Windows Authenticode signing of the payload
  follows the Windows signing flow when the owner switches it on (K5).
* **Serving:** `kshana-pro packs serve --catalog <dir> --data <dir>` with
  `KSHANA_PRO_SIGNING_KEY` (licences), `KSHANA_PRO_PACK_SIGNING_KEY` and
  `KSHANA_PRO_PACK_MASTER_SECRET` in the environment; TLS in front, like the payments
  listener. `--data` is the same directory as the payments service (register and ledger).
* **Air-gapped customers (C2):** `kshana-pro packs issue ... --out pack.kpack` writes the pack
  and prints the licence secret; both go to the customer on the medium of their choice.
* App builds trust the pack key set at build time in `KSHANA_PRO_PACK_PUBKEY` (64 hex, several
  comma separated). A build without it trusts no pack.

## 4. Endpoints

| Route | Sends | Gets |
|---|---|---|
| `POST /v1/licence/activate` | the token, the device hash | a token bound to the device (one seat) |
| `POST /v1/licence/check` | licence id, device hash, app version, pack version, nonce | a signed answer |
| `GET /v1/revocations` | | the signed revocation list |
| `POST /v1/packs/fetch` | the device token, device hash, pack id, platform | the pack, licence secret in `Kshana-Pack-Secret` |

The background check sends nothing personal: no name, no e-mail, no token text. Its answer
is signed with the licence key and carries the request's nonce, so a forged or replayed
answer is refused and changes nothing. A pack is issued only for a token bound to the asking
device, unexpired, unrevoked, of a paid licence.

## 5. Licence lifecycle

| Step | What happens |
|---|---|
| Purchase (Stripe, when on) or invoice | A licence is opened in the ledger, paid through the term; its first token lasts 35 days (or the term, when shorter) |
| First start | The app activates a seat: a token bound to this device; seats are counted per licence |
| Every start, then about daily | The automatic check: active, a fresh 35-day token when the current one is a day old; ended, the signed "ended" answer is stored |
| Offline | The stored token keeps Pro working until its own expiry (at most 35 days), then Pro locks; the next check that reaches the server puts it right |
| 14, 7, 1 days before the end | Notices ("Renew soon", "Renew now", "last day") |
| End (expiry, or the server says ended) | Pro locks at once (Evaluation tier); the pack does not open |
| 30-day renewal window | Daily notice; the locked pack stays on disk; a renewal (same licensee and device) unlocks it with no download |
| Window over | The pack's files are deleted; the licence counts as expired |

* **C1 subscription tokens:** 35 days, renewed online while paid. Stripe payments move the
  paid-through date (`invoice.paid`); a cancellation, failed and unrecovered payment, refund or
  chargeback ends it. Invoice customers are active until their invoiced term end:
  `kshana-pro license issue-invoice --licensee ... --tier pro --invoice <ref> --term-days 365
  --data <dir>`, and `extend-invoice --licence <id> --until <unix>` when the next invoice is paid.
* **C2 offline tokens** (`issue-invoice --offline`): one token for the whole prepaid term, the
  same 30-day renewal window after it, renewal by a new signed file; never renewed online.
* **Revocation:** revoking a licence (refund, chargeback, subscription end, vendor) revokes
  every token issued for it, its renewals and every device token, and rewrites the signed
  revocation list. The app picks it up from the next check, the next update or by hand.
* **Seats:** `seats` in the licence (at least one). A device that already holds a seat gets a
  fresh token for it; a new device beyond the count is refused until a seat is released.
* **Clock rollback (basics):** the app keeps the latest time it has seen (`clock.json` in the
  licence store and in the pack store). A clock more than 6 hours behind it counts as set
  back: the status says so and the later time is used, so nothing gains time. Deleting the
  file and setting the clock back before the first check is not caught; the 35-day online
  renewal bounds what an offline machine can gain.

## 6. What the loader refuses

In order: a pack not signed by a trusted pack key (or with any manifest byte changed); a
pack for another device; a revoked licence; an ended licence (expired, or the server said it
ended) and a pack past its window; a payload that does not decrypt with this licence secret
on this device (wrong device, wrong secret, a changed byte); a payload whose SHA-256 differs
from the manifest. The extension hook then refuses any kind the signed manifest does not
list, any id without the `pro-` prefix, a collision with a built-in Pro kind, feature, the
server-side extension or another pack, and any id the tier manifest lists at a tier other
than `pro`. Kinds of a loaded pack are at the `pro` edition and run only under a licence tier
that unlocks them.

## 7. Deletion

`PackStore::enforce` deletes, for a pack whose window is over, exactly `<id>.kpack`,
`<id>.key`, `<id>.state.json` and `unpacked/<id>/`, inside the directory `pro-packs` of the
app data directory. It refuses to act when that directory is not named `pro-packs` or is a
symbolic link, and never follows a link. Reports, exports, the licence store and any other
file are left alone.

## 8. Tests

* `tests/pro_packs.rs`: issue through the endpoint, install, bad signature, wrong device,
  wrong secret, changed ciphertext, load through the extension hook (edition and tier gate),
  watermark trace, the hook's refusals, the endpoint's refusals (unbound, other device,
  expired, revoked, macOS without team ID), notices at 14, 7 and 1 days, locked in the window
  (refuses to load), daily notices, renewal in the window opens the same file with no
  download, a clock set back does not reopen it, deletion of our files only after the window,
  revocation locks at once.
* `tests/licence_lifecycle.rs`: 35-day tokens, seats bound to devices, renewal while paid and
  refused after, offline tokens, revocation of every token of a licence, the automatic check
  (active refreshes, forged and replayed answers refused, offline within the token works,
  offline past it locks, back online renews, ended locks, revoked locks), the request carries
  no personal data, clock rollback.
* `tests/licensing.rs`: warnings at 14, 7, 1 days; the 30-day window gives Evaluation only.

## 8a. Packs in the catalogue

* `pro-forensics` provides `pro-incident-forensics` (incident forensics, roadmap 0.42 E1;
  docs/INCIDENT-FORENSICS.md). Payload: `kshana-pro forensics pack-payload <dir>` writes
  `forensics/method.json` (method id and parameter hash), which the host's
  `forensics::ForensicsActivator` checks before it registers the kind. The kind is listed at
  `pro` in the tier manifest (`license::tiers::PACK_DELIVERED_KINDS`) and refused until the pack
  loads. Tests: `tests/forensics_pack.rs`.

## 9. Not built here

* The native library loading in the Desktop host (dlopen or LoadLibrary behind the
  `loader::Activator` trait, with the team ID check on macOS) belongs to the app.
* Windows payload signing waits for the Windows signing flow (K5).
* The pack master secret and pack signing key are generated and stored by the owner
  (docs/LICENSING-KEYS.md); none is in this repository.
