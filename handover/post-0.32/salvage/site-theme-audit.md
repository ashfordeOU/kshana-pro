# Kshana → Ashforde theme/branding drift audit

Repository: `/home/user/kshana-src` (tag **v0.31.0**). Audited on disk only; nothing pushed.

## Executive summary — the headline finding

**The repository was never ported to the Ashforde theme at all.** None of the canonical
Ashforde design tokens exist anywhere in the tree:

- `--gold`, `--paper`, `--serif` custom properties: **0 occurrences** (grep over all CSS/HTML/SVG).
- Gold `#B6985A` and its family (`#9A7E45 #7D6636 #8E7645`): **0 occurrences.**
- Canonical ink/navy (`#11161E #161C27 #0E141F #0B1119 #0B0E14 #15140F`): **0 occurrences.**
- Fonts **Cormorant Garamond / EB Garamond / IBM Plex Mono / Tiro Devanagari Sanskrit**: **0 occurrences** (Cormorant appears only inside the binary `web/assets/pro/mission-dossier.pdf`).
- Wordmark **"Ashforde®"** with the registered mark: **0 occurrences** (across 1,569 "Ashforde" mentions).
- Sanskrit motto **यतो धर्मस्ततो जयः / Yato dharmas tato jayaḥ**: **0 occurrences.**
- Positioning line **"Sovereign Engineering · High Assurance"**: **0 occurrences.**
- Mission line **"For the missions that cannot fail."**: **0 occurrences** as a branding line (only a prose echo "missions that cannot fail" in `web/studio/lib/tour.mjs:70`).

What is actually in the repo are **three distinct non-Ashforde themes**, all legacy:

1. **Site "Observatory" theme** (`web/theme.css`) — soft blue-grey ground, six domain colours (red/purple/blue/teal/green/amber), fonts **Geist / Geist Mono / Unbounded / Bricolage Grotesque**. Tokens self-label "0.29.2".
2. **Shared "Kshana combo" tokens** (`web/tokens.css`, and a *different* `web/studio/tokens.css`) — same blue-grey family, fonts Bricolage Grotesque / Geist / Geist Mono.
3. **Exported report theme** (the 9 `*.report.html` files) — warm paper/brown (`--accent:#9a5b22`, `#e0a36a`), monospace/system-ui fonts, labelled "Kshana v0.30.0".

The product is still branded **"Kshana"** (wordmark `kshana`), with **"by Ashforde"** as the entity line and the tagline **"क्षण · the precise instant"**. Essentially every user-visible surface is off-brand relative to the Ashforde canon.

---

## 1. Canonical theme as found in-repo (and token inconsistencies)

There is **no Ashforde token set** in the repo. The authoritative token files are:

| File | Role | Fonts defined | Ground / ink |
|---|---|---|---|
| `web/theme.css` | Site "Observatory" theme (loaded after tokens.css) | `--sans:Geist`, `--mono:Geist Mono`, `--display:Unbounded` | light `--bg:#E8ECF3` / `--ink:#182033`; dark `--bg:#060A14` / `--ink:#EAF0FF` |
| `web/tokens.css` | Shared site+playground tokens | `--f-display:Bricolage Grotesque`, `--f-text:Geist`, `--f-mono:Geist Mono` | light `--bg:#E8ECF3` / `--ink:#182033`; dark `--bg:#07080E` |
| `web/studio/tokens.css` | Studio tokens (**differs** from `web/tokens.css`) | same family | blue-grey |

Token-level inconsistencies (independent of Ashforde):
- `web/theme.css:12` and `web/tokens.css:34` both carry stale comment markers **"0.29.2: soft ground…"** — the tokens pre-date 0.31.0.
- `web/tokens.css` and `web/studio/tokens.css` are **not byte-identical** (`diff -q` → DIFFERENT), despite `tokens.css`'s own header claiming build.py keeps them byte-identical — a real drift between site and Studio.
- `web/theme.css` defines a `--display:"Unbounded"` while `web/tokens.css` defines `--f-display:"Bricolage Grotesque"` — two different display fonts in the two token files loaded on the same pages.
- The only on-palette colour that coincidentally appears is cream **`#FBFAF7`** (9×) — but solely inside the 9 report.html exports, not in the site chrome.

---

## 2. Off-palette colours

**189 distinct hex values** in `web/` CSS/HTML/SVG; **162 distinct** in `docs/`. Virtually all are off the Ashforde palette. Grouped by role (count = occurrences in web CSS/HTML/SVG):

