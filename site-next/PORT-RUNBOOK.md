# Port runbook: the redesigned site into the engine repository's `web/`

Installing a new engine package into the Studio, re-recording it and rebuilding the site come first: `ASSEMBLY-RUNBOOK.md`. Written 2026-09-30 after a full rehearsal; updated the same day for local fonts and scripts, the 404 page, the recorder fix and old embed tabs. The port is one command. Everything after it is a
check. Nothing here pushes, tags or touches the live kshana.dev.

Python is `/opt/homebrew/bin/python3` (the shell's `python3` may be 3.9 and fails).

## Where things are

| Thing | Path |
|---|---|
| Worktree the port writes into (branch `feat/site-redesign`, from `feat/docs-v29`) | `~/Code/kshana-wt-site` |
| The port tool | `~/Code/kshana-wt-site/web/tools/port_site.py` |
| Old-address list (source of the redirects and of the test) | `~/Code/kshana-wt-site/web/tools/legacy-urls.json` |
| Site input (built in place) | `~/Code/kshana-site-next/explorations/2026-09-27-combo` |
| Studio input, rehearsal | `~/Code/kshana-site-next/explorations/2026-09-27-playground` |
| Studio input, new | `~/Code/kshana-site-next/explorations/2026-09-30-studio-next` |
| Fonts and script libraries, fetched once (network) | `~/Code/kshana-wt-site/web/tools/fetch_third_party.py` → `web/fonts/`, `web/vendor/` |
| Headless browser check | `~/Code/kshana-site-next/scratch/port/check_site.mjs` |
| Local server that answers like GitHub Pages (404 page) | `~/Code/kshana-site-next/scratch/port/serve_pages.py` |
| Logs, staged deploy tree, screenshots | `~/Code/kshana-site-next/scratch/port/{logs,pages-mirror,shots}` |

Never run the port in `~/Code/kshana` itself. The tool always writes into the checkout it
lives in, so running the copy in the worktree is what keeps the main checkout untouched.

## 0. Before the port: the inputs must be final and must match the checkout

1. The site is built from the checkout you port into. The site reads its facts from the
   checkout named in `combo/src/data/sources.json` (today: `~/Code/kshana-wt-docs-v29`).
   For the final port, build it against the worktree so the pages, the docs pages and the
   repository agree (the worktree has commits docs-v29 does not, including reworded lines
   in `docs/CAPABILITY.md`, `docs/REPORTS.md` and `docs/RELEASING.md`):

   ```sh
   cd ~/Code/kshana-site-next/explorations/2026-09-27-combo
   KSHANA_REPO=~/Code/kshana-wt-site KSHANA_DOCS_REPO=~/Code/kshana-wt-site /opt/homebrew/bin/python3 build.py
   /opt/homebrew/bin/python3 check.py          # expect: OK: N pages ...
   python3 ../../features/serve.py . 8847 &    # a preview that serves clean URLs (/evidence -> evidence.html)
   /opt/homebrew/bin/python3 check.py --nav    # browser check of every link on that preview (port 8847);
                                               # expect: OK: phone: 0 BAD of N · desktop: 0 BAD of N
   ```

   The docs pages are built from the checkout's committed HEAD, so commit first.
2. If the engine version was bumped, the site must be rebuilt after the bump. The port
   refuses a site whose version is not `Cargo.toml`'s.
3. After any change to the ledger or the counts in the worktree, regenerate the repository's
   own files first (`cargo run --bin gen_validation_artifacts`, `python3 tools/gen_og_card.py`),
   then rebuild the site.

4. Fonts and script libraries. Nothing to do unless the site or the Studio starts asking
   for a font set or a library version that is not under `web/fonts/` or `web/vendor/`.
   Then the port refuses and names it; run this once (the only step that needs the
   network), commit what it writes, and port again:

   ```sh
   cd ~/Code/kshana-wt-site && /opt/homebrew/bin/python3 web/tools/fetch_third_party.py \
     --from ~/Code/kshana-site-next/explorations/2026-09-27-combo \
     --from ~/Code/kshana-site-next/explorations/2026-09-27-playground \
     --from ~/Code/kshana-site-next/explorations/2026-09-30-studio-next
   ```

   Today it holds two font sets (the site's: Geist, Geist Mono, Unbounded; the Studio's:
   Bricolage Grotesque, Geist, Geist Mono; 19 files, 569 kB, 4 licence texts) and
   three.js 0.160.0 with OrbitControls and its licence.

## 1. The port (one command)

```sh
cd ~/Code/kshana-wt-site && /opt/homebrew/bin/python3 web/tools/port_site.py \
  --site   ~/Code/kshana-site-next/explorations/2026-09-27-combo \
  --studio ~/Code/kshana-site-next/explorations/2026-09-30-studio-next
```

Use `--studio ~/Code/kshana-site-next/explorations/2026-09-27-playground` for the current
Studio (what the rehearsal ported). Add `--studio-skip <folder>` to leave a Studio folder
out. Add `--check` to see what would change without writing.

Read what it prints:

- `ported vX.Y.Z: … N written, M removed` is success. `port refused: …` means it wrote
  nothing; each `FAIL:` line says what to fix in the input.
- `note:` lines are things it did on your behalf and that should be fixed at the source:
  - **Engine reports naming a temporary path.** Fixed at the source on 2026-09-30: the
    three recorders in `combo/src/tools/` now run the engine on `scenarios/<file>.toml`
    from a scratch folder laid out like the repository, and the nine reports were
    regenerated (results byte-identical, only the printed command changed). The port keeps
    its rewrite as a guard; if this note ever prints again, a recorder regressed.
  - **Source comments naming `~/Code/kshana`.** Fixed in `2026-09-30-studio-next`
    (`app.js`, `lib/counts.test.mjs`). The old `2026-09-27-playground` still has them, so
    the note prints when that Studio is ported; the port rewords them.
  - **Studio data files that differ** from `web/`'s generated copies: expected while the
    Studio folder carries older copies; the repository's are ported.
  - **Studio scenario files that differ** from the checkout's `scenarios/`: the
    checkout's are ported, and the recorded runs for those files predate the change.
    Re-record them if the list is not empty.
  - **"the Studio page ported here is not the one in the site build's own copy"**:
    expected when `--studio` is newer than the Studio the site was built with. If it
    appears with the same Studio, `build.py` changed how it edits the Studio page and
    `studio_index()` in the tool must follow.

Run it a second time: it must say `0 written, 0 removed`.

## 2. Checks after the port

All from `~/Code/kshana-wt-site`. Every one must pass before a commit.

```sh
# a. the port is stable and the tree is exactly what it writes
/opt/homebrew/bin/python3 web/tools/port_site.py --site … --studio … --check      # OK: web/ matches the port

# b. build outputs (the WebAssembly package). The real build:
./web/build.sh
#    on a saturated machine, stage an existing package instead of compiling
#    (the Pages workflow never does this; it always compiles from the tag):
KSHANA_WASM_PKG=~/Code/kshana-site-next/scratch/wasm-v29/pkg ./web/build.sh

# c. node tests: 2 repository tests, the smoke test, and every Studio module test
node web/site.test.mjs
node web/legacy-urls.test.mjs
node web/smoke.mjs
for f in web/studio/lib/*.test.mjs; do node --test "$f" >/dev/null 2>&1 && echo "ok   $f" || echo "FAIL $f"; done

# d. ci.yml's own guard: every test file under web/ has a step
diff <(find web -name '*.test.mjs' -not -path 'web/pkg/*' -not -path 'web/studio/pkg/*' | sort) \
     <(grep -oE '^ +run: node web/[A-Za-z0-9_./-]+\.test\.mjs$' .github/workflows/ci.yml | awk '{print $3}' | sort -u)
#    a new Studio with a new lib/*.test.mjs needs a new step in .github/workflows/ci.yml

# e. the Rust docs-sync tests that read web/ (needs a debug build; slow under load)
cargo test -j4 --no-fail-fast \
  --test coverage_figure_doc_sync --test matrix_total_no_stale_count_doc_sync --test no_overclaims \
  --test readme_code_fences_doc_sync --test scenario_count_doc_sync --test scenario_examples_doc_sync \
  --test verification_rows_cite_evidence_that_exists --test verification_artifacts_doc_sync \
  --test web_validation_counts_doc_sync --test verification_rows_name_a_test_that_exists --test figures_doc_sync

# f. repository guards
scripts/check-no-attribution.sh          # after `git add`, it reads tracked files
scripts/check-version-sync.sh
scripts/check-toolchain.sh
actionlint .github/workflows/ci.yml .github/workflows/pages.yml
cargo fmt --check

# g. mirror of pages.yml's staging step, then serve what would be deployed
M=~/Code/kshana-site-next/scratch/port/pages-mirror
rsync -a --delete --exclude '*.test.mjs' --exclude 'smoke.mjs' --exclude 'build.sh' --exclude '/README.md' \
  --exclude '/tools/' --exclude '/PORT-MANIFEST.json' web/ $M/site/
/opt/homebrew/bin/python3 ~/Code/kshana-site-next/scratch/port/serve_pages.py $M/site 8861   # leave running
#    it answers a missing address with status 404 and /404.html, as GitHub Pages does

# h. headless browser check against http://127.0.0.1:8861/ (about 10 to 30 minutes under load)
node ~/Code/kshana-site-next/scratch/port/check_site.mjs
#    expect: BROWSER CHECK: clean, and "third-party hosts requested by any page: none".
#    Report: scratch/port/browser-check.json, screenshots: scratch/port/shots/
```

What each failure means:

| Failing check | Meaning | Fix |
|---|---|---|
| `site.test.mjs`: "was changed after the port wrote it" | A ported file was edited by hand | Change the source, rerun the port |
| `site.test.mjs`: count or version assertions; Rust `web_validation_counts`, `matrix_total`, `scenario_count` | The site was built from an older engine or ledger than the checkout | Rebuild the site from the checkout (step 0), rerun the port |
| `site.test.mjs`: broken internal links, "the Studio does not bundle" | A page links to a page, anchor or Studio scenario that is not there | Fix the site or the Studio; this is the check that catches a Studio swap that dropped a scenario the pages link to |
| `site.test.mjs`: "names a developer's machine" | A `/Users/…` or `~/Code` path reached a published file | Fix the source; the port only rewrites the two known cases |
| `site.test.mjs`: "make a browser fetch from a third-party host"; the port: "asks Google Fonts for a font set that is not under web/fonts/" | A page, stylesheet or script loads from another host, or asks for fonts not fetched yet | Step 0.4, or remove the third-party load from the source |
| `site.test.mjs`: `404.html` assertions; the port: "404.html: the home page has no …" | The home page's shell changed shape | Update `not_found_page()` in the port tool |
| the port: "old embed tab … is not a Studio tab" | The Studio renamed or dropped a tab an old embed link maps to | Update `embedTabs` in `web/tools/legacy-urls.json` |
| `legacy-urls.test.mjs` / the port: "legacy redirect target … no element with id" | A page lost a section an old address lands on | Restore the id, or point the address elsewhere in `web/tools/legacy-urls.json` |
| Rust `scenario_examples_doc_sync` | A capability card's `run` scenario is not in the Studio's catalogue or bundle | Add it to the Studio |
| Rust `readme_code_fences_doc_sync` | The Studio uses an engine function the package does not export | Fix the Studio or the bindings |

## 3. What to look at

- `http://127.0.0.1:8861/` in a browser: Home, Missions, Capabilities, Evidence,
  Developers, Editions, Docs, and the Studio from the top-right button. The engine chip
  in the Studio must read "Live engine · vX.Y.Z" and a Run must finish with "Ran locally".
- The screenshots in `scratch/port/shots/` (every page, light and dark, 390 and 1440 px;
  viewport captures, named `<page>--<theme>-<width>.png`, plus `studio-run--*.png`).
- Old addresses, by hand: `http://127.0.0.1:8861/#playground`, `/#ledger`,
  `/?embed=1&scenario=integrity-raim.toml&seed=7&tab=fom` (must open the Overview tab).
- A missing address: `http://127.0.0.1:8861/docs/no/such/page` shows the 404 page with
  working styles and links (`shots/404-*.png`).
- In the browser's network panel, every request goes to 127.0.0.1: no fonts.googleapis.com,
  fonts.gstatic.com or cdn.jsdelivr.net.
- `git -C ~/Code/kshana-wt-site status --short`: only files under `web/` change on a rerun.

## 4. Commit

```sh
cd ~/Code/kshana-wt-site && git add -A web && scripts/check-no-attribution.sh && \
git -c user.name=ashfordeOU -c user.email=236818772+ashfordeOU@users.noreply.github.com commit -m "site: port the redesigned site and the Studio into web/"
```

No trailers. No push, no tag, until the founder's go.

## What the port changed in the repository (once, already committed)

- `web/`: the single-page front end (`app.js`, `style.css`, 15 modules and their 14 tests)
  is gone. The Studio's modules and tests live in `web/playground/lib/`.
- `web/build.sh` builds the package, copies it next to the Studio (`web/playground/pkg/`)
  and stages `web/scenarios/`.
- `web/fonts/` and `web/vendor/` are committed: kshana.dev makes no third-party request
  for fonts, styles or scripts. The port rewrites every page's Google Fonts link and
  every script-host address to these local copies. The site source itself is unchanged
  (its preview host only allows Google Fonts).
- `web/404.html` is written by the port from the home page's shell.
- `.gitignore`: `web/assets/` is now tracked site content; `web/playground/pkg/` is ignored.
- `.github/workflows/pages.yml`: the staging step also drops `tools/` and the manifest,
  and fails if the staged tree lacks the home page, the Studio, its package, the 404 page
  or the local fonts and libraries.
- `.github/workflows/ci.yml`: the web job lists the new test files (2 + 20 since the Studio swap: `kinds`, `stages` and `packs` were added), stages the
  package next to the Studio, and its completeness guard covers the whole of `web/`.
- `scripts/check-version-sync.sh`: checks the home page's version, the Studio's
  `channels.json` and the port manifest instead of the old cache-buster strings.
- Rust tests updated to pin the new site's equivalents: `coverage_figure_doc_sync`,
  `matrix_total_no_stale_count_doc_sync`, `no_overclaims`, `readme_code_fences_doc_sync`,
  `scenario_count_doc_sync` (new test for the pages' kind count),
  `scenario_examples_doc_sync`, `verification_artifacts_doc_sync` (comments),
  `web_validation_counts_doc_sync`.

## Old addresses (all tested by `web/legacy-urls.test.mjs` and by the browser check)

Redirected by `legacy-redirects.js` on the home page:

| Old | Lands on |
|---|---|
| `/#playground` | `/studio/` |
| `/#ledger`, `/#ldg-<row>` | `evidence.html#ledger` |
| `/#validation` | `evidence.html#matrix` |
| `/#standards` | `evidence.html#standards` |
| `/#cite` | `evidence.html#cite` |
| `/#explore` | `capabilities.html#explorer` |
| `/#mcp` | `developers.html#mcp` |
| `/#editions` | `editions.html` |
| `/#ashforde` | `editions.html#contact` |
| `/#s=<scenario>` (share links) | `/studio/#s=<scenario>` |
| `/?embed=1&…` (embed links) | `/studio/?embed=1&…` |
| `/playground/`, `/playground/index.html?…` (the Studio until 0.29.1) | `/studio/?…`, query and fragment kept (the port writes `web/playground/index.html`, noindex, which loads `legacy-redirects.js`; `paths` in `legacy-urls.json`) |
| an embed link's `tab=fom` | `tab=overview` (the figures-of-merit table is part of Overview now) |
| an embed link's `tab=orbit3d` | `tab=orbit` |

Kept where they were: `/`, `/index.html`, `/#top`, `/#capabilities`, `/?lang=…` (loads the
home page; the new site is English only), `/og-card.png`, `/og-card.svg`, `/favicon.svg`,
`/robots.txt`, `/sitemap.xml`, `/.well-known/security.txt`, `/capabilities.json`,
`/data/verification-matrix.json`, `/pkg/kshana.js`, `/pkg/kshana_bg.wasm`,
`/scenarios/<file>.toml`.

Not kept (assets of the old page, linked by nothing published): `/app.js`, `/style.css`,
the old `/<module>.mjs` files, `/assets/fonts/*`. They now get the 404 page.

Old embed tab names: the single-page site had five result tabs (`fom`, `timeseries`,
`stability`, `orbit3d`, `sweep`). Three kept their names; `fom` and `orbit3d` are mapped.
`tab=fom` is the only tab name any release tag's documentation ever published (52 times).

## 0.29.2: the Studio at /studio/, and crawl files (written 2026-10-01)

- The Studio is served at `https://kshana.dev/studio/` (its folder is `web/studio/`). Every site
  link and "Open in Kshana Studio" deep link is `/studio/?scenario=…&tab=…`. The old
  `/playground/` and `/playground/index.html?…` forward there with the query and fragment kept.
- The site build (`build.py`) now writes what crawlers read: each page's title, description,
  canonical, Open Graph, social-card and schema.org tags; `<noscript>` text for the script-drawn
  sections; `robots.txt`, `sitemap.xml` (lastmod from git), `llms.txt`, `llms-full.txt` and the
  IndexNow key file `/<key>.txt` (key in `combo/src/data/indexnow.json`). `check.py` rules 21 to 26
  hold them. The port carries them over unchanged and no longer writes its own tags or sitemap;
  `robots.txt` is no longer a file `web/` owns.
- Repository changes the port needs (port tool, old-address list and its test, site test,
  `build.sh`, `ci.yml`, `pages.yml`, `.gitignore`, `check-version-sync.sh`, four Rust tests):
  `audit/port-0292-studio-seo/repo.patch` (`git apply` it in the worktree before the port).

### After a deploy: IndexNow ping (once per deploy; never from a preview)

```sh
curl -sS -X POST https://api.indexnow.org/indexnow -H 'Content-Type: application/json; charset=utf-8' \
  --data-binary @$HOME/Code/kshana-site-next/explorations/2026-09-27-combo/.build/indexnow.json
```

The body is every sitemap address, written by `build.py`; a 200 or 202 reply is success (Bing,
Yandex, Seznam, Naver and others share IndexNow pings; Google does not use IndexNow).

### Google Search Console (the founder, once)

1. Open https://search.google.com/search-console, "Add property", choose **Domain** and enter
   `kshana.dev` (covers every address and both http and https).
2. Google shows a TXT record (`google-site-verification=…`). Add it at the DNS host for
   kshana.dev as a TXT record on the bare domain (`@`), wait for it to resolve, press Verify.
   Keep the record afterwards: removing it un-verifies the property.
3. Sitemaps → submit `https://kshana.dev/sitemap.xml`.
4. URL inspection → `https://kshana.dev/` → "Request indexing"; repeat for `/studio/` and `/docs/`.
5. Optional, same idea for Bing: https://www.bing.com/webmasters → "Import from Google Search
   Console" (or the DNS TXT method), then submit the same sitemap.
