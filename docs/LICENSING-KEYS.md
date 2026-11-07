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