### 2a. Pure black/white & generic greys (expected paper/ink, found bootstrap-ish)
| Colour | Count | Where |
|---|---|---|
| `#000` | 66 | report.html print/fallback `:root` blocks, SVG strokes |
| `#FFF` | 41 | report.html, buttons, SVG |
| `#FFFFFF` | 10 | `web/theme.css` `--btn-ink`, print overrides |
| `#999` | 18 | report.html `--line` fallback (`report.html:57`) |
| `#F2F2F2` | 9 | report.html `--card` fallback (`report.html:47`) |
| `#444` | 9 | report.html `--muted` fallback |

The forced-colours/print `:root` in each report.html (`report.html:47`) sets `--bg:#fff; --fg:#000; --accent:#000` — pure black/white where Ashforde expects paper/ink.

### 2b. Observatory blue-grey ground & ink (site chrome) — all off-palette
`#E8ECF3 #DEE4EE #D3DBE8 #F5F7FB #EDF1F7 #182033 #3A455E #4E5971 #66718A`
(light) and `#060A14 #0A1122 #0E1730 #07080E #0D0F18 #12141F #181B28 #0D1528 #111B33 #05060C #0C0E18 #151826 #EAF0FF #F2F4FB #EEF0FA #A3AAC2 #A7B4D2 #858CA6 #8088A6` (dark). Defined in `web/theme.css`, `web/tokens.css`, `web/css/*.css`. These replace the Ashforde ink `#11161E` / paper `#F6F4F0` entirely.

### 2c. Six "domain" colours (brand chrome, not data-viz) — off-palette
These are used as UI chrome across every page (nav accents, card rules, icon tiles), so they read as brand colour, not legitimate viz series:
- **Interference/red**: `#B8262B #E5383B #FF5B54 #C9321F #B42D1B #B2361A #FF6A5C #FF9484 #FF6B63`
- **Spoofing/purple**: `#6236E0 #6B3FF0 #9C7DFF #A58BFF #C3A6FF #B3A8EE #6A2CD6 #6B4FD8 #5A4F8A #1E1446 #1A1240`
- **Timing/blue**: `#1C4FE0 #1F55F0 #6A98FF #5A8CFF #2F55C8 #3552A8 #86AAFF #7FA6FF #7EA0EC`
- **Orbits/teal-cyan**: `#006C7D #008795 #24C8D6 #2CD0DE #3DDCF7 #0795BA #066A86 #0A6478 #0A6A78 #6FE3F5 #5FDDF5 #4FC3A1 #1F6F8B #1F6F8B`
- **Integrity/green**: `#0A6F38 #0F8A47 #33CC78 #3DDC84 #A8EE5E #B9F07A #B8F38A #1F6F43 #1F7A3E #5FA82A #2F6E0A #3FB950 #6FD39B`
- **Nav/amber**: `#FFB224 #FFB547 #F5A300 #7C4800 #764600 #8A4700 #7E4B00 #F0C060 #F2E65A`
- **Magenta** (commercial/partner): `#A8247F #A0207A #F45CCB #F57ED6`

Defined canonically in `web/tokens.css:37-39` (`--int --spf --tim --orb --itg --nav` + `*-card` + `s-*` variants) and `web/theme.css:18` (`--cyan #066A86`, `--magenta #A8247F`, `--lime #2F6E0A`, `--amber #764600`, `--coral #B42D1B`). None are Ashforde gold.

### 2d. Report-export warm theme (closest to Ashforde spirit, still wrong accent)
In all 9 `*.report.html` (`report.html:8-10`): light `--bg:#fbfaf7 --fg:#1d1a15 --muted:#62594b --line:#d9d2c5 --card:#f3efe7 --accent:#9a5b22 --val:#1f6f43 --mod:#8a5a00 --par:#5a4f8a`; dark `--bg:#12100c --fg:#ece6da --muted:#a79d8c --line:#3a3328 --card:#1c1914 --accent:#e0a36a --val:#6fd39b --mod:#f0c060 --par:#b3a8ee`. The accent **`#9a5b22` / `#e0a36a`** is a burnt-orange/brown, **not** gold `#B6985A`; `#ece6da` is near `#ECE6D8` but not exact.

### 2e. OG card (`web/og-card.svg`) — fully legacy
Ground `#080A12 #0B0B10 #040508`, warm wash `#241E18 #241E19`, accent **`#C79E63`** (warm gold-brown, not `#B6985A`), wordmark fill `#F4F6FB`, sub `#CDD3DF`, green validation badge `#0D1A10`/`#3FB950`/`#E7F6EC`, footer `#8A93A3`.

