# D14: RustSec submission (for the founder to open; not opened)

Status 2026-10-11: OPENED by the founder as https://github.com/rustsec/advisory-db/pull/3355 (fork ashfordeOU/advisory-db,
branch kshana-mcp-d14, head 4fcfe95; the advisory file on that branch is byte-identical to the patch below). Not yet merged:
when it merges, record the assigned RUSTSEC ID here and in D14-mcp-receiver-log-path.md. If GitHub assigns a CVE to
GHSA-8wpf-hqg5-6xjr, add it to `aliases` on the fork branch.

Target: https://github.com/rustsec/advisory-db, a PR adding `crates/kshana-mcp/RUSTSEC-0000-0000.md`.
Prepared 2026-10-11 against advisory-db `7eebec6`. Author of the commit: Ashforde OÜ. A `git am`-able patch follows the PR text.
GHSA: GHSA-8wpf-hqg5-6xjr (published 2026-10-11 12:48Z). CVE: requested, not assigned at preparation time. If one is
assigned before the PR is opened, add it to `aliases` next to the GHSA ID.

## PR title

Add RUSTSEC-0000-0000 for kshana-mcp

## PR body (the repository's template, filled in)

```
## Affected crate(s)

- kshana-mcp (crates.io)

## Links to upstream issue(s) or PR(s)

- GitHub security advisory: https://github.com/ashfordeOU/kshana/security/advisories/GHSA-8wpf-hqg5-6xjr
- Fix: https://github.com/ashfordeOU/kshana/pull/74 (released as kshana-mcp 0.34.1)

## Severity

Medium (CVSS 3.1 4.3, CVSS:3.1/AV:N/AC:L/PR:N/UI:R/S:U/C:L/I:N/A:N). A scenario input passed to a kshana-mcp tool
could name a file on the host; the server opened it, and error output could include fragments of the file and reveal
whether a path existed. Content relayed into an agent session could use this to learn limited information about files
readable by the server process. CWE-73, CWE-209. Fixed in 0.34.1: inline content only, file-naming inputs refused.

## Checklist

- [x] Advisory filename(s) starts with `RUSTSEC-0000-0000` as the ID
- [x] `date` field is set to the public disclosure date (2026-10-11)
- [x] Contains a concise and descriptive title after advisory metadata
- [x] Asked maintainer(s) if publishing an advisory is appropriate (this is the maintainer's own advisory)
```

## Patch

```
From 4da15852aa8fba1465b0f2784c49c0bbce77abbf Mon Sep 17 00:00:00 2001
From: =?UTF-8?q?Ashforde=20O=C3=9C?= <ashfordegithub@cbaweja.org>
Date: Sun, 11 Oct 2026 12:50:34 +0000
Subject: [PATCH] Add RUSTSEC-0000-0000 for kshana-mcp

---
 crates/kshana-mcp/RUSTSEC-0000-0000.md | 21 +++++++++++++++++++++
 1 file changed, 21 insertions(+)
 create mode 100644 crates/kshana-mcp/RUSTSEC-0000-0000.md

diff --git a/crates/kshana-mcp/RUSTSEC-0000-0000.md b/crates/kshana-mcp/RUSTSEC-0000-0000.md
new file mode 100644
index 0000000..7a35ac5
--- /dev/null
+++ b/crates/kshana-mcp/RUSTSEC-0000-0000.md
@@ -0,0 +1,21 @@
+```toml
+[advisory]
+id = "RUSTSEC-0000-0000"
+package = "kshana-mcp"
+date = "2026-10-11"
+url = "https://github.com/ashfordeOU/kshana/security/advisories/GHSA-8wpf-hqg5-6xjr"
+categories = ["file-disclosure"]
+keywords = ["mcp", "path", "information-disclosure"]
+aliases = ["GHSA-8wpf-hqg5-6xjr"]
+cvss = "CVSS:3.1/AV:N/AC:L/PR:N/UI:R/S:U/C:L/I:N/A:N"
+
+[versions]
+patched = [">= 0.34.1"]
+unaffected = ["< 0.27.0"]
+```
+
+# kshana-mcp scenario tools could read host files named in tool input
+
+In affected versions, scenario inputs passed to the MCP server's tools could name a file on the host. The server opened
+it, and error output could include fragments of the file and reveal whether a path existed. Version 0.34.1 accepts
+inline content only and refuses file-naming inputs before anything is opened.
-- 
2.43.0

```
