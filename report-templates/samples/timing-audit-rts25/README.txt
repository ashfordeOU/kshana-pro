Timing audit report (rts25-traceability), fleet demo-ldn-fra, generated 2026-11-15T09:30:00Z.

report.html            the report to read
report.json            the same, as data
fleet-assessment.json  the holdover assessment of every clock
history/               each clock's hash-chained evidence history
template.json          the mapping template used
signature.json         SHA-256 of every file above, signed with Ed25519

Check it, offline and without a licence:
kshana-pro dossier verify <this directory> --trust <the signer's public key>
or: python3 scripts/verify-dossier-signature.py <this directory> --trust <key>

Each item is mapped to a provision; the report is not a statement that any
system meets the framework.
