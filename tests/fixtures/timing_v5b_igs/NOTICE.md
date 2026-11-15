# timing_v5b_igs fixture (V5b data set D-R2)

`clocks.txt` (SHA-256 `3da6d0a5a7facdc1e564fdeed5f5ada890a2f9cdab6a218fa79a83608a3230a4`) is the
same file the open engine's test suite cut (its `clock_state_igs_holdout_oracle` fixture):
International GNSS Service (IGS) final combined satellite clocks at 30 s,
`IGS0OPSFIN_20252290000_01D_30S_CLK.CLK.gz` to `..._20252420000_...` (2025-08-17 to 2025-08-30),
from the BKG mirror of the IGS products (https://igs.bkg.bund.de/root_ftp/IGS/products/), GPS
Block IIF satellites G03 G06 G08 G09 G10 G24 G25 G26 G27 G30 G32, decimated to 300 s. V5 did not
use it.

Layout: a `@<prn> ... first_bias_s=<s>` header per clock, then `<t / 300 s> <increment>` lines,
the increment of (bias - first_bias) since the previous line in integer units of 1e-13 s. The
header's noise values were fitted by the open engine's tuning script; the predictor does not use
them. G03 has one gap of 288 epochs.

Terms: IGS data and products are openly available for use without restriction, with attribution
to the IGS and its contributing organisations. Acknowledgement: International GNSS Service;
Johnston, Riddell and Hausler, "The International GNSS Service", Springer Handbook of Global
Navigation Satellite Systems (2017), 967-982.

Free-running satellite clocks: no GNSS removal took place; holdover is emulated.
