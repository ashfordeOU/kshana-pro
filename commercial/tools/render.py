#!/usr/bin/env python3
# PROPRIETARY — Kshana Pro overlay. Internal commercial tooling. Not part of the open-source engine.
"""Render Ashforde OÜ invoices, quotations and order forms to print-ready HTML.

Every document is built from two files and nothing else:

* ``commercial/payment-details.toml`` — the single source of the seller's identity,
  bank account, payment-reference scheme and value added tax (VAT) status;
* one deal file, for example ``commercial/deals/example-deal.toml`` — the buyer, the
  line items and the document numbers and dates.

The renderer REFUSES (non-zero exit, every problem named) while a required payment
detail is empty or invalid, or while a deal value is still ``"PLACEHOLDER"``. So the
day the bank account exists, filling in one file makes every document renderable.
``--draft`` renders a preview anyway, stamped "DRAFT — NOT VALID FOR PAYMENT", with the
missing values shown as pending.

Standard library only; needs Python 3.11 or later (for ``tomllib``).

Usage::

    python3 commercial/tools/render.py check [--payment FILE]
    python3 commercial/tools/render.py render {invoice,quotation,order-form} --deal FILE
        [--payment FILE] [--out DIR] [--draft]

Exit codes: 0 success; 2 bad arguments or unreadable file; 3 payment details not
ready; 4 deal not ready.
"""

from __future__ import annotations

import argparse
import datetime as _dt
import html
import re
import sys
from decimal import Decimal, InvalidOperation, ROUND_HALF_UP
from pathlib import Path

if sys.version_info < (3, 11):  # pragma: no cover - guarded for older interpreters
    sys.stderr.write("render.py needs Python 3.11 or later (it uses the standard tomllib module).\n")
    sys.exit(2)

import tomllib  # noqa: E402  (after the version guard on purpose)

HERE = Path(__file__).resolve().parent
COMMERCIAL = HERE.parent
DEFAULT_PAYMENT = COMMERCIAL / "payment-details.toml"
DEFAULT_OUT = COMMERCIAL / "out"

PLACEHOLDER = "PLACEHOLDER"
DOC_TYPES = ("invoice", "quotation", "order-form")

EXIT_OK = 0
EXIT_USAGE = 2
EXIT_PAYMENT = 3
EXIT_DEAL = 4

# Printed on every order form until counsel has approved the order-form wording. Change
# it (and nothing else) once commercial/counsel/COMMERCIAL-LICENCE-FINALISATION.md is closed.
ORDER_FORM_TEMPLATE_STATUS = "Order-form wording: working draft, pending review by counsel."

# ISO 13616 IBAN lengths for the Single Euro Payments Area (SEPA) countries most likely
# to hold the account. A country missing here still gets the mod-97 checksum test.
IBAN_LENGTHS = {
    "AD": 24, "AT": 20, "BE": 16, "BG": 22, "CH": 21, "CY": 28, "CZ": 24, "DE": 22,
    "DK": 18, "EE": 20, "ES": 24, "FI": 18, "FR": 27, "GB": 22, "GI": 23, "GR": 27,
    "HR": 21, "HU": 28, "IE": 22, "IS": 26, "IT": 27, "LI": 21, "LT": 20, "LU": 20,
    "LV": 21, "MC": 27, "MT": 31, "NL": 18, "NO": 15, "PL": 28, "PT": 25, "RO": 24,
    "SE": 24, "SI": 19, "SK": 24, "SM": 27, "VA": 22,
}

BIC_RE = re.compile(r"^[A-Z]{4}[A-Z]{2}[A-Z0-9]{2}([A-Z0-9]{3})?$")
CURRENCY_RE = re.compile(r"^[A-Z]{3}$")
KMKR_RE = re.compile(r"^EE[0-9]{9}$")
REF_FIELDS = {"invoice_number", "deal_id"}


class NotReady(Exception):
    """Raised with the full list of problems when a document may not be rendered."""

    def __init__(self, kind: str, problems: list[str]):
        super().__init__("\n".join(problems))
        self.kind = kind
        self.problems = problems


# ─────────────────────────────── small helpers ───────────────────────────────


def load_toml(path: Path) -> dict:
    with open(path, "rb") as fh:
        return tomllib.load(fh)


