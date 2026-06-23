// PROPRIETARY — Kshana Pro overlay. NOT Apache-2.0. See LICENSE.
//
//! # Ashforde brand assets for the commercial study report
//!
//! Self-contained brand furniture for [`crate::report`]'s customer-facing HTML
//! face: the design tokens (Observatory-Navy / brass / ivory / ink), the serif type
//! stack with safe offline fallbacks (no web-font fetch — PDF/offline-safe), and the
//! Ashforde seal embedded **inline as a data URI** so the rendered HTML carries the
//! mark with zero external requests.
//!
//! The seal is baked in at compile time (not read from disk at render time) so a
//! report is reproducible and portable to any machine. If the constant were ever
//! empty, the renderer falls back to a plain serif "ASHFORDE" wordmark rather than
//! failing — the brand is never load-bearing for a successful render.

/// Observatory-Navy — the report background.
pub const NAVY: &str = "#0e1726";
/// Observatory-Navy (lighter panel).
pub const NAVY_PANEL: &str = "#131923";
/// Brass — the accent / rule colour.
pub const BRASS: &str = "#c7aa66";
/// Brass (muted).
pub const BRASS_MUTED: &str = "#b6985a";
/// Ivory — primary text on navy.
pub const IVORY: &str = "#ece6d8";
/// Ivory (lightest).
pub const IVORY_LIGHT: &str = "#f6f4f0";
/// Ink — text on ivory panels.
pub const INK: &str = "#212121";

/// Heading type stack (serif, offline-safe fallbacks; no web-font fetch).
pub const FONT_HEAD: &str = "'Cormorant Garamond', Georgia, 'Times New Roman', serif";
/// Body type stack (serif, offline-safe fallbacks; no web-font fetch).
pub const FONT_BODY: &str = "'EB Garamond', Georgia, 'Times New Roman', serif";

