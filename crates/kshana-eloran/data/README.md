# kshana-eloran data: GRI / chain database and UTC(k) list

PROPRIETARY — Kshana Pro.

Two files live here. Both are embedded in the crate with `include_str!` (see `src/gridb.rs`):

| File | Format tag | What it holds |
|---|---|---|
| `gri_db.json` | `kshana-eloran-gri-db` v1 | LORAN-C, eLoran and Chayka chains worldwide, current and historical |
| `utc_k.json` | `kshana-eloran-utc-k` v1 | Laboratories that publish a UTC(k) in BIPM Circular T |

## Chain database (`gri_db.json`)

Each chain entry has an `id` (unique key; several entries may share a GRI if they belong to
different eras), `gri_us`, `name`, `region`, `countries`, `system`, `status`,
`operational_from` / `operational_to` (partial ISO dates), `time_ref`, `sources`,
`confidence`, `notes` and `stations`. Station fields follow `chain::Station` (`id`, `name`,
`role`, `position {lat_deg, lon_deg}`, `power_kw`, `emission_delay_us`), plus
`coding_delay_us`, `sources`, `confidence` and `notes`.

**A value no source gave is `null`. Nothing is estimated or made up.** A chain whose stations
lack a position, power or (for secondaries) emission delay is *incomplete*.
`DbChain::to_chain` refuses an incomplete chain and lists what is missing. The caller can
supply the values with `gridb::Overrides`, or with `skip_incomplete_stations` for
secondaries.

### Confidence levels

* `verified`: two independent sources agree, or one authoritative primary source gives the
  value (here, the USCG Loran-C User Handbook table).
* `single-source`: one secondary source (a web page, a paper, a code repository, a quoted
  excerpt).
* `unverified`: the sources conflict, the value comes from a misaligned or low-precision table,
  or it was inferred.

A chain's confidence describes the entry as a whole. A station's confidence describes that
station's values; the notes say which field a lower confidence applies to.

### Sources used

1. **USCG, Loran-C User Handbook, COMDTPUB P16562.6 (1992), Appendix B, Table B-1**
   (WGS 84 positions, emission delays, coding delays and nominal powers):
   <https://www.loran.org/otherarchives/1992%20Loran-C%20User%20Handbook%20-%20USCG.pdf>.
   This is the primary source for the US/Canadian chains (5930, 5990, 7930, 7960, 7980, 8290,
   8970, 9610, 9940, 9960, 9990), the USCG overseas chains (7970, 7990, 9970, 9980), Chayka
   8000 and 7950, and Saudi 8990 / 7170. The values were transcribed from page images.
2. NGA Pub. 117 *Radio Navigational Aids*, 2005 and 2014 editions, Chapter 6 (station lists
   and positions to 1 arcsec, including the post-1995 NELS, FERNS, Saudi and Indian chains).
   2005 edition: <https://www.govinfo.gov/content/pkg/GOVPUB-D5_300-PURL-LPS59185/pdf/GOVPUB-D5_300-PURL-LPS59185.pdf>.
   The NGA download of chapter 6 itself could not be retrieved.
3. Wikipedia, "Loran-C", section "List of LORAN-C transmitters", wikitext retrieved
   2026-11-18 (positions, designators, shutdown dates).
4. B. Clarke, "LORAN-C Chains", <https://prc68.com/I/Loran-c.shtml> (positions and coding
   delays; matches the handbook digit for digit).
5. ENSM Le Havre / SHOM, "Documentation nautique" v2.2, table "6731 - Chaîne Sud Europe"
   (NELS 6731 emission delays and powers).
6. Commissioners of Irish Lights, Notice to Mariners 13/2012 (Anthorn: GRI 6731, ED 27300,
   station Y, position).
7. CheolJ, "Loran-c-reference-code" README (emission delays for 7430, 8390 and 9930 W/Z).
8. Korean eLoran papers: Seo et al., ENC 2013; Son et al., IALA 2018; Kim et al.,
   arXiv:2109.08990.