### 2f. README badge colours (`README.md:21-32,447,580`)
`labelColor=0A1226` (dark navy) everywhere, with accents `#066A86` (teal), `#377D0C` (green), `#3F4B67`, `#7E4B00`, `#B8288F` (magenta). All legacy, no gold.

---

## 3. Font drift

Canonical Ashforde fonts (**Cormorant Garamond, EB Garamond, IBM Plex Mono, Tiro Devanagari Sanskrit**) are **entirely absent**. The repo ships and uses:

| Font in use | Where | Should be |
|---|---|---|
| **Bricolage Grotesque** (display) | `web/tokens.css:50`, `web/fonts/bricolage-grotesque-geist-geist-mono.css`, woff2 in `web/fonts/files/` | Cormorant Garamond |
| **Unbounded** (display, site) | `web/theme.css` `--display`, `web/fonts/geist-geist-mono-unbounded.css`, 5 woff2 | Cormorant Garamond |
| **Geist** (body) | `web/theme.css --sans`, `web/tokens.css --f-text` | EB Garamond |
| **Geist Mono** (mono) | `--mono` / `--f-mono`; 36 `font-family:'Geist Mono'` decls | IBM Plex Mono |
| **Noto Sans Devanagari** | `web/fonts/noto-sans-devanagari.css` (for क्षण) | Tiro Devanagari Sanskrit (for यतो धर्मस्ततो जयः) |
| **Helvetica Neue / Helvetica / Arial** | `web/og-card.svg` (2× `font-family="Helvetica Neue, Helvetica, Arial"`), `docs/assets/*` SVG charts | — |
| **Kohinoor Devanagari, Devanagari MT** | `web/og-card.svg` (Devanagari text) | Tiro Devanagari Sanskrit |
| **system-ui / Roboto fallback** | report.html charts `font-family:system-ui,-apple-system,Segoe UI,Roboto,sans-serif` (8×); generic `sans-serif` in SVG chart labels | — |
| **ui-monospace / Menlo / Consolas** | report.html body (`report.html:24,36`) | IBM Plex Mono |

Note: apparent "Inter" (667) and "Roboto" (17) raw grep hits are overwhelmingly the English word *interference* / *interop* etc. and the Roboto fallback above — **no `font-family: Inter` declaration exists**. The four Ashforde font families are nowhere. Font licence files on disk (`web/fonts/OFL-*.txt`) are for bricolage-grotesque, geist, geist-mono, unbounded, noto-sans-devanagari — the legacy set.

---

## 4. Wordmark / ® / tagline drift

| Canonical Ashforde | In repo | Count |
|---|---|---|
| Wordmark "A S H F O R D E ®" | wordmark is **`kshana`** (lowercase); `Ashforde®` with ® | **0** |
| "Ashforde" always with ® in headings/logos | "Ashforde" / "by Ashforde" / "by Ashforde OÜ" without ® | 1,569 total; "by Ashforde OÜ" 72×, "by Ashforde" 69× |
| Mission "For the missions that cannot fail." | absent (prose echo only, `web/studio/lib/tour.mjs:70`) | 0 |
| Positioning "Sovereign Engineering · High Assurance" | absent | 0 |
| Sanskrit "यतो धर्मस्ततो जयः / Yato dharmas tato jayaḥ" | absent; instead **क्षण** (87× in web) | 0 |
| — | Old tagline **"the precise instant"** | 91× |
| — | Old hero line **"Rehearse the minute GNSS goes dark"** | 26× |
| — | Old descriptor **"PNT-resilience simulator"** | 8× |

Representative anchors: `web/index.html:104` (`v0.31.0 … by Ashforde` — no ®), `web/index.html:380` (`Kshana · क्षण · the precise instant`), `web/index.html:386` (`Kshana v0.31.0 · by Ashforde OÜ`), `web/og-card.svg` (`Kshana` + क्षण + "PNT-RESILIENCE SIMULATOR"). The JSON-LD Organization in `web/index.html:37` correctly names "Ashforde OÜ" / `ashforde.org`, but `WebSite`/`SoftwareApplication` names remain "Kshana".

---

## 5. Stale version strings

Engine is **0.31.0**, but older versions are pervasive, especially in rendered assets:

