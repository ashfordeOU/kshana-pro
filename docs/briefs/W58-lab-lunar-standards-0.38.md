# W58 brief: Kshana 0.38 WS-G, lab round trip, LunaNet AFS check, standards packs, education, governance drafts

Read first: docs/briefs/_COMMON-0.38.md (rules), docs/plans/0.38-PLAN.md "Scope extension" WS-G, strategy memo
moves 5, 7, 9 and 10 and "What NOT to do"; kshana docs/TEST-BENCH.md and docs/compliance/.

## Branch you own
- kshana: feat/lab-lunar-standards-0.38 from origin/main.

## Items
G1 Publish kshana.test-conditions/1 as docs/spec/TEST-CONDITIONS.md (+ JSON Schema), generalise bench-export to it,
   and add a round-trip evidence pack command linking scenario hash, lab-conditions hash, receiver log hash and
   score. Worked example with gps-sdr-sim producing a FILE only; cabled/conducted replay is documented as operator
   responsibility and never automated. No transmit path, no vendor formats from reverse engineering.
G2 LunaNet LNIS v5 Augmented Forward Signal: code generation and message format in the IQ layer plus
   `kshana lunanet-afs-check` verifying a recorded/generated stream against the published spec, pinned to v5.
   VALIDATED only if the spec's own tables give test values (pre-register first); otherwise MODELLED.
G3 Standards packs, each = scenario pack + scoring + compliance-report rows, paraphrased, "supports evidence for":
   DHS RPCF levels 1–4 test profiles (docs/compliance/dhs-rpcf-test-profiles.md); IMO MSC.1/Circ.1575 integrity /
   multi-system RAIM pack; ICAO Doc 9849 App. F monitor-assess-mitigate RFI pack; EU-ARAIM Table A-3 reproduction as
   a public vector set (pre-registered bars against the published table). W55 does the NIST IR 8323 rows: don't
   duplicate.
G4 Education: 6 notebooks under notebooks/course/ (jamming, spoofing, holdover, integrity, OSNMA, interference map),
   runnable in the browser/Pyodide or Jupyter, with the advisory statement.
G5 Drafts for owner approval ONLY, under docs/drafts/ and clearly marked DRAFT: JOSS paper (paper.md + bib), a
   TSC charter amendment to GOVERNANCE.md (no named members, no placeholders that look like people), oracle-registry
   process, LICENSES/ split proposal (CC-BY-4.0 for specs/compliance text, CC0 for vectors/manifests). Do not
   change the live GOVERNANCE.md or licences.

## Done when
Commands/kinds on every surface (or N/A with reason); docs; bars pre-registered; gates green; report per common rules.
