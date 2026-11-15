# timing_v5_igs fixture (V5 data set D-R)

`clocks.txt` (SHA-256 `d813620393b4b4905aaedc4eaff8a5f2aad29a2a356b54cb2f06ad9810055de3`) is the
same file the open engine's test suite cut (its `clock_state_ext_igs_fresh_oracle` fixture):
International GNSS Service (IGS) final combined satellite clocks at 30 s,
`IGS0OPSFIN_20252440000_01D_30S_CLK.CLK.gz` to `..._20252570000_...` (2025-09-01 to 2025-09-14),
from the BKG mirror of the IGS products (https://igs.bkg.bund.de/root_ftp/IGS/products/), GPS
Block IIF satellites G03 G06 G08 G09 G10 G24 G25 G26 G27 G30 G32, decimated to 300 s.

Layout: a `@<prn> ... first_bias_s=<s> ...` header per clock, then `<t / 300 s> <increment>`
lines, the increment of (bias - first_bias) since the previous line in integer units of 1e-13 s.
The other header values are noise parameters fitted by the open engine's tuning script; the V5
predictor does not use them.

Terms: IGS data and products are openly available for use without restriction, with attribution
to the IGS and its contributing organisations. Acknowledgement: International GNSS Service;
Johnston, Riddell and Hausler, "The International GNSS Service", Springer Handbook of Global
Navigation Satellite Systems (2017), 967-982.

These clocks are free-running satellite clocks. No GNSS removal took place: V5 emulates holdover
by freezing the estimated state at the handover time.