| Version | Count (web, non-txt) | Notable locations |
|---|---|---|
| `v0.29.3` | 450 | Studio screenshots' alt-text, `web/*.html` badges/labels |
| `0.29.3` | 337 | ditto |
| `0.30.0` | 88 | 9 report.html exports ("Kshana v0.30.0"), chart SVG footers |
| `v0.30.0` | 56 | report chart `<text>` footers, README hero alt-text |
| `0.29.0/.1/.2` | 22 / 12 / 11 | tokens comments, misc |

- **SVG chart footers** embedded in report.html read `Kshana v0.30.0 · scenario … · kshana.dev` (e.g. `web/assets/runs/constellation-multi-gnss-coverage/report.html:84`) and `Kshana 0.30.0` (`:92,:116`).
- **README.md**: 10 stale pins (`0.30.0` / `0.29.3`) vs 7× `0.31.0`. Lines `README.md:48` (hero alt "engine v0.30.0"), `:89,:106,:112,:128` (studio/site alt "v0.29.3"), `:168` ("engine v0.30.0"), `:386,:597,:618,:634` ("kshana 0.30.0"). The release **badge** `README.md:25` is correct at `v0.31.0`.
- `README.crates.md`, `README.npm.md`, `README.pypi.md`: 1 stale pin each (vs 2× 0.31.0).
- Screenshots visibly show **"Live engine · v0.29.3"** and **"Recorded result · v0.29.3"** (see §6).

---

## 6. Image / screenshot inventory + stale findings

### Raster images (selected; all under repo)
| Path | Bytes | Referenced by | Theme |
|---|---|---|---|
| `docs/assets/readme/studio/studio-home-light.jpg` | 251,422 | README.md:106 | **Old** — "Kshana STUDIO", blue-grey, v0.29.3 (verified visually) |
| `…/studio-home-dark.jpg` | 244,866 | README (dark) | Old |
| `…/studio-task-light.jpg` / `-dark.jpg` | 196,178 / 187,419 | README.md:89 | Old |
| `…/studio-advanced-light.jpg` / `-dark.jpg` | 244,405 / 234,251 | README.md:112 | Old |
| `docs/assets/readme/site/site-strip-light.jpg` | 87,894 | README.md:128 | **Old** — "Rehearse the minute GNSS goes dark", "क्षण · the precise instant", v0.29.3, "by Ashforde" no ® (verified visually) |
| `…/site-strip-dark.jpg` | 85,475 | README (dark) | Old |
| `docs/assets/kshana-logo.png` | 202,679 | docs | Kshana mark |
| `docs/assets/kshana-mark.png` / `kshana-wordmark.png` | 36,640 / 27,686 | docs | Kshana mark/wordmark |
| `docs/assets/readme/kshana-mark-dark.png` / `-light.png` | 12,558 / 12,462 | README | Kshana mark |
| `web/og-card.png` | 136,994 | `og:image` in HTML heads | **Old** (matches og-card.svg §2e) |
| `web/assets/brand/kshana-mark-mask.png`, `web/studio/assets/brand/kshana-mark-mask.png`, `tools/readme-src/kshana-mark-mask.png` | 8,896 each | brand mask | Kshana mark |
| `web/assets/pro/mission-dossier-page-01.webp` / `-02.webp` | 30,400 / 47,528 | editions/pro | Pro dossier (contains Cormorant inside the source PDF) |
| planets (`earth-*`, `mars-*`, `moon-*` jpg/webp) | 4K–524K | 3D globe | neutral (legitimate textures) |
| `docs/assets/figures/*.png`, `docs/assets/diagrams/*.png`, `paper/crossover/*.png` | — | docs/paper | data-viz (series colours may be legit, but use Observatory palette) |

### SVGs (text — grep for off-palette)
All brand/figure SVGs use the legacy palette. Key ones:
- `web/favicon.svg` (49,059 B) — **identical size to `docs/assets/kshana-mark.svg`**; the Kshana mark (PNG-embedded), labelled "Kshana". Also inlined as a data-URI `<link rel=icon>` in every HTML head (e.g. `web/index.html:10`) — a base64 Kshana-mark PNG, light+dark.
- `web/og-card.svg` (2,330 B) — legacy (§2e).
- `docs/assets/kshana-banner.svg` (195,073 B), `kshana-wordmark.svg`, `kshana-mark.svg`, `readme/kshana-logo-{light,dark}.svg` — Kshana wordmark/mark; logo fill `#0A1226`.
- `docs/assets/readme/{hero,architecture,coverage-map,flow-*,leo-pass,lband-waterfall,research,solar-system,campaign-timeline}-{light,dark}.svg` — figure SVGs in the Observatory palette.
- `docs/assets/figures/*.svg`, `docs/assets/diagrams/*.svg` — data-viz; Observatory series colours.

