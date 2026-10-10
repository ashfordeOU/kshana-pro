# D14: public advisory (GHSA, ashfordeOU/kshana), final text

Status: APPROVED by the founder on 2026-10-10 ("approve advisory as proposed and make it better if needed").
Publish only after v0.34.1 is live on every channel and W2 has confirmed it. Then file the RustSec entry below.
Public text is generic by policy. Field-level detail stays in D14-mcp-receiver-log-path.md (branch defects/d14-mcp-path).

## Correction to the affected range (coordinator, 2026-10-10)

The earlier draft said `>= 0.30.0`. Checking the tags shows the range is wider. kshana-mcp has run any scenario it is sent
(`kshana::api::run_toml`) since before 0.26. The first scenario fields that name a file on disk appear in the engine at
v0.27.0 (ephemeris, reflector, station and normal-point paths; a CSV path from 0.27.3; spectrum recordings from 0.29;
planetary kernels from 0.30; receiver logs later). The first published kshana-mcp of that line is 0.27.1. Stricter
direction: affected `>= 0.27.0, < 0.34.1`. Versions before 0.27.0 had no file-naming scenario fields, so they are not listed.

## GitHub form fields

| Field | Value |
|---|---|
| Ecosystem | Rust (crates.io) |
| Package | `kshana-mcp` |
| Affected versions | `>= 0.27.0, < 0.34.1` |
| Patched version | `0.34.1` |
| Severity | Medium |
| CVSS v3.1 vector | `CVSS:3.1/AV:N/AC:L/PR:N/UI:R/S:U/C:L/I:N/A:N` (4.3) |
| Weaknesses | CWE-73 External Control of File Name or Path; CWE-209 Generation of Error Message Containing Sensitive Information |
| CVE | Request a CVE ID from GitHub: yes |
| Credits | none (found in internal review) |

Why this vector: the attacker needs no account, only to get content into an AI agent session that uses the server
(UI:R: an agent or user relays the tool call). Confidentiality impact is Low: fragments of files readable by the server
process and whether a path exists. No integrity or availability impact, because the server never writes or deletes files
and does not crash.

## Title

kshana-mcp: scenario tools could read host files named in tool input

## Description (paste as is)

### Impact

`kshana-mcp` exposes Kshana's simulation and analysis tools to MCP clients such as AI agents. In affected versions, some
scenario inputs could name a file on the host instead of carrying the data inline. When a tool ran such a scenario, the
server opened that file. Errors and diagnostics returned to the client could include fragments of the file's contents,
and the response showed whether the path existed.

An MCP client, or untrusted content that a client relays into a tool call (for example text an agent has read from a web
page or a document), could use this to learn limited information about files readable by the server process. The
server never writes, deletes or uploads files.

Not affected: the `kshana` command-line tool and the Rust, Python and WebAssembly libraries. These are run directly by
the user, and reading local files the user names is their intended behaviour.

### Patches

Fixed in `kshana-mcp` 0.34.1. Every MCP tool now accepts inline content only. Any input that names a file is refused
with a generic error before anything is opened, and inline input is capped at 4 MiB. A regression test checks that every
tool applies this check. `kshana-mcp` 0.34.1 requires engine 0.34.1, so it cannot be built against an engine without
the check.

Upgrade one of these ways:

- `cargo install kshana-mcp --locked --force`
- pull container image `ghcr.io/ashfordeou/kshana-mcp:0.34.1`
- update the MCP registry entry to 0.34.1

### Workarounds

If you cannot upgrade yet:

- run the server as an unprivileged user, in a container or sandbox where no sensitive files are readable;
- do not relay untrusted content into its tool calls.

### Notes

Kshana is advisory software. It is not type-approved navigation equipment.

## RustSec entry (file after the GHSA is published)

Open a PR to rustsec/advisory-db adding `crates/kshana-mcp/RUSTSEC-0000-0000.md`. The ID is assigned on merge.

```toml
[advisory]
id = "RUSTSEC-0000-0000"
package = "kshana-mcp"
date = "2026-10-10"
url = "https://github.com/ashfordeOU/kshana/security/advisories/<GHSA-ID>"
categories = ["file-disclosure"]
keywords = ["mcp", "path", "information-disclosure"]
aliases = ["<GHSA-ID>", "<CVE-ID if assigned>"]
cvss = "CVSS:3.1/AV:N/AC:L/PR:N/UI:R/S:U/C:L/I:N/A:N"

[versions]
patched = [">= 0.34.1"]
unaffected = ["< 0.27.0"]
```

```markdown
# kshana-mcp scenario tools could read host files named in tool input

In affected versions, scenario inputs passed to the MCP server's tools could name a file on the host. The server opened
it, and error output could include fragments of the file and reveal whether a path existed. Version 0.34.1 accepts
inline content only and refuses file-naming inputs before anything is opened.
```

## Publishing steps for the founder (after W2 confirms 0.34.1 everywhere)

1. Open https://github.com/ashfordeOU/kshana/security/advisories/new
2. Fill in the form fields above, paste the title and description, and tick "Request CVE ID".
3. Publish. GitHub shows the advisory to Dependabot users of `kshana-mcp`.
4. Send the GHSA ID to the coordinator. W2 then prepares the RustSec PR with the IDs filled in, for you to open.
