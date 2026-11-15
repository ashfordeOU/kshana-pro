# Run report templates

Every engine run writes a self-contained HTML report (`kshana <scenario>.toml` writes
`<name>.report.html`). Its look is the engine's own, defined by the `STYLE` constant in the public
repo's `src/advanced_report.rs`: a warm "paper" document theme (accent `#9a5b22` light /
`#e0a36a` dark, evidence colours for VALIDATED, MODELLED and PARTNER), deliberately separate from the
site tokens.

- `report-theme.css`: a mirror of that stylesheet at 0.31.0, with the block's SHA-256. The engine
  source stays the single source; change the theme there, in an engine release, never here.
- `samples/`: the nine reports kshana.dev publishes, exactly as the 0.31.0 engine wrote them
  (inputs: the scenario file of the same name under the public repo's `scenarios/`).
- `timing/`: the Pro timing audit templates (roadmap 0.42 T3): `rts25-traceability.json` and
  `nist-ir-8323-profile.json`. Each item is mapped to a provision; nothing claims conformity.
- `samples/timing-audit-rts25/`: a sample timing audit report from the demo fleet
  (`examples/timing-fleet/`, two real IGS clocks and synthetic logs), signed with a published
  demo key (`samples/timing-audit-rts25.demo-public-key.txt`; rebuild with
  `scripts/timing-sample-report.sh`). Verify: `kshana-pro dossier verify
  samples/timing-audit-rts25 --trust <that key>`.

Aligning the report with the site tokens is an engine change, so the published reports would then
need re-recording with that release. See `../SOURCES.md`.