**Stale-screenshot verdict:** every UI screenshot referenced by README (studio × 3 pairs, site-strip pair) and `web/og-card.png` is the **old Observatory theme** (blue-grey ground, teal/green/red/purple domains, Unbounded/Geist type, v0.29.3/v0.30.0, "Kshana" wordmark, क्षण tagline, "by Ashforde" without ®). None show gold/ink/paper, Cormorant/EB Garamond, "Ashforde®", or the यतो धर्मस्ततो जयः motto.

---

## 7. README drift

Across `README.md` (171 KB), `README.crates.md`, `README.npm.md`, `README.pypi.md`:
- **Logo**: `README.md:11` embeds `docs/assets/readme/kshana-logo-light.svg` (Kshana mark), not an Ashforde wordmark.
- **Badges** (`README.md:21-32,447,580`): legacy palette `labelColor=0A1226`, teal `#066A86`, green `#377D0C`, magenta `#B8288F`, etc.
- **Screenshots**: all old-theme (see §6), with alt-text documenting the old copy ("Rehearse the minute GNSS goes dark", "Kshana · क्षण · the precise instant").
- **Stale version pins**: 10 in README.md (`0.30.0`/`0.29.3`), 1 each in crates/npm/pypi (see §5).
- **Naming/tagline**: "Kshana" product name and "by Ashforde" (no ®) throughout; no "Ashforde®", no Sovereign Engineering/High Assurance, no Sanskrit motto.

---

## 8. Studio / report / scenario drift

- **Studio** (`web/studio/`): uses its own `web/studio/tokens.css` which **differs** from `web/tokens.css` (drift), plus `web/studio/studio.css` with off-palette colours (`#F2E65A #4FC3A1 #1F6F8B #0B1F3A …`). Screenshots show "Kshana STUDIO" / "KSHANA STUDIO" label, Observatory palette, "Live engine · v0.29.3".
- **Report exports** (9 files): `web/assets/missions-runs/*.report.html` (5), `web/assets/waterfall/*/report.html` (2), `web/assets/runs/*/report.html` (2). All use the warm-brown report theme (§2d), monospace/system-ui fonts, "Kshana run report", "Kshana v0.30.0", footer "AshfordeOU/kshana source & docs" — a **third** theme inconsistent with both the site and Ashforde. These are generated by the Rust engine (`src/advanced_report.rs`), so the drift originates in the engine's report template, not just the committed HTML.
- **Scenario catalogue**: Studio data (`web/studio/data/*.json`, `web/studio/scenarios/`) and the "16 areas / 139 scenarios" copy are on the legacy brand; capability/verification counts shown ("83 of 223", "123/249") appear in both README badges and screenshots using legacy colours.
- **Port manifest** `web/PORT-MANIFEST.json` (71 KB) contains no version pins flagged, but tracks the legacy page set.

---

## 9. Favicon / OG / meta

- **Favicon**: `web/favicon.svg` = Kshana mark; also inlined per-page as a light/dark base64 Kshana-mark PNG data-URI (`web/index.html:10` and every HTML head). No Ashforde mark.
- **OG image**: `og:image` → `https://kshana.dev/og-card.png` (`web/index.html:27`), rendered from `web/og-card.svg` (legacy, §2e) per `web/og-card.rendered-from.json`.
- **apple-touch-icon**: none found.
- **theme-color meta**: **none found** in any HTML (so no colour to correct, but also none set — a gap).
- **CNAME / canonical domain**: `web/CNAME` = `kshana.dev`; JSON-LD and links point at `kshana.dev` with `ashforde.org` only as the org URL/email. GitHub handle `ashfordeOU/kshana` (legitimate, not a ® context).

---

## 10. Other leftovers

