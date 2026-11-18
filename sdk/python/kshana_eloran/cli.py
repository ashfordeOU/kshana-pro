"""Wrapper over the ``kshana-pro eloran`` command line."""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import tempfile
from pathlib import Path
from typing import Any, Dict, Iterable, List, Optional, Sequence, Union

PathLike = Union[str, os.PathLike]


class EloranError(RuntimeError):
    """A ``kshana-pro eloran`` command failed (the message is its error output)."""

    def __init__(self, args: Sequence[str], code: int, stderr: str):
        self.args_list = list(args)
        self.code = code
        self.stderr = stderr
        super().__init__(f"kshana-pro {' '.join(args)} failed ({code}): {stderr.strip()}")


def _p(p: PathLike) -> str:
    return os.fspath(p)


class Eloran:
    """Drive the eLoran simulator through the ``kshana-pro`` binary.

    ``binary``: path or name of ``kshana-pro`` (default: ``$KSHANA_PRO_BIN`` or the one on
    ``PATH``). ``license``: a licence token passed as ``--license``; ``dev=True`` passes
    ``--dev`` (development builds only). Every method raises :class:`EloranError` on failure.
    """

    def __init__(self, binary: Optional[str] = None, license: Optional[str] = None,
                 dev: bool = False, timeout: Optional[float] = None):
        b = binary or os.environ.get("KSHANA_PRO_BIN") or shutil.which("kshana-pro") or "kshana-pro"
        self.binary = b
        self.license = license
        self.dev = dev
        self.timeout = timeout

    # ------------------------------------------------------------------ plumbing
    def _auth(self) -> List[str]:
        if self.license:
            return ["--license", self.license]
        if self.dev:
            return ["--dev"]
        return []

    def call(self, *args: str, licensed: bool = True) -> str:
        """Run ``kshana-pro eloran <args>`` and return its standard output."""
        argv = ["eloran", *args] + (self._auth() if licensed else [])
        r = subprocess.run([self.binary, *argv], capture_output=True, text=True, timeout=self.timeout)
        if r.returncode != 0:
            raise EloranError(argv, r.returncode, r.stderr or r.stdout)
        return r.stdout

    @staticmethod
    def _json(path: PathLike) -> Any:
        with open(path, "r", encoding="utf-8") as f:
            return json.load(f)

    @staticmethod
    def _scenario_path(scenario: Union[PathLike, str, Dict[str, Any]], tmp: Path) -> str:
        """Accept a path, TOML text, or a dict (serialised with :func:`dumps_toml`)."""
        from .scenario import dumps_toml

        if isinstance(scenario, dict):
            p = tmp / "scenario.toml"
            p.write_text(dumps_toml(scenario), encoding="utf-8")
            return str(p)
        s = _p(scenario) if not isinstance(scenario, str) else scenario
        if "\n" in s or s.lstrip().startswith("["):
            p = tmp / "scenario.toml"
            p.write_text(s, encoding="utf-8")
            return str(p)
        return s

    # ------------------------------------------------------------------ scenarios
    def example(self) -> str:
        """The example scenario (TOML text)."""
        return self.call("example", licensed=False)

    def validate(self, scenario) -> str:
        """Validate a scenario; returns its SHA-256."""
        with tempfile.TemporaryDirectory() as d:
            out = self.call("validate", self._scenario_path(scenario, Path(d)), licensed=False)
        return out.strip().rsplit("sha256=", 1)[-1]

    def run(self, scenario, out: PathLike, format: str = "ri16") -> Dict[str, Any]:
        """Simulate to ``<out>.sigmf-*``, ``<out>.truth.csv``; returns the run record."""
        with tempfile.TemporaryDirectory() as d:
            self.call("run", self._scenario_path(scenario, Path(d)), "--out", _p(out), "--format", format)
        return self._json(f"{_p(out)}.run.json")

    def rx(self, scenario, recording: PathLike, out_dir: PathLike, truth: Optional[PathLike] = None,
           per_loop: bool = False, cri_blanking: bool = True) -> Dict[str, Any]:
        """Software receiver on a recording; returns the receiver report."""
        with tempfile.TemporaryDirectory() as d:
            args = ["rx", self._scenario_path(scenario, Path(d)), "--recording", _p(recording), "--out", _p(out_dir)]
            if truth:
                args += ["--truth", _p(truth)]
            if per_loop:
                args.append("--per-loop")
            if not cri_blanking:
                args.append("--no-cri-blanking")
            self.call(*args)
        return self._json(Path(out_dir) / "rx-report.json")

    def gnss(self, scenario, nav: PathLike, out: PathLike, fs_hz: Optional[float] = None,
             gps: bool = True, galileo: bool = True, cn0_dbhz: Optional[float] = None) -> str:
        """GPS L1 / Galileo E1 recording on the scenario's trajectory and time base."""
        with tempfile.TemporaryDirectory() as d:
            args = ["gnss", self._scenario_path(scenario, Path(d)), "--nav", _p(nav), "--out", _p(out)]
            if fs_hz:
                args += ["--fs", str(fs_hz)]
            if not gps:
                args.append("--no-gps")
            if not galileo:
                args.append("--no-galileo")
            if cn0_dbhz is not None:
                args += ["--cn0", str(cn0_dbhz)]
            return self.call(*args)

    def stream(self, scenario, device: str, pps_start: bool = False, external_ref: bool = False,
               external_pps: bool = False, gain_db: Optional[float] = None) -> str:
        """Real-time output to ``file:<base>`` or ``soapy:<args>`` (conducted use only)."""
        with tempfile.TemporaryDirectory() as d:
            args = ["stream", self._scenario_path(scenario, Path(d)), "--device", device]
            args += [f for f, on in (("--pps-start", pps_start), ("--external-ref", external_ref),
                                      ("--external-pps", external_pps)) if on]
            if gain_db is not None:
                args += ["--gain-db", str(gain_db)]
            return self.call(*args)

    def monitor(self, scenario, out_dir: PathLike, recording: Optional[PathLike] = None,
                interval_s: Optional[float] = None) -> Dict[str, Any]:
        """Live monitor: writes ``monitor.jsonl`` and ``monitor.html``; returns the summary."""
        with tempfile.TemporaryDirectory() as d:
            args = ["monitor", self._scenario_path(scenario, Path(d)), "--out", _p(out_dir)]
            if recording:
                args += ["--recording", _p(recording)]
            if interval_s:
                args += ["--interval", str(interval_s)]
            self.call(*args)
        from .io import read_monitor

        return read_monitor(Path(out_dir) / "monitor.jsonl")["summary"]

    # ------------------------------------------------------------------ database / references
    def gri_list(self, region: Optional[str] = None, active_on: Optional[str] = None,
                 search: Optional[str] = None) -> List[str]:
        args = ["gri", "list"]
        for k, v in (("--region", region), ("--active-on", active_on), ("--search", search)):
            if v:
                args += [k, v]
        return [l for l in self.call(*args, licensed=False).splitlines() if l.strip()]

    def gri_show(self, chain: str) -> List[Dict[str, Any]]:
        txt = self.call("gri", "show", chain, licensed=False)
        dec = json.JSONDecoder()
        out, i = [], 0
        while i < len(txt):
            while i < len(txt) and txt[i].isspace():
                i += 1
            if i >= len(txt):
                break
            obj, i = dec.raw_decode(txt, i)
            out.append(obj)
        return out

    def gri_snippet(self, chain: str) -> str:
        return self.call("gri", "snippet", chain, licensed=False)

    def circular_t(self, path: PathLike, labs: Iterable[str], start_utc: str, duration_s: float = 86400.0) -> str:
        """``[[time_refs]]`` TOML fitted to BIPM Circular T values."""
        return self.call("timeref", "circular-t", _p(path), "--labs", ",".join(labs), "--start", start_utc,
                         "--duration", str(duration_s), licensed=False)

    # ------------------------------------------------------------------ bench / calibration
    def calibrate_receiver(self, tic: PathLike, inputs: PathLike, out: PathLike, unit: str = "s",
                           column: Optional[int] = None) -> Dict[str, Any]:
        args = ["calibrate", "receiver", "--tic", _p(tic), "--inputs", _p(inputs), "--out", _p(out), "--unit", unit]
        if column is not None:
            args += ["--column", str(column)]
        self.call(*args)
        return self._json(out)

    def awg(self, scenario, out: PathLike, bits: Optional[int] = None, max_samples: Optional[int] = None) -> Dict[str, Any]:
        with tempfile.TemporaryDirectory() as d:
            args = ["awg", self._scenario_path(scenario, Path(d)), "--out", _p(out)]
            if bits:
                args += ["--bits", str(bits)]
            if max_samples:
                args += ["--max-samples", str(max_samples)]
            self.call(*args)
        return self._json(f"{_p(out)}.awg.json")

    def accept_software(self, out: PathLike) -> Dict[str, Any]:
        self.call("accept", "software", "--out", _p(out))
        return self._json(out)

    def sync_measure(self, tic: PathLike, out: PathLike, unit: str = "ns", column: Optional[int] = None,
                     target_ns: Optional[float] = None) -> Dict[str, Any]:
        args = ["sync", "measure", "--tic", _p(tic), "--out", _p(out), "--unit", unit]
        if column is not None:
            args += ["--column", str(column)]
        if target_ns is not None:
            args += ["--target-ns", str(target_ns)]
        self.call(*args)
        return self._json(out)

    # ------------------------------------------------------------------ propagation / validation
    def asfgrid(self, scenario, box: Sequence[float], res_deg: float, out: PathLike) -> Dict[str, str]:
        with tempfile.TemporaryDirectory() as d:
            self.call("asfgrid", self._scenario_path(scenario, Path(d)), "--box", ",".join(str(x) for x in box),
                      "--res", str(res_deg), "--out", _p(out))
        return {"csv": f"{_p(out)}.asf.csv", "geojson": f"{_p(out)}.asf.geojson"}

    def fidelity(self, recording: PathLike, meta: PathLike, scenario, out: PathLike) -> Dict[str, Any]:
        with tempfile.TemporaryDirectory() as d:
            self.call("fidelity", _p(recording), "--meta", _p(meta), "--scenario",
                      self._scenario_path(scenario, Path(d)), "--out", _p(out))
        return self._json(out)

    def import_log(self, log: PathLike, profile: PathLike, out: PathLike) -> Dict[str, Any]:
        self.call("import", _p(log), "--profile", _p(profile), "--out", _p(out))
        return self._json(out)

    def conformance_list(self) -> List[str]:
        return [l for l in self.call("conformance", "list", licensed=False).splitlines() if l.strip()]

    def campaign_run(self, plan: PathLike, out_dir: PathLike) -> Dict[str, Any]:
        self.call("campaign", "run", _p(plan), "--out", _p(out_dir))
        return self._json(Path(out_dir) / "campaign.json")

    def campaign_verify(self, out_dir: PathLike) -> str:
        return self.call("campaign", "verify", _p(out_dir), licensed=False).strip()

    # ------------------------------------------------------------------ system
    def plan(self, plan: PathLike, out: PathLike) -> Dict[str, Any]:
        self.call("plan", _p(plan), "--out", _p(out))
        return self._json(f"{_p(out)}.plan.json")

    def twin(self, config: PathLike, out: PathLike) -> Dict[str, Any]:
        self.call("twin", _p(config), "--out", _p(out))
        return self._json(out)

    # ------------------------------------------------------------------ bench readiness
    def bench_catalog(self, category: Optional[str] = None, lf_only: bool = False,
                      catalog: Optional[PathLike] = None) -> List[Dict[str, Any]]:
        """Hardware catalogue entries (datasheet values with sources). Free."""
        args = ["bench", "catalog", "--json"]
        if category:
            args += ["--category", category]
        if lf_only:
            args.append("--lf")
        if catalog:
            args += ["--catalog", _p(catalog)]
        return json.loads(self.call(*args, licensed=False))

    def bench_show(self, device_id: str) -> Dict[str, Any]:
        return json.loads(self.call("bench", "show", device_id, licensed=False))

    def bench_check(self, inventory: PathLike, catalog: Optional[PathLike] = None) -> Dict[str, Any]:
        """Readiness of a bench: findings per role, runnable acceptance tests."""
        args = ["bench", "check", "--inventory", _p(inventory), "--json"]
        if catalog:
            args += ["--catalog", _p(catalog)]
        return json.loads(self.call(*args))

    def bench_twin(self, device_id: str, out: PathLike, fs_hz: float = 2e6,
                   levels_dbfs: Sequence[float] = (0.0, -20.0, -40.0, -60.0)) -> Dict[str, Any]:
        """Predicted eLoran output of a catalogue device (spec twin, MODELLED)."""
        self.call("bench", "twin", device_id, "--fs", str(fs_hz),
                  "--levels", ",".join(str(x) for x in levels_dbfs), "--out", _p(out))
        return self._json(out)

    def bench_kit(self, inventory: PathLike, out_dir: PathLike, catalog: Optional[PathLike] = None) -> Dict[str, Any]:
        """Write the bench kit (assessment, wiring, procedure, profiles, twin predictions)."""
        args = ["bench", "kit", "--inventory", _p(inventory), "--out", _p(out_dir)]
        if catalog:
            args += ["--catalog", _p(catalog)]
        self.call(*args)
        return self._json(Path(out_dir) / "assessment.json")

    def bench_rehearse(self, inventory: PathLike, out: PathLike, catalog: Optional[PathLike] = None) -> Dict[str, Any]:
        """Acceptance suite with every device replaced by its spec twin."""
        args = ["bench", "rehearse", "--inventory", _p(inventory), "--out", _p(out)]
        if catalog:
            args += ["--catalog", _p(catalog)]
        self.call(*args)
        return self._json(out)

    def bench_probe(self, inventory: Optional[PathLike] = None, hosts: Sequence[str] = (),
                    out: Optional[PathLike] = None) -> str:
        """Find and identify connected instruments (read-only). Needs pro-eloran-hw."""
        args = ["bench", "probe"]
        if inventory:
            args += ["--inventory", _p(inventory)]
        for h in hosts:
            args += ["--host", h]
        if out:
            args += ["--out", _p(out)]
        return self.call(*args)

    def bench_accept(self, inventory: PathLike, profiles: PathLike, out: PathLike,
                     select: Optional[Sequence[str]] = None) -> Dict[str, Any]:
        """Acceptance suite on the real bench. Needs pro-eloran-hw."""
        args = ["bench", "accept", "--inventory", _p(inventory), "--profiles", _p(profiles), "--out", _p(out)]
        if select:
            args += ["--select", ",".join(select)]
        self.call(*args)
        return self._json(out)
