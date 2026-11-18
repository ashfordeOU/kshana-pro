# timing_v5c_igs fixture (V5c data set D-R3)

`clocks.txt` (SHA-256 `a76a398df30297c47199e5bd615ac3b7fb6d7f6678ed066229336aa6955bad48`):
International GNSS Service (IGS) final combined satellite clocks at 30 s,
`IGS0OPSFIN_20252780000_01D_30S_CLK.CLK.gz` to `IGS0OPSFIN_20252910000_01D_30S_CLK.CLK.gz`
(2025-10-05 to 2025-10-18, GPS weeks 2387 and 2388), downloaded on 2026-11-16 from the BKG mirror
of the IGS products (https://igs.bkg.bund.de/root_ftp/IGS/products/2387/ and /2388/), GPS Block IIF
satellites G03 G06 G08 G09 G10 G24 G25 G26 G27 G30 G32, decimated to 300 s. All eleven clocks are
complete (4 032 epochs each). Only this derived file is committed; the source files are not.

Cut by `tools/v5c/cut_v5c.py`: the open engine's extract rule (`scripts/fetch_igs_clocks.sh`: `AS`
records, seconds since 2025-10-05T00:00:00Z, the bias field) and the decimation and layout of its
cutter (`tests/fixtures/clock_state_igs_holdout_oracle/generate.py`), re-implemented without
numpy, scipy or allantools (not installed here). That cutter's header noise values need those
libraries and are not read by the predictor; they are written as 0 here.

Layout: a `@<prn> ... first_bias_s=<s>` header per clock, then `<t / 300 s> <increment>` lines,
the increment of (bias - first_bias) since the previous line in integer units of 1e-13 s.

Source files (SHA-256):

    9ab3785c49620918122fe7bb701fce90807fbe265c782cd1d48a69fcecba37dc  IGS0OPSFIN_20252780000_01D_30S_CLK.CLK.gz
    558133bbce621af557b9bf477ff4ac68cdd1bed4cc302ffe8fb9081eb0851fc1  IGS0OPSFIN_20252790000_01D_30S_CLK.CLK.gz
    a2eab8dc52aedd5347bb9756450759521c5b368b8e52d2cb5fb8a1a007252421  IGS0OPSFIN_20252800000_01D_30S_CLK.CLK.gz
    3ebaab527b7625bfbd3543275a9c32dd88099f07fc6be24e8aa9f36bbddcc1bb  IGS0OPSFIN_20252810000_01D_30S_CLK.CLK.gz
    6bf0b8b62132d05ab6f048f31724a055ad04607773fa3eb1100255b055905771  IGS0OPSFIN_20252820000_01D_30S_CLK.CLK.gz
    5fa6f754d7ba49ed649788333e2448b1e96e966458f6f20af19f9eae5cbdf5ac  IGS0OPSFIN_20252830000_01D_30S_CLK.CLK.gz
    6bc7cf499fbde3585fb9a39c3672d18b14f86119faaa7d870e9e1c1057bc0bf5  IGS0OPSFIN_20252840000_01D_30S_CLK.CLK.gz
    50d06addd31e5c09c132ce092c11705134a8c9a8ab8080c648e83139197c7921  IGS0OPSFIN_20252850000_01D_30S_CLK.CLK.gz
    7918840bb60f727fcbfa2caa3130a1d64555669bae0870cbbb529daa67b425c4  IGS0OPSFIN_20252860000_01D_30S_CLK.CLK.gz
    b12e8c07de3b2f8ff0450cb3dd5fa57690dd767e6f08d68195a9941edc9c56af  IGS0OPSFIN_20252870000_01D_30S_CLK.CLK.gz
    52f4df21b5b89d3a4a696c900921c450eddb8295bace9f8828f9431f743eaf19  IGS0OPSFIN_20252880000_01D_30S_CLK.CLK.gz
    7fc83f167e8c02884d927f36e9a782693a310569cead764f80d1d7387cdcef12  IGS0OPSFIN_20252890000_01D_30S_CLK.CLK.gz
    39c37584c524c1a22e411536278a37c47b2868d871a1bd24cafe25e93e34c3ea  IGS0OPSFIN_20252900000_01D_30S_CLK.CLK.gz
    5e4cec280f3cf46adb21621043ac7684c342763612cdc5a27964c06f4882cd0d  IGS0OPSFIN_20252910000_01D_30S_CLK.CLK.gz

Terms: IGS data and products are openly available for use without restriction, with attribution
to the IGS and its contributing organisations. Acknowledgement: International GNSS Service;
Johnston, Riddell and Hausler, "The International GNSS Service", Springer Handbook of Global
Navigation Satellite Systems (2017), 967-982.

Free-running satellite clocks: no GNSS removal took place; holdover is emulated.
