#!/usr/bin/env python3
"""Serve a built site the way the host (kshana.dev) does: clean URLs resolve to their .html file.

  /evidence          -> evidence.html        /docs/changelog -> docs/changelog.html
  /docs/ and /       -> index.html           /evidence.html  -> evidence.html (old links keep working)
  anything missing   -> 404.html when the site has one, with status 404

Standard library only. Usage:
  python3 features/serve.py [site-dir] [port]      defaults: explorations/2026-09-27-combo, 8847
"""
import http.server
import os
import sys
from urllib.parse import urlsplit, unquote

ROOT = os.path.abspath(sys.argv[1] if len(sys.argv) > 1 else os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "explorations", "2026-09-27-combo"))
PORT = int(sys.argv[2]) if len(sys.argv) > 2 else 8847


class Handler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *a, **k):
        super().__init__(*a, directory=ROOT, **k)

    def send_head(self):
        sp = urlsplit(self.path)
        rel = unquote(sp.path).lstrip("/")
        full = os.path.join(ROOT, rel)
        if rel and not rel.endswith("/") and not os.path.exists(full) and os.path.isfile(full + ".html"):
            self.path = "/" + rel + ".html" + (("?" + sp.query) if sp.query else "")
        elif not os.path.exists(full) and os.path.isfile(os.path.join(ROOT, "404.html")):
            self.path = "/404.html"
            f = open(os.path.join(ROOT, "404.html"), "rb")
            self.send_response(404)
            self.send_header("Content-Type", "text/html; charset=utf-8")
            self.send_header("Content-Length", str(os.fstat(f.fileno()).st_size))
            self.end_headers()
            return f
        return super().send_head()

    def log_message(self, *a):
        pass


if __name__ == "__main__":
    http.server.ThreadingHTTPServer.allow_reuse_address = True
    print(f"serving {ROOT} on http://127.0.0.1:{PORT}/ (clean URLs resolve to .html)")
    http.server.ThreadingHTTPServer(("127.0.0.1", PORT), Handler).serve_forever()
