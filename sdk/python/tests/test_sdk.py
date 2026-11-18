"""SDK tests. Unit tests always run; integration tests run when KSHANA_PRO_BIN points at a
kshana-pro binary (development build: --dev entitlement)."""

import json
import os
import struct
import sys
import tempfile
import tomllib
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from kshana_eloran import (Eloran, EloranError, ScenarioBuilder, dumps_toml, read_monitor,  # noqa: E402
                           read_sigmf, read_truth)

BIN = os.environ.get("KSHANA_PRO_BIN")
ROOT = Path(__file__).resolve().parents[3]


def builder():
    b = (ScenarioBuilder("2026-11-18T00:00:00Z", 1.2, fs_hz=1_000_000, output_scale_uv_m=1e6)
         .static_receiver(55.0, 3.0)
         .time_ref("UTC(A)", offset_ns=50.0)
         .ground("simple"))
    st = ScenarioBuilder.station
    return b.chain("sdk", 79800, [
        st("M", "master", 56.5, 1.0, time_ref="UTC(A)"),
        st("X", "secondary", 53.5, 1.5, 13000.0, time_ref="UTC(A)"),
        st("Y", "secondary", 55.5, 6.0, 31000.0, time_ref="UTC(A)"),
        st("Z", "secondary", 54.0, 5.5, 52000.0, time_ref="UTC(A)"),
    ])


class Unit(unittest.TestCase):
    def test_toml_writer_round_trips(self):
        doc = builder().doc
        back = tomllib.loads(dumps_toml(doc))
        self.assertEqual(back["scene"]["fs_hz"], 1_000_000)
        self.assertEqual(len(back["chains"]), 1)
        self.assertEqual(len(back["chains"][0]["stations"]), 4)
        self.assertEqual(back["chains"][0]["stations"][1]["position"]["lat_deg"], 53.5)
        self.assertEqual(back["time_refs"][0]["id"], "UTC(A)")
        self.assertEqual(tomllib.loads(dumps_toml({"a": 1.0, "s": "x\"y"})), {"a": 1.0, "s": "x\"y"})

    def test_readers(self):
        with tempfile.TemporaryDirectory() as d:
            base = Path(d) / "r"
            meta = {"global": {"core:datatype": "ri16_le", "core:sample_rate": 1e6, "core:num_channels": 2}}
            (base.with_suffix(".sigmf-meta")).write_text(json.dumps(meta))
            (base.with_suffix(".sigmf-data")).write_bytes(struct.pack("<4h", 32767, -32767, 0, 16384))
            r = read_sigmf(base)
            self.assertEqual(len(r.channels), 2)
            self.assertAlmostEqual(float(r.channels[0][0]), 1.0)
            self.assertAlmostEqual(float(r.channels[1][0]), -1.0)
            self.assertAlmostEqual(float(r.channels[1][1]), 16384 / 32767)
            t = Path(d) / "t.csv"
            t.write_text("t_emit_s,chain,station,group,toa_s,sky_delay_us\n0.1,c,M,5,0.2,\n")
            rows = read_truth(t)
            self.assertEqual(rows[0]["group"], 5)
            self.assertIsNone(rows[0]["sky_delay_us"])
            m = Path(d) / "m.jsonl"
            m.write_text('{"type":"header","header":{"fs_hz":1}}\n{"type":"snapshot","snapshot":{"t_s":0.5}}\n')
            mon = read_monitor(m)
            self.assertEqual(len(mon["snapshots"]), 1)
            self.assertIsNone(mon["summary"])

    def test_missing_binary_raises(self):
        e = Eloran(binary=sys.executable, dev=True)
        with self.assertRaises(EloranError):
            e.call("definitely-not-a-command")


@unittest.skipUnless(BIN, "set KSHANA_PRO_BIN to run the integration tests")
class Integration(unittest.TestCase):
    def setUp(self):
        self.e = Eloran(binary=BIN, dev=True, timeout=600)
        self.tmp = tempfile.TemporaryDirectory()
        self.d = Path(self.tmp.name)

    def tearDown(self):
        self.tmp.cleanup()

    def test_simulate_receive_monitor(self):
        sc = builder().doc
        self.assertEqual(len(self.e.validate(sc)), 64)
        rec = self.e.run(sc, self.d / "s")
        self.assertGreater(rec["stats"]["truth_records"], 0)
        recording = read_sigmf(self.d / "s")
        self.assertEqual(recording.fs_hz, 1e6)
        truth = read_truth(self.d / "s.truth.csv")
        self.assertTrue(truth)
        rep = self.e.rx(sc, self.d / "s", self.d / "rx", truth=self.d / "s.truth.csv")
        errs = rep["truth_comparison"]["stations"]
        self.assertEqual(len(errs), 4)
        self.assertTrue(all(s["cycle_errors"] == 0 for s in errs))
        self.assertLess(rep["truth_comparison"]["position_error_m"], 50.0)
        summary = self.e.monitor(sc, self.d / "mon")
        self.assertGreaterEqual(summary["stations_tracked"], 4)
        self.assertTrue((self.d / "mon" / "monitor.html").exists())

    def test_database_conformance_and_tools(self):
        self.assertTrue(any("9960" in l for l in self.e.gri_list()))
        self.assertIn("[[chains]]", self.e.gri_snippet("9960"))
        self.assertTrue(self.e.gri_show("9960"))
        self.assertTrue(any(l.startswith("CONF-") for l in self.e.conformance_list()))
        acc = self.e.accept_software(self.d / "acc.json")
        self.assertEqual(len(acc["results"]), 5)
        tic = self.d / "tic.csv"
        tic.write_text("\n".join(str(3.0 + 0.1 * ((i * 7919) % 11 - 5) / 5) for i in range(300)))
        sync = self.e.sync_measure(tic, self.d / "sync.json")
        self.assertEqual(sync["n"], 300)

    def test_licence_is_required(self):
        e = Eloran(binary=BIN)  # no licence, no --dev
        with self.assertRaises(EloranError) as c:
            e.run(builder().doc, self.d / "x")
        self.assertIn("enterprise", str(c.exception).lower())


if __name__ == "__main__":
    unittest.main()


@unittest.skipUnless(BIN, "KSHANA_PRO_BIN not set")
class Bench(unittest.TestCase):
    def test_catalogue_check_and_rehearsal(self):
        e = Eloran(binary=BIN, dev=True)
        lf = {x["id"] for x in e.bench_catalog(lf_only=True)}
        self.assertIn("redpitaya-stemlab-125-14", lf)
        self.assertNotIn("ettus-usrp-b210", lf)
        inv = ROOT / "crates/kshana-eloran/scenarios/bench/redpitaya-lf-bench.toml"
        self.assertTrue(e.bench_check(inv)["ready"])
        with tempfile.TemporaryDirectory() as d:
            r = e.bench_rehearse(inv, Path(d) / "r.json")
            self.assertFalse([x for x in r["report"]["results"] if x["verdict"] in ("fail", "error")])
