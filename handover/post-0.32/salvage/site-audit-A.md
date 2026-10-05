# Kshana site theme-drift audit (audit A)

Worked only from files on disk at `/home/user/kshana-src`. Nothing pushed. Engine version of record: **0.31.0**.

> **Framing correction (important).** The brief treated `web/tokens.css` as the whole truth and
> flagged Unbounded as a possible stray. In fact the live site theme is **two layers**:
> `tokens.css` (2026-09-27) *plus* `web/theme.css` — "the Observatory system (founder's choice,
> 2026-09-28)", loaded after tokens and before `site.css` on every public page, which **re-points
> the shared tokens** (`--bg`, `--ink`, `--f-display`, the accent palette). So the current site
> theme = tokens.css + theme.css (Observatory), and **Unbounded is the intentional site display
> font**, not drift. The Studio is the only surface that stays on pure tokens.css (Bricolage). The
> earlier Ashforde gold/Garamond anchor was wrong *and* tokens.css-alone is incomplete — Observatory
> is the real reference. No Garamond/gold/serif remnants exist anywhere in `web/` (verified).

---

## 1. Canonical current theme (confirmed token set)

**Layer 1 — `web/tokens.css` (site + Studio, byte-identical copy in `web/studio/tokens.css`).**
- Light ground: `--bg #E8ECF3 --bg-2 #DEE4EE --surface #F5F7FB --surface-2 #EDF1F7`; ink `--ink #182033 --ink-2 #3A455E --ink-3 #4E5971`; lines `rgba(28,44,84,.12/.2)`.
- Six failure-domain colours (legitimate, one per domain): `--int #B8262B --spf #6236E0 --tim #1C4FE0 --orb #006C7D --itg #0A6F38 --nav #7C4800`; card variants `#E5383B #6B3FF0 #1F55F0 #008795 #0F8A47 #F5A300`; series `--s-* #FF5B54 #A58BFF #6A98FF #2CD0DE #3DDC84 #FFB224`.
- "space" tokens (stay dark in both themes): `#05060C #0C0E18 #151826` ink `#EEF0FA #A3AAC2 #8088A6`.
- Dark theme (`prefers-color-scheme`/`data-theme`): `--bg #07080E --bg-2 #0D0F18 --surface #12141F --surface-2 #181B28`; ink `#F2F4FB #A6ADC4 #858CA6`; domains `#FF5B54 #9C7DFF #5A8CFF #24C8D6 #33CC78 #FFB224`.
- Print: `#FFFFFF #F2F4F8 #F6F8FB`.
- Fonts: `--f-display "Bricolage Grotesque"`, `--f-text "Geist"`, `--f-mono "Geist Mono"`.
- Header carries a stale `0.29.2` comment (×2, in both token files) describing the ground.

**Layer 2 — `web/theme.css` (Observatory, site pages only; NOT loaded by Studio).** Overrides:
- Keeps the same light ground (`#E8ECF3/#DEE4EE`), adds `--bg-3 #D3DBE8`, `--ink-4 #66718A`.
- Observatory accent palette (light): `--cyan #066A86 --magenta #A8247F --lime #2F6E0A --amber #764600 --coral #B42D1B`; `--modelled #7389BF`; button `--btn-bg #182033`.
- Observatory accent palette (dark): `--cyan #3DDCF7 --magenta #F45CCB --lime #A8EE5E --amber #FFB547 --coral #FF6A5C`; `--modelled #4F6FB0`; dark ground `#060A14 #0A1122 #0E1730 #0D1528 #111B33`, ink `#EAF0FF #A7B4D2 #8190B0 #56637F`.
- Fonts: `--sans Geist`, `--mono Geist Mono`, **`--display "Unbounded", "Geist"`**, and re-points `--f-display: var(--sans)` (body display name maps to Geist; the big display type uses `var(--display)` = Unbounded).

**Load order on every public page** (verified in index.html): `fonts/geist-geist-mono-unbounded.css` → `fonts/noto-sans-devanagari.css` → `tokens.css` → `theme.css` → `site.css` → `css/*.css`. Both `--int…--nav` domain tokens (13 refs in site.css) and `--cyan…--coral` Observatory tokens (26 in site.css, 46 in css/home.css) are in active use — they coexist by design.

---

## 2. Colour drift vs the token set

