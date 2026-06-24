#!/usr/bin/env python3
"""Generate the Timing Protection Level paper figures from the REAL solved data.
All inputs are the JammerTest 2024 (Zenodo 15911589) derived products."""
import csv, math, json, statistics as st
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt

DATA="data"; FIG="figures"
plt.rcParams.update({"font.size":9,"axes.grid":True,"grid.alpha":0.3,
                     "figure.dpi":150,"savefig.bbox":"tight"})

def load_clock(fn):
    return [(float(r["t_min"]),float(r["clock_ns"]),int(r["n_sats"]),float(r["xsat_sigma_ns"]))
            for r in csv.DictReader(open(f"{DATA}/{fn}"))]

def holdover(rows,t_lo,t_hi):
    pre=[(t,c) for t,c,_,_ in rows if t_lo<=t<t_hi]
    n=len(pre); mt=sum(t for t,_ in pre)/n; mc=sum(c for _,c in pre)/n
    b1=sum((t-mt)*(c-mc) for t,c in pre)/sum((t-mt)**2 for t,_ in pre); b0=mc-b1*mt
    return lambda t:b0+b1*t, b1

def binned(rows,key,nb=60):
    b={}
    for r in rows: b.setdefault(int(r[0]*nb/ (rows[-1][0]) if False else int(r[0])),[]).append(r)
    return b

s211=load_clock("clock_2.1.1.csv")
hold,drift=holdover(s211,2,10)

# ---- Fig 1: the silent time pull vs the receiver's claimed accuracy ----------
pv=list(csv.DictReader(open(f"{DATA}/spoof_2.1.1.navpvt.csv")))
t0=int(pv[0]["itow_ms"])
pt=[((int(r["itow_ms"])-t0)/60000.0,float(r["tAcc_ns"])) for r in pv]
# served error per 15 s bin (median), trusted epochs only for the clean line
bins={}
for t,c,ns,xs in s211: bins.setdefault(round(t*4)/4,[]).append((c-hold(t)))
xt=sorted(bins); ye=[abs(st.median(bins[k])) for k in xt]
fig,ax=plt.subplots(figsize=(5.4,3.1))
ax.semilogy(xt,ye,color="#b00020",lw=1.4,label="solved served-time error")
ax.semilogy([t for t,_ in pt],[max(a,1) for _,a in pt],color="#1565c0",lw=1.2,
            label="receiver-reported accuracy (tAcc)")
ax.axhline(1000,color="gray",ls="--",lw=0.9)
ax.text(0.5,1250,"1 $\\mu$s timing budget",fontsize=7,color="gray")
ax.axhline(1.01e6,color="#cc6600",ls=":",lw=0.9)
ax.text(0.5,1.30e6,"$\\sim$1 ms capture",fontsize=7,color="#cc6600")
ax.set_xlabel("time (min)"); ax.set_ylabel("time error (ns, log)")
ax.set_title("Spoof 2.1.1: served-time error vs receiver-reported accuracy")
ax.legend(fontsize=6.8,loc="center left"); ax.set_ylim(8,3e6)
fig.savefig(f"{FIG}/fig1_silent_pull.pdf"); plt.close(fig)

# ---- Fig 2: cross-satellite clock-consistency monitor ------------------------
mea=load_clock("clock_meacon.csv")
fig,ax=plt.subplots(figsize=(5.4,2.9))
for rows,lab,col in [(s211,"spoofing 2.1.1","#b00020"),(mea,"meaconing 3.1.1","#2e7d32")]:
    b={}
    for t,c,ns,xs in rows: b.setdefault(round(t*2)/2,[]).append(xs)
    xs_=sorted(b); ax.semilogy(xs_,[st.median(b[k]) for k in xs_],lw=1.2,label=lab,color=col)
ax.axhline(22,color="gray",ls=":",lw=0.9); ax.text(0.5,12.5,"clean floor 22 ns",fontsize=7,color="gray")
ax.axhline(110,color="k",ls="--",lw=0.8); ax.text(0.5,140,"5$\\sigma$ alarm (110 ns)",fontsize=7)
ax.set_xlabel("time (min)"); ax.set_ylabel("cross-sat clock consistency (ns, log)")
ax.set_title("Model-free monitor: cross-satellite consistency under attack")
ax.legend(fontsize=7,loc="upper right"); fig.savefig(f"{FIG}/fig2_monitor.pdf"); plt.close(fig)

# ---- TPL closed form (mirrors kshana::tpl / security::min_detectable_offset_ns
#      and holdover::coast_phase_sigma EXACTLY, so the figures reproduce Table II) -
RMON=(22.1e-9)**2   # per-sample monitor phase variance, s^2 (measured 22.1 ns floor)
KSIG=5.0            # k-sigma alarm
TAU=1.0             # monitor window, s
MSAMP=1.0           # averaged samples
def qf(a1,rw,dr): return (a1*a1, 3*rw*rw, 20*dr*dr)
def coast_ns(q,t):
    qw,qr,qd=q; return math.sqrt(qw*t + qr*t**3/3 + qd*t**5/20)*1e9
