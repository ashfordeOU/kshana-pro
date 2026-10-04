# Design tokens

`tokens.css` (the unified Kshana tokens of 2026-09-27: light ground `--bg #E8ECF3`, surface `#F5F7FB`,
ink `#182033`, the six failure-domain colours `--int --spf --tim --orb --itg --nav`, fonts) and
`theme.css` (the Observatory layer of 2026-09-28 that every public page loads after it: accents
`--cyan --magenta --lime --amber --coral` and their dark variants).

Both are links to the single source in `../site-next/explorations/2026-09-27-combo/src/`. The site
build copies them to the site root and the Studio; the port puts them in the public repo's `web/`
(`web/studio/tokens.css` differs only by its font path). See `../SOURCES.md`.
