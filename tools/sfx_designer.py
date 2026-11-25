#!/usr/bin/env python3
"""Sound and music designer for Nightmason's synth, served on localhost.

    python3 tools/sfx_designer.py [--port 8765] [--no-browser] [--music]

/      sound effects (tools/sfx_designer.html): starts from src/content.rs (`pub mod sfx`), saves
       to tools/sfx_designs.json. Turning designs into content.rs entries is a separate step.
/music the sequencer (tools/music_designer.html): instruments and songs, saved to
       tools/music_designs.json.
Both pages share tools/synth.js, a copy of the game's synth. Stdlib only, 127.0.0.1 only.
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
PAGES = {
    "/": ("sfx_designer.html", "text/html; charset=utf-8"),
    "/index.html": ("sfx_designer.html", "text/html; charset=utf-8"),
    "/music": ("music_designer.html", "text/html; charset=utf-8"),
    "/synth.js": ("synth.js", "text/javascript; charset=utf-8"),
}
# Saved work: API path -> (file, JSON type it must be).
STORES = {"/api/designs": ("sfx_designs.json", list), "/api/music": ("music_designs.json", dict)}


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
        if self.path in PAGES:
            name, ctype = PAGES[self.path]
            self.send(200, open(os.path.join(TOOLS, name), "rb").read(), ctype)
        elif self.path == "/api/game":
            self.send(200, json.dumps(game_sounds()))
        elif self.path in STORES:
            path = os.path.join(TOOLS, STORES[self.path][0])
            if os.path.exists(path):
                self.send(200, open(path, "rb").read())
            else:
                self.send(404, "null")
        else:
            self.send(404, "null")

    def do_POST(self):
        if self.path not in STORES:
            return self.send(404, "null")
        name, kind = STORES[self.path]
        n = int(self.headers.get("Content-Length", 0))
        if n > 4_000_000:
            return self.send(413, '{"error": "too big"}')
        try:
            data = json.loads(self.rfile.read(n))
            assert isinstance(data, kind)
        except (ValueError, AssertionError):
            return self.send(400, json.dumps({"error": f"expected a JSON {kind.__name__}"}))
        path = os.path.join(TOOLS, name)
        with open(path, "w") as f:
            json.dump(data, f, indent=1)
            f.write("\n")
        self.send(200, json.dumps({"saved": os.path.relpath(path, ROOT), "items": len(data)}))

    def log_message(self, fmt, *args):
        pass


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--port", type=int, default=8765)
    ap.add_argument("--no-browser", action="store_true")
    ap.add_argument("--music", action="store_true", help="open the music page")
    a = ap.parse_args()
    srv = ThreadingHTTPServer(("127.0.0.1", a.port), Handler)
    url = f"http://127.0.0.1:{a.port}/" + ("music" if a.music else "")
    print(f"Sound designer at http://127.0.0.1:{a.port}/, music at http://127.0.0.1:{a.port}/music  (Ctrl+C to stop)")
    if not a.no_browser:
        webbrowser.open(url)
    try:
        srv.serve_forever()
    except KeyboardInterrupt:
        pass


if __name__ == "__main__":
    main()