Scanned `web/**` (`*.html/.css/.js/.svg`, excluding `fonts/`, `vendor/`, and the `report.html` exports which are a separate theme — see §6). **85 distinct non-token hex colours.** None are gold/Garamond-era. Breakdown:

**(a) Legitimate — not drift:**
- macOS "traffic-light" window dots `#FF5F57 #FEBC2E #28C840` (×3 each, in `css/capabilities.css`, `css/home.css`, `css/missions.css`) — console/scope chrome.
- Data-viz / chart series in the Observatory hue family: `css/docs.css` `#E2E9FA #8C9BBB #B9A0FF #86AAFF #5FDDF5 #F57ED6 #FF9484 #8391B2`; `css/pro-power.css` `#6FE3F5 #B9F07A #7FA6FF #FF6B63` (plus token-matching `#FFB224 #3DDC84`). Series colours, legitimately hardcoded.
- `og-card.svg` art palette (owned file, see §8).

**(b) Token-bypass drift — in-palette but hardcoded instead of `var()` (low severity):**
- Near-white blue-grey tints: `css/light.css` (`#DCE3EE #E8EDF5 #F0F3F8 #E3E9F2 #D8DFEB #E1E6EF #ECF0F6 #E9EEF5 #DFE5EF #D0D9E6 #E4EAF2 #EEF2F8 #C9D4EF`), `css/docs.css` (`#F3F6FC #E9EEF8 #1B2540 #56627E`), `css/capabilities.css` (`#C9D4EF #0A1226 #03050B #5B6A8F #8A96B3`). All in the correct blue-grey family; just not tokenized.

**(c) Near-duplicate domain colours (should reference the six domain tokens):**
- `css/capabilities.css`: `#C9321F` (≈int), `#6B4FD8` (≈spf), `#2F55C8`/`#3552A8` (≈tim), `#1F7A3E` (≈itg), `#7E4B00` (≈nav). `css/docs.css`: `#6A2CD6` (≈spf), `#2F6A0B` (≈lime). Off-by-a-few from the real tokens — minor correctness drift.

**(d) Distinct pro/editions accent (intentional, un-tokenized):**
- `editions.html` `#C3A6FF` (lavender, ×19–20), `css/editions.css` `#1E1446` (deep indigo), `css/pro-power.css` `#C3A6FF`. A deliberate "Pro" purple accent not in the token set. `site.css` `#1A1240` (one-off deep indigo) is the same family.

**(e) Studio-only accents:** `studio/index.html` + `studio/advanced/index.html` `#0795BA` (cyan ×8), `#5FA82A` (green ×2). Studio runs on pure tokens.css, so these are its local accents.

> The only genuinely *off-theme* colour family is **warm tan/brown/gold**, and it lives only in the
> report exports (§6) and the OG card's tan accent `#C79E63` (§8) — a second "paper" aesthetic, not
> the blue-grey Observatory site. Treat it as a deliberate-but-inconsistent second theme, not as
> prior-theme contamination.

---

## 3. Font drift / the Unbounded question

**Verdict: Unbounded is the intentional, site-wide Observatory display font. Not drift. The brief's premise (only missions/evidence use Unbounded) is incorrect.**

- `fonts/geist-geist-mono-unbounded.css` (Geist + Geist Mono + Unbounded) is loaded by **every public page**: index, missions, evidence, capabilities, developers, editions, 404, and all ~60 `docs/*.html`.
- `fonts/bricolage-grotesque-geist-geist-mono.css` is loaded by **only two files**: `studio/index.html` and `studio/advanced/index.html`.
- `fonts/noto-sans-devanagari.css` loads on all public pages — for the क्षण (Kshana) Devanagari wordmark; legitimate.
- No serif / Garamond / Georgia / Playfair anywhere (verified). All `font-family` values resolve to Geist / Geist Mono / Unbounded / Bricolage (Studio) / Noto Devanagari.

So: **Site = Unbounded display (Observatory). Studio = Bricolage display (pure tokens.css).** The split is intentional and consistent — the only loose end is that `tokens.css` still *documents* Bricolage in its header and sets `--f-display:"Bricolage Grotesque"`, which `theme.css` then overrides on the site. Not a bug, but a documentation mismatch.

---

## 4. Token-file parity

**Verdict: PASS — the only difference is the expected font-href path rewrite. NON-issue.**

