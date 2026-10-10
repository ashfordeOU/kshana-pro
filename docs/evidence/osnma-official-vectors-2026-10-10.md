# OSNMA verifier: official test-vector run (evidence record)

Run by W12 on 2026-10-10, offline. The vectors are not redistributed and no vector content is reproduced here.

- **Verifier:** Kshana `feat/osnma-verify` at b01b42b174a5394d4cfa91ffc51c83f2eafec4f9. An earlier run at 57f8cda gave identical results; at b01b42b the CRC check is on.
- **Source:** official OSNMA test vectors, Annex B of the Galileo OSNMA Receiver Guidelines v1.3.
- **Archive:** `Test_vectors.zip`, 10,319,251 bytes, sha256 `ef9b9afc6ef9e1c57393415cf2a1dc80c4035e7c06123907ea0bb97dd3ccf370`, from the GSC website. Its readme states it is an annex to Receiver Guidelines v1.3.
- **Setup:** the verifier was given only a Merkle root and the public keys of one tree per scenario. The chain was established from the signed DSM-KROOT in the stream, and the page CRC was checked.
- **Test:** opt-in, `KSHANA_OSNMA_VECTORS=<dir>`, reading a local directory.

Columns: tags ok / failed / pending / discarded; KROOT verified; PKR verified; satellites authenticated.

| Scenario | Tree | Tags (ok / failed / pending / discarded) | KROOT | PKR | Sats |
|---|---|---|---|---|---|
| configuration_1 | 1 | 12508 / 0 / 118 / 590 | 176 | 0 | 24 of 28 |
| configuration_2 | 2 | 10227 / 0 / 0 / 0 | 115 | 21 | 25 of 25 |
| crev_step1 | 2 | 6480 / 0 / 1261 / 0 | 113 | 0 | 25 of 25 |
| crev_step2 | 2 | 6547 / 0 / 1185 / 0 | 35 | 30 | 25 of 25 |
| crev_step3 | 2 | 13313 / 0 / 0 / 0 | 198 | 1 | 25 of 25 |
| eoc_step1 | 2 | 9656 / 0 / 66 / 0 | 183 | 0 | 25 of 25 |
| eoc_step2 | 2 | 9315 / 0 / 0 / 0 | 230 | 0 | 25 of 25 |
| nmt_step1 | 2 | 13680 / 0 / 6 / 0 | 92 | 0 | 25 of 25 |
| nmt_step2 | 2 | 13566 / 0 / 18 / 0 | 32 | 0 | 25 of 25 |
| nmt_step3 | 3 | 12720 / 0 / 0 / 0 | 140 | 18 | 25 of 25 |
| npk_step1 | 2 | 12852 / 0 / 54 / 0 | 146 | 35 | 25 of 25 |
| npk_step2 | 2 | 13242 / 0 / 0 / 0 | 101 | 42 | 25 of 25 |
| npk_step3 | 2 | 13187 / 0 / 0 / 0 | 160 | 26 | 25 of 25 |
| oam_step1 | 3 | 6408 / 0 / 1231 / 0 | 109 | 0 | 25 of 25 |
| oam_step2 | 3 | 0 / 0 / 1139 / 0 | 0 | 0 | 0 of 0 (alert-message set, nothing to authenticate) |
| pkrev_step1 | 2 | 6530 / 0 / 1239 / 0 | 103 | 0 | 25 of 25 |
| pkrev_step2 | 2 | 4644 / 0 / 1165 / 0 | 24 | 22 | 25 of 25 |
| pkrev_step3 | 2 | 11930 / 0 / 0 / 0 | 86 | 15 | 25 of 25 |

**Total:** 176,805 tags authenticated and 0 failed, in 18 of 18 scenarios. 17 scenarios authenticate tags. The alert-message set (oam_step2) has nothing to authenticate, so the test asserts only that 0 tags failed there.

Notes:
- **Pending** means tags in sub-frames whose NMA status is "don't use", or tags not yet decidable because the key or data had not been received.
- **Discarded** (configuration_1 only) means reserved-ADKD tags set aside per Guidelines 5.5.2.

Legal basis: counsel record on kshana-pro `legal/osnma-annex-e`, §4.9. This is advisory software, not type-approved navigation equipment.
