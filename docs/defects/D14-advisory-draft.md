# D14 — public advisory draft (GHSA, ashfordeOU/kshana)

Status: DRAFT for founder approval. Publish only after v0.34.1 is released on every channel.
Public text is generic by policy; field-level detail stays in D14-mcp-receiver-log-path.md (branch defects/d14-mcp-path).

## Form fields

- Ecosystem / package: crates.io `kshana-mcp`
- Affected versions: `>= 0.30.0, < 0.34.1`
- Patched version: `0.34.1`
- Severity: Medium (provisional; CVSS to be confirmed when publishing)
- CWE: CWE-22 (Improper Limitation of a Pathname to a Restricted Directory), CWE-209 (Information Exposure Through an Error Message)
- Credit: internal review

## Title

kshana-mcp: scenario tools could read local files named in agent-supplied input

## Description

### Impact

The `kshana-mcp` server exposes scenario tools to MCP clients. In affected versions, some scenario
inputs could name a file on the host instead of carrying the data inline. The server process
would then open that file. Error and diagnostic messages returned to the client could include
fragments of the file's contents and revealed whether a path existed.

An MCP client, or content the client relays into a tool call, could therefore learn limited
information about files readable by the server process. The server never writes or transmits
files, and the Kshana CLI and library APIs, which are run directly by the user, are not affected.

### Patches

Version 0.34.1 makes the MCP scenario tools accept inline content only. Inputs that name a file
are refused with a generic error before anything is opened, and inline input size is capped.
A regression test checks that every MCP tool goes through this check.

Upgrade with `cargo install kshana-mcp --locked --force`, or update the container image or
MCP registry entry to 0.34.1.

### Workarounds

Until you upgrade, run the server as an unprivileged user in a container or sandbox with no
sensitive files readable, and do not relay untrusted content into its tool calls.

### Note

Kshana is advisory software. It is not type-approved navigation equipment.
