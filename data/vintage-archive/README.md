# As-issued data vintages

As-issued vintages of Earth-orientation, GNSS and timing products, captured daily for pre-registered forecast-skill tests (as-issued forecasts are overwritten upstream and cannot be recreated). Private until each source's licence is checked. One folder per UTC date, each with manifest.tsv (URL, HTTP status, retrieval time) and SHA256SUMS. Capture: `python3 tools/archive_vintage.py`.