def floor_ns(q):
    # k * monitor_sigma_s(q_wf,q_rw,r,tau,m) * 1e9  (security::min_detectable_offset_ns)
    qw,qr,_=q
    var=RMON/MSAMP + qw*TAU + qr*TAU**3/3.0
    return KSIG*math.sqrt(max(var,0.0))*1e9
def tpl(q,t): return floor_ns(q)+coast_ns(q,t)
FLOOR=floor_ns(qf(2.8e-9,4.4e-10,1e-11))  # ~111 ns for the calibrated TCXO

OSC=[("TCXO (this receiver, measured)",qf(2.8e-9,4.4e-10,1e-11),"#b00020"),
     ("OCXO (typical)",qf(1e-11,1e-12,1e-13),"#1565c0"),
     ("Rubidium (typical)",qf(3e-11,3e-13,1e-14),"#2e7d32")]
lat=[1,2,5,10,20,30,60,120,300]
# ---- Fig 3: TPL vs latency by oscillator class ------------------------------
fig,ax=plt.subplots(figsize=(5.4,3.1))
for name,q,col in OSC:
    ax.loglog(lat,[tpl(q,t) for t in lat],marker="o",ms=3,lw=1.3,label=name,color=col)
ax.axhline(1.01e6,color="k",ls="--",lw=1.0)
ax.text(300,1.25e6,"uncontrolled pull (1.01 ms)",fontsize=7,ha="right",va="bottom")
ax.axhline(51,color="gray",ls=":",lw=1.0)
ax.text(300,36,"receiver claimed 51 ns",fontsize=7,color="gray",ha="right",va="bottom")
ax.set_xlabel("detection latency (s, log)"); ax.set_ylabel("Timing Protection Level (ns, log)")
ax.set_title("Computed TPL versus detection latency, by oscillator class")
ax.legend(fontsize=7,loc="upper left"); ax.set_ylim(20,3e6)
fig.savefig(f"{FIG}/fig3_tpl_latency.pdf"); plt.close(fig)

# ---- Fig 4: floor-sensitivity band (real TCXO) ------------------------------
# Sweep the long-tau red-noise PSDs q_rw, q_drift by 10^{-/+1} (one decade in
# q-space), EXACTLY as kshana::tpl::tpl_band(decades=1). NB scaling the PSD by 10
# is scaling the Allan LEVEL by sqrt(10); a level decade would be two q-decades.
def scale_q(q,f):
    qw,qr,qd=q; return (qw, qr*f, qd*f)
q=qf(2.8e-9,4.4e-10,1e-11)
fig,ax=plt.subplots(figsize=(5.4,2.9))
nom=[tpl(q,t) for t in lat]
lo=[tpl(scale_q(q,0.1),t) for t in lat]
hi=[tpl(scale_q(q,10.0),t) for t in lat]
ax.fill_between(lat,lo,hi,alpha=0.2,color="#b00020",label="$\\pm$1 decade red-noise floor")
ax.loglog(lat,nom,color="#b00020",lw=1.5,label="nominal")
ax.set_xlabel("detection latency (s, log)"); ax.set_ylabel("TPL (ns, log)")
ax.set_title("TPL is floor-governed at long coast (reported as a band)")
ax.legend(fontsize=7,loc="upper left"); fig.savefig(f"{FIG}/fig4_band.pdf"); plt.close(fig)

# ---- Fig 5: measured Allan deviation of the real receiver clock --------------
adev=json.load(open(f"{DATA}/real_calibration.json"))["clean_clock_oadev"]
taus=sorted(int(k) for k in adev); ys=[adev[str(t)] for t in taus]
fig,ax=plt.subplots(figsize=(5.4,2.7))
ax.loglog(taus,ys,marker="s",ms=4,color="#b00020",label="measured (clean clock)")
ax.loglog(taus,[2.8e-9/math.sqrt(t) for t in taus],ls="--",color="gray",lw=1.0,label="white-FM $\\propto\\tau^{-1/2}$")
ax.set_xlabel("$\\tau$ (s, log)"); ax.set_ylabel("overlapping ADEV (log)")
ax.set_title("Measured receiver-clock Allan deviation (clean segment)")
ax.legend(fontsize=7); fig.savefig(f"{FIG}/fig5_adev.pdf"); plt.close(fig)
print("figures written:")
import os
for f in sorted(os.listdir(FIG)): print("  ",f)
# emit key numbers for the paper
print("drift ns/min=%.0f"%drift)
print("TPL real TCXO @1s=%.0f @60s=%.0f"%(tpl(OSC[0][1],1),tpl(OSC[0][1],60)))
print("TPL OCXO @300s=%.0f, Rb @300s=%.0f"%(tpl(OSC[1][1],300),tpl(OSC[2][1],300)))
