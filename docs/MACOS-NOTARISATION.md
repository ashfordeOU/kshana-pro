# macOS signing and notarisation for Kshana Desktop: a checklist

Status (15 November 2026): **done for Kshana Desktop Pro.** The Apple Developer account and both
Developer ID certificates (Application and Installer, issued to the team in `APPLE_TEAM_ID`, expiring
1 February 2027) exist, and the signing and notarisation secrets are in this repository.
`.github/workflows/apple-signing-check.yml` proves them without signing anything (last green run
13 November 2026), and `.github/workflows/desktop-pro-release.yml` builds the universal `.app`,
signs it with the hardened runtime, notarises and staples the app and the `.dmg`, and checks both
with `codesign`, `spctl` and `stapler` (green runs on 13 November 2026; the `.dmg` stays in a private
draft release, nothing is published). It runs on the owner's self-hosted Mac. The open Kshana
Desktop's signing is handled in the open repository. The checklist below stays as the reference
for renewing the certificates and for a new machine.

It covers two applications. Both are Tauri 2 (the lock has `tauri` 2.12.5), and both build `.app`
and `.dmg` on macOS:

| Application | Repository | Bundle identifier | Version today |
|---|---|---|---|
| Kshana Desktop | open repository, `apps/desktop/` | `dev.kshana.desktop` | 0.40.0 |
| Kshana Desktop Pro | kshana-pro, `apps/desktop-pro/` | `dev.kshana.desktop.pro` | 0.2.0 |

Both are distributed outside the Mac App Store, with **Developer ID** signing, the **hardened
runtime** and **notarisation**. Neither uses the App Sandbox.

## 1. Accounts (the publisher, once)

- [ ] **Enrol in the Apple Developer Program as an organisation.** The name must be Ashforde OÜ's
  legal name; Apple shows it to users as the developer. Enrolling an organisation needs a
  D-U-N-S number for the company, and the person enrolling must be able to bind it legally. The
  membership is paid yearly.
- [ ] **Note the Team ID**, a 10-character string from the membership page. It is part of the
  signing identity's name and of the notarisation call.
- [ ] **Create the Developer ID Application signing identity.** Only the Account Holder can create
  Developer ID identities.
  - Do it on a Mac: Keychain Access, or Xcode under Settings > Accounts > Manage.
  - Export the key and Apple's issued identity together as one password-protected `.p12` file.
  - Keep the `.p12` and its password where the other release secrets are kept
    (docs/LICENSING-KEYS.md section 4). The `.p12` is the publisher's identity: anyone holding
    it can sign as Ashforde OÜ.
  - A "Developer ID Installer" identity is needed only for a `.pkg` installer. Tauri's macOS
    outputs here are `.app` and `.dmg`, so it is not needed.
- [ ] **Create a notarisation credential.** The App Store Connect API key is the better choice for
  automated builds:
  - In App Store Connect, go to Users and Access > Integrations > Team keys, and create a key with
    the Developer role.
  - Keep three things: the **Issuer ID**, the **Key ID** and the downloaded `.p8` file. The `.p8`
    can be downloaded only once.
  - The other route is an Apple ID with an app-specific password and the Team ID. It works, but it
    ties releases to one person's Apple ID.

## 2. Configuration in the repositories (no secrets in either)

- [ ] In both `tauri.conf.json` files, add a `bundle.macOS` block:
  ```json
  "macOS": { "minimumSystemVersion": "11.0", "hardenedRuntime": true }
  ```
  - Tauri 2 already enables `hardenedRuntime` by default; writing it out documents the choice.
  - The minimum system version is a product decision: 11.0 is the first macOS that runs on
    Apple silicon.
- [ ] **Entitlements: none expected.** Check each capability below against the signed build, and
  add an `entitlements` file only for a capability that actually fails.
  - The WebAssembly engine runs inside WKWebView's own web-content process, under Apple's own
    entitlements. The app's CSP already allows `'wasm-unsafe-eval'` for it.
  - The live device panel opens serial ports (`/dev/cu.*`) and runs `/bin/stty`. A non-sandboxed
    app under the hardened runtime may do both with no entitlement.
  - The app makes no outbound connection of its own; the device panel's `tcp:` sources are
    client sockets. Without the App Sandbox, no network entitlement exists or is needed.
