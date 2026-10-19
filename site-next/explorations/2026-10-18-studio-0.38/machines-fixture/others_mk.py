# Writes the invented fixtures for the 0.38 machine-input checks: a stationary IMU log (imu-still.csv, 100 Hz, white noise,
# for the IMU fit card), an RTKLIB .pos track (track.pos) and an MQTT input stream (stream.jsonl). Run: python3 others_mk.py OUTDIR
import random, sys, os
out = sys.argv[1]; os.makedirs(out, exist_ok=True); random.seed(5)
with open(os.path.join(out, "imu-still.csv"), "w") as f:
    f.write("t_s,gx,gy,gz,ax,ay,az\n")
    for i in range(6000):
        f.write(f"{i*0.01:.2f},0,0,{1e-3*(random.random()-.5):e},{1e-2*(random.random()-.5):e},0,9.80665\n")
with open(os.path.join(out, "track.pos"), "w") as f:
    f.write("% program   : RTKLIB (invented fixture)\n% GPST          latitude(deg) longitude(deg)  height(m)   Q  ns\n")
    for s in range(120):
        f.write(f"2025/09/15 12:{s//60:02d}:{s%60:02d}.000   59.911500000   10.750000000    30.000   1  10\n")
with open(os.path.join(out, "stream.jsonl"), "w") as f:
    f.write('{"t_s":0,"state":"nominal","score":97}\n{"t_s":1,"state":"untrusted","score":20,"alarms":["cn0-drop"]}\n')
