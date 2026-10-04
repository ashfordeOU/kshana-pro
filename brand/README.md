# Kshana brand masters

The product brand of Kshana (the founder's mark, the wordmark, क्षण, the blue-grey theme). Never
redraw the mark: recolour or resize the mask (`kshana-mark-mask.png`) only, as the site does.

- `kshana-mark.svg`, `kshana-mark.png`, `kshana-wordmark.*`, `kshana-logo.png`, `kshana-banner.svg`:
  as published in the public repo's `docs/assets/`.
- `favicon.svg`: the public `web/favicon.svg` (the pages themselves inline a theme-aware data URI,
  from `site-next/.../src/shell.html`).
- `kshana-mark-mask.png`, `apple-touch-icon.png`: links into the site source (`assets/brand/`); the
  icon is drawn by `site-next/.../src/tools/gen_touch_icon.py`.
- `og-card.svg`, `og-card.png`: the social card, drawn by the public `tools/gen_og_card.py`.
- `fonts/`: Unbounded, Geist and Geist Mono (SIL Open Font License texts included).

`SHA256SUMS` lists every file as copied on 2026-10-04 (engine 0.31.0). See `../SOURCES.md`.
