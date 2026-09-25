# PROPRIETARY — Kshana Pro overlay. Internal commercial tooling. Not part of the open-source engine.
"""Tests for commercial/tools/render.py.

Run from the repository root:

    python3 -m unittest discover -s commercial/tools -p 'test_*.py'

The tests never depend on whether the shipped payment-details.toml has been filled in:
they build their own empty and filled copies, so they stay green before and after the
bank account exists.
"""

from __future__ import annotations

import copy
import io
import sys
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from decimal import Decimal
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import render  # noqa: E402

SHIPPED_PAYMENT = HERE.parent / "payment-details.toml"
EXAMPLE_DEAL = HERE.parent / "deals" / "example-deal.toml"

# Documentation-only account data. GB82 WEST 1234 5698 7654 32 is the example IBAN
# printed in ISO 13616 and by the European Committee for Banking Standards; it passes
# the checksum and belongs to nobody.
DOC_IBAN = "GB82 WEST 1234 5698 7654 32"


def empty_payment() -> dict:
    pd = render.load_toml(SHIPPED_PAYMENT)
    pd = copy.deepcopy(pd)
    for key in ("account_holder", "iban", "bic", "bank_name", "bank_address", "currency"):
        pd["bank"][key] = ""
    pd["payment_reference"] = {"scheme": "", "format": ""}
    pd["tax"] = {"vat_status": "", "kmkr_number": ""}
    return pd


def filled_payment(**over) -> dict:
    pd = empty_payment()
    pd["bank"].update(
        account_holder=pd["seller"]["legal_name"],
        iban=DOC_IBAN,
        bic="WESTGB2L",
        bank_name="Example Bank",
        bank_address="1 Example Road, Example City",
        currency="EUR",
    )
    pd["payment_reference"] = {"scheme": "unstructured", "format": "ASH {invoice_number}"}
    pd["tax"] = {"vat_status": "not-registered", "kmkr_number": ""}
    for dotted, value in over.items():
        section, key = dotted.split("__")
        pd[section][key] = value
    return pd


def example_deal() -> dict:
    return copy.deepcopy(render.load_toml(EXAMPLE_DEAL))


def priced_deal() -> dict:
    deal = example_deal()
    deal["deal"]["example"] = False  # a real deal copied from the example
    deal["deal"]["payment_terms_days"] = 30
    deal["items"][0]["unit_price"] = "1250.50"
    deal["items"][0]["quantity"] = "2"
    deal["items"][1]["unit_price"] = "99.99"
    return deal


class ExampleDealGuard(unittest.TestCase):
    def test_example_deal_is_refused_unless_draft(self):
        deal = example_deal()
        deal["deal"]["example"] = True
        deal["deal"]["payment_terms_days"] = 30
        deal["items"][0]["unit_price"] = "1250.50"
        deal["items"][1]["unit_price"] = "99.99"
        problems = render.deal_problems(deal, "invoice", filled_payment(), draft=False)
        self.assertTrue(any("example = true" in p for p in problems), problems)
        draft_problems = render.deal_problems(deal, "invoice", filled_payment(), draft=True)
        self.assertFalse(any("example = true" in p for p in draft_problems), draft_problems)


class ShippedFiles(unittest.TestCase):
    def test_shipped_payment_file_parses_and_seller_is_complete(self):
        pd = render.load_toml(SHIPPED_PAYMENT)
        problems = render.payment_problems(pd)
        # Before go-live only bank, reference and tax fields may be missing; after go-live none.
        for pr in problems:
            self.assertTrue(
                pr.startswith(("[bank]", "[payment_reference]", "[tax]")), f"unexpected problem: {pr}"
            )

    def test_shipped_example_deal_is_placeholder_priced_only(self):
        problems = render.deal_problems(example_deal(), "invoice", filled_payment(), draft=False)
        self.assertTrue(problems)
        # The example flag is its own refusal; the remaining ones must all be placeholders.
        problems = [p for p in problems if "example = true" not in p]
        for pr in problems:
            self.assertIn("PLACEHOLDER", pr)


