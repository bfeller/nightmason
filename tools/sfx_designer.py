#!/usr/bin/env python3
"""Sound designer for Nightmason's synth: serves tools/sfx_designer.html on localhost.

    python3 tools/sfx_designer.py [--port 8765] [--no-browser]

The page starts from the sounds in src/content.rs (`pub mod sfx`) and saves your work to
tools/sfx_designs.json. Turning those designs into content.rs entries is a separate step
(the page's "Copy Rust" gives the exact text). Stdlib only, listens on 127.0.0.1 only.
"""
import argparse
import json
import os
import re
import webbrowser
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

TOOLS = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(TOOLS)
CONTENT = os.path.join(ROOT, "src", "content.rs")
PAGE = os.path.join(TOOLS, "sfx_designer.html")
DESIGNS = os.path.join(TOOLS, "sfx_designs.json")


def parse_value(text):
    text = text.strip()
    if text.startswith("("):
        return [float(x) for x in text[1:-1].split(",")]
    try:
        return float(text)
    except ValueError:
        return text  # a wave name


def game_sounds():
    """The `sfx` module of content.rs as [{name, doc, voices: [{field: value}]}]."""
    src = open(CONTENT).read()
    m = re.search(r"pub mod sfx \{\n(.*?)\n\}", src, re.S)
    if not m:
        return []
    sounds = []
    pat = r"((?:[ \t]*///[^\n]*\n)*)[ \t]*pub const (\w+): Sfx = &\[(.*?)\];"
    for c in re.finditer(pat, m.group(1), re.S):
        doc = " ".join(l.strip()[3:].strip() for l in c.group(1).strip().splitlines())
        voices = []
        for v in re.finditer(r"Voice \{(.*?)\.\.VOICE \}", c.group(3), re.S):
            fields = re.findall(r"(\w+): (\([^)]*\)|[^,(]+),", v.group(1))
            voices.append({k: parse_value(val) for k, val in fields})
        sounds.append({"name": c.group(2), "doc": doc, "voices": voices})
    return sounds


class Handler(BaseHTTPRequestHandler):
    def send(self, code, body, ctype="application/json"):
        data = body if isinstance(body, bytes) else body.encode()
        self.send_response(code)
        self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(len(data)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        if self.path in ("/", "/index.html"):
            self.send(200, open(PAGE, "rb").read(), "text/html; charset=utf-8")
        elif self.path == "/api/game":
            self.send(200, json.dumps(game_sounds()))
        elif self.path == "/api/designs":
            if os.path.exists(DESIGNS):
                self.send(200, open(DESIGNS, "rb").read())
            else:
                self.send(404, "null")
        else:
            self.send(404, "null")

    def do_POST(self):
        if self.path != "/api/designs":
            return self.send(404, "null")
        n = int(self.headers.get("Content-Length", 0))
        if n > 2_000_000:
            return self.send(413, '{"error": "too big"}')
        try:
            designs = json.loads(self.rfile.read(n))
            assert isinstance(designs, list)
        except (ValueError, AssertionError):
            return self.send(400, '{"error": "expected a JSON list of sounds"}')
        with open(DESIGNS, "w") as f:
            json.dump(designs, f, indent=1)
            f.write("\n")
        self.send(200, json.dumps({"saved": os.path.relpath(DESIGNS, ROOT), "sounds": len(designs)}))

    def log_message(self, fmt, *args):
        pass


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--port", type=int, default=8765)
    ap.add_argument("--no-browser", action="store_true")
    a = ap.parse_args()
    srv = ThreadingHTTPServer(("127.0.0.1", a.port), Handler)
    url = f"http://127.0.0.1:{a.port}/"
    print(f"Sound designer at {url}  (Ctrl+C to stop; designs save to tools/sfx_designs.json)")
    if not a.no_browser:
        webbrowser.open(url)
    try:
        srv.serve_forever()
    except KeyboardInterrupt:
        pass


if __name__ == "__main__":
    main()
