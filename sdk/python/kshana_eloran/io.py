"""Readers for simulator outputs: SigMF recordings, truth tables, monitor streams."""

from __future__ import annotations

import csv
import json
import os
import struct
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Dict, List, Union

try:  # optional
    import numpy as _np  # type: ignore
except Exception:  # pragma: no cover
    _np = None

PathLike = Union[str, os.PathLike]


@dataclass
class Recording:
    fs_hz: float
    channels: List[Any]
    meta: Dict[str, Any] = field(default_factory=dict)

    @property
    def duration_s(self) -> float:
        return (len(self.channels[0]) / self.fs_hz) if self.channels else 0.0


def read_sigmf(base: PathLike, max_samples: int = 0) -> Recording:
    """Read a real-sample SigMF recording (``ri16_le`` or ``rf32_le``, interleaved channels)
    written by the simulator; ``ri16`` is scaled to ±1.0. Returns NumPy arrays when NumPy is
    installed, lists otherwise."""
    base = Path(base)
    meta = json.loads(base.with_suffix(".sigmf-meta").read_text(encoding="utf-8")) \
        if not str(base).endswith(".sigmf-meta") else json.loads(base.read_text(encoding="utf-8"))
    g = meta["global"]
    dt = g["core:datatype"]
    n_ch = int(g.get("core:num_channels", 1))
    fs = float(g["core:sample_rate"])
    raw = base.with_suffix(".sigmf-data").read_bytes()
    if dt == "ri16_le":
        width, fmt, scale = 2, "h", 1.0 / 32767.0
    elif dt == "rf32_le":
        width, fmt, scale = 4, "f", 1.0
    else:
        raise ValueError(f"unsupported SigMF datatype {dt} (ri16_le or rf32_le)")
    frames = len(raw) // (width * n_ch)
    if max_samples:
        frames = min(frames, max_samples)
    if _np is not None:
        a = _np.frombuffer(raw[: frames * width * n_ch], dtype="<i2" if width == 2 else "<f4").astype(float) * scale
        chans = [a[c::n_ch].copy() for c in range(n_ch)]
    else:
        vals = struct.unpack(f"<{frames * n_ch}{fmt}", raw[: frames * width * n_ch])
        chans = [[v * scale for v in vals[c::n_ch]] for c in range(n_ch)]
    return Recording(fs, chans, meta)


_NUM = {"t_emit_s", "toa_s", "delay_ns", "geom_ns", "pf_ns", "sf_ns", "asf_ns", "ecd_us", "field_uv_m", "snr_db",
        "sky_delay_us", "sky_ratio_db", "time_ref_offset_ns", "emission_error_ns", "rx_lat_deg", "rx_lon_deg",
        "rx_heading_deg", "bearing_deg", "distance_km"}


def read_truth(path: PathLike) -> List[Dict[str, Any]]:
    """Read ``<base>.truth.csv`` into dictionaries with numeric fields converted."""
    out = []
    with open(path, newline="", encoding="utf-8") as f:
        for row in csv.DictReader(f):
            r: Dict[str, Any] = {}
            for k, v in row.items():
                if k == "group":
                    r[k] = int(v)
                elif k in _NUM:
                    r[k] = None if v == "" else float(v)
                else:
                    r[k] = v
            out.append(r)
    return out


def read_monitor(path: PathLike) -> Dict[str, Any]:
    """Read a ``monitor.jsonl`` stream: ``{"header", "snapshots", "summary"}`` (the summary is
    ``None`` while the run is still going)."""
    header, snaps, summary = None, [], None
    with open(path, encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            rec = json.loads(line)
            t = rec.get("type")
            if t == "header":
                header = rec["header"]
            elif t == "snapshot":
                snaps.append(rec["snapshot"])
            elif t == "summary":
                summary = rec["summary"]
    return {"header": header, "snapshots": snaps, "summary": summary}