def _s(value) -> str:
    """A value as a stripped string ('' for missing)."""
    if value is None:
        return ""
    return str(value).strip()


def _get(table: dict, dotted: str):
    cur = table
    for part in dotted.split("."):
        if not isinstance(cur, dict) or part not in cur:
            return None
        cur = cur[part]
    return cur


def _is_placeholder(value) -> bool:
    return isinstance(value, str) and value.strip().upper() == PLACEHOLDER


def _norm_name(name: str) -> str:
    return " ".join(name.split()).casefold()


def compact_iban(iban: str) -> str:
    return "".join(iban.split()).upper()


def format_iban(iban: str) -> str:
    c = compact_iban(iban)
    return " ".join(c[i : i + 4] for i in range(0, len(c), 4))


def _mod97(alnum: str) -> int:
    digits = "".join(str(int(ch, 36)) for ch in alnum)
    return int(digits) % 97


def iban_problem(iban: str) -> str | None:
    """None when the IBAN passes the ISO 13616 checks, else a reason."""
    c = compact_iban(iban)
    if not re.fullmatch(r"[A-Z]{2}[0-9]{2}[A-Z0-9]{11,30}", c):
        return "is not shaped like an IBAN (two letters, two check digits, 11–30 letters or digits)"
    want = IBAN_LENGTHS.get(c[:2])
    if want is not None and len(c) != want:
        return f"has {len(c)} characters; a {c[:2]} IBAN has {want}"
    if _mod97(c[4:] + c[:4]) != 1:
        return "fails the ISO 13616 mod-97 checksum (a digit is probably mistyped)"
    return None


def rf_reference(body: str) -> str:
    """ISO 11649 structured creditor reference for ``body`` ("RF" + check digits + body)."""
    body = body.upper()
    check = 98 - _mod97(body + "RF00")
    return f"RF{check:02d}{body}"


def rf_is_valid(ref: str) -> bool:
    ref = ref.replace(" ", "").upper()
    return bool(re.fullmatch(r"RF[0-9]{2}[A-Z0-9]{1,21}", ref)) and _mod97(ref[4:] + ref[:4]) == 1


def _date(value, label: str, problems: list[str]) -> _dt.date | None:
    if isinstance(value, _dt.datetime):
        return value.date()
    if isinstance(value, _dt.date):
        return value
    s = _s(value)
    if not s:
        problems.append(f"{label} is empty")
        return None
    if _is_placeholder(s):
        problems.append(f"{label} is still PLACEHOLDER")
        return None
    try:
        return _dt.date.fromisoformat(s)
    except ValueError:
        problems.append(f"{label} = {s!r} is not a date (use YYYY-MM-DD)")
        return None


def _money(d: Decimal) -> str:
    return format(d.quantize(Decimal("0.01"), rounding=ROUND_HALF_UP), ",.2f")


def _qty(d: Decimal) -> str:
    return format(d.normalize(), "f") if d == d.to_integral() else format(d, "f")


# ─────────────────────────────── validation ───────────────────────────────