class Refusal(unittest.TestCase):
    def test_empty_payment_details_name_every_field(self):
        problems = render.payment_problems(empty_payment())
        for field in ("account_holder", "iban", "bic", "bank_name", "bank_address", "currency"):
            self.assertIn(f"[bank] {field} is empty", problems)
        self.assertIn("[payment_reference] scheme is empty", problems)
        self.assertIn("[payment_reference] format is empty", problems)
        self.assertIn("[tax] vat_status is empty", problems)

    def test_every_document_type_refuses_without_payment_details(self):
        for doc in render.DOC_TYPES:
            with self.assertRaises(render.NotReady) as ctx:
                render.render(doc, empty_payment(), priced_deal())
            self.assertEqual(ctx.exception.kind, "payment")

    def test_cli_exit_codes_and_nothing_written_on_refusal(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp)
            pay = tmp / "pay.toml"
            pay.write_text(_toml(empty_payment()), encoding="utf-8")
            out = tmp / "out"
            err = io.StringIO()
            with redirect_stderr(err):
                self.assertEqual(render.main(["check", "--payment", str(pay)]), render.EXIT_PAYMENT)
                code = render.main(
                    ["render", "invoice", "--deal", str(EXAMPLE_DEAL), "--payment", str(pay), "--out", str(out)]
                )
            self.assertEqual(code, render.EXIT_PAYMENT)
            self.assertIn("REFUSED", err.getvalue())
            self.assertFalse(out.exists())

            pay.write_text(_toml(filled_payment()), encoding="utf-8")
            with redirect_stdout(io.StringIO()):
                self.assertEqual(render.main(["check", "--payment", str(pay)]), render.EXIT_OK)
            err = io.StringIO()
            with redirect_stderr(err):
                code = render.main(
                    ["render", "invoice", "--deal", str(EXAMPLE_DEAL), "--payment", str(pay), "--out", str(out)]
                )
            self.assertEqual(code, render.EXIT_DEAL)
            self.assertIn("PLACEHOLDER", err.getvalue())

    def test_bad_iban_checksum_is_refused(self):
        pd = filled_payment(bank__iban="GB82 WEST 1234 5698 7654 33")
        self.assertTrue(any("mod-97" in p for p in render.payment_problems(pd)))

    def test_wrong_length_iban_is_refused(self):
        pd = filled_payment(bank__iban="EE38 2200 2210 2014 568")
        self.assertTrue(any("EE IBAN has 20" in p for p in render.payment_problems(pd)))

    def test_account_holder_must_be_the_company(self):
        pd = filled_payment(bank__account_holder="A Private Person")
        self.assertTrue(any("account_holder" in p and "legal name" in p for p in render.payment_problems(pd)))
        # Case and spacing differences are not a different holder.
        pd = filled_payment(bank__account_holder="  ashforde   oü ")
        self.assertEqual(render.payment_problems(pd), [])

    def test_bad_bic_currency_and_reference_format(self):
        pd = filled_payment(bank__bic="WEST", bank__currency="euro")
        pd["payment_reference"]["format"] = "ASH-{order}"
        problems = render.payment_problems(pd)
        self.assertTrue(any(p.startswith("[bank] bic") for p in problems))
        self.assertTrue(any(p.startswith("[bank] currency") for p in problems))
        self.assertTrue(any("{invoice_number}" in p for p in problems))
        self.assertTrue(any("unknown field" in p for p in problems))

    def test_registered_without_kmkr_is_refused(self):
        pd = filled_payment(tax__vat_status="registered")
        self.assertTrue(any("kmkr_number" in p for p in render.payment_problems(pd)))

    def test_currency_mismatch_is_refused(self):
        deal = priced_deal()
        deal["deal"]["currency"] = "GBP"
        problems = render.deal_problems(deal, "invoice", filled_payment(), draft=False)
        self.assertTrue(any("differs from the bank account currency" in p for p in problems))


