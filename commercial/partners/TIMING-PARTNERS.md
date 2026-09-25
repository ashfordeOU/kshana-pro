<!-- PROPRIETARY — Kshana Pro overlay. Internal commercial record. Not part of the open-source engine. -->

# Timing partners: laboratories, test houses and metrology institutes

**Owner:** Ashforde OÜ · **Status:** researched list, nobody contacted · **Date:** 2026-09-25

Organisations in the European Union and the United Kingdom that work on telecom timing,
GNSS holdover or timing resilience, with the public route to reach each one. Every
entry cites the public page it rests on; each page was fetched on 2026-09-25 unless the
row says otherwise. Draft emails are in [`OUTREACH-DRAFTS.md`](OUTREACH-DRAFTS.md).
**Nothing has been sent.**

Abbreviations: GNSS (Global Navigation Satellite System), NMI (national metrology
institute, the body that keeps a country's official time and measurement standards),
UTC (Coordinated Universal Time; UTC(k) is the realisation kept by laboratory k), PTP
(Precision Time Protocol, IEEE 1588), NTP (Network Time Protocol), ePRTC (enhanced
primary reference time clock), CNI (critical national infrastructure), PNT (positioning,
navigation and timing), EU (European Union), JRC (the European Commission's Joint
Research Centre), T-BC (telecom boundary clock), ITU-T (International Telecommunication
Union Telecommunication Standardization Sector), IEEE (Institute of Electrical and Electronics Engineers), ESA (European Space Agency), UK (United Kingdom), HTTP 403 (the web server's "access refused" response). Organisation names: INRIM (Istituto Nazionale di Ricerca Metrologica, Italy), LNE-SYRTE (Laboratoire national de métrologie et d'essais – Systèmes de Référence Temps-Espace, France), RISE (Research Institutes of Sweden), IIS (Fraunhofer Institute for Integrated Circuits), METAS (Swiss Federal Institute of Metrology), VSL (the Dutch national metrology institute, formerly Van Swinden Laboratorium), TQE 32 (the number NPL prints on that report).

## Why partners, and what to ask for

The 2026-09-25 first-principles review found three things a partner can supply that
Ashforde cannot supply alone:

1. **Measured data**, so that holdover predictions can be checked against real clocks
   and a MODELLED figure can become a VALIDATED one;
2. **Hardware credibility**: a laboratory or test house that measures, while Kshana
   predicts before purchase;
3. **A channel** to the operators who buy timing studies, and a continuity answer for
   buyers who worry about a one-maintainer supplier.

So each row carries one of three asks: **data** (a validation data set or a joint
comparison), **channel** (offer the study to their clients, or subcontract), or
**standards** (review of the open method).

## Shortlist, in suggested order

| # | Organisation | Country | What they do that matters here | Ask | Public contact route | Sources |
|---|---|---|---|---|---|---|
| 1 | **National Physical Laboratory (NPL)**, Time and Frequency | United Kingdom | UK NMI. Runs NPLTime, a GNSS-independent time service over fibre, traceable to UTC(NPL), for users including mobile network operators; its service nodes carry caesium clocks for holdover. Leads the UK National Timing Centre programme. Published NPL Report TQE 32, *Holdover Atomic Clock Landscape Review* (July 2024), on holdover clocks for CNI. | data, standards | General contact page <https://www.npl.co.uk/contact>; products and services enquiries <https://www.npl.co.uk/products-services/contact-us>; the contact page lists `measurement_services@npl.co.uk` | <https://www.npl.co.uk/research/time-frequency>; <https://www.npl.co.uk/products-services/time-frequency/npltime>; <https://www.npl.co.uk/ntc>; <https://eprintspublications.npl.co.uk/9987/1/TQE32.pdf> |
| 2 | **VSL**, Time and Frequency | Netherlands | Dutch NMI; maintains UTC(VSL). Distributes time by NTP, PTP and White Rabbit, and characterises and calibrates a customer's whole timing chain (fibre, grandmaster and boundary clocks, servers) with an uncertainty budget. Names telecom, financial markets and energy as users. | data, channel | `vsl@vsl.nl`; contact page <https://www.vsl.nl/en/contact> | <https://www.vsl.nl/en/areas-of-expertise/time-and-frequency/>; <https://www.vsl.nl/en/themes/security-and-resilience/> |
| 3 | **Metrosert**, national measurement standards | Estonia | Estonia's NMI. Its page says it is **developing** a national time and frequency standard, commissioned by the Ministry of Economic Affairs and Communications, to support companies that depend on accurate time. Same country as Ashforde OÜ's registration. | data, standards | Contact page <https://metrosert.ee/en/contact/> | <https://metrosert.ee/en/time-and-frequency_national/> |
| 4 | **Chronos Technology** | United Kingdom | Timing and GNSS specialist founded 1986; consultancy, installation and support for resilient synchronisation (GNSS, PTP, NTP) for CNI, defence and industry; publishes on holdover in 5G base stations. | channel | `sales@chronos.uk`; contact page <https://chronos.uk/contact/> | <https://chronos.uk/services/secure-reliable-and-traceable-timing-services/>; <https://chronos.uk/chronos-times/mitigating-loss-of-clock-accuracy-or-synchronisation-in-5g-cellular-base-stations/> |
| 5 | **Physikalisch-Technische Bundesanstalt (PTB)**, Working Group 4.42 Dissemination of Time | Germany | German NMI. The group realises and disseminates legal time for Germany and runs international time comparisons. Germany is Ashforde's ESA-recorded operative nationality. | data, standards | PTB contact page <https://www.ptb.de/cms/en/meta/contact.html> (the working-group page names its head; use the general route first) | <https://www.ptb.de/cms/en/ptb/fachabteilungen/abt4/fb-44/ag-442.html> |
| 6 | **European Commission Joint Research Centre (JRC)**, Complementary PNT | EU (Ispra, Italy) | "Researches, tests and assesses complementary Position, Navigation and Timing technologies" for a resilient EU system; hosts a GNSS testing and demonstration hub; offers open access to its research infrastructures. | standards, data | The C-PNT page and the JRC "Open access to JRC Research Infrastructures" and "Labs as a Service" routes linked from it; general Commission contact <https://commission.europa.eu/about/contact_en> | <https://joint-research-centre.ec.europa.eu/projects-and-activities/complementary-position-navigation-and-timing_en> |
| 7 | **INRIM**, Time and Frequency | Italy | Italian NMI; realises UTC(IT); offers calibration of time and frequency standards and traceability to UTC for oscillators hosted at institutes and industry. | data | Contacts page <https://www.inrim.it/en/contacts> | <https://www.inrim.it/en/research/scientific-sectors/time-and-frequency>; <http://rime.inrim.it/labtf/calibration-activities/> |
| 8 | **RISE**, National Laboratory for Time and Frequency | Sweden | Swedish NMI for time; maintains Swedish national time UTC(SP); runs time-distribution services; its time underpins a national network of ePRTC clocks independent of GNSS (per a partner's announcement). The page returned HTTP 403 to a script on 2026-09-25; the description is from search results. | data | Via the laboratory page | <https://www.ri.se/en/expertise-areas/expertises/national-laboratory-for-time-and-frequen>; <https://www.hoptroff.com/news/hoptroff-partners-with-rise-sweden> |
| 9 | **LNE-SYRTE** (Observatoire de Paris) | France | French national reference for time and frequency; generates UTC(OP), the basis of French legal time. | data, standards | "Nous contacter" page linked from <https://syrte.obspm.fr/spip/fr/services/ref-temps/index.html> | same page |
| 10 | **Fraunhofer IIS**, Test and Application Center L.I.N.K. | Germany | Test environment for positioning, navigation and communication, with multi-GNSS simulators, record-and-replay, interference detection and a mobile measurement laboratory. Positioning focus rather than telecom timing. | channel | Fraunhofer IIS contact pages linked from the centre page, for example <https://www.iis.fraunhofer.de/en/kontakt.html> | <https://www.iis.fraunhofer.de/en/ff/lv/lok/test/link.html> |
| 11 | **Calnex Solutions** | United Kingdom | Test instrumentation for network synchronisation; its Paragon-neo tests T-BC compliance to ITU-T G.8273.2, including holdover performance. A measurement-tool vendor, not a test house: a partner for data formats and a possible referral route, and a competitor only for analysis software bundled with its hardware. | data (log formats), channel | Contact form <https://calnexsol.com/contact-us/> | <https://calnexsol.com/network-synchronisation/> (holdover detail from search results) |
| 12 | **EANTC** (European Advanced Networking Test Center) | Germany (Berlin) | Independent test lab running multi-vendor interoperability events that include time synchronisation for 5G transport. Its site returned HTTP 403 to a script on 2026-09-25; description from its published test reports. | channel | Via <https://eantc.de/> | <https://eantc.de/wp-content/uploads/2023/12/EANTC-MPLSSDNInterop2024-TestReport-v1.3.pdf> |

## Not on the list, and why

- **Spirent, Keysight, Safran and other simulator vendors.** They run funded CNI
  assessment work and radio-frequency simulation; the first-principles review
  recommends complementing them, not approaching them first.
- **Royal Institute of Navigation (RIN).** A professional society, not a laboratory or
  test house. It is a sensible standards contact alongside NPL; its site refused a
  script on 2026-09-25, so no contact route is recorded here.
- **Laboratories outside the EU and UK** (for example METAS in Switzerland) are out of
  scope for this list.

## Rules for using this list

- Use the **general or service** contact route in each row. Individual names appear on
  some public pages; do not write to a named person until the organisation replies.
- **Do not attach or quote non-public material.** The timing study offer is behind the
  Pack B export-control hold (`legal/EXPORT-CONTROL-DETERMINATION.md` section 5) and
  has placeholder prices. Talk about the open engine, the published preprint and the
  validation question only.
- Everything Kshana produces today is described as **MODELLED** unless it reproduces a
  published reference; say so in every conversation.
- Record each contact below when it happens.

| Date | Organisation | Route | Sent by | Reply |
|---|---|---|---|---|
| — | — | — | — | nothing sent |
