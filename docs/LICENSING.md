# Licensing: tokens, activation, issuing

How a Kshana Pro licence is made, carried, checked and withdrawn (roadmap F10b, F10c,
F10d, F10f). The code is `src/license/` (library) and `src/cli/license_cmd.rs` (the
`kshana-pro license` commands). The signing key procedure is in
[LICENSING-KEYS.md](LICENSING-KEYS.md).

Abbreviations used on this page, in full:

| Abbreviation | Meaning |
|---|---|
| CLI | command-line interface |
| HTTPS | HyperText Transfer Protocol over TLS |
| ID | identifier |
| JSON | JavaScript Object Notation |
| SHA-256 | Secure Hash Algorithm, 256-bit |
| TLS | Transport Layer Security |

## 1. The token

A token is `payload_hex.signature_hex`: the JSON claims and an Ed25519 signature over
them. The vendor signs with a private key kept offline; a build holds public keys only,
so a customer binary cannot mint tokens.

| Claim | Since | Meaning |
|---|---|---|
| `licensee` | v1 | Who holds the licence |
| `tier` | v1 | `evaluation`, `pro` or `enterprise` |
| `expires_unix` | v1 | Expiry, Unix seconds; `0` is perpetual |
| `features` | v1 | Extra Pro features granted above the tier |
| `v` | v2 | `2` (absent on v1 tokens) |
| `id` | v2 | Token ID (`kt_` and 24 hex), what a revocation list names. A v1 token is named `sha256:` and the SHA-256 of its text |
| `products` | v2 | Any of `cli`, `desktop`, `station`, `service`. A v2 token names at least one; a v1 token covers all |
| `seats` | v2 | Seats bought (shown in the apps, kept in the register) |
| `stations` | v2 | Stations allowed at once. Absent: one Station, the open allowance. More than one is the Pro "Station fleet" feature |
| `machine` | v2 | Optional binding: the machine hash from `kshana-pro license machine-id` (a salted SHA-256 of the hardware ID; the raw ID never leaves the machine) |
| `kid` | v2 | Key ID of the signing key: the first 16 hex characters of the SHA-256 of its public key |
| `not_before_unix` | v2 | The token is refused before this time |
| `issued_unix` | v2 | When it was issued |

A claims value with no v2 field set serialises exactly as a v1 token did, so v1 tokens
keep verifying unchanged (`tests::v1_claims_serialise_exactly_as_before`).

**Checks, in order:** format; key (a retired key ID, or one the build does not know, is
refused by name); signature; tier; claims version (newer than 2 is refused); product;
not-before; machine binding (a bound token fails closed when this machine's ID cannot
be read); expiry.

**Key rotation.** A build trusts several public keys: `KSHANA_PRO_LICENSE_PUBKEY` takes
one key or a comma-separated list at build time, and `KSHANA_PRO_LICENSE_RETIRED` lists
retired key IDs (or keys). A token names its key by `kid`, so the right key is chosen;
a v1 token (no `kid`) is tried against every active key. A retired key's tokens are
refused, including v1 tokens it signed. A key can also be retired after a release, by
the signed revocation list (section 3), without a rebuild.

## 2. Every Pro feature has a tier

`feature_min_tier` maps each Pro scenario kind and each Pro command feature to the tier
that unlocks it. Anything unmapped falls through to Enterprise (fail closed). The test
`every_pro_kind_and_command_has_an_explicit_tier` fails when a Pro kind from the
registry, or a command feature listed in `PRO_COMMAND_FEATURES`, has no explicit entry,
so a new Pro feature cannot ship without a decision on its tier.

The 0.41 edition matrix (signed 7 November 2026) adds three features that are not
scenario kinds: `pro-station-fleet` (Pro), `pro-service-ha` (Enterprise),
`pro-desktop` (Pro), and `pro-signed-dossier` (Pro) for the signed dossier.

## 3. Activation, renewal, revocation (customer side)

Nothing here needs a network.