def payment_problems(pd: dict) -> list[str]:
    """Every reason the payment-details file is not ready. Empty list means ready."""
    p: list[str] = []
    if pd.get("schema") != 1:
        p.append("schema must be 1")

    for key in ("seller.legal_name", "seller.registry_code", "seller.email"):
        if not _s(_get(pd, key)):
            p.append(f"[{key.split('.')[0]}] {key.split('.')[1]} is empty")
    addr = _get(pd, "seller.registered_address")
    if not isinstance(addr, list) or not [x for x in addr if _s(x)]:
        p.append("[seller] registered_address is empty")

    bank = pd.get("bank") or {}
    for key in ("account_holder", "iban", "bic", "bank_name", "bank_address", "currency"):
        if not _s(bank.get(key)):
            p.append(f"[bank] {key} is empty")
    holder = _s(bank.get("account_holder"))
    seller = _s(_get(pd, "seller.legal_name"))
    if holder and seller and _norm_name(holder) != _norm_name(seller):
        p.append(
            f"[bank] account_holder {holder!r} is not the seller's legal name {seller!r}; "
            "company documents may only name a company account"
        )
    iban = _s(bank.get("iban"))
    if iban:
        why = iban_problem(iban)
        if why:
            p.append(f"[bank] iban {why}")
    bic = _s(bank.get("bic")).upper()
    if bic and not BIC_RE.fullmatch(bic):
        p.append("[bank] bic must be 8 or 11 letters/digits (4 bank, 2 country, 2 location, optional 3 branch)")
    cur = _s(bank.get("currency"))
    if cur and not CURRENCY_RE.fullmatch(cur):
        p.append("[bank] currency must be a three-letter ISO 4217 code such as EUR")

    ref = pd.get("payment_reference") or {}
    scheme = _s(ref.get("scheme"))
    fmt = _s(ref.get("format"))
    if not scheme:
        p.append("[payment_reference] scheme is empty")
    elif scheme not in ("unstructured", "rf"):
        p.append('[payment_reference] scheme must be "unstructured" or "rf"')
    if not fmt:
        p.append("[payment_reference] format is empty")
    else:
        fields = set(re.findall(r"{([^{}]*)}", fmt))
        if "invoice_number" not in fields:
            p.append("[payment_reference] format must contain {invoice_number}")
        unknown = fields - REF_FIELDS
        if unknown:
            p.append(f"[payment_reference] format uses unknown field(s): {', '.join(sorted(unknown))}")

    tax = pd.get("tax") or {}
    status = _s(tax.get("vat_status"))
    if not status:
        p.append("[tax] vat_status is empty")
    elif status not in ("not-registered", "registered"):
        p.append('[tax] vat_status must be "not-registered" or "registered"')
    elif status == "registered" and not KMKR_RE.fullmatch(_s(tax.get("kmkr_number")).upper()):
        p.append('[tax] kmkr_number must be "EE" followed by nine digits when vat_status is "registered"')
    return p


def payment_reference(pd: dict, deal: dict, invoice_number: str) -> tuple[str | None, str | None]:
    """(reference, problem). Assumes payment_problems(pd) is empty."""
    ref = pd["payment_reference"]
    fmt = _s(ref["format"])
    try:
        body = fmt.format(invoice_number=invoice_number, deal_id=_s(_get(deal, "deal.id")))
    except (KeyError, IndexError, ValueError) as exc:
        return None, f"[payment_reference] format could not be expanded: {exc}"
    if _s(ref["scheme"]) == "rf":
        # Separators such as "-" or "/" in an invoice number are dropped: an ISO 11649
        # reference body is letters and digits only.
        compact = re.sub(r"[^A-Za-z0-9]", "", body)
        if not compact:
            return None, "[payment_reference] the rf reference body is empty after dropping separators"
        if len(compact) > 21:
            return None, f"[payment_reference] an rf reference body is at most 21 characters; {compact!r} has {len(compact)}"
        return rf_reference(compact), None
    return body, None


def _items(deal: dict, draft: bool, problems: list[str]) -> list[dict]:
    raw = deal.get("items")
    if not isinstance(raw, list) or not raw:
        problems.append("the deal has no [[items]]")
        return []
    out = []
    for n, it in enumerate(raw, start=1):
        desc = _s(it.get("description"))
        if not desc:
            problems.append(f"item {n}: description is empty")
        try:
            qty = Decimal(_s(it.get("quantity")) or "x")
            if qty <= 0:
                raise InvalidOperation
        except InvalidOperation:
            problems.append(f"item {n}: quantity must be a positive number")
            qty = None
        price_raw = it.get("unit_price")
        price = None
        if _is_placeholder(price_raw):
            if not draft:
                problems.append(f"item {n}: unit_price is still PLACEHOLDER (prices are founder-set, legal/PRICING.md)")
        else:
            try:
                price = Decimal(_s(price_raw) or "x")
                if price < 0:
                    raise InvalidOperation
            except InvalidOperation:
                problems.append(f"item {n}: unit_price must be a non-negative decimal or PLACEHOLDER")
        out.append({"description": desc, "quantity": qty, "unit": _s(it.get("unit")), "unit_price": price})
    return out


