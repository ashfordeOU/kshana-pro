# Payments and paywall (off by default)

Roadmap F10g. Code: `src/payments/` (library), `src/payments_cmd.rs` (the
`kshana-pro payments` commands); configuration: `commercial/payments.toml`; tests:
`tests/payments.rs` with the webhook bodies in `tests/fixtures/payments/`.

Abbreviations used on this page, in full:

| Abbreviation | Meaning |
|---|---|
| HMAC | hash-based message authentication code |
| HTTP | HyperText Transfer Protocol |
| ID | identifier |
| JSON | JavaScript Object Notation |
| SHA-256 | Secure Hash Algorithm, 256-bit |
| TLS | Transport Layer Security |
| URL | uniform resource locator |
| VAT | value added tax |

## 1. What it does

A merchant of record (Paddle or Lemon Squeezy) sells a Kshana edition and handles VAT
and sales tax. Its signed webhook reaches the payments service, which:

| Provider event | Action |
|---|---|
| Paddle `transaction.completed` (first purchase); Lemon Squeezy `order_created` (paid) | Issue a token for the edition (tier, products, seats = quantity, expiry = edition days) through the issuance register; write it to the outbox for the buyer |
| Paddle `transaction.completed` with `origin = subscription_recurring`; Lemon Squeezy `subscription_payment_success` with `billing_reason = renewal` | Renew the subscription's token by the edition's days |
| Lemon Squeezy `subscription_created` | Link the subscription to the order's token |
| Paddle `adjustment.created/updated` (full approved refund, or chargeback); Lemon Squeezy `order_refunded` (full), `subscription_payment_refunded` | Revoke the token that payment issued; rewrite the signed revocation list |
| Paddle `subscription.canceled`; Lemon Squeezy `subscription_expired` | Revoke the subscription's current token (Lemon Squeezy's `subscription_cancelled` is ignored: the licence ends when the subscription expires) |
| Anything else | Ignored and logged |

The revocation list (`<data_dir>/revocations.txt`) is what the next app update carries
(docs/LICENSING.md section 3), so a refunded licence stops at that update.

## 2. The switch

`enabled` in `payments.toml`, **false** in the committed file. While it is false:

* `kshana-pro payments serve` refuses to start;
* a delivery to an issuer built anyway is answered 404 and writes nothing;
* the paywall (`kshana-pro payments checkout`, and the apps' "Upgrade") returns
  "payments are off" and shows no checkout and no price.

The payments code makes no outbound connection in any state: it builds checkout URLs
and listens for the provider. A test reads the sources and fails if one ever opens a
connection or runs a program.

## 3. Refusals

| Delivery | Answer | Effect |
|---|---|---|
| No signature, malformed, or not matching (wrong secret, edited body) | 401 | None |
| Paddle signed timestamp older or newer than the tolerance (300 s) | 401 | None: a recorded delivery replayed later is refused |
| An event already handled (Paddle `event_id`; Lemon Squeezy body digest, as its signature carries no time or ID) | 200, "replay" | Not acted on again |
| Older than the last state applied to the same order or subscription | 409 | None (out of order) |
| A refund, renewal, link or cancellation for an order or subscription not seen yet | 409 | None; the provider retries, and the retry is handled once the purchase is in |
| The other provider's endpoint | 404 | None |

Every delivery, applied or refused, is one line of `<data_dir>/events.jsonl`.

## 4. Signatures

* **Paddle Billing:** `Paddle-Signature: ts=<unix>;h1=<hex>`, HMAC-SHA256 with the
  destination's secret over `<ts>:<raw body>`; several `h1` are accepted while a secret
  is rotated.
* **Lemon Squeezy:** `X-Signature: <hex>`, HMAC-SHA256 of the raw body with the
  webhook's signing secret.

The HMAC is checked against the RFC 4231 vectors; comparison takes constant time.

## 5. Running it (after gate 12)

Vendor side only: the service holds a signing key, its own (docs/LICENSING-KEYS.md
section 4), and never runs in a customer build or service. TLS is terminated in front
of it by a reverse proxy.

```bash
kshana-pro payments check --config /etc/kshana-payments/payments.toml
kshana-pro payments serve --config /etc/kshana-payments/payments.toml --addr 127.0.0.1:8090
```

Delivery to the buyer: each token is written to `<data_dir>/outbox/<token-id>.json`
(address, subject, body, token); the mail step that sends it is the operator's.

## 6. What is not done, and who decides

* **Owner gate 12:** choose the provider, set the prices and the provider price IDs
  (every one is `PLACEHOLDER`; `mode = "live"` is refused until none is), open the
  business bank account, have the accountant confirm the VAT treatment. Then create the
  provider account, the webhook destination and its secret, and switch `enabled` on.
* **Provider sandbox:** no account exists, so the tests use self-made bodies in the
  providers' published shapes, signed with their published schemes. The first run
  against the provider's sandbox should record real deliveries into
  `tests/fixtures/payments/` and re-run the suite on them.
* **E-mail sending** from the outbox, and the site's pricing page and checkout page
  (Paddle opens its checkout from a script on that page), are built when the provider
  is chosen.