```bash
kshana-pro license machine-id              # only for a machine-bound token: send this hash
kshana-pro license activate <token>        # or --file licence.txt; verified, then stored
                                           # (checked for the cli product; for the service, add --product service)
kshana-pro license status                  # state, tier, expiry, products, warnings
kshana-pro license revocations install revocations.txt   # comes with each update
```

The store is `$KSHANA_PRO_HOME` when set, else the per-user configuration directory
(`~/Library/Application Support/Kshana`, `$XDG_CONFIG_HOME/kshana` or
`~/.config/kshana`, `%APPDATA%\Kshana`); `--store <dir>` overrides it. The token is
written readable by its owner only. With no `--license`, every command uses the
activated token.

| State | When | Runs at |
|---|---|---|
| `active` | Valid, more than 14 days left, or perpetual | The token's tier |
| `expiring` | 14 days or less left. Warned at 14 days ("Renew soon"), 7 days ("Renew now") and 1 day ("Renew now (last day)") | The token's tier |
| `grace` | Expired (or ended by the licence server) less than 30 days ago: the renewal window. Pro is locked; a renewal unlocks it without a new download | Evaluation, with a daily warning |
| `expired` | Expired 30 days ago or more (the Pro pack is deleted) | Evaluation |

Owner, 15 November 2026 (roadmap 0.42 C1): subscription tokens last 35 days and the app
renews them automatically while the subscription is active; the 30-day period after expiry
is a renewal window only, with no Pro use. Details, the automatic check and the Pro pack:
docs/PRO-PACKS.md.
| `revoked` | On an installed revocation list | Evaluation |
| `invalid` | Wrong product, machine, unknown or retired key, not yet valid, damaged | Evaluation |

**Revocation** arrives with updates: each release of Desktop, Station and the CLI
packages carries the newest signed list, and an air-gapped site installs it by hand. A
revoked token stops at the next update; nothing asks a server in the meantime. A list
has a `sequence` that only grows; a store refuses a list older than the one it holds,
so an old list cannot be replayed to bring a revoked token back.

**Online refresh** is opt-in and off by default. Only when the store's `refresh.toml`
holds `enabled = true` and a vendor `url` does `kshana-pro license refresh` fetch
`<url>/revocations.txt` and `<url>/token/<token-id>` over HTTPS (the system `curl`).
A fetched list or token is installed only after it verifies, and a token for another
licensee is refused. With refresh off the command stops before any network call
(`tests/licensing.rs`, `online_refresh_is_off_by_default_and_opt_in`).

## 4. Issuing (vendor side)

```bash
kshana-pro license issue --tier pro --licensee "Example Lab" --days 365 \
    --product desktop,cli --seats 3 --register register.jsonl
kshana-pro license trial --licensee "Example Lab" --product desktop --register register.jsonl
kshana-pro license renew kt_... --days 365 --register register.jsonl
kshana-pro license revoke kt_... --reason refund --register register.jsonl
kshana-pro license revocations build --register register.jsonl --out revocations.txt
kshana-pro license reminders --register register.jsonl --within 30
kshana-pro license register verify register.jsonl
```

The **issuance register** is an append-only JSON Lines file, one line per issue, trial,
renewal and revocation (who, tier, products, seats, Stations, expiry, key ID, source,
note, and the token itself so it can be sent again). Each line carries the SHA-256 of
the line before it; an edited, reordered or removed line breaks the chain, and nothing
is appended to, or built from, a broken register.

* **Trial:** 30 days of Pro for the products named, one per licensee.
* **Renewal:** a new token with the same claims, valid from the later of now and the
  old expiry; a revoked or perpetual token is not renewed.
* **Revocation list:** every revoked token ID, plus any retired key IDs, signed; its
  sequence is the register's length.
* **Reminders:** tokens expiring within the window (or in grace) that are neither
  renewed nor revoked.

Payments (F10g) write to the same register through the same functions.

## 5. Tests

`src/license/` unit tests (v1 and v2, each v2 claim, rotation and retired keys, a
swapped key ID, machine binding, tier coverage) and `tests/licensing.rs` (a trial, a
renewal and a revocation end to end from the register; the grace period; revocation
list replay; refresh off and on; the `kshana-pro license` commands driven as a customer
and a vendor).