def deal_problems(deal: dict, doc_type: str, pd: dict, draft: bool) -> list[str]:
    """Every reason this deal cannot be rendered as ``doc_type``."""
    p: list[str] = []
    if deal.get("schema") != 1:
        p.append("schema must be 1")
    if _get(deal, "deal.example") is True and not draft:
        p.append("[deal] example = true: this is the fictional example deal; copy it to a new file for a real deal, or render it with --draft")
    for key in ("deal.id", "deal.currency", "buyer.legal_name"):
        if not _s(_get(deal, key)):
            p.append(f"[{key.split('.')[0]}] {key.split('.')[1]} is empty")
    addr = _get(deal, "buyer.address")
    if not isinstance(addr, list) or not [x for x in addr if _s(x)]:
        p.append("[buyer] address is empty")

    cur = _s(_get(deal, "deal.currency"))
    bank_cur = _s(_get(pd, "bank.currency"))
    if cur and bank_cur and cur != bank_cur:
        p.append(f"[deal] currency {cur} differs from the bank account currency {bank_cur}")

    _items(deal, draft, p)

    section = doc_type.replace("-", "_")
    if not isinstance(deal.get(section), dict):
        p.append(f"the deal has no [{section}] table")
        return p
    if not _s(_get(deal, f"{section}.number")):
        p.append(f"[{section}] number is empty")

    terms = _get(deal, "deal.payment_terms_days")
    if _is_placeholder(terms):
        if not draft:
            p.append("[deal] payment_terms_days is still PLACEHOLDER (founder-set, legal/PRICING.md)")
    elif terms is None or not str(terms).strip().isdigit():
        p.append("[deal] payment_terms_days must be a whole number of days or PLACEHOLDER")

    if doc_type == "quotation":
        d = _date(_get(deal, "quotation.date"), "[quotation] date", p)
        v = _date(_get(deal, "quotation.valid_until"), "[quotation] valid_until", p)
        if d and v and v < d:
            p.append("[quotation] valid_until is before date")
    elif doc_type == "order-form":
        _date(_get(deal, "order_form.date"), "[order_form] date", p)
        s = _date(_get(deal, "order_form.term_start"), "[order_form] term_start", p)
        e = _date(_get(deal, "order_form.term_end"), "[order_form] term_end", p)
        if s and e and e < s:
            p.append("[order_form] term_end is before term_start")
        if _s(_get(deal, "order_form.tier")) not in ("Evaluation", "Pro", "Enterprise"):
            p.append('[order_form] tier must be "Evaluation", "Pro" or "Enterprise" (legal/COMMERCIAL-LICENSE.md)')
        if not _s(_get(deal, "order_form.licence_reference")):
            p.append("[order_form] licence_reference is empty")
    elif doc_type == "invoice":
        _date(_get(deal, "invoice.issue_date"), "[invoice] issue_date", p)
        if _get(deal, "invoice.due_date") is not None:
            _date(_get(deal, "invoice.due_date"), "[invoice] due_date", p)

    if _s(_get(pd, "tax.vat_status")) == "registered":
        treatment = _s(_get(deal, "vat.treatment"))
        if treatment == "standard":
            try:
                rate = Decimal(_s(_get(deal, "vat.rate_percent")) or "x")
                if not (0 <= rate <= 100):
                    raise InvalidOperation
            except InvalidOperation:
                p.append("[vat] rate_percent must be a number from 0 to 100 for standard treatment")
        elif treatment == "reverse-charge":
            if not _s(_get(deal, "buyer.vat_id")):
                p.append("[vat] reverse-charge needs the buyer's VAT identification number in [buyer] vat_id")
        else:
            p.append('[vat] treatment must be "standard" or "reverse-charge" when the seller is VAT-registered')
    return p


# ─────────────────────────────── rendering ───────────────────────────────

