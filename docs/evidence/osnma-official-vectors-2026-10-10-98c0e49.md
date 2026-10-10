# OSNMA official test vectors — re-run after the critic's blocker fixes (verifier at 98c0e49)

Date: 2026-10-10. Branch feat/osnma-verify, verifier at 98c0e49 (fixes 0c5d230 + 98c0e49), reported by W12.
Archive: Test_vectors.zip, sha256 `ef9b9afc6ef9e1c57393415cf2a1dc80c4035e7c06123907ea0bb97dd3ccf370` (same as the earlier run).
Setup: Merkle root and public keys of one tree only; chains from signed DSM-KROOT; CRC checked.
Supersedes the count in osnma-official-vectors-2026-10-10.md (176,805 at b01b42b): the fixed verifier no longer
ping-pongs chains (EOC/NPK sets gain); revoked-chain sets lose slow-MAC tags of the revoked chain by design.

Result: **18/18 sets, 186,382 tags authenticated, 0 failed.**

Columns as reported (verbatim): tags ok/…; KROOT verified; PKR verified; satellites authenticated at end; ADKD ok 0/4/12;
authenticated in last quarter; alert; revocations.

```
configuration_1: 12508/119; 176; 0; 24/24; 8524/1065/2919; 2998; -; 0
configuration_2: 10227/0; 115; 21; 25/25; 6988/877/2362; 2703; -; 0
crev_step1: 6480/126; 233; 0; 0/25 (chain revoked, withdrawn by design); 4520/562/1398; 0; -; 1
crev_step2: 6649/1185; 123; 30; 25/25; 4628/586/1435; 3079; -; 0
crev_step3: 13313/0; 198; 1; 25/25; 9052/1132/3129; 3065; -; 0
eoc_step1: 13638/66; 183; 0; 25/25; 9292/1158/3188; 3090; -; 0
eoc_step2: 12744/80; 230; 0; 25/25; 8896/1120/2728; 3120; -; 0
nmt_step1: 13680/6; 92; 0; 25/25; 9320/1162/3198; 3156; -; 0
nmt_step2: 13566/18; 32; 0; 25/25; 9244/1151/3171; 3060; -; 0
nmt_step3: 12720/0; 140; 18; 25/25; 8680/1090/2950; 2994; -; 0
npk_step1: 12852/54; 146; 35; 25/25; 8752/1089/3011; 2970; -; 0
npk_step2: 13242/0; 101; 42; 25/25; 9028/1133/3081; 3114; -; 0
npk_step3: 13187/0; 160; 26; 25/25; 8968/1118/3101; 2975; -; 0
oam_step1: 6408/66; 109; 0; 0/25 (alert verified, withdrawn); 4472/556/1380; 0; verified 1; 0
oam_step2: 0/89; 4; 0; 0/0 (alert verified); 0/0/0; 0; verified 1; 0
pkrev_step1: 6530/128; 126; 20; 0/25 (key revoked, withdrawn); 4540/565/1425; 0; -; 2
pkrev_step2: 6708/1165; 56; 37; 25/25; 4664/591/1453; 3108; -; 0
pkrev_step3: 11930/0; 86; 15; 25/25; 8148/1023/2759; 3092; -; 0
TOTAL 186,382 tags authenticated, 0 failed, 18/18 sets.
```

Parameters exercised by the vectors (asserted by the test): HMAC-SHA-256, SHA-256 chain, 128-bit keys, 40-bit tags,
MACLT 33 (configuration_1) and 34 (others), ECDSA P-256/P-521 keys per set. CMAC-AES, SHA3-256 chains and other sizes:
KATs and synthetic tests only.

Notable: the alert (OAM) messages verify only after the fix to DSM-PKR NPK length for NPKT 4; before it no alert had
ever verified (RootMismatch on all 63 OAM messages).

Ledger caveats to carry: certificate revocation lists not read; revocation announced by the unauthenticated header is
final per session (fail-safe denial of service only).