`diff web/tokens.css web/studio/tokens.css` → single hunk, line 6:
- site: `href="fonts/bricolage-grotesque-geist-geist-mono.css"`
- studio: `href="../fonts/bricolage-grotesque-geist-geist-mono.css"`

All tokens, both light and dark, are byte-identical. (Both also carry the same stale `0.29.2` comment — see §9.)

---

## 5. Stale versions (engine is 0.31.0)

Most `0.29.x`/`0.30.0` hits in `llms-full.txt`, `docs/changelog.html`, `docs/overview.html`, `CHANGELOG.md` are **changelog history — legitimate, not drift.** The ones that matter are **live version stamps**:

**Live stamps still at v0.30.0 (DRIFT):**
- `web/missions.html` — "Engine v0.30.0" badges + provenance ("Engine v0.30.0, build da5ecd…") (27 hits; **zero** 0.31.0 in file).
- `web/capabilities.html` — "engine v0.30.0" provenance throughout (30 hits; zero 0.31.0).
- `web/editions.html` (19), `web/evidence.html` (22), `web/developers.html` (24) — all stamp 0.30.0, **none carry 0.31.0**.
- `web/llms.txt` (short) — tagline "Version 0.30.0 … by Ashforde OÜ" (×1).
- README chart SVGs footer-stamped 0.30.0 (8 files): `docs/assets/readme/{hero,architecture,flow-pipeline,lband-waterfall}-{light,dark}.svg`.
- Report exports stamp 0.30.0 (see §6).
- `web/data/series.json` (43), `web/assets/capabilities/capabilities.json` (20), `web/channels.json` (8), `web/studio/data/areas.json` (15) — recorded-run data stamped with the run's engine version.

**Correct (0.31.0):** `web/index.html` (21 refs), `web/PORT-MANIFEST.json` (`"version":"0.31.0"`), `Cargo.toml`, `README.md` (7).

**Internal contradiction:** `PORT-MANIFEST.json` declares the port at **0.31.0**, but the ported page content and recorded runs are **0.30.0**. The site source (kshana-site-next) was built on 0.30.0 recorded runs and ported under an 0.31.0 label. Fixing this needs re-recorded runs + re-port, not a string swap (pages are port-locked — see §11).

---

## 6. Report-export theme + version

`web/assets/**/report.html` (9 files: 5 `missions-runs/*.report.html`, 2 `waterfall/*/report.html`, 2 `runs/*/report.html`).

- **Theme: a separate warm "paper" theme, NOT the site tokens.** Palette: `#9a5b22` (brown), `#e0a36a` (tan), `#f0c060` (amber-gold), `#ece6da`/`#fbfaf7`/`#f3efe7` (creams), dark warm `#3a3328 #1c1914 #12100c`, accents `#6fd39b`/`#1f6f43` (green), `#b3a8ee`/`#5a4f8a` (lilac). No `#E8ECF3`/`#182033`/`#066A86`. It doesn't even match the site's *print* tokens (which are blue-grey→white).
- **Generator: `src/advanced_report.rs`** (in this repo, editable). It hardcodes the warm palette — confirmed `9a5b22`, `e0a36a ×2`, `ece6da ×2`, `f0c060 ×2` among 28 hex. It has **no hardcoded version** — the 0.30.0 in the exports is the engine version at record time.
- **Version: 0.30.0** across all report.html exports (stale).

**Flags:** (1) report theme is warm/tan while the site is blue-grey Observatory — a deliberate but unexplained second theme; reconcile or document it. (2) exports are stale at 0.30.0. Fixing the theme is in-repo (edit `advanced_report.rs`); fixing the committed exports needs re-generation + re-port.

---

## 7. Screenshots / README images

**Verdict: screenshots are CURRENT (Observatory theme); README chart SVGs are on-theme but version-stale.**

- `docs/assets/readme/site/SHOTS.json` + `studio/SHOTS.json`: real screenshots of kshana.dev built from `web/`, **captured 2026-10-01** — after the Observatory theme landed (2026-09-28), so they show the live theme. Strips: `site-strip-light.jpg`, `site-strip-dark.jpg` (Home hero, Evidence, Editions), `studio-{home,task,advanced}-{light,dark}.jpg`. No drawing/editing.
- README.md embeds 42 `<img>` (HTML, not markdown) from `docs/assets/readme/`: logo, hero, the three site/studio JPEG strips, and chart SVGs (campaign-timeline, lband-waterfall, coverage-map, solar-system, leo-pass, flow-*, architecture, research), each light+dark.
- Chart SVGs use the **Observatory palette** (verified hero-light.svg: `#066A86` cyan, `#B8288F` magenta, `#C9321F` coral/int, `#7E4B00` nav, `#377D0C` lime, `#0A1226` ink, `#5A6680`/`#3F4B67` blue-greys) — on-theme. But 8 of them footer-stamp **0.30.0** (§5).