CSS = """
@page { size: A4; margin: 18mm 16mm 20mm 16mm; }
:root { --ink:#1b1f24; --muted:#5b6470; --rule:#d5d9df; --accent:#8a1c1c; --paper:#ffffff; }
* { box-sizing: border-box; }
body { margin:0; background:var(--paper); color:var(--ink);
       font: 10.5pt/1.45 "Helvetica Neue", Helvetica, Arial, sans-serif; }
main { max-width: 178mm; margin: 0 auto; padding: 12mm 0; }
header { display:flex; justify-content:space-between; align-items:flex-start;
         border-bottom: 2px solid var(--ink); padding-bottom: 6mm; margin-bottom: 6mm; }
h1 { font-size: 20pt; letter-spacing: .02em; margin: 0 0 2mm 0; }
h2 { font-size: 10.5pt; text-transform: uppercase; letter-spacing: .08em; color: var(--muted);
     margin: 6mm 0 2mm 0; }
.seller { text-align:right; font-size: 9.5pt; }
.seller strong { font-size: 11pt; }
.meta { display:grid; grid-template-columns: 1fr 1fr; gap: 6mm; }
.kv { display:grid; grid-template-columns: 38mm 1fr; row-gap: 1mm; }
.kv dt { color: var(--muted); } .kv dd { margin:0; }
table { width:100%; border-collapse: collapse; margin-top: 3mm; }
th, td { padding: 2mm 1.5mm; border-bottom: 1px solid var(--rule); vertical-align: top; }
th { text-align:left; font-size: 9pt; color: var(--muted); text-transform: uppercase; letter-spacing:.05em; }
td.num, th.num { text-align:right; white-space:nowrap; font-variant-numeric: tabular-nums; }
tr.total td { border-top: 2px solid var(--ink); border-bottom: none; font-weight: 700; }
.box { border: 1px solid var(--ink); padding: 4mm; margin-top: 6mm; break-inside: avoid; }
.note { color: var(--muted); font-size: 9pt; }
.pending { color: var(--accent); font-weight: 700; }
.draft { border: 3px solid var(--accent); color: var(--accent); text-align:center; font-weight: 800;
         letter-spacing: .12em; padding: 3mm; margin-bottom: 6mm; }
.sign { display:grid; grid-template-columns: 1fr 1fr; gap: 10mm; margin-top: 10mm; }
.sign div { border-top: 1px solid var(--ink); padding-top: 2mm; min-height: 22mm; }
footer { margin-top: 10mm; border-top: 1px solid var(--rule); padding-top: 3mm; font-size: 8.5pt; color: var(--muted); }
@media print { main { padding: 0; } .draft { -webkit-print-color-adjust: exact; print-color-adjust: exact; } }
"""

TITLES = {"invoice": "Invoice", "quotation": "Quotation", "order-form": "Order form"}


def _e(s) -> str:
    return html.escape(_s(s))


def _lines(values) -> str:
    return "<br>".join(_e(v) for v in (values or []) if _s(v))


def _seller_block(pd: dict) -> str:
    s = pd.get("seller") or {}
    return (
        f'<div class="seller"><strong>{_e(s.get("legal_name"))}</strong><br>'
        f'{_lines(s.get("registered_address"))}<br>'
        f'Registry code {_e(s.get("registry_code"))}<br>'
        f'{_e(s.get("email"))}' + (f'<br>{_e(s.get("website"))}' if _s(s.get("website")) else "") + "</div>"
    )


def _buyer_block(deal: dict) -> str:
    b = deal.get("buyer") or {}
    rows = [f"<strong>{_e(b.get('legal_name'))}</strong>", _lines(b.get("address"))]
    if _s(b.get("registry_code")):
        rows.append(f"Registry code {_e(b.get('registry_code'))}")
    if _s(b.get("vat_id")):
        rows.append(f"VAT identification number {_e(b.get('vat_id'))}")
    if _s(b.get("contact_name")) or _s(b.get("contact_email")):
        rows.append(f"Attention: {_e(b.get('contact_name'))} {_e(b.get('contact_email'))}".strip())
    return "<br>".join(r for r in rows if r)


def _kv(rows: list[tuple[str, str]]) -> str:
    return '<dl class="kv">' + "".join(f"<dt>{k}</dt><dd>{v}</dd>" for k, v in rows) + "</dl>"


def _pending(text: str = "pending") -> str:
    return f'<span class="pending">[{html.escape(text)}]</span>'


