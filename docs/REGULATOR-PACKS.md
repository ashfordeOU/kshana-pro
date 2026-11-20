# Regulator, type-approval and framework pre-assessment packs

Research 2026-11-20 gaps, epic E29 (B2901 to B2906). Module `src/regpacks/`, command
`kshana-pro regpack`, tests `tests/regulator_packs.rs`.

These packs show, requirement by requirement, which piece of signed Kshana evidence speaks to a
requirement and what is missing. They are **pre-assessments**: aids for preparing a submission
to a notified body, class society, flag administration, regulator or buyer. No pack states that
a product, ship or operator meets a document; that judgement belongs to the assessor and the
authority. Every output carries the label:

> MODELLED: a pre-assessment of the evidence supplied against the requirements as paraphrased
> in the catalogue. Not reviewed by any notified body, class society or authority; it does not
> state that any product or operator meets the document.

## Commands and licences

| Command | What it does | Feature (tier) |
|---|---|---|
| `regpack list`, `show <id>`, `example <name>` | Catalogues and example inputs | free |
| `regpack assess <id> --evidence <dirs> [--standards-report <json>] [--subject <toml>]` | Pre-assessment against a catalogue | `pro-regulator-pack` (Pro) |
| `regpack bam <alert-map.toml> --pntdp <json> \| --pntdp-vector <name>` | BAM alert-map check and replay | `pro-regulator-pack` (Pro) |
| `regpack mass <mass.toml>` | MASS override independence and OE PNT template | `pro-regulator-pack` (Pro) |
| `regpack links <links.toml>` | Space link-security evidence | `pro-regulator-pack` (Pro) |
| `regpack complaint <request.toml> [--episodes <json>] [--evidence <dirs>]` | Interference report annex (RR Appendix 10 headings) | `pro-interference-complaint` (Pro) |
| `regpack bid <bid.toml> --bom <bom.csv> [--evidence <dirs>]` | Bid evidence attestation | `pro-bid-attestation` (Pro) |
| `regpack verify <dir> --trust <hex>` | Signature check (and the bid chain) | free |

Every producing command takes `--out <dir> --signer <name> --key-file <file> [--trust <hex>]
[--utc <iso>] [--license <token>]` and signs its directory with the dossier signature
(`signed_dossier`, Ed25519).

## Catalogues

`report-templates/regpacks/<id>.json`, format `kshana-pro-regulator-catalogue/1`. Each row has
the clause, the **basis** for the citation, a short paraphrase (never a long quotation; paywalled
standards are not reproduced), the evidence formats that speak to it and the gap. A clause is
cited only where the research or the open engine read the document; every other row says exactly
`clause to be confirmed`. Every catalogue has at least one row Kshana does not assess.

| Id | Document, version | How it was read |
|---|---|---|
| `imo-msc401-432` | IMO MSC.401(95) as amended by MSC.432(98) | Topic level (research 09b G81); no clause numbers |
| `imo-circ-1575` | IMO MSC.1/Circ.1575 (16 Jun 2017) | Paragraphs as read in full by the open engine's `imo-circ-1575` standards pack |
| `bam-pnt-alerts` | IMO MSC.302(87); IEC 62923-1:2018 and -2 | Not read (paywalled); topic level |
| `iacs-ur-e27` | IACS UR E27 Rev.1 (Sep 2023) | Topic level; Appendix 2 as cited in research 09b |
| `etsi-en-303-413` | ETSI EN 303 413 (RED), version to be confirmed | Topic level |
| `imo-mass-code` | MASS Code, MSC.595(111) | Read in full in research 07b; 16.3.2 EP4 and 16.5 EP1 as cited there |
| `nist-csf2-pnt` | NIST IR 8323 Rev. 2 ipd on CSF 2.0 | CSF 2.0 category identifiers; the draft profile text not read |
| `eu-space-act-cyber` | EU Space Act proposal COM(2025) 335 | Topic level; check against the adopted text |

## Pre-assessment (`assess`)

Reads each `--evidence` directory, finds every known evidence format in its top-level JSON files,
verifies its signature (and, with `--trust`, its signer). Only usable evidence (valid signature,
signer not refused) is put against a row. An open-engine standards-pack report
(`kshana standards-pack run <id>`, schema `kshana.standards-pack-report/1`) can be added with
`--standards-report`: it carries no signature, so it is listed as an unsigned report with its
SHA-256, and the pre-assessment that cites it is signed. Each row is `evidence supplied`,
`no evidence supplied` (Kshana has a product for it but none was given) or `not assessed by
Kshana`. The page is laid out as a test report: scope, subject (`--subject`), results per
requirement, evidence register, gaps.

## BAM alert-map check (`bam`)

The maker's alert map ties each PNT-integrity condition (`integrity:none|low|medium`,
`status:raised`, `no-solution`, `flag:<incomplete|invalid|stale|frozen|jump|inconsistent|
disagreement>`, the conditions of the PNT data processing reference `kshana-pro pntdp`) to an
alert identifier, priority and category. Rules (Kshana's, written into the output, configurable
in `[policy]`; not standard text): MAP-0 known and unique conditions; MAP-1 every required
condition mapped; MAP-2 allowed priority, category A/B/C; MAP-3 an identifier, and one
identifier never used with two priorities; MAP-4 loss of integrity at or above the policy's
minimum priority; MAP-5 (note) alert text present. The map is then replayed over
`pntdp run --json` output (or a built-in vector) to give the raised/cleared timeline and the
onsets of conditions with no alert.

## MASS override independence (`mass`)

Walks the owner's declared component graph from every component of the override path and lists
each chain that reaches a GNSS component or a controlled system. Lists the Operational Envelope
conditions with PNT content and the evidence named for each (none named is a gap). Kshana does
not inspect the ship.

## Interference report annex (`complaint`)

Arranged under the headings of ITU RR Appendix 10 (item numbering and Article 15 paragraph
numbers to be confirmed against the edition the filer names). Observations come from the
request and from `kshana-pro reports episodes` output. **Export-control and safety rule:**
every observation is reduced to the grid cell (0.5 degrees by default; finer is refused) that
holds it; no finer position leaves the module; every particular of the interfering station is
"not determined". The pack describes effects and areas; it never locates, identifies or
attributes an emitter. The filing is the administration's act.

## Link-security evidence (`links`)

From the operator's declared link and command configuration: cleartext links, command paths
without authentication, protection that can be switched off, missing key-management
descriptions, and the operator's own "no encryption, no fly" policy (every command path
encrypted, authenticated, protection not switchable). Kshana does not observe any link.

## Bid evidence attestation (`bid`)

BOM columns: `part,description,maker,origin_country,export_regime,classification,declared_by`.
Each line is `itar declared` (regime or classification names ITAR or USML), `not listed
(declared)`, `other regime declared` or `not declared`. The attestation lists them, adds the
verified test evidence with each signature hash, and a chain hash over the BOM and the evidence;
the BOM is copied into the signed directory and `regpack verify` recomputes the chain. Kshana
does not classify items.

## Not built in this pass

- RED and R-Mode draft performance test scenarios as test-kit packs (B2901: the catalogues name
  the test-kit evidence; new scenario packs are a follow-up).
- MASS experience-building data kit as a dataset schema (B2902: covered by the catalogue row and
  the existing interference episodes and forensics formats).
- CEPT authorised-jamming test coordination records, broadcast hijack evidence, Res. 676 checks
  and the ITU FAQ operator checklist (B2903): the source texts were not read.
- FAA GNSS interference resource guide, DHS BP.AS and national robustness standards mappings
  (B2904): only titles in the research.