- **Token file duplication/drift**: `web/tokens.css` vs `web/studio/tokens.css` not byte-identical despite the "byte-identical" contract comment (`web/tokens.css` header). Two display fonts (`Unbounded` in theme.css vs `Bricolage Grotesque` in tokens.css).
- **Stale "0.29.2" comments** baked into `web/theme.css:12`, `web/tokens.css:34`.
- **Light-mode stylesheet** `web/css/light.css` carries version strings and legacy tokens.
- **SVG chart footers** hard-code "Kshana v0.30.0 … kshana.dev" inside report.html (URL-encoded `<text>` elements), so regenerating from 0.31.0 is required.
- **Forced-colors/print `:root`** in report.html collapses to pure `#fff`/`#000`/`#999` (bootstrap-ish), not paper/ink.
- **Mission-concept leak**: "missions that cannot fail" appears as prose in `web/studio/lib/tour.mjs:70` but not as the branded positioning line — suggests a partial awareness of the Ashforde mission that never became the wordmark/tagline.
- **Entity string** "Ashforde OÜ" is present and correct in NOTICE, Cargo.toml, pyproject.toml, JSON-LD — so legal entity rebrand happened, but **visual + wordmark rebrand did not**.

---

## 11. Prioritized fix list

### P1 — user-visible brand breaks (every visitor sees these)
1. **Adopt the Ashforde token set.** Replace the Observatory/Kshana palette in `web/theme.css`, `web/tokens.css`, `web/studio/tokens.css` with gold `#B6985A` (+`#9A7E45 #7D6636 #8E7645`), ink `#11161E` family, paper `#F6F4F0 #FBFAF7 #ECE6D8 #EFECE5`, text `#212121`. Introduce `--gold --ink --paper --serif` names.
2. **Swap fonts to Cormorant Garamond / EB Garamond / IBM Plex Mono / Tiro Devanagari Sanskrit**; replace the `web/fonts/` woff2 + @font-face for Bricolage/Geist/Unbounded/Noto Devanagari; remove Helvetica/Arial/system-ui/Roboto fallbacks as brand faces.
3. **Rebrand the wordmark** to "A S H F O R D E ®" (letter-spaced, with ®) across headings/logos; replace the `kshana` wordmark in `web/index.html`, nav, footer, and all pages.
4. **Replace taglines**: "the precise instant" (91×) and "Rehearse the minute GNSS goes dark" (26×) → "Sovereign Engineering · High Assurance" and "For the missions that cannot fail."; swap क्षण motto for **यतो धर्मस्ततो जयः / Yato dharmas tato jayaḥ**.
5. **Regenerate favicon, og-card.svg/png, apple-touch-icon** with the Ashforde mark/colours; set a `theme-color` meta to ink/gold.
6. **Replace all README/site screenshots** (studio × 6, site-strip × 2, og-card) with new Ashforde-themed captures.

### P2 — high-impact but secondary surfaces
7. **Re-theme the 9 report.html exports** (and the generator `src/advanced_report.rs`) from warm-brown `#9a5b22`/`#e0a36a` to gold/ink/paper; fix embedded SVG chart footers.
8. **Fix stale versions**: bump all `v0.29.3` / `0.30.0` strings to `0.31.0` in README×4, report.html×9, screenshots, chart SVGs; regenerate rendered assets so the engine stamp reads 0.31.0.
9. **README badges**: restyle `labelColor=0A1226` + teal/green/magenta to the Ashforde palette.
10. **Reconcile token files**: make `web/studio/tokens.css` match `web/tokens.css` (honour the byte-identical contract) and settle one display font.

### P3 — minor / cleanup
11. Re-theme docs figure/diagram SVGs (`docs/assets/**`) and data-viz series to an Ashforde-derived viz ramp (series colours may legitimately differ but should harmonize).
12. Remove stale "0.29.2" comments in token files; drop legacy `Kohinoor/Devanagari MT` font stacks in og-card.
13. Audit `web/assets/pro/mission-dossier*` (the one asset that *does* reference Cormorant) for consistency with the new canon.
14. Convert the `tour.mjs:70` prose echo into the canonical mission line.

---

### Appendix — grep evidence (key counts)
- Ashforde tokens `--gold/--paper/--serif`: 0 files. `#B6985A`: 0. Canonical ink hexes: 0. Cormorant: only in `web/assets/pro/mission-dossier.pdf` (binary).
- `Ashforde®`: 0 / `Ashforde`: 1,569. यतो धर्मस्ततो जयः: 0 / क्षण: 87 (web).
- "Sovereign Engineering" / "High Assurance" / "For the missions that cannot fail.": 0.
- 189 distinct hex in web CSS/HTML/SVG; 162 in docs. Top chrome offenders: `#000`×66, `#FFF`×41, Observatory ground/ink and 6-domain colours throughout.
- Stale versions (web, non-txt): v0.29.3 ×450, 0.29.3 ×337, 0.30.0 ×88, v0.30.0 ×56.
- 9 exported report.html; all warm-brown theme + "Kshana v0.30.0".
