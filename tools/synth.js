// Shared by tools/sfx_designer.html and tools/music_designer.html: a JS copy of the game's synth
// (src/audio.rs) and the voice-editing cards. Keep the synth in step with audio.rs: the sound
// designer's output matches the game's sample for sample, 32-bit float rounding included.
"use strict";
const Synth = (() => {
  const RATE = 24000, MASTER = 0.8, TAU = Math.PI * 2;
  const WAVES = ["Square", "Saw", "Tri", "Sine", "Noise"];
  /// audio.rs VOICE.
  const DEFAULT = {
    wave: "Square", freq: 440, slide: 0, jump: [0, 1], duty: 0.5, vib: [0, 0], delay: 0,
    attack: 0, hold: 0, decay: 0.1, lp: 0, lp_slide: 0, hp: 0, vol: 0.5,
  };
  const FIELDS = Object.keys(DEFAULT);
  /// Instrument voices add two switches. fixed: ignore the note's pitch (drums); otherwise
  /// `freq` is the pitch when the note is A4. gate: hold for as long as the note lasts, then
  /// `decay` is the release; otherwise a one-shot.
  const INST_DEFAULT = { ...DEFAULT, fixed: false, gate: true };
  const INST_FIELDS = Object.keys(INST_DEFAULT);
  const clone = x => JSON.parse(JSON.stringify(x));
  const full = v => clone({ ...DEFAULT, ...v });
  const fullInst = v => clone({ ...INST_DEFAULT, ...v });
  const voiceLen = v => v.delay + v.attack + v.hold + v.decay;
  const nextSeed = seed => (Math.imul(seed, 1664525) + 1013904223) >>> 0;
  const F = Math.fround;

  /// Add one voice into `out` from sample `at`: audio.rs Playing::next. `ratio` scales the
  /// pitch (music notes), `extra` lengthens the hold (gated notes), in seconds.
  function renderVoice(out, at, v, gain, seed, ratio = 1, extra = 0) {
    const dt = 1 / RATE, dt32 = F(dt);
    let rng = (seed | 1) >>> 0, phase = 0, noise = 0, lp = 0, hp = 0, hpIn = 0;
    const freq = F(F(v.freq) * F(ratio)), slide = F(v.slide), j0 = F(v.jump[0]), j1 = F(v.jump[1]);
    const delay = F(v.delay), vr = F(v.vib[0]), vd = F(v.vib[1]), hold = v.hold + extra;
    for (let n = 0; at + n < out.length; n++) {
      const t = F(F(F(n) * dt32) - delay);
      if (t < 0) continue;
      let env;
      if (t < v.attack) env = t / v.attack;
      else if (t < v.attack + hold) env = 1;
      else {
        const d = (t - v.attack - hold) / Math.max(v.decay, dt);
        if (d >= 1) break;
        env = (1 - d) * (1 - d);
      }
      let f = F(freq * F(Math.pow(2, F(slide * t))));
      if (t >= j0) f = F(f * j1);
      if (vd !== 0) f = F(f * F(1 + F(vd * F(Math.sin(F(F(F(TAU) * vr) * t))))));
      phase = F(phase + F(Math.min(Math.max(f, 1), RATE / 2) * dt32));
      if (phase >= 1) {
        phase = F(phase - Math.floor(phase));
        rng = (rng ^ (rng << 13)) >>> 0;
        rng = (rng ^ (rng >>> 17)) >>> 0;
        rng = (rng ^ (rng << 5)) >>> 0;
        noise = (rng >>> 8) / (1 << 23) - 1;
      }
      const p = phase;
      let x;
      switch (v.wave) {
        case "Square": x = p < v.duty ? 0.6 : -0.6; break;
        case "Saw": x = 1.2 * p - 0.6; break;
        case "Tri": x = p < 0.5 ? 4 * p - 1 : 3 - 4 * p; break;
        case "Sine": x = Math.sin(TAU * p); break;
        default: x = noise;
      }
      if (v.lp > 0) {
        const c = Math.min(Math.max(v.lp * Math.pow(2, v.lp_slide * t), 20), RATE * 0.45);
        lp += (1 - Math.exp(-TAU * c * dt)) * (x - lp);
        x = lp;
      }
      if (v.hp > 0) {
        const rc = 1 / (TAU * v.hp);
        hp = rc / (rc + dt) * (hp + x - hpIn);
        hpIn = x;
        x = hp;
      }
      out[at + n] += x * env * v.vol * gain;
    }
  }

  /// audio.rs Mixer::render's output stage.
  function finish(out) {
    for (let i = 0; i < out.length; i++) out[i] = Math.tanh(out[i] * MASTER) * (32000 / 32768);
    return out;
  }

  /// A sound effect: all its voices at once (muted ones skipped but still take their seed,
  /// as in Mixer::start).
  function render(voices, gain) {
    const len = Math.max(0.01, ...voices.map(voiceLen));
    const out = new Float32Array(Math.ceil(len * RATE) + 240);
    let seed = 0x9e3779b9;
    for (const v of voices) {
      seed = nextSeed(seed);
      if (!v._mute) renderVoice(out, 0, v, gain, seed);
    }
    return finish(out);
  }

  // ---- Playback ----
  let ctx = null, cur = null;
  function audio() { return (ctx ??= new AudioContext()); }
  /// Play rendered samples. opts: loop, speaker (rough small-speaker filter), offset (s).
  function source(data, opts) {
    const c = audio();
    const buf = c.createBuffer(1, data.length, RATE);
    buf.copyToChannel(data, 0);
    const src = c.createBufferSource();
    src.buffer = buf;
    src.loop = !!opts.loop;
    let node = src;
    if (opts.speaker) {
      const hi = c.createBiquadFilter(); hi.type = "highpass"; hi.frequency.value = 350;
      const lo = c.createBiquadFilter(); lo.type = "lowpass"; lo.frequency.value = 7000;
      node.connect(hi); hi.connect(lo); node = lo;
    }
    node.connect(c.destination);
    return src;
  }
  /// Play without stopping what's playing (note previews over a running song).
  function preview(data, opts = {}) { source(data, opts).start(); }
  function play(data, opts = {}) {
    const c = audio();
    stop();
    const src = source(data, opts);
    const buf = src.buffer;
    const offset = Math.min(opts.offset || 0, Math.max(0, buf.duration - 0.01));
    src.start(0, offset);
    cur = { src, start: c.currentTime - offset, len: buf.duration, loop: src.loop };
    src.onended = () => { if (cur?.src === src) cur = null; };
    return cur;
  }
  function stop() {
    if (cur) { cur.src.onended = null; try { cur.src.stop(); } catch {} cur = null; }
  }
  /// Seconds into what's playing (wrapped when looping), or null.
  function position() {
    if (!cur) return null;
    const t = ctx.currentTime - cur.start;
    return cur.loop ? t % cur.len : Math.min(t, cur.len);
  }

  // ---- Voice editing ----
  // Parameter rows: [path, label, min, max, kind] where kind is "lin", "log" or "logoff" (0 = off).
  const GROUPS = [
    ["Pitch", [
      [["freq"], "pitch Hz", 20, 12000, "log"],
      [["slide"], "slide oct/s", -8, 8, "lin"],
      [["jump", 0], "jump at s", 0, 1, "lin"],
      [["jump", 1], "jump ×", 0.25, 4, "log"],
      [["duty"], "duty", 0.05, 0.95, "lin"],
      [["vib", 0], "vibrato Hz", 0, 40, "lin"],
      [["vib", 1], "vibrato depth", 0, 0.5, "lin"],
    ]],
    ["Envelope", [
      [["delay"], "delay s", 0, 1, "lin"],
      [["attack"], "attack s", 0, 1, "lin"],
      [["hold"], "hold s", 0, 1, "lin"],
      [["decay"], "decay s", 0.005, 1.5, "log"],
      [["vol"], "volume", 0, 1, "lin"],
    ]],
    ["Filter", [
      [["lp"], "low-pass Hz", 20, 12000, "logoff"],
      [["lp_slide"], "lp sweep oct/s", -8, 8, "lin"],
      [["hp"], "high-pass Hz", 20, 8000, "logoff"],
    ]],
  ];
  const get = (v, p) => p.length === 1 ? v[p[0]] : v[p[0]][p[1]];
  const set = (v, p, x) => { if (p.length === 1) v[p[0]] = x; else v[p[0]][p[1]] = x; };
  const toPos = (x, min, max, kind) => kind === "lin" ? (x - min) / (max - min) * 1000
    : kind === "logoff" && x <= 0 ? 0 : Math.log(x / min) / Math.log(max / min) * 1000;
  const fromPos = (p, min, max, kind) => {
    if (kind === "lin") return min + (max - min) * p / 1000;
    if (kind === "logoff" && p <= 0) return 0;
    return min * Math.pow(max / min, p / 1000);
  };
  function tidy(x) {
    if (x === 0) return 0;
    const a = Math.abs(x);
    const d = a >= 1000 ? 0 : a >= 100 ? 1 : a >= 10 ? 2 : a >= 1 ? 3 : 4;
    return +x.toFixed(d);
  }

  /// Fill `el` with one card per voice. opts.onEdit() runs after every change;
  /// opts.instrument adds the fixed/gate switches and labels pitch relative to A4.
  function voiceCards(el, voices, opts) {
    const redraw = () => { voiceCards(el, voices, opts); opts.onEdit(); };
    el.innerHTML = "";
    voices.forEach((v, i) => {
      const card = document.createElement("div");
      card.className = "voice" + (v._mute ? " muted" : "");
      card.innerHTML = `<div class="head"><b>Voice ${i + 1}</b>
        <button class="small" data-a="solo">solo</button>
        <button class="small" data-a="mute">${v._mute ? "unmute" : "mute"}</button>
        <button class="small" data-a="dup">copy</button>
        <button class="small" data-a="del">✕</button></div>
        <div class="waves">${WAVES.map(w => `<button class="${w === v.wave ? "on" : ""}" data-w="${w}">${w}</button>`).join("")}</div>`;
      card.querySelectorAll("[data-w]").forEach(b => b.onclick = () => { v.wave = b.dataset.w; redraw(); });
      card.querySelector("[data-a=mute]").onclick = () => { v._mute = !v._mute; redraw(); };
      card.querySelector("[data-a=solo]").onclick = () => {
        const solo = voices.every((o, j) => (j === i) !== !!o._mute);
        voices.forEach((o, j) => o._mute = solo ? false : j !== i); redraw();
      };
      card.querySelector("[data-a=dup]").onclick = () => { voices.splice(i + 1, 0, clone(v)); redraw(); };
      card.querySelector("[data-a=del]").onclick = () => { voices.splice(i, 1); redraw(); };
      if (opts.instrument) {
        const sw = document.createElement("div");
        sw.className = "switches";
        sw.innerHTML = `<label title="Ignore the note's pitch (drums, clicks)"><input type="checkbox" data-s="fixed"> fixed pitch</label>
          <label title="Hold while the note lasts, then release over 'decay'. Off = one-shot"><input type="checkbox" data-s="gate"> hold for note length</label>`;
        sw.querySelectorAll("[data-s]").forEach(cb => {
          cb.checked = !!v[cb.dataset.s];
          cb.onchange = () => { v[cb.dataset.s] = cb.checked; redraw(); };
        });
        card.appendChild(sw);
      }
      for (const [group, params] of GROUPS) {
        card.insertAdjacentHTML("beforeend", `<div class="group">${group}</div>`);
        for (const [path, label0, min, max, kind] of params) {
          if (path[0] === "duty" && v.wave !== "Square") continue;
          const label = opts.instrument && path[0] === "freq" && !v.fixed ? "pitch @A4" : label0;
          const row = document.createElement("div");
          row.className = "param";
          row.innerHTML = `<label>${label}</label><input type="range" min="0" max="1000" step="1"><input type="number" step="any">`;
          const [lab, range, num] = row.children;
          const sync = () => {
            const x = get(v, path);
            range.value = toPos(x, min, max, kind);
            num.value = x;
            row.classList.toggle("off", kind === "logoff" && x <= 0);
          };
          range.oninput = () => { set(v, path, tidy(fromPos(+range.value, min, max, kind))); sync(); opts.onEdit(); };
          num.onchange = () => { const x = parseFloat(num.value); if (isFinite(x)) set(v, path, x); sync(); opts.onEdit(); };
          lab.title = path.join(".");
          sync();
          card.appendChild(row);
        }
      }
      el.appendChild(card);
    });
  }

  /// Nudge every active parameter a little.
  function mutate(voices) {
    for (const v of voices) for (const [, params] of GROUPS) for (const [path, , min, max, kind] of params) {
      const x = get(v, path);
      if (x === 0 && kind !== "lin") continue;
      if (path[0] === "jump" && path[1] === 1 && x === 1) continue;
      if ((path[0] === "vib" || path[0] === "delay") && x === 0) continue;
      const y = kind === "lin" ? x + (Math.random() - 0.5) * (max - min) * 0.06 : x * Math.exp((Math.random() - 0.5) * 0.25);
      set(v, path, tidy(Math.min(max, Math.max(kind === "lin" ? min : 0, y))));
    }
  }

  /// Shared styles for the voice cards.
  const CSS = `
    .voices { display: grid; grid-template-columns: repeat(auto-fill, minmax(340px, 1fr)); gap: 12px; }
    .voice { background: var(--panel); border: 1px solid var(--line); border-radius: 8px; padding: 10px; }
    .voice.muted { opacity: 0.45; }
    .voice .head { display: flex; gap: 6px; align-items: center; margin-bottom: 8px; }
    .voice .head b { margin-right: auto; }
    .voice .switches { display: flex; gap: 14px; margin-top: 8px; color: var(--dim); }
    .group { font-size: 11px; text-transform: uppercase; letter-spacing: .06em; color: var(--dim); margin: 8px 0 2px; }
    .param { display: grid; grid-template-columns: 96px 1fr 80px; gap: 6px; align-items: center; height: 24px; }
    .param label { color: var(--dim); white-space: nowrap; }
    .param.off label { text-decoration: line-through; }
    .waves { display: flex; gap: 3px; }
    .waves button { padding: 2px 8px; }
    .waves button.on { background: #2c4a63; border-color: #4a7aa3; }`;
  if (typeof document !== "undefined") {
    const st = document.createElement("style");
    st.textContent = CSS;
    document.head.appendChild(st);
  }

  return {
    RATE, WAVES, DEFAULT, FIELDS, INST_DEFAULT, INST_FIELDS, clone, full, fullInst, voiceLen,
    nextSeed, renderVoice, finish, render, play, preview, stop, position, audio, GROUPS, get, set, tidy,
    voiceCards, mutate,
  };
})();
if (typeof module !== "undefined") module.exports = Synth;
