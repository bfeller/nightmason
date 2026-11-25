#!/usr/bin/env python3
"""Generate a sound effect with fal's cassetteai/sound-effects-generator (host-side only).

    python3 tools/gen_sfx.py NAME "PROMPT" [--duration SECONDS]

Writes assets/sfx/NAME.wav. Each run is one billable generation (about $0.01). FAL_KEY is read
from the environment or from the project's .env (git-ignored); it never goes into the game binary.
Stdlib only, uses fal's queue API: submit, poll status, fetch result, download the WAV.
"""
import argparse
import json
import os
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

ENDPOINT = "cassetteai/sound-effects-generator"
QUEUE = "https://queue.fal.run/" + ENDPOINT
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT_DIR = os.path.join(ROOT, "assets", "sfx")


def load_key():
    key = os.environ.get("FAL_KEY")
    if key:
        return key
    try:
        with open(os.path.join(ROOT, ".env")) as f:
            for line in f:
                name, sep, value = line.strip().partition("=")
                if sep and name.strip() == "FAL_KEY":
                    return value.strip().strip("'\"")
    except FileNotFoundError:
        pass
    sys.exit("FAL_KEY not set (environment or .env)")


def fal_url(url, hosts=("fal.run", "fal.ai", "fal.media")):
    """Only follow https URLs on fal's own domains, so the key is never sent elsewhere."""
    u = urllib.parse.urlparse(url)
    host = u.hostname or ""
    if u.scheme != "https" or not any(host == h or host.endswith("." + h) for h in hosts):
        sys.exit(f"refusing non-fal URL from API: {url}")
    return url


def call(url, key, body=None):
    data = None if body is None else json.dumps(body).encode()
    req = urllib.request.Request(url, data=data, method="POST" if data else "GET")
    req.add_header("Authorization", "Key " + key)
    if data:
        req.add_header("Content-Type", "application/json")
    try:
        with urllib.request.urlopen(req, timeout=60) as r:
            return json.load(r)
    except urllib.error.HTTPError as e:
        detail = e.read().decode(errors="replace")[:500]
        if e.code == 403:
            sys.exit(f"fal refused (403): {detail}\n"
                     "If it says 'Exhausted balance', add credits at "
                     "https://fal.ai/dashboard/usage-billing/credits")
        sys.exit(f"fal HTTP {e.code} on {url}: {detail}")


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("name", help="output file stem, e.g. zombie_groan")
    ap.add_argument("prompt", help='e.g. "short wooden thunk of an axe hitting a tree"')
    ap.add_argument("--duration", type=int, default=2, help="seconds, 1-30 (default 2)")
    a = ap.parse_args()
    if not 1 <= a.duration <= 30:
        ap.error("--duration must be 1-30")
    if not a.name.replace("_", "").replace("-", "").isalnum():
        ap.error("name must be letters, digits, _ or -")
    key = load_key()

    # Submit once. If anything about the submit is unclear we stop rather than resubmit,
    # since a second submit is a second billable job.
    sub = call(QUEUE, key, {"prompt": a.prompt, "duration": a.duration})
    rid = sub.get("request_id")
    status_url = fal_url(sub.get("status_url") or f"{QUEUE}/requests/{rid}/status")
    result_url = fal_url(sub.get("response_url") or f"{QUEUE}/requests/{rid}")
    print(f"request {rid} queued", file=sys.stderr)

    deadline = time.time() + 300
    while True:
        st = call(status_url, key).get("status")
        if st == "COMPLETED":
            break
        if time.time() > deadline:
            sys.exit(f"request {rid} still {st} after 5 min; fetch later: {result_url}")
        time.sleep(1.5)

    res = call(result_url, key)
    audio = (res.get("audio_file") or {}).get("url")
    if not audio:
        sys.exit(f"request {rid}: no audio_file in result: {json.dumps(res)[:500]}")
    os.makedirs(OUT_DIR, exist_ok=True)
    out = os.path.join(OUT_DIR, a.name + ".wav")
    with urllib.request.urlopen(fal_url(audio), timeout=60) as r, open(out, "wb") as f:
        f.write(r.read())
    print(f"request {rid}: wrote {os.path.relpath(out, ROOT)} ({os.path.getsize(out)} bytes)")


if __name__ == "__main__":
    main()
