<!-- PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE. -->
# Kshana Desktop Pro

The free Kshana Desktop (the open repository's `apps/desktop`) with the Pro overlay linked
in (roadmap F10e). A separate download: the free build carries no Pro code
(`scripts/free-build-check.sh` checks a free binary).

- **Engine bridge.** The free Desktop's command names (`engine_call`,
  `engine_native_fns`), so the Studio's Desktop client is unchanged. A scenario of a Pro
  kind runs through `kshana_pro::registry::run_gated` under the Desktop licence; every
  other scenario on the open engine. `list_kinds` adds the Pro kinds the licence runs.
- **Licence page** (`pro/licence.html`, the first page): state, tier, licensee, expiry,
  products, seats, warnings (30 and 7 days, grace); paste a token to activate it offline
  (`desktop` product); this machine's ID for a bound licence; remove the licence.
- **Upgrade.** Shown only while payments are on in the build's
  `commercial/payments.toml`; with the committed configuration (off) there is no offer and
  no network call (a test checks it).

Build (needs a checkout of the open repository that carries the Desktop,
`web/studio/lib/desktop.mjs`):

```bash
apps/desktop-pro/build-frontend.sh <open repository checkout>
cargo test  --manifest-path apps/desktop-pro/src-tauri/Cargo.toml
cargo build --manifest-path apps/desktop-pro/src-tauri/Cargo.toml
(cd apps/desktop-pro/src-tauri && cargo tauri build)    # unsigned bundles
```

Not done: signing and notarisation (the publisher's Apple and Windows accounts); the
Devices panel (L2b) comes with the free Desktop.