def _vat(pd: dict, deal: dict, subtotal: Decimal | None) -> tuple[Decimal | None, str, str]:
    """(vat amount or None, label, note). Only called for renderable deals."""
    status = _s(_get(pd, "tax.vat_status"))
    if status == "registered":
        kmkr = _e(_get(pd, "tax.kmkr_number"))
        treatment = _s(_get(deal, "vat.treatment"))
        if treatment == "reverse-charge":
            return (
                Decimal("0"),
                "VAT (reverse charge)",
                f"Seller VAT number (KMKR) {kmkr}. Reverse charge: VAT is to be accounted for by the "
                "recipient under Article 196 of Council Directive 2006/112/EC.",
            )
        rate = Decimal(_s(_get(deal, "vat.rate_percent")) or "0")
        amount = None if subtotal is None else (subtotal * rate / Decimal(100))
        return amount, f"VAT {_qty(rate)}%", f"Seller VAT number (KMKR) {kmkr}."
    if status == "not-registered":
        return None, "", "The seller is not registered for value added tax (VAT). No VAT is charged."
    return None, "", "VAT status: " + _pending()


def _items_table(items: list[dict], currency: str, pd: dict, deal: dict) -> tuple[str, Decimal | None]:
    rows = []
    subtotal: Decimal | None = Decimal("0")
    for it in items:
        qty, price = it["quantity"], it["unit_price"]
        if qty is None or price is None:
            line = None
            subtotal = None
        else:
            line = qty * price
            if subtotal is not None:
                subtotal += line
        rows.append(
            "<tr>"
            f"<td>{_e(it['description'])}</td>"
            f"<td class='num'>{_qty(qty) if qty is not None else _pending()} {_e(it['unit'])}</td>"
            f"<td class='num'>{_money(price) if price is not None else _pending('price to be set')}</td>"
            f"<td class='num'>{_money(line) if line is not None else _pending()}</td>"
            "</tr>"
        )
    vat_amount, vat_label, vat_note = _vat(pd, deal, subtotal)
    foot = [
        f"<tr><td colspan='3' class='num'>Subtotal ({_e(currency)})</td>"
        f"<td class='num'>{_money(subtotal) if subtotal is not None else _pending()}</td></tr>"
    ]
    total = subtotal
    if vat_label:
        foot.append(
            f"<tr><td colspan='3' class='num'>{_e(vat_label)}</td>"
            f"<td class='num'>{_money(vat_amount) if vat_amount is not None else _pending()}</td></tr>"
        )
        total = None if (subtotal is None or vat_amount is None) else subtotal + vat_amount
    foot.append(
        f"<tr class='total'><td colspan='3' class='num'>Total ({_e(currency)})</td>"
        f"<td class='num'>{_money(total) if total is not None else _pending()}</td></tr>"
    )
    table = (
        "<table><thead><tr><th>Description</th><th class='num'>Quantity</th>"
        f"<th class='num'>Unit price ({_e(currency)})</th><th class='num'>Amount ({_e(currency)})</th></tr></thead>"
        f"<tbody>{''.join(rows)}{''.join(foot)}</tbody></table>"
        f"<p class='note'>{vat_note}</p>"
    )
    return table, total


def _bank_box(pd: dict, deal: dict, ready: bool, heading: str, reference_for: str | None) -> str:
    if not ready:
        return (
            f'<div class="box"><h2>{html.escape(heading)}</h2>'
            f"<p>{_pending('payment details pending: commercial/payment-details.toml is incomplete')}</p></div>"
        )
    b = pd["bank"]
    rows = [
        ("Account holder", _e(b["account_holder"])),
        ("IBAN (International Bank Account Number)", _e(format_iban(b["iban"]))),
        ("BIC (Business Identifier Code)", _e(_s(b["bic"]).upper())),
        ("Bank", f"{_e(b['bank_name'])}<br>{_e(b['bank_address'])}"),
        ("Currency", _e(b["currency"])),
    ]
    if reference_for is not None:
        ref, _ = payment_reference(pd, deal, reference_for)
        rows.append(("Payment reference", f"<strong>{_e(ref)}</strong>"))
    return f'<div class="box"><h2>{html.escape(heading)}</h2>{_kv(rows)}</div>'


def _page(title: str, number: str, body: str, draft: bool, pd: dict) -> str:
    seller = _e(_get(pd, "seller.legal_name"))
    banner = '<div class="draft">DRAFT — NOT VALID FOR PAYMENT</div>' if draft else ""
    return (
        "<!DOCTYPE html>\n<html lang=\"en\"><head><meta charset=\"utf-8\">"
        f"<title>{html.escape(title)} {html.escape(number)}</title>"
        "<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">"
        f"<style>{CSS}</style></head><body><main>{banner}"
        f"<header><div><h1>{html.escape(title)}</h1><div>No. {html.escape(number)}</div></div>{_seller_block(pd)}</header>"
        f"{body}"
        f"<footer>{seller} · Registry code {_e(_get(pd, 'seller.registry_code'))} · "
        f"{_e(_get(pd, 'seller.email'))}</footer>"
        "</main></body></html>\n"
    )