---

## 8. OG card / favicon / meta

- **`og-card.svg`** (owned, regenerated by `tools/gen_og_card.py`): dark ground `#080A12/#0B0B10/#040508` (≈ Observatory dark), green "validated" badge `#3FB950/#0D1A10/#E7F6EC`, but an **eyebrow label + rule in warm tan `#C79E63`** over a `#241E18` brown radial — the warm family again, inconsistent with the site's cyan accent. Fonts are **system stacks** ("Helvetica Neue…", "Kohinoor Devanagari…"), **not** Unbounded/Geist. Acceptable for a static raster card but off-brand. No version string (states a validated count instead). `og-card.rendered-from.json` pins the PNG to the SVG via SHA — intact.
- **`favicon.svg`** (owned): a self-contained data-URI with light+dark PNG variants swapped by `prefers-color-scheme` — theme-aware, consistent with the dual-theme approach. No version stamp.
- **meta: GAP.** `<meta name="theme-color">` is present on **0 of 80** HTML pages; `apple-touch-icon` on **0 of 80**. Neither matches nor mismatches the theme — they're simply absent. Minor, but worth adding (and a theme-color would want the `#E8ECF3` light / `#060A14` dark grounds).

---

## 9. Other leftovers

- **Stale `0.29.2` comment** in both `tokens.css` and `studio/tokens.css` ("/* 0.29.2: soft ground… */", ×2 each) and in `theme.css` ("/* 0.29.2: a soft blue-grey ground… */"). Cosmetic, but stale.
- **`tokens.css` header still documents Bricolage** as `--f-display` and the Bricolage font link, while the live site uses Unbounded via `theme.css`. Documentation lag, not a render bug.
- **Dual accent systems coexist** in `site.css`: the six domain tokens (`--int…`) and the Observatory accents (`--cyan…`). Intentional, but a future consolidation candidate.
- **Near-duplicate domain hardcodes** in `css/capabilities.css`/`css/docs.css` (§2c) — should reference tokens.
- No commented-out old theme blocks, no Garamond/gold, no duplicated templates found.

---

## 10. Public / Pro boundary

### How the repo is built (constrains everything below)
`web/README.md` + `PORT-MANIFEST.json`: most of `web/` is **ported** by `web/tools/port_site.py` from a *built site folder* and *Studio folder* that live **outside this repo** (kshana-site-next on the Mac). Ported files are SHA-pinned in `PORT-MANIFEST.json` and **`site.test.mjs` fails on any hand-edit** — so they are effectively read-only here. The design-token single source is `explorations/2026-09-27-combo/src/tokens.css` (per the header) — **also outside this repo**. So the theme source of truth is currently **Mac-locked, not in version control reachable here.**

Confirmed ported (edit = breaks the port): `tokens.css`, `theme.css`, `site.css`, `css/home.css`, `index.html`, `missions.html`, `assets/pro/*`, `assets/**/report.html`.
Confirmed owned by `web/` (editable here): `og-card.svg/png`, `favicon.svg`, `capabilities.json`, `data/*` matrices, `fonts/`, `vendor/`, `build.sh`, `tools/`.

### Pro / proprietary-adjacent inventory (what's in the public repo)
| Item | What it is | Public OK? |
|---|---|---|
| `web/editions.html` + `css/editions.css` + `css/pro-power.css` | "Editions / Kshana Pro" **marketing** page (110 "Pro" refs), purple pro accent | **Yes** — marketing describing Pro is fine public. |
| `web/assets/pro/mission-dossier.pdf` | A **Pro mission-dossier sample PDF** | **Proprietary asset** — should live in kshana-pro, pulled in at port time. |
| `web/assets/pro/mission-dossier-page-0{1,2}.webp` | Rendered preview pages of the dossier | Preview webp may stay public as teaser; the **source PDF** should not be the system of record here. |
| `web/assets/**/report.html` (Advanced report exports) | Pro-style report samples (warm theme) | Samples OK public; **report template/theme** (`src/advanced_report.rs`) is the engine's — keep, but treat the *design* as a tracked template. |
| Design-token source `explorations/2026-09-27-combo/src/tokens.css` | **Not in this repo** (Mac) | **Risk** — the brand's single source of truth is Mac-locked. |
| Brand marks `assets/brand/*`, `docs/assets/readme/kshana-{logo,mark}-*` | Logos/wordmarks | Public distribution copies fine; **masters** belong in kshana-pro/brand. |

