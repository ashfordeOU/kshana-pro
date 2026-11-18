"""Scenario construction: a small TOML writer and a fluent builder."""

from __future__ import annotations

import json
from typing import Any, Dict, List, Optional


def _scalar(v: Any) -> str:
    if isinstance(v, bool):
        return "true" if v else "false"
    if isinstance(v, int):
        return str(v)
    if isinstance(v, float):
        if v != v or v in (float("inf"), float("-inf")):
            raise ValueError("TOML cannot hold NaN/inf here")
        s = repr(v)
        return s if ("." in s or "e" in s or "E" in s) else s + ".0"
    if isinstance(v, str):
        return json.dumps(v)  # basic string with escapes
    if isinstance(v, dict):
        return "{ " + ", ".join(f"{_key(k)} = {_scalar(x)}" for k, x in v.items()) + " }"
    if isinstance(v, (list, tuple)):
        return "[" + ", ".join(_scalar(x) for x in v) + "]"
    raise TypeError(f"unsupported TOML value {v!r}")


def _key(k: str) -> str:
    return k if k.replace("_", "").replace("-", "").isalnum() else json.dumps(k)


def _is_table_array(v: Any) -> bool:
    return isinstance(v, list) and len(v) > 0 and all(isinstance(x, dict) for x in v)


def dumps_toml(doc: Dict[str, Any]) -> str:
    """Serialise nested dicts/lists to TOML (tables, arrays of tables, inline values)."""
    lines: List[str] = []

    def emit(table: Dict[str, Any], prefix: str) -> None:
        simple = {k: v for k, v in table.items() if not isinstance(v, dict) and not _is_table_array(v)}
        for k, v in simple.items():
            lines.append(f"{_key(k)} = {_scalar(v)}")
        for k, v in table.items():
            name = f"{prefix}.{_key(k)}" if prefix else _key(k)
            if isinstance(v, dict):
                lines.append("")
                lines.append(f"[{name}]")
                emit(v, name)
            elif _is_table_array(v):
                for item in v:
                    lines.append("")
                    lines.append(f"[[{name}]]")
                    emit(item, name)

    emit(doc, "")
    return "\n".join(lines).lstrip("\n") + "\n"


class ScenarioBuilder:
    """Build a scenario dictionary step by step, then :meth:`toml` or pass it to
    :class:`kshana_eloran.Eloran` methods directly."""

    def __init__(self, start_utc: str, duration_s: float, fs_hz: int = 1_000_000, seed: int = 1,
                 output_scale_uv_m: float = 1e5):
        self.doc: Dict[str, Any] = {
            "scene": {"start_utc": start_utc, "duration_s": float(duration_s), "fs_hz": int(fs_hz),
                      "seed": int(seed), "output_scale_uv_m": float(output_scale_uv_m)},
            "receiver": {"type": "static", "lat_deg": 0.0, "lon_deg": 0.0},
            "outputs": [{"type": "e_field"}],
            "chains": [],
        }

    def static_receiver(self, lat_deg: float, lon_deg: float, h_m: float = 0.0, heading_deg: float = 0.0):
        self.doc["receiver"] = {"type": "static", "lat_deg": lat_deg, "lon_deg": lon_deg, "h_m": h_m,
                                "heading_deg": heading_deg}
        return self

    def waypoints(self, points: List[Dict[str, float]]):
        self.doc["receiver"] = {"type": "waypoints", "points": points}
        return self

    def crossed_loops(self):
        self.doc["outputs"] = [{"type": "h_loop", "axis_deg": 0.0}, {"type": "h_loop", "axis_deg": 90.0}]
        return self

    def time_ref(self, id: str, offset_ns: float = 0.0, drift_ns_per_day: float = 0.0, **extra):
        self.doc.setdefault("time_refs", []).append({"id": id, "offset_ns": offset_ns,
                                                     "drift_ns_per_day": drift_ns_per_day, **extra})
        return self

    def ground(self, model: str = "simple", **params):
        self.doc.setdefault("propagation", {})["ground"] = {"model": model, **params}
        return self

    def sky(self, model: str, **params):
        self.doc.setdefault("propagation", {})["sky"] = {"model": model, **params}
        return self

    def noise(self, rms_uv_m_20khz: float, seed: int = 0):
        self.doc.setdefault("interference", []).append({"type": "wgn", "rms_uv_m_20khz": rms_uv_m_20khz, "seed": seed})
        return self

    def interference(self, **spec):
        self.doc.setdefault("interference", []).append(dict(spec))
        return self

    def chain(self, name: str, gri_us: int, stations: List[Dict[str, Any]], ldc: Optional[Dict[str, Any]] = None):
        c: Dict[str, Any] = {"name": name, "gri_us": int(gri_us), "stations": stations}
        if ldc:
            c["ldc"] = ldc
        self.doc["chains"].append(c)
        return self

    @staticmethod
    def station(id: str, role: str, lat_deg: float, lon_deg: float, emission_delay_us: float = 0.0,
                power_kw: float = 400.0, time_ref: str = "", **extra) -> Dict[str, Any]:
        s = {"id": id, "role": role, "position": {"lat_deg": lat_deg, "lon_deg": lon_deg},
             "power_kw": power_kw, "emission_delay_us": emission_delay_us}
        if time_ref:
            s["time_ref"] = time_ref
        s.update(extra)
        return s

    def toml(self) -> str:
        return dumps_toml(self.doc)