def _terms_text(deal: dict) -> str:
    terms = _get(deal, "deal.payment_terms_days")
    return _pending("payment terms to be set") if _is_placeholder(terms) else f"{_s(terms)} days from the invoice date"


def render(doc_type: str, pd: dict, deal: dict, draft: bool = False) -> str:
    """Return the HTML for ``doc_type``. Raises NotReady unless ready (or ``draft``)."""
    if doc_type not in DOC_TYPES:
        raise ValueError(f"unknown document type {doc_type!r}")
    pay = payment_problems(pd)
    if pay and not draft:
        raise NotReady("payment", pay)
    dp = deal_problems(deal, doc_type, pd if not pay else {}, draft)
    if dp:
        raise NotReady("deal", dp)
    ready = not pay
    currency = _s(_get(deal, "deal.currency"))
    items = _items(deal, draft, [])
    table, total = _items_table(items, currency, pd if ready else {**pd, "tax": {}}, deal)
    section = doc_type.replace("-", "_")
    number = _s(_get(deal, f"{section}.number"))
    parties = f'<div class="meta"><div><h2>Bill to</h2>{_buyer_block(deal)}</div><div>'

    if doc_type == "invoice":
        issue = _date(_get(deal, "invoice.issue_date"), "", [])
        due_raw = _get(deal, "invoice.due_date")
        if due_raw is not None:
            due_text = _e(_date(due_raw, "", []).isoformat())
        elif _is_placeholder(_get(deal, "deal.payment_terms_days")):
            due_text = _pending("due date follows the payment terms")
        else:
            due_text = _e((issue + _dt.timedelta(days=int(_s(_get(deal, "deal.payment_terms_days"))))).isoformat())
        ref = _pending()
        if ready:
            r, problem = payment_reference(pd, deal, number)
            if problem:
                raise NotReady("payment", [problem])
            ref = _e(r)
        meta = _kv(
            [
                ("Invoice number", _e(number)),
                ("Invoice date", _e(issue.isoformat())),
                ("Due date", due_text),
                ("Deal", _e(_get(deal, "deal.id"))),
                ("Payment reference", ref),
            ]
        )
        body = (
            parties + f"<h2>Details</h2>{meta}</div></div>"
            f"<h2>{_e(_get(deal, 'deal.title'))}</h2>{table}"
            + _bank_box(pd, deal, ready, "Pay by bank transfer to", number)
            + "<p class='note'>Please quote the payment reference exactly, so the payment can be matched to this invoice.</p>"
        )
        return _page("Invoice", number, body, draft, pd)

    if doc_type == "quotation":
        meta = _kv(
            [
                ("Quotation number", _e(number)),
                ("Date", _e(_date(_get(deal, "quotation.date"), "", []).isoformat())),
                ("Valid until", _e(_date(_get(deal, "quotation.valid_until"), "", []).isoformat())),
                ("Payment terms", _terms_text(deal)),
            ]
        )
        body = (
            parties.replace("Bill to", "Prepared for") + f"<h2>Details</h2>{meta}</div></div>"
            f"<h2>{_e(_get(deal, 'deal.title'))}</h2>{table}"
            + _bank_box(pd, deal, ready, "Payment will be requested by invoice, payable to", None)
            + "<p class='note'>This quotation is not an invoice. Work is supplied under the Kshana Pro "
            "Commercial Licence Agreement and a signed order form. Study reports are modelled "
            "decision-support evidence, not a certification.</p>"
        )
        return _page("Quotation", number, body, draft, pd)

    # order form
    of = deal["order_form"]
    meta = _kv(
        [
            ("Order form number", _e(number)),
            ("Date", _e(_date(of.get("date"), "", []).isoformat())),
            ("Licence tier", _e(of.get("tier"))),
            ("Term", f"{_e(_date(of.get('term_start'), '', []).isoformat())} to {_e(_date(of.get('term_end'), '', []).isoformat())}"),
            ("Payment terms", _terms_text(deal)),
        ]
    )
    body = (
        parties.replace("Bill to", "Licensee") + f"<h2>Details</h2>{meta}</div></div>"
        f"<h2>{_e(_get(deal, 'deal.title'))}</h2>{table}"
        f"<h2>Terms</h2><p>This order form is governed by: {_e(of.get('licence_reference'))}. "
        "Where this order form and that agreement conflict, the agreement governs unless this "
        "order form expressly says otherwise.</p>"
        + _bank_box(pd, deal, ready, "Fees are invoiced and payable by bank transfer to", None)
        + '<div class="sign"><div>For ' + _e(_get(pd, "seller.legal_name"))
        + "<br>Name, title, date</div><div>For " + _e(_get(deal, "buyer.legal_name"))
        + "<br>Name, title, date</div></div>"
        f"<p class='note'>{html.escape(ORDER_FORM_TEMPLATE_STATUS)}</p>"
    )
    return _page("Order form", number, body, draft, pd)