class Rendering(unittest.TestCase):
    def test_invoice_renders_with_correct_totals_and_bank_block(self):
        page = render.render("invoice", filled_payment(), priced_deal())
        self.assertIn("GB82 WEST 1234 5698 7654 32", page)
        self.assertIn("WESTGB2L", page)
        # 2 × 1,250.50 + 1 × 99.99 = 2,600.99
        self.assertIn("2,600.99", page)
        self.assertIn("ASH INV-EXAMPLE-0001", page)
        self.assertIn("2026-10-31", page)  # 2026-10-01 + 30 days
        self.assertIn("not registered for value added tax (VAT)", page)
        self.assertIn("IBAN (International Bank Account Number)", page)
        self.assertNotIn("DRAFT", page)
        self.assertNotIn('<span class="pending">', page)

    def test_quotation_and_order_form_render(self):
        q = render.render("quotation", filled_payment(), priced_deal())
        self.assertIn("Q-2026-EX001", q)
        self.assertIn("2026-10-25", q)
        self.assertIn("30 days from the invoice date", q)
        o = render.render("order-form", filled_payment(), priced_deal())
        self.assertIn("OF-2026-EX001", o)
        self.assertIn("legal/COMMERCIAL-LICENSE.md", o)
        self.assertIn(render.ORDER_FORM_TEMPLATE_STATUS, o)

    def test_draft_renders_shipped_state_with_banner(self):
        for doc in render.DOC_TYPES:
            page = render.render(doc, empty_payment(), example_deal(), draft=True)
            self.assertIn("DRAFT — NOT VALID FOR PAYMENT", page)
            self.assertIn("payment details pending", page)
            self.assertIn("price to be set", page)

    def test_rf_reference_matches_the_iso_11649_example(self):
        self.assertEqual(render.rf_reference("539007547034"), "RF18539007547034")
        self.assertTrue(render.rf_is_valid("RF18 5390 0754 7034"))
        pd = filled_payment()
        pd["payment_reference"] = {"scheme": "rf", "format": "{invoice_number}"}
        ref, problem = render.payment_reference(pd, priced_deal(), "INV-EXAMPLE-0001")
        self.assertIsNone(problem)
        self.assertTrue(ref.endswith("INVEXAMPLE0001"))
        self.assertTrue(render.rf_is_valid(ref))

    def test_rf_reference_too_long_is_refused(self):
        pd = filled_payment()
        pd["payment_reference"] = {"scheme": "rf", "format": "{deal_id}{invoice_number}{invoice_number}"}
        with self.assertRaises(render.NotReady):
            render.render("invoice", pd, priced_deal())

    def test_vat_registered_standard_and_reverse_charge(self):
        pd = filled_payment(tax__vat_status="registered", tax__kmkr_number="EE123456789")
        deal = priced_deal()
        deal["vat"] = {"treatment": "standard", "rate_percent": "24"}
        page = render.render("invoice", pd, deal)
        # 2,600.99 × 24% = 624.2376 → 624.24; total 3,225.23 (rounded per line of the total)
        self.assertIn("624.24", page)
        self.assertIn("3,225.23", page)
        self.assertIn("EE123456789", page)
        deal["vat"] = {"treatment": "reverse-charge", "rate_percent": ""}
        with self.assertRaises(render.NotReady):
            render.render("invoice", pd, deal)  # no buyer VAT identification number yet
        deal["buyer"]["vat_id"] = "DE123456789"
        page = render.render("invoice", pd, deal)
        self.assertIn("Article 196", page)

    def test_buyer_text_is_escaped(self):
        deal = priced_deal()
        deal["buyer"]["legal_name"] = "<script>alert(1)</script> & Co"
        page = render.render("invoice", filled_payment(), deal)
        self.assertNotIn("<script>alert", page)
        self.assertIn("&lt;script&gt;", page)

    def test_money_rounding_is_half_up(self):
        self.assertEqual(render._money(Decimal("0.005")), "0.01")
        self.assertEqual(render._money(Decimal("1234567.891")), "1,234,567.89")


def _toml(pd: dict) -> str:
    """Tiny writer for the payment-details shape only (the standard library cannot write TOML)."""

    def val(v):
        if isinstance(v, list):
            return "[" + ", ".join(val(x) for x in v) + "]"
        if isinstance(v, int):
            return str(v)
        return '"' + str(v).replace("\\", "\\\\").replace('"', '\\"') + '"'

    lines = [f"schema = {pd['schema']}"]
    for section in ("seller", "bank", "payment_reference", "tax"):
        lines.append(f"[{section}]")
        for k, v in pd[section].items():
            lines.append(f"{k} = {val(v)}")
    return "\n".join(lines) + "\n"


if __name__ == "__main__":
    unittest.main()
