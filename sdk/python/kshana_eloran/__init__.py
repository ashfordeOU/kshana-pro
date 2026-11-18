"""Python SDK for the Kshana Pro eLoran/GNSS simulator.

Three layers, no third-party dependencies (NumPy is used when installed):

* :class:`Eloran` drives the ``kshana-pro eloran`` command line (every capability:
  simulation, software receiver, GNSS bridge, hardware streaming, AWG compilation,
  acceptance, sync measurement, Circular T, ASF grids, fidelity, log import, conformance,
  campaigns, network planner, digital twin, live monitor).
* :class:`ServiceClient` talks to a ``kshana-pro serve`` instance over HTTP (the
  ``/eloran/*`` routes: example, validate, run, rx, GRI database).
* Readers and builders: :func:`read_sigmf`, :func:`read_truth`, :func:`read_monitor`,
  :class:`ScenarioBuilder` and :func:`dumps_toml`.

Proprietary. Licensing follows the Kshana Pro tier manifest: simulator features need an
Enterprise licence (``license=`` token or ``dev=True`` for development builds).
"""

from .cli import Eloran, EloranError
from .service import ServiceClient, ServiceError
from .io import read_sigmf, read_truth, read_monitor, Recording
from .scenario import ScenarioBuilder, dumps_toml

__all__ = [
    "Eloran",
    "EloranError",
    "ServiceClient",
    "ServiceError",
    "read_sigmf",
    "read_truth",
    "read_monitor",
    "Recording",
    "ScenarioBuilder",
    "dumps_toml",
]
__version__ = "0.1.0"
