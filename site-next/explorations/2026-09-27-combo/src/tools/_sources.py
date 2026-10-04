"""The one engine checkout every recorder in src/tools runs and reads (src/data/sources.json: repo).

KSHANA_ENGINE_TREE overrides it, then KSHANA_REPO. The checkout is only read: every recorder copies
the scenario it runs into a scratch directory first, because the engine writes its outputs next to
the scenario file.
"""
import json, os

_HERE = os.path.dirname(os.path.abspath(__file__))


def engine_tree():
    cfg = json.load(open(os.path.join(os.path.dirname(_HERE), "data", "sources.json"), encoding="utf-8"))
    return os.path.expanduser(os.environ.get("KSHANA_ENGINE_TREE") or os.environ.get("KSHANA_REPO") or cfg["repo"])


def studio_dir():
    """The Kshana Studio folder the site is built with (src/data/sources.json: studio, relative to the
    site folder; KSHANA_STUDIO overrides it). The same folder build.py copies into playground/."""
    site = os.path.dirname(os.path.dirname(_HERE))
    cfg = json.load(open(os.path.join(os.path.dirname(_HERE), "data", "sources.json"), encoding="utf-8"))
    return os.path.normpath(os.path.join(site, os.path.expanduser(os.environ.get("KSHANA_STUDIO") or cfg.get("studio") or "../2026-09-30-studio-next")))
