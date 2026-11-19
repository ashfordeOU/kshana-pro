"""HTTP client for the ``kshana-pro serve`` eLoran routes (standard library only)."""

from __future__ import annotations

import json
import urllib.error
import urllib.parse
import urllib.request
from typing import Any, Dict, Optional


class ServiceError(RuntimeError):
    def __init__(self, status: int, body: str):
        self.status = status
        self.body = body
        super().__init__(f"HTTP {status}: {body[:500]}")


class ServiceClient:
    """``base_url`` such as ``http://127.0.0.1:8080``; ``token`` an API bearer token."""

    def __init__(self, base_url: str, token: Optional[str] = None, timeout: float = 120.0):
        self.base = base_url.rstrip("/")
        self.token = token
        self.timeout = timeout

    def _req(self, method: str, path: str, body: Optional[bytes] = None, ctype: Optional[str] = None,
             query: Optional[Dict[str, str]] = None):
        url = self.base + path
        if query:
            url += "?" + urllib.parse.urlencode({k: v for k, v in query.items() if v is not None})
        req = urllib.request.Request(url, data=body, method=method)
        if ctype:
            req.add_header("Content-Type", ctype)
        if self.token:
            req.add_header("Authorization", f"Bearer {self.token}")
        try:
            with urllib.request.urlopen(req, timeout=self.timeout) as r:
                return r.status, r.read().decode("utf-8")
        except urllib.error.HTTPError as e:
            raise ServiceError(e.code, e.read().decode("utf-8", "replace")) from None

    def example(self) -> str:
        return self._req("GET", "/eloran/example")[1]

    def validate(self, scenario_toml: str) -> Dict[str, Any]:
        return json.loads(self._req("POST", "/eloran/validate", scenario_toml.encode(), "text/plain")[1])

    def run(self, scenario_toml: str, format: Optional[str] = None) -> Dict[str, Any]:
        """A synchronous run (200) or a queued job (202): the JSON either way."""
        return json.loads(self._req("POST", "/eloran/run", scenario_toml.encode(), "text/plain",
                                    {"format": format} if format else None)[1])

    def rx(self, run_id: str, scenario_toml: Optional[str] = None, per_loop: bool = False,
           cri_blanking: bool = True, truth: bool = True) -> Dict[str, Any]:
        body: Dict[str, Any] = {"run": run_id, "per_loop": per_loop, "cri_blanking": cri_blanking, "truth": truth}
        if scenario_toml:
            body["scenario"] = scenario_toml
        return json.loads(self._req("POST", "/eloran/rx", json.dumps(body).encode(), "application/json")[1])

    def monitor(self, scenario_toml: str, html: bool = False):
        """Live-monitor result (dict) or, with ``html=True``, the dashboard page (str)."""
        st, body = self._req("POST", "/eloran/monitor", scenario_toml.encode(), "text/plain",
                             {"format": "html" if html else "json"})
        return body if html else json.loads(body)

    def gri(self, region: Optional[str] = None, active_on: Optional[str] = None,
            search: Optional[str] = None) -> Dict[str, Any]:
        return json.loads(self._req("GET", "/eloran/gri", query={"region": region, "active_on": active_on,
                                                                 "search": search})[1])

    def gri_chain(self, chain: str) -> Dict[str, Any]:
        return json.loads(self._req("GET", "/eloran/gri/" + urllib.parse.quote(chain, safe=""))[1])
