#!/usr/bin/env python3
"""Writes the SYNTHETIC demo inputs of docs/TIMING-ASSURANCE.md "Timing networks (E19)".

Every file here is made up: no real appliance, link, station or log. Run from this folder:
    python3 generate.py
then write the two derived posture files with the CLI:
    kshana-pro timing posture firmware posture/firmware.img --sha256 "$(sha256sum posture/firmware.img | cut -d' ' -f1)" \
        --vendor Microchip --model "TimeProvider 4100" --version 2.5 --utc 2026-11-20T12:00:00Z --json posture/gm-2.fwcheck.json
    kshana-pro timing posture sbom station posture/station-firmware.toml --utc 2026-11-20T12:00:00Z --out posture/station-firmware.cdx.json
"""
import datetime
import math
import random

random.seed(19)
T0 = 1_793_000_000  # 2026-10-27T09:33:20Z


def write(name, text):
    with open(name, "w", newline="\n") as f:
        f.write(text)


# --- posture --------------------------------------------------------------------------
write("posture/inventory.toml", '''title = "SYNTHETIC exchange timing appliances"

[[appliances]]
id = "gm-1"
site = "LD4"
vendor = "Microchip"
model = "TimeProvider 4100"
firmware = "2.4"
management = "internal"
services = ["https", "ssh", "snmpv2c"]

[[appliances]]
id = "gm-2"
site = "FR2"
vendor = "Microchip"
model = "TimeProvider 4100"
firmware = "2.5"
management = "isolated"
firmware_check = "gm-2.fwcheck.json"

[[appliances]]
id = "ntp-edge"
site = "LD4"
vendor = "Example Vendor"
model = "NTP-1"
firmware = "1.0"
management = "internet"
upgrade_signing = "device-verifies"
services = ["https", "ntp", "ntp-mode6"]
sbom = "ntp-edge.cdx.json"
''')
write("posture/components.toml", '''# EXAMPLE component advisories (illustrative ids; not real advisories).
[[component_advisory]]
id = "EXAMPLE-OPENSSH-1"
component = "openssh"
fixed_in = "9.8p1"
hazard = "ILLUSTRATIVE: a bundled OpenSSH below the fixed version"
source = "example only"
verification = "example"

[[component_advisory]]
id = "EXAMPLE-LIBXML2-1"
component = "libxml2"
fixed_in = "2.12.7"
hazard = "ILLUSTRATIVE: a bundled libxml2 below the fixed version"
source = "example only"
verification = "example"
''')
write("posture/ntp-edge.cdx.json", '''{
  "bomFormat": "CycloneDX",
  "specVersion": "1.5",
  "version": 1,
  "metadata": {"component": {"type": "firmware", "bom-ref": "fw", "name": "NTP-1 firmware", "version": "1.0"}},
  "components": [
    {"type": "application", "bom-ref": "openssh", "name": "OpenSSH", "version": "9.6p1"},
    {"type": "library", "bom-ref": "curl", "name": "curl", "version": "8.10.0"},
    {"type": "library", "bom-ref": "libxml2", "name": "libxml2", "version": "2.11.4"}
  ],
  "dependencies": [{"ref": "fw", "dependsOn": ["openssh", "curl", "libxml2"]}]
}
''')
write("posture/firmware.img", "SYNTHETIC firmware image 2.5\n")
write("posture/station-firmware.toml", '''name = "SYNTHETIC station firmware"
version = "0.9.0"
supplier = "Example operator"

[[components]]
name = "linux"
version = "6.6.30"
type = "operating-system"
licenses = ["GPL-2.0-only"]

[[components]]
name = "chrony"
version = "4.6"
type = "application"
purl = "pkg:generic/chrony@4.6"
licenses = ["GPL-2.0-only"]
depends_on = ["linux"]

[[components]]
name = "linuxptp"
version = "4.4"
type = "application"
purl = "pkg:generic/linuxptp@4.4"
licenses = ["GPL-2.0-or-later"]
depends_on = ["linux"]
file = "firmware.img"
''')

# --- chain ----------------------------------------------------------------------------
write("chain/circular-t.csv", "mjd,utc_minus_utck_ns,u_ns\n61334,1.0,2.0\n61339,2.0,2.0\n61344,3.0,2.2\n")
d = dict(tx_m=300e-9, rx_m=250e-9, tx_s=310e-9, rx_s=260e-9)
fixed = sum(d.values())
alpha = 2.6e-4


def delay_ms(rt, a):
    return d["tx_m"] + d["rx_s"] + (1 + a) / (2 + a) * (rt - fixed)


