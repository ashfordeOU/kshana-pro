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


def read_recorded(folder, name):
    """The Studio recording `name` (no extension) in `folder`, as a dict: recorded/<name>.json.gz
    (gzip, as the Studio's tools_record.mjs writes it), else the plain .json of an older Studio."""
    import gzip
    import json
    gz = os.path.join(folder, name + ".json.gz")
    if os.path.exists(gz):
        with gzip.open(gz, "rt", encoding="utf-8") as f:
            return json.load(f)
    with open(os.path.join(folder, name + ".json"), encoding="utf-8") as f:
        return json.load(f)


def recorded_names(folder):
    """The names of the recordings in `folder` (recorded/<name>.json.gz or .json; not index.json)."""
    out = set()
    for p in os.listdir(folder):
        for ext in (".json.gz", ".json"):
            if p.endswith(ext) and p != "index.json":
                out.add(p[: -len(ext)])
                break
    return out