- [ ] **Decide the architectures.** The choices are a universal build
  (`cargo tauri build --target universal-apple-darwin`, after
  `rustup target add aarch64-apple-darwin x86_64-apple-darwin`) or two separate builds. The
  universal `.dmg` is simpler for users and roughly doubles the binary.
- [ ] **Pro only: the licence key must be compiled in.** `KSHANA_PRO_LICENSE_PUBKEY` (or the
  embedded keyring) must be set in the release build, or Desktop Pro runs its open features only
  (apps/desktop-pro/src-tauri/src/bridge.rs, `keyring()`). This is not part of signing, but it
  belongs in the same release step.

## 3. Secrets for the build (CI or a release Mac)

Tauri 2 signs and notarises during `cargo tauri build` when these environment variables are set.
Check the names against the Tauri version in use at the time; these are Tauri 2's.

| Variable | Value |
|---|---|
| `APPLE_CERTIFICATE` | the `.p12` from section 1, base64-encoded (`base64 -i identity.p12`) |
| `APPLE_CERTIFICATE_PASSWORD` | its password |
| `APPLE_SIGNING_IDENTITY` | the identity's full name, `Developer ID Application: <legal name> (<TEAMID>)` |
| `APPLE_API_ISSUER`, `APPLE_API_KEY`, `APPLE_API_KEY_PATH` | the API key: Issuer ID, Key ID, and the path of the `.p8` on the build machine |
| `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID` | only for the Apple ID route instead of the API key |

Where each build runs:

- [ ] **Open repository.** `.github/workflows/desktop.yml` builds on GitHub-hosted `macos-latest`.
  - Add a separate, tag-triggered release job that holds these secrets.
  - Never expose the secrets to `pull_request` runs: a fork's pull request must not be able to
    sign.
  - Store them as environment secrets, and require approval for that environment.
- [ ] **kshana-pro.** `.github/workflows/desktop-pro.yml` runs on the self-hosted Mac. The same
  variables apply.
  - Prefer an environment on the job.
  - Make sure the runner's keychain does not keep the imported identity between runs. Tauri
    imports `APPLE_CERTIFICATE` into a temporary keychain for the build.

## 4. Checking a signed build

Run these on the built bundle (`src-tauri/target/<target>/release/bundle/macos/`), then on the
`.dmg`:

```sh
codesign -dv --verbose=4 "Kshana Desktop.app"              # Authority=Developer ID Application: …, flags=0x10000(runtime)
codesign --verify --deep --strict --verbose=2 "Kshana Desktop.app"
spctl --assess --type execute --verbose=4 "Kshana Desktop.app"   # "accepted, source=Notarized Developer ID"
xcrun stapler validate "Kshana Desktop.app"                # the notarisation ticket is stapled
spctl --assess --type open --context context:primary-signature --verbose=4 "Kshana Desktop_<version>_<arch>.dmg"
xcrun stapler validate "Kshana Desktop_<version>_<arch>.dmg"
```

- [ ] If the `.dmg` has no stapled ticket, or is not signed, notarise and staple it after the build:
  ```sh
  xcrun notarytool submit <dmg> --key <p8> --key-id <id> --issuer <issuer> --wait
  xcrun stapler staple <dmg>
  ```
  On the first signed release, record which step Tauri 2.12 already does for the `.dmg`, and keep
  this page in step.
- [ ] **Gatekeeper on a clean Mac.** Download the `.dmg` through a browser, so it gets the
  quarantine flag. Open it and start the app: there must be no warning beyond the standard
  "downloaded from the Internet" confirmation. Run one scenario, open the trust page's live
  device panel, and close the app.
- [ ] Keep `xcrun notarytool log <submission-id>` for the release record. It lists every file
  Apple examined.

## 5. After the first signed release

- [ ] Remove "unsigned" from both READMEs (apps/desktop/README.md "Not done yet", and
  apps/desktop-pro/README.md), and from the open repository's `docs/SURFACES.md` row.
- [ ] **Signed automatic updates** are a separate key: Tauri's updater signs update packages with
  its own key pair (`cargo tauri signer generate`), which is not the Apple identity. Add it to the
  release secrets when updates are switched on.
- [ ] **Renewal.** A Developer ID identity is valid for several years, and the membership is
  yearly. A lapsed membership stops new notarisation, but builds already signed and notarised
  keep working. Put both dates in the release calendar.