cal_rt = 49e-6 + fixed
pps = delay_ms(cal_rt, alpha) - delay_ms(cal_rt, 0.0)
link_rt = 490e-6 + fixed
fd = "{ tx_m = 300e-9, rx_m = 250e-9, tx_s = 310e-9, rx_s = 260e-9 }"
write("chain/chain.toml", f'''title = "SYNTHETIC exchange time feed"
end_point = "PoP-1 PTP output"
date = "2026-10-28"
coverage_k = 2
limit_s = 100e-9

[reference]
lab = "UTC(k)"
circular_t = "circular-t.csv"

[[hops]]
name = "lab to PoP White Rabbit"
type = "wr"
round_trip_s = {link_rt!r}
u_round_trip_s = 5e-12
fixed_delays_s = {fd}
u_fixed_s = 2e-12
alpha_configured = 0.0
calibration = {{ id = "CAL-WR-01", date = "2026-07-01", valid_days = 365, by = "example laboratory" }}
[hops.alpha_calibration]
round_trip_s = {cal_rt!r}
u_round_trip_s = 5e-12
fixed_delays_s = {fd}
u_fixed_s = 2e-12
pps_offset_s = {pps!r}
u_pps_offset_s = 20e-12

[[hops]]
name = "PoP boundary clock"
type = "ptp"
te_class = "class-b"
asymmetry_s = 2e-9
u_asymmetry_s = 1e-9
calibration = {{ id = "CAL-BC-01", date = "2026-09-01", valid_days = 365 }}

[onboarding]
format = "csv"
path = "onboarding.csv"
u_ref_s = 5e-9
''')
# Predicted end-point offset: 2.4 ns reference - half the WR asymmetry + 2 ns.
pred = 2.4e-9 - alpha * 490e-6 / (2 + alpha) / 2 + 2e-9
write("chain/onboarding.csv", "".join(
    f"{T0 + 10 * i},{pred + random.gauss(0, 1e-9):.6e}\n" for i in range(360)))

# --- crosscheck -----------------------------------------------------------------------
lat, lon = 48.8566, 2.3522
r = 6_371_000.0
p1, p2 = math.radians(50.0157), math.radians(lat)
a = math.sin((p2 - p1) / 2) ** 2 + math.cos(p1) * math.cos(p2) * math.sin(math.radians(lon - 9.0109) / 2) ** 2
dcf_delay = 2 * r * math.asin(math.sqrt(a)) / 299_792_458.0
n = 720
series = {
    "gnss": lambda i: (20e-3 if 300 <= i < 420 else 0.0) + random.gauss(0, 5e-8),
    "dcf77": lambda i: dcf_delay + random.gauss(0, 3e-4),
    "nts-a": lambda i: random.gauss(0, 1e-3),
    "eloran": lambda i: random.gauss(0, 5e-8),
}
for name, f in series.items():
    rows = [f"{T0 + 10 * i},{f(i):.6e}\n" for i in range(n) if not (name == "eloran" and i >= 600)]
    write(f"crosscheck/{name}.csv", "".join(rows))
write("crosscheck/pop.toml", f'''node = "pop-1"
profile = "wisp"
bin_s = 60
k = 3
node_limit_s = 0.010
stale_s = 30
[location]
lat_deg = {lat}
lon_deg = {lon}

[[sources]]
name = "gnss"
class = "gnss"
format = "csv"
path = "gnss.csv"
u_s = 1e-6

[[sources]]
name = "dcf77"
class = "radio-time"
transmitter = "dcf77"
format = "csv"
path = "dcf77.csv"
u_s = 1e-3

[[sources]]
name = "nts-a"
class = "nts"
root = "operator-a"
format = "csv"
path = "nts-a.csv"
u_s = 3e-3

[[sources]]
name = "eloran"
class = "eloran"
format = "csv"
path = "eloran.csv"
u_s = 1e-6
''')

# --- station --------------------------------------------------------------------------
day = int(datetime.datetime(2026, 10, 1, tzinfo=datetime.timezone.utc).timestamp())
rows = ["pass_id,satellite,antenna,provider,customer,scheduled_aos,scheduled_los,actual_aos,actual_los,bytes_planned,bytes_delivered,frames_bad,frames_total,status"]
iso = lambda t: datetime.datetime.fromtimestamp(t, datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
for k in range(60):
    aos = day + k * 12 * 3600 + 1800
    los = aos + 600
    prov = "broker-x" if k % 3 == 0 else "direct"
    cust = f"cust-{k % 4 + 1}"
    sat = f"SAT-{'ABC'[k % 3]}"
    ant = f"ant-{k % 2 + 1}"
    if k == 7:
        rows.append(f"p{k},{sat},{ant},{prov},{cust},{iso(aos)},{iso(los)},,,1000000,0,,,failed")
    elif k == 19:
        rows.append(f"p{k},{sat},{ant},{prov},{cust},{iso(aos)},{iso(los)},,,,,,,cancelled-customer")
    else:
        late = 95 if k == 33 else 3
        data = 950000 if k == 41 else 1000000
        rows.append(f"p{k},{sat},{ant},{prov},{cust},{iso(aos)},{iso(los)},{iso(aos + late)},{iso(los)},1000000,{data},0,20000,ok")
write("station/passes.csv", "\n".join(rows) + "\n")
clock = []
for m in range(0, 31 * 24 * 60, 5):
    t = day + 60 * m
    off = random.gauss(0, 2e-6)
    if 50 * 12 * 3600 <= t - day < 50 * 12 * 3600 + 7200:
        off = 0.4  # a stepped station clock for two hours around pass p50
    clock.append(f"{t},{off:.6e}")
write("station/clock.csv", "\n".join(clock) + "\n")
write("station/station.toml", '''station = "GS-1"
operator = "Example teleport (SYNTHETIC)"
passes = "passes.csv"
from = "2026-10-01"
to = "2026-11-01"

[clock]
format = "csv"
path = "clock.csv"

[antenna]
beamwidth_deg = 2.0
pointing_fraction = 0.1
altitude_km = 500

[sla]
min_contact_fraction = 0.95
max_aos_delay_s = 30
min_data_fraction = 0.99
max_frame_error_rate = 1e-4
target_success_rate = 0.95
''')