9. BIPM Time Signals 2025 (BPL); BIPM Circular T 466 (the UTC(k) list).
10. A SlideShare deck (NELS 7499 emission delays, known only from a web-search excerpt).
    These values are marked `single-source`.

### Known gaps (incomplete entries)

* NELS 7001 Bø and 9007 Ejde: emission delays and powers not found. 7499: Værlandet power
  unknown, and its emission delays are single-source. 6731 Lessay: Anthorn's power unknown.
* UK Anthorn eLoran (timing, from 2016): the secondary emission delay, power and 2026 status
  are not verified.
* FERNS / East Asia: Korean, Japanese and Chinese station powers; emission delays for 9930 X/Y,
  8930 and 6780; the position and delay of the Korean testbed transmitter 9930-V.
* 5980 Russian-American and 7270 Newfoundland: only coding delays are published (not emission
  delays).
* Saudi 8830 / 7030 and Indian 5543 / 6042: emission delays and powers. The 5543 secondary
  designators conflict between sources.
* 7170 (1992): the handbook's delay columns are shifted by one row, so the delays are left null.
* BPL (Pucheng): GRI 60 000 µs unverified; position to 1 arcmin.
* Not covered: China's new ground-based timing stations and any announced future deployments
  (UK, US, other). Add these with a user merge file when they are published.
* Chayka chains use the Chayka phase codes. `to_chain` produces the LORAN-C signal; supply a
  Chayka `PulseSignal` yourself if you need it.
* Most chain `time_ref` values are single-source or unverified (see each chain's notes). Many
  are null.

## Updating the database

* **Without rebuilding**: write a user file with the same header and call
  `GriDb::merge_json`. Each entry is a patch. An existing `id` is updated field by field:
  stations are merged by id, `sources` are appended, and `remove_stations` deletes stations. An
  unknown `id` is added and must give `gri_us`, `name`, `status` and stations (a new station
  needs a `role`). The merged database is validated, and nothing changes if validation fails.

  ```json
  { "format": "kshana-eloran-gri-db", "version": 1, "updated": "2027-01-01",
    "chains": [
      { "id": "6731-lessay", "stations": [ { "id": "Y", "power_kw": 250, "sources": ["…"] } ] },
      { "id": "5000-new", "gri_us": 50000, "name": "5000 New", "status": "planned",
        "operational_from": "2030", "stations": [ … ] } ] }
  ```

* **In the embedded file**: edit `gri_db.json` directly. Give every value a source and a
  confidence, never fill a gap by guessing, bump `updated`, and run `cargo test` in
  `crates/kshana-eloran`. The tests check that the file parses, that GRIs are in range and
  unique per era (two entries with the same GRI need known, non-overlapping operating
  periods), that every complete chain converts and passes `Chain::validate`, and the cited
  spot values.

## UTC(k) list (`utc_k.json`)

There is one entry per laboratory in BIPM Circular T 466 (2026-11-13) section 1:
<https://webtai.bipm.org/ftp/pub/tai/Circular-T/cirt/cirt.466>.

* `code`, `utc_k` and `city` come from Circular T itself (`confidence: verified`).
* `country` and `institution` are not in Circular T. They were added from general knowledge.
  `institution_confidence` is `general-knowledge` for long-established laboratories and
  `unverified` for the others.
* `ct466_last_utc_minus_utck_ns` and `ct466_max_abs_ns` are a snapshot of UTC − UTC(k) (ns)
  for 2026-09-26 to 2026-10-31. They are **illustrative only**.

`gridb::default_time_ref(id)` returns an ideal `TimeRefModel` (`UTC(k) = UTC`).
`gridb::illustrative_time_ref(id)` sets the offset from the snapshot
(`x = UTC(k) − UTC = −(UTC − UTC(k))`). To refresh the list, take the newest
`cirt.NNN` and update the codes, cities and snapshot values.