### Recommended `kshana-pro` layout (private, authoritative; public repo ports from it)
```
kshana-pro/
  site-next/            # the kshana-site-next source tree (pages, css, studio source)
                        #   → the "built site folder" + "Studio folder" port_site.py consumes
  design-tokens/        # explorations/.../src/tokens.css + theme.css SOURCE (the real single source)
                        #   build copies → web/tokens.css, web/studio/tokens.css, web/theme.css
  brand/                # master logos, wordmarks, favicon source, OG-card source + gen inputs
  report-templates/     # the report theme spec (mirror of advanced_report.rs styling) + sample inputs
  pro-assets/           # mission-dossier.pdf (master) + any paid-only PDFs/datasets
  README.md             # maps each folder to where it lands in the public repo at port time
```
**How the public port consumes it:** `port_site.py` already takes `--site <built folder> --studio <folder>` — point those at `kshana-pro/site-next/`. Extend the port to also copy `design-tokens/` → `web/tokens.css`/`web/studio/tokens.css`/`web/theme.css`, and `pro-assets/mission-dossier.pdf` + generated previews → `web/assets/pro/`. Everything stays SHA-pinned in `PORT-MANIFEST.json`, so the public repo remains a deterministic, reproducible export and **nothing is Mac-locked**. Getting `design-tokens/` and `site-next/` into kshana-pro is the single highest-leverage move — it is the only copy of the theme source today.

---

## 11. Prioritized fix list

**P1 — correctness / stale-facing-users**
- **Version drift: whole site presents engine v0.30.0** while engine is 0.31.0 (missions, capabilities, editions, evidence, developers pages; llms.txt tagline; 8 README chart SVGs; 9 report exports; recorded-run JSON). PORT-MANIFEST already says 0.31.0, so content and manifest contradict. — **[needs kshana-site-next source re-port]** (re-record runs under 0.31.0, rebuild, re-port). llms.txt tagline specifically is part of the ported crawl set — same path.
- **Get the theme source + site source into kshana-pro** (`design-tokens/`, `site-next/`). Today the only copy is on the Mac; a lost Mac loses the brand. — **[needs kshana-site-next source re-port]** (repo move, then it becomes in-repo).

**P2 — theme consistency**
- **Reconcile the warm report/OG palette with the Observatory site.** Either adopt the site tokens in `src/advanced_report.rs` (and re-generate/re-port the exports) or document the warm "paper report" theme as an intentional separate system. The generator edit is **[in-repo fixable]**; the committed exports are **[needs re-port]**.
- **og-card.svg tan accent `#C79E63` + system fonts** — align the accent to the site (cyan/Observatory) and/or embed brand fonts; owned file. — **[in-repo fixable]** (via `tools/gen_og_card.py`).
- **Near-duplicate domain hardcodes** in `css/capabilities.css`/`css/docs.css` → use `--int…--nav` tokens. — **[needs kshana-site-next source re-port]** (ported CSS).

**P3 — polish**
- **Add `<meta name="theme-color">` and `apple-touch-icon`** (absent on all 80 pages); theme-color `#E8ECF3` light / `#060A14` dark. — **[needs kshana-site-next source re-port]** (page `<head>` is ported) for the HTML; icon asset itself can be owned.
- **Tokenize the in-palette blue-grey hardcodes** in `css/light.css`/`css/docs.css`/`css/capabilities.css`. — **[needs kshana-site-next source re-port]**.
- **Clean stale `0.29.2` comments** in `tokens.css`, `studio/tokens.css`, `theme.css`; update tokens.css header to reflect Unbounded as the live site display font. — **[needs kshana-site-next source re-port]** (ported).
- **Tokenize the Pro lavender `#C3A6FF` / `#1E1446`** into a named pro accent. — **[needs kshana-site-next source re-port]**.
