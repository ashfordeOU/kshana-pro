# Licence signing keys: custody, backup, rotation

The procedure for the Ed25519 key that signs every Kshana Pro licence token and
revocation list (roadmap F10d). The token format and commands are in
[LICENSING.md](LICENSING.md).

## 1. Custody

* The signing key (`KSHANA_PRO_SIGNING_KEY`, 64 hex characters) is created on a
  machine that is offline at the time (`kshana-pro license keygen`) and is never stored
  on a networked machine, in a repository, in continuous integration or in a chat.
* It lives on an encrypted removable drive (the **primary**) kept by the vendor. The
  issuance register (`register.jsonl`) sits on the same drive.
* Tokens are signed on the offline machine; the token text is carried out on paper or a
  removable drive. The payments service (F10g), when switched on, holds a **separate**
  key of its own (section 4), never the primary.
* Continuous integration uses a throwaway key (`scripts/release-smoke.sh`) that signs
  nothing that ships.

## 2. Backup

* Two backups: an encrypted drive in a second location, and a paper copy of the 64 hex
  characters in a sealed envelope in a safe. Each backup is labelled with the key ID
  (`kshana-pro license pubkey` prints it).
* Once a year, and after any change, restore from each backup on the offline machine
  and check that `license pubkey` prints the expected key ID. Record the check (date,
  key ID, result) in the vendor's records.
* The register is copied to the backup drive after every issuing session;
  `kshana-pro license register verify` must pass on the copy.

## 3. Rotation

Rotate every two years, and at once if a key may have been exposed.

1. `kshana-pro license keygen` on the offline machine: new key pair and key ID.
2. Add the new public key to the release builds:
   `KSHANA_PRO_LICENSE_PUBKEY=<old>,<new>`. Ship that release. Both keys verify.
3. Sign new tokens and revocation lists with the new key only. Renew existing
   customers' tokens with the new key as they come due (`license renew`).
4. When every current token has been reissued under the new key (the register shows
   which key signed each line), retire the old key: build the next revocation list with
   `--retire <old key ID>` (signed by the new key), and in the following release move
   the old key from `KSHANA_PRO_LICENSE_PUBKEY` to `KSHANA_PRO_LICENSE_RETIRED`.
5. Destroy the old private key and its backups; record the date.

**If the key is exposed:** skip the overlap. Generate the new key, reissue every
current token from the register under it, publish a revocation list that retires the
exposed key at once, and ship a release with the old key retired. Tokens signed by the
exposed key stop at that update.

## 4. The payments key

When payments go live (F10g, owner gate 12), the webhook service issues tokens without a
person. It gets its own key pair, added to the builds as a second trusted key. If the
service is compromised, only that key is retired; tokens issued by hand are unaffected.
Its private key is held in the service's secret store, never in a repository.

## 5. The Arksa issuer key (roadmap 0.42 X1)

Tokens carrying the `arksa` claim unlock the Arksa-exclusive kinds (docs/PROTECTION.md).
Only Arksa's issuer signs them, with a key pair of its own, kept apart from the Pro
licence key:

* The private key is created and kept like the primary (section 1: offline, encrypted
  drive, two backups), by Arksa's issuer, never by the Pro vendor's issuing process, the
  payments service or continuous integration, and never in a repository.
* Its public key goes only into builds made with the `arksa` feature (Arksa servers and
  contracted Arksa Sovereign installs), through `KSHANA_PRO_ARKSA_ISSUER_PUBKEY` at build
  time. Default builds, the packages and Desktop Pro have no Arksa issuer slot filled, so
  they refuse every token that carries the claim.
* Tokens are made with `license::issue_arksa`. A token carrying the claim verifies only
  under this key; a Pro key cannot grant it.
* Rotation and exposure follow section 3: add the new key to the builds, reissue, retire the
  old key ID (the retired list applies to the issuer keys as well).
* The repository holds only a placeholder public key (`PLACEHOLDER_ARKSA_ISSUER_PUBKEY`), a
  test key whose private half is in the tests. A release build never trusts it.

## 6. The tier manifest key (roadmap 0.42 X2)

The signed tier manifest (`commercial/tier-manifest.json`) states the edition of every Pro
kind; each build verifies it at start-up.

* A separate Ed25519 key pair, kept like the primary (section 1). Sign the manifest on the
  offline machine whenever it changes (Ed25519 over the exact file bytes, hex).
* A release build sets `KSHANA_PRO_TIER_MANIFEST_PUBKEY` (the public key) and
  `KSHANA_PRO_TIER_MANIFEST_SIG` (the signature of the manifest being built).
* The committed signature is made with a test key (public half
  `PLACEHOLDER_MANIFEST_PUBKEY`, private half only in `tests/tier_manifest.rs`); it serves
  development and continuous integration only.