# ─────────────────────────────── command line ───────────────────────────────


def _report(kind: str, problems: list[str], source: str) -> None:
    what = "payment details" if kind == "payment" else "deal"
    sys.stderr.write(f"REFUSED: the {what} in {source} are not ready. Fix every item below:\n")
    for pr in problems:
        sys.stderr.write(f"  - {pr}\n")
    if kind == "payment":
        sys.stderr.write(
            "Fill commercial/payment-details.toml from the bank's account confirmation, then run "
            "'render.py check'. Use --draft for a watermarked preview in the meantime.\n"
        )


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description="Render Ashforde OÜ commercial documents to print-ready HTML.")
    sub = ap.add_subparsers(dest="cmd", required=True)
    c = sub.add_parser("check", help="report whether payment-details.toml is complete and valid")
    c.add_argument("--payment", type=Path, default=DEFAULT_PAYMENT)
    r = sub.add_parser("render", help="render one document for one deal")
    r.add_argument("doc_type", choices=DOC_TYPES)
    r.add_argument("--deal", type=Path, required=True)
    r.add_argument("--payment", type=Path, default=DEFAULT_PAYMENT)
    r.add_argument("--out", type=Path, default=DEFAULT_OUT)
    r.add_argument("--draft", action="store_true", help="render a watermarked preview even when not ready")
    try:
        args = ap.parse_args(argv)
    except SystemExit as exc:
        return EXIT_USAGE if exc.code else EXIT_OK

    try:
        pd = load_toml(args.payment)
    except (OSError, tomllib.TOMLDecodeError) as exc:
        sys.stderr.write(f"cannot read payment details {args.payment}: {exc}\n")
        return EXIT_USAGE

    if args.cmd == "check":
        problems = payment_problems(pd)
        if problems:
            _report("payment", problems, str(args.payment))
            return EXIT_PAYMENT
        print(f"READY: {args.payment} is complete and valid.")
        if _s(_get(pd, "seller.registry_check")) == "verify":
            print("NOTE: [seller] registry_check is still 'verify'; re-check the identity against the e-Business Register.")
        return EXIT_OK

    try:
        deal = load_toml(args.deal)
    except (OSError, tomllib.TOMLDecodeError) as exc:
        sys.stderr.write(f"cannot read deal {args.deal}: {exc}\n")
        return EXIT_USAGE
    try:
        page = render(args.doc_type, pd, deal, draft=args.draft)
    except NotReady as nr:
        _report(nr.kind, nr.problems, str(args.payment if nr.kind == "payment" else args.deal))
        return EXIT_PAYMENT if nr.kind == "payment" else EXIT_DEAL
    number = _s(_get(deal, f"{args.doc_type.replace('-', '_')}.number"))
    safe = re.sub(r"[^A-Za-z0-9._-]", "_", number)
    args.out.mkdir(parents=True, exist_ok=True)
    path = args.out / f"{args.doc_type}-{safe}{'-DRAFT' if args.draft else ''}.html"
    path.write_text(page, encoding="utf-8")
    print(path)
    return EXIT_OK


if __name__ == "__main__":
    sys.exit(main())