/// The Ashforde seal, embedded inline as a base64 SVG data URI (no external fetch).
pub const SEAL_DATA_URI: &str = "data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCA1MTIgNTEyIj48cGF0aCBkPSJNNDYwLjgwIDI1Ni4wMCBMNDM5LjYyIDI3Mi4wNiBMNDU3LjY5IDI5MS41NiBMNDM0LjA0IDMwMy43MSBMNDQ4LjQ1IDMyNi4wNSBMNDIzLjA1IDMzMy45MCBMNDMzLjM2IDM1OC40MCBMNDA2Ljk5IDM2MS43MiBMNDEyLjg5IDM4Ny42NCBMMzg2LjMzIDM4Ni4zMyBMMzg3LjY0IDQxMi44OSBMMzYxLjcyIDQwNi45OSBMMzU4LjQwIDQzMy4zNiBMMzMzLjkwIDQyMy4wNSBMMzI2LjA1IDQ0OC40NSBMMzAzLjcxIDQzNC4wNCBMMjkxLjU2IDQ1Ny42OSBMMjcyLjA2IDQzOS42MiBMMjU2LjAwIDQ2MC44MCBMMjM5Ljk0IDQzOS42MiBMMjIwLjQ0IDQ1Ny42OSBMMjA4LjI5IDQzNC4wNCBMMTg1Ljk1IDQ0OC40NSBMMTc4LjEwIDQyMy4wNSBMMTUzLjYwIDQzMy4zNiBMMTUwLjI4IDQwNi45OSBMMTI0LjM2IDQxMi44OSBMMTI1LjY3IDM4Ni4zMyBMOTkuMTEgMzg3LjY0IEwxMDUuMDEgMzYxLjcyIEw3OC42NCAzNTguNDAgTDg4Ljk1IDMzMy45MCBMNjMuNTUgMzI2LjA1IEw3Ny45NiAzMDMuNzEgTDU0LjMxIDI5MS41NiBMNzIuMzggMjcyLjA2IEw1MS4yMCAyNTYuMDAgTDcyLjM4IDIzOS45NCBMNTQuMzEgMjIwLjQ0IEw3Ny45NiAyMDguMjkgTDYzLjU1IDE4NS45NSBMODguOTUgMTc4LjEwIEw3OC42NCAxNTMuNjAgTDEwNS4wMSAxNTAuMjggTDk5LjExIDEyNC4zNiBMMTI1LjY3IDEyNS42NyBMMTI0LjM2IDk5LjExIEwxNTAuMjggMTA1LjAxIEwxNTMuNjAgNzguNjQgTDE3OC4xMCA4OC45NSBMMTg1Ljk1IDYzLjU1IEwyMDguMjkgNzcuOTYgTDIyMC40NCA1NC4zMSBMMjM5Ljk0IDcyLjM4IEwyNTYuMDAgNTEuMjAgTDI3Mi4wNiA3Mi4zOCBMMjkxLjU2IDU0LjMxIEwzMDMuNzEgNzcuOTYgTDMyNi4wNSA2My41NSBMMzMzLjkwIDg4Ljk1IEwzNTguNDAgNzguNjQgTDM2MS43MiAxMDUuMDEgTDM4Ny42NCA5OS4xMSBMMzg2LjMzIDEyNS42NyBMNDEyLjg5IDEyNC4zNiBMNDA2Ljk5IDE1MC4yOCBMNDMzLjM2IDE1My42MCBMNDIzLjA1IDE3OC4xMCBMNDQ4LjQ1IDE4NS45NSBMNDM0LjA0IDIwOC4yOSBMNDU3LjY5IDIyMC40NCBMNDM5LjYyIDIzOS45NCBaIiBmaWxsPSJub25lIiBzdHJva2U9IiNjN2FhNjYiIHN0cm9rZS13aWR0aD0iMi44MiIgc3Ryb2tlLWxpbmVqb2luPSJyb3VuZCIgb3BhY2l0eT0iMS4wIi8+PGNpcmNsZSBjeD0iMjU2LjAiIGN5PSIyNTYuMCIgcj0iMTY5Ljk4IiBmaWxsPSJub25lIiBzdHJva2U9IiNjN2FhNjYiIHN0cm9rZS13aWR0aD0iMS45NyIgb3BhY2l0eT0iMC44NSIvPjxnIHN0cm9rZT0iI2M3YWE2NiIgc3Ryb2tlLXdpZHRoPSIxLjY5IiBvcGFjaXR5PSIwLjgiPjxsaW5lIHgxPSIzNTguNDAiIHkxPSIyNTYuMDAiIHgyPSI0MjUuOTgiIHkyPSIyNTYuMDAiLz48bGluZSB4MT0iMzU0LjkxIiB5MT0iMjgyLjUwIiB4Mj0iNDIwLjE5IiB5Mj0iMzAwLjAwIi8+PGxpbmUgeDE9IjM0NC42OCIgeTE9IjMwNy4yMCIgeDI9IjQwMy4yMSIgeTI9IjM0MC45OSIvPjxsaW5lIHgxPSIzMjguNDEiIHkxPSIzMjguNDEiIHgyPSIzNzYuMjAiIHkyPSIzNzYuMjAiLz48bGluZSB4MT0iMzA3LjIwIiB5MT0iMzQ0LjY4IiB4Mj0iMzQwLjk5IiB5Mj0iNDAzLjIxIi8+PGxpbmUgeDE9IjI4Mi41MCIgeTE9IjM1NC45MSIgeDI9IjMwMC4wMCIgeTI9IjQyMC4xOSIvPjxsaW5lIHgxPSIyNTYuMDAiIHkxPSIzNTguNDAiIHgyPSIyNTYuMDAiIHkyPSI0MjUuOTgiLz48bGluZSB4MT0iMjI5LjUwIiB5MT0iMzU0LjkxIiB4Mj0iMjEyLjAwIiB5Mj0iNDIwLjE5Ii8+PGxpbmUgeDE9IjIwNC44MCIgeTE9IjM0NC42OCIgeDI9IjE3MS4wMSIgeTI9IjQwMy4yMSIvPjxsaW5lIHgxPSIxODMuNTkiIHkxPSIzMjguNDEiIHgyPSIxMzUuODAiIHkyPSIzNzYuMjAiLz48bGluZSB4MT0iMTY3LjMyIiB5MT0iMzA3LjIwIiB4Mj0iMTA4Ljc5IiB5Mj0iMzQwLjk5Ii8+PGxpbmUgeDE9IjE1Ny4wOSIgeTE9IjI4Mi41MCIgeDI9IjkxLjgxIiB5Mj0iMzAwLjAwIi8+PGxpbmUgeDE9IjE1My42MCIgeTE9IjI1Ni4wMCIgeDI9Ijg2LjAyIiB5Mj0iMjU2LjAwIi8+PGxpbmUgeDE9IjE1Ny4wOSIgeTE9IjIyOS41MCIgeDI9IjkxLjgxIiB5Mj0iMjEyLjAwIi8+PGxpbmUgeDE9IjE2Ny4zMiIgeTE9IjIwNC44MCIgeDI9IjEwOC43OSIgeTI9IjE3MS4wMSIvPjxsaW5lIHgxPSIxODMuNTkiIHkxPSIxODMuNTkiIHgyPSIxMzUuODAiIHkyPSIxMzUuODAiLz48bGluZSB4MT0iMjA0LjgwIiB5MT0iMTY3LjMyIiB4Mj0iMTcxLjAxIiB5Mj0iMTA4Ljc5Ii8+PGxpbmUgeDE9IjIyOS41MCIgeTE9IjE1Ny4wOSIgeDI9IjIxMi4wMCIgeTI9IjkxLjgxIi8+PGxpbmUgeDE9IjI1Ni4wMCIgeTE9IjE1My42MCIgeDI9IjI1Ni4wMCIgeTI9Ijg2LjAyIi8+PGxpbmUgeDE9IjI4Mi41MCIgeTE9IjE1Ny4wOSIgeDI9IjMwMC4wMCIgeTI9IjkxLjgxIi8+PGxpbmUgeDE9IjMwNy4yMCIgeTE9IjE2Ny4zMiIgeDI9IjM0MC45OSIgeTI9IjEwOC43OSIvPjxsaW5lIHgxPSIzMjguNDEiIHkxPSIxODMuNTkiIHgyPSIzNzYuMjAiIHkyPSIxMzUuODAiLz48bGluZSB4MT0iMzQ0LjY4IiB5MT0iMjA0LjgwIiB4Mj0iNDAzLjIxIiB5Mj0iMTcxLjAxIi8+PGxpbmUgeDE9IjM1NC45MSIgeTE9IjIyOS41MCIgeDI9IjQyMC4xOSIgeTI9IjIxMi4wMCIvPjwvZz48Y2lyY2xlIGN4PSIyNTYuMCIgY3k9IjI1Ni4wIiByPSI5NC4yMSIgZmlsbD0ibm9uZSIgc3Ryb2tlPSIjYzdhYTY2IiBzdHJva2Utd2lkdGg9IjIuMjUiIG9wYWNpdHk9IjEuMCIvPjxnIGZpbGw9Im5vbmUiIHN0cm9rZT0iI2M3YWE2NiIgc3Ryb2tlLXdpZHRoPSIxLjQxIiBvcGFjaXR5PSIwLjYiPjxjaXJjbGUgY3g9IjMxMS4zMCIgY3k9IjI1Ni4wMCIgcj0iMjkuNDkiLz48Y2lyY2xlIGN4PSIzMDMuODkiIGN5PSIyODMuNjUiIHI9IjI5LjQ5Ii8+PGNpcmNsZSBjeD0iMjgzLjY1IiBjeT0iMzAzLjg5IiByPSIyOS40OSIvPjxjaXJjbGUgY3g9IjI1Ni4wMCIgY3k9IjMxMS4zMCIgcj0iMjkuNDkiLz48Y2lyY2xlIGN4PSIyMjguMzUiIGN5PSIzMDMuODkiIHI9IjI5LjQ5Ii8+PGNpcmNsZSBjeD0iMjA4LjExIiBjeT0iMjgzLjY1IiByPSIyOS40OSIvPjxjaXJjbGUgY3g9IjIwMC43MCIgY3k9IjI1Ni4wMCIgcj0iMjkuNDkiLz48Y2lyY2xlIGN4PSIyMDguMTEiIGN5PSIyMjguMzUiIHI9IjI5LjQ5Ii8+PGNpcmNsZSBjeD0iMjI4LjM1IiBjeT0iMjA4LjExIiByPSIyOS40OSIvPjxjaXJjbGUgY3g9IjI1Ni4wMCIgY3k9IjIwMC43MCIgcj0iMjkuNDkiLz48Y2lyY2xlIGN4PSIyODMuNjUiIGN5PSIyMDguMTEiIHI9IjI5LjQ5Ii8+PGNpcmNsZSBjeD0iMzAzLjg5IiBjeT0iMjI4LjM1IiByPSIyOS40OSIvPjwvZz48ZyB0cmFuc2Zvcm09InRyYW5zbGF0ZSgyMTYuNDkzLDI5MS4zMjgpIHNjYWxlKDAuMTExNDQsLTAuMTExNDQpIiBmaWxsPSIjZWNlNmQ4Ij48cGF0aCBkPSJNMTc5IDI0NSAxOTIgMjcwSDQ3Mkw0NzkgMjQ1Wk03MTEgMTJRNzE1IDEyIDcxNS4wIDYuMFE3MTUgMCA3MTEgMFE2ODEgMCA2NDkuNSAyLjBRNjE4IDQgNTg5IDRRNTU2IDQgNTMxLjUgMi4wUTUwNyAwIDQ3NyAwUTQ3MyAwIDQ3My4wIDYuMFE0NzMgMTIgNDc3IDEyUTUyMiAxMiA1MzEuMCAyOC4wUTU0MCA0NCA1MTYgOTFMMzA3IDUxNkwzNDggNTgzTDE1MCAxMzdRMTI0IDc2IDE0OC41IDQ0LjBRMTczIDEyIDIzMiAxMlEyMzcgMTIgMjM3LjAgNi4wUTIzNyAwIDIzMiAwUTIwMiAwIDE3Ny4wIDIuMFExNTIgNCAxMTQgNFE3NiA0IDUzLjUgMi4wUTMxIDAgLTIgMFEtNiAwIC02LjAgNi4wUS02IDEyIC0yIDEyUTI1IDEyIDQ0LjUgMjMuMFE2NCAzNCA4My4wIDYzLjVRMTAyIDkzIDEyNSAxNDZMMzQwIDYzMVEzNDIgNjM0IDM0OS4wIDYzNC4wUTM1NiA2MzQgMzU3IDYzMUw2MDIgMTM3UTYyNSA4OSA2NDMuMCA2MS41UTY2MSAzNCA2NzcuMCAyMy4wUTY5MyAxMiA3MTEgMTJaIi8+PC9nPjwvc3ZnPg==";

/// The seal as an `<img>`, or a plain serif "ASHFORDE" wordmark fallback if the
/// embedded data URI is somehow empty (the brand never fails a render).
pub fn seal_img_html(px: u32) -> String {
    if SEAL_DATA_URI.is_empty() {
        format!(
            "<div style=\"font-family:{FONT_HEAD};font-size:{}px;letter-spacing:.18em;             color:{BRASS};font-weight:700\">ASHFORDE</div>",
            px / 4
        )
    } else {
        format!(
            "<img src=\"{SEAL_DATA_URI}\" alt=\"Ashforde seal\" width=\"{px}\" height=\"{px}\"              style=\"display:block\"/>"
        )
    }
}
