# Test vectors for the Pro service's OIDC verifier (src/serve/oidc.rs), signed by an independent
# implementation (Python `cryptography`). Run from this folder: python3 make_tokens.py
# It writes jwks.json, oidc.toml and tokens.json. The private keys are generated per run and discarded.
import base64, hashlib, hmac, json
from cryptography.hazmat.primitives import hashes
from cryptography.hazmat.primitives.asymmetric import ec, padding, rsa
from cryptography.hazmat.primitives.asymmetric.utils import decode_dss_signature

NOW = 1_800_000_000
b64 = lambda b: base64.urlsafe_b64encode(b).rstrip(b"=").decode()
num = lambda n, size: n.to_bytes(size, "big")

ec_key = ec.generate_private_key(ec.SECP256R1())
rsa_key = rsa.generate_private_key(public_exponent=65537, key_size=2048)
other_ec = ec.generate_private_key(ec.SECP256R1())
ecn, rsn = ec_key.public_key().public_numbers(), rsa_key.public_key().public_numbers()
jwks = {"keys": [
    {"kty": "EC", "crv": "P-256", "kid": "ec1", "use": "sig", "alg": "ES256", "x": b64(num(ecn.x, 32)), "y": b64(num(ecn.y, 32))},
    {"kty": "RSA", "kid": "rsa1", "use": "sig", "alg": "RS256", "n": b64(num(rsn.n, 256)), "e": b64(num(rsn.e, 3))},
]}

def sign(alg, kid, claims, key=None, header_extra=None):
    h = {"alg": alg, "typ": "JWT", "kid": kid, **(header_extra or {})}
    si = (b64(json.dumps(h, separators=(",", ":")).encode()) + "." + b64(json.dumps(claims, separators=(",", ":")).encode())).encode()
    if alg == "ES256":
        r, s = decode_dss_signature((key or ec_key).sign(si, ec.ECDSA(hashes.SHA256())))
        sig = num(r, 32) + num(s, 32)
    elif alg == "RS256":
        sig = rsa_key.sign(si, padding.PKCS1v15(), hashes.SHA256())
    elif alg == "HS256":
        sig = hmac.new(b"secret", si, hashlib.sha256).digest()
    else:
        sig = b""
    return si.decode() + "." + b64(sig)

good = {"iss": "https://idp.example/realms/ops", "aud": ["kshana-pro", "other"], "sub": "alice", "exp": NOW + 600,
        "nbf": NOW - 60, "kshana_role": ["reader", "submitter"], "kshana_tenant": "site-a"}
def c(**kw):
    d = dict(good); d.update(kw)
    return {k: v for k, v in d.items() if v is not None}

es = sign("ES256", "ec1", good)
tampered = es.split(".")
tampered[1] = b64(json.dumps(c(kshana_role="admin"), separators=(",", ":")).encode())
tokens = {
    "now": NOW,
    "pass": {
        "es256": [sign("ES256", "ec1", good), {"id": "alice", "tenant": "site-a", "role": "submitter"}],
        "rs256": [sign("RS256", "rsa1", c(sub="bob", aud="kshana-pro", kshana_role="admin", kshana_tenant="site-b")), {"id": "bob", "tenant": "site-b", "role": "admin"}],
        # ECDSA signatures from this signer are not normalised to low S, so a batch covers both halves.
        **{f"es256-{i}": [sign("ES256", "ec1", c(jti=str(i))), {"id": "alice", "tenant": "site-a", "role": "submitter"}] for i in range(8)},
        "leeway": [sign("ES256", "ec1", c(exp=NOW - 30)), {"id": "alice", "tenant": "site-a", "role": "submitter"}],
    },
    "refuse": {
        "tampered": [".".join(tampered), "Signature"],
        "other-key": [sign("ES256", "ec1", good, key=other_ec), "Signature"],
        "expired": [sign("ES256", "ec1", c(exp=NOW - 120)), "Expired"],
        "no-exp": [sign("ES256", "ec1", c(exp=None)), "Expired"],
        "not-yet": [sign("ES256", "ec1", c(nbf=NOW + 120)), "NotYetValid"],
        "issuer": [sign("ES256", "ec1", c(iss="https://evil.example")), "Issuer"],
        "audience": [sign("ES256", "ec1", c(aud="someone-else")), "Audience"],
        "unknown-kid": [sign("ES256", "nope", good), "UnknownKey"],
        "alg-none": [sign("none", "ec1", good), "Algorithm"],
        "hs256": [sign("HS256", "ec1", good), "Algorithm"],
        "alg-key-mismatch": [sign("ES256", "rsa1", good), "Algorithm"],
        "no-role": [sign("ES256", "ec1", c(kshana_role=None)), "Claims"],
        "bad-role": [sign("ES256", "ec1", c(kshana_role="root")), "Claims"],
        "all-tenants": [sign("ES256", "ec1", c(kshana_tenant="*")), "Claims"],
        "no-tenant": [sign("ES256", "ec1", c(kshana_tenant=None)), "Claims"],
    },
}
open("jwks.json", "w").write(json.dumps(jwks, indent=1) + "\n")
open("tokens.json", "w").write(json.dumps(tokens, indent=1) + "\n")
open("oidc.toml", "w").write('issuer = "https://idp.example/realms/ops"\naudience = "kshana-pro"\njwks_file = "jwks.json"\nrole_claim = "kshana_role"\ntenant_claim = "kshana_tenant"\n')
