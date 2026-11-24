#!/usr/bin/env python3
"""Drive the game with scripted key events through a FIFO and grab screenshots (testing aid).

drive.py <binary> <workdir> <script>   script: ';'-separated steps
  hold KEY N   hold KEY for N frames      tap KEY      press+release
  wait N       wait N frames              shot NAME    save <workdir>/NAME.png
Extra env (NM_SEED, NM_TICK...) passes through to the game. The world is saved in <workdir>/save
unless NM_SAVE is set, so running again with the same workdir continues it.
"""
import os, shutil, struct, subprocess, sys, time
KEYS = {"w": 17, "a": 30, "s": 31, "d": 32, "space": 57, "j": 36, "b": 48, "enter": 28, "esc": 1, "p": 25, "tab": 15, "q": 16, "1": 2, "2": 3, "3": 4, "4": 5, "5": 6, "6": 7, "7": 8, "8": 9, "9": 10, "n": 49, "e": 18}
binary, work, script = sys.argv[1:4]
fifo, fb = os.path.join(work, "keys.fifo"), os.path.join(work, "fb.raw")
if os.path.exists(fifo):
    os.unlink(fifo)
os.mkfifo(fifo)
env = dict(os.environ, NM_FB=fb, NM_INPUT=fifo)
env.setdefault("NM_SAVE", os.path.join(work, "save"))
game = subprocess.Popen([binary], env=env)
out = open(fifo, "wb", buffering=0)
ev = lambda code, val: out.write(struct.pack("<qqHHi", 0, 0, 1, code, val))
F = 1 / 30
time.sleep(0.2)
for step in script.split(";"):
    a = step.split()
    if not a:
        continue
    if a[0] == "hold":
        ev(KEYS[a[1]], 1); time.sleep(int(a[2]) * F); ev(KEYS[a[1]], 0)
    elif a[0] == "tap":
        ev(KEYS[a[1]], 1); time.sleep(2 * F); ev(KEYS[a[1]], 0); time.sleep(2 * F)
    elif a[0] == "wait":
        time.sleep(int(a[1]) * F)
    elif a[0] == "shot":
        time.sleep(2 * F)
        shutil.copy(fb, fb + ".snap")
        here = os.path.dirname(os.path.abspath(__file__))
        subprocess.run([sys.executable, "-I", os.path.join(here, "fbshot.py"), fb + ".snap", os.path.join(work, a[1] + ".png"), "2"])
game.terminate(); game.wait()
