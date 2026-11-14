'use strict';
// Behaviour scenes: a small scripted sequencer, and the phase timeline under every viewer.
//
// A scene (from a mob script) is plain data, time in ticks (docs/showcase/README.md has it all):
//   { id, title, intro, ticks, eye, target,
//     phases: 'rev' | null,                  draw the attack phases of that actor's clips
//     spans: [{phase, from, to, clip}],      or give the phase spans by hand (wins over phases)
//     env: { background, fog, fogColor, light, sun, ground, lamp: {actor, at, color, intensity, distance} },
//     actors: { rev: { model: 'revenant', hold: {socket: item}, size: 1,
//                      clips: [[tick, clip, from, fade], ...],  clip from that tick on; from: the
//                                                               clip's own start tick, fade: ticks
//                      pos:   [[tick, [x, z]] | SC.orbit(...), ...],   ground position, blocks
//                      lift:  [[tick, y] | SC.hop(...), ...],          height above the ground
//                      face:  [[tick, 'player' | degrees | null | 'path'], ...],
//                      turn: 12,                                degrees per tick it can turn
//                      show:  [t0, t1] | [[t0, t1], ...],       visible only in these windows
//                      state: [[tick, ['rage']], ...] } },      mob state flags (state parts)
//     props: [{ box: [w, h, d], at: [x, z], y, color, show },  a textured block-like box
//             { model: '_shelter', at, yaw, y, size, clips, show },   a prop figure from a glb
//             { make: (group, T) => (t) => {} }],           anything else, drawn by the scene
//     captions: [[tick, text], ...], marks: [[tick, label], ...],
//     camera: [[tick, eye [x, y, z], target [x, y, z]], ...] }   optional: the camera flies
//                                              between keys (smoothstep); orbiting is off then
// Yaw 0 looks along +z. Positions, heights and yaws are worked out tick by tick once, then
// interpolated between ticks.
(function () {
  const T = THREE;
  const FADE = 3; // ticks of crossfade between clips

  // An arc around a point: from angle a0 to a1 (degrees, 0 = +z), radius r0 to r1, ticks t0..t1.
  SC.orbit = (t0, t1, centre, r0, r1, a0, a1) => ({ orbit: { t0, t1, centre, r0, r1, a0, a1 } });
  // A hop for `lift`: a parabola from base up to base + h and back, ticks t0..t1.
  SC.hop = (t0, t1, h, base) => ({ hop: { t0, t1, h, base: base || 0 } });

  // Keys [[tick, value], ...] (value a number or [x, z]) with SC.orbit / SC.hop spans expanded,
  // sampled at every tick 0..ticks, linear between keys, held before the first and after the last.
  function sampled(spec, ticks, zero) {
    const keys = [];
    for (const k of spec || []) {
      if (k.orbit) {
        const o = k.orbit;
        for (let t = o.t0; t <= o.t1; t++) {
          const u = (t - o.t0) / (o.t1 - o.t0);
          const a = ((o.a0 + (o.a1 - o.a0) * u) * Math.PI) / 180, r = o.r0 + (o.r1 - o.r0) * u;
          keys.push([t, [o.centre[0] + Math.sin(a) * r, o.centre[1] + Math.cos(a) * r]]);
        }
      } else if (k.hop) {
        const o = k.hop;
        for (let t = o.t0; t <= o.t1; t++) {
          const u = (t - o.t0) / (o.t1 - o.t0);
          keys.push([t, o.base + 4 * o.h * u * (1 - u)]);
        }
      } else keys.push(k);
    }
    if (!keys.length) keys.push([0, zero]);
    keys.sort((a, b) => a[0] - b[0]);
    const mix = (a, b, u) => (typeof a === 'number' ? a + (b - a) * u : [a[0] + (b[0] - a[0]) * u, a[1] + (b[1] - a[1]) * u]);
    const out = [];
    let i = 0;
    for (let t = 0; t <= ticks; t++) {
      while (i < keys.length - 1 && keys[i + 1][0] <= t) i++;
      const a = keys[i], b = keys[Math.min(i + 1, keys.length - 1)];
      const u = b[0] > a[0] ? Math.max(0, Math.min(1, (t - a[0]) / (b[0] - a[0]))) : 0;
      out.push(mix(a[1], b[1], u));
    }
    return out;
  }

  const wrap = (a) => Math.atan2(Math.sin(a), Math.cos(a));

  // The camera of a scene with `camera` keys at time t: {eye, target}, eased between keys.
  SC.cameraAt = (scene, t) => {
    const keys = scene.camera;
    let i = 0;
    while (i < keys.length - 1 && keys[i + 1][0] <= t) i++;
    const a = keys[i], b = keys[Math.min(i + 1, keys.length - 1)];
    let u = b[0] > a[0] ? Math.max(0, Math.min(1, (t - a[0]) / (b[0] - a[0]))) : 0;
    u = u * u * (3 - 2 * u);
    const mix = (p, q) => p.map((v, k) => v + (q[k] - v) * u);
    return { eye: mix(a[1], b[1]), target: mix(a[2], b[2]) };
  };

  // The last key at or before t of [[tick, value], ...], or fallback.
  function keyAt(list, t, fallback) {
    let v = fallback;
    for (const k of list || []) if (k[0] <= t) v = k[1];
    return v;
  }

  // Model props become actors that stand still; they can still run clips and have windows.
  function withProps(scene) {
    const actors = Object.assign({}, scene.actors);
    (scene.props || []).forEach((p, i) => {
      if (!p.model) return;
      actors['prop' + i] = { model: p.model, size: p.size, clips: p.clips || [], pos: [[0, p.at || [0, 0]]],
        lift: [[0, p.y || 0]], face: [[0, p.yaw || 0]], show: p.show };
    });
    return actors;
  }

  // Works out every actor's position, height and yaw for every tick.
  function plan(scene, actors) {
    const names = Object.keys(actors);
    const pos = {}, lift = {}, yaw = {};
    for (const n of names) {
      pos[n] = sampled(actors[n].pos, scene.ticks, [0, 0]);
      lift[n] = sampled(actors[n].lift, scene.ticks, 0);
    }
    for (const n of names) {
      const a = actors[n];
      const turn = ((a.turn || 360) * Math.PI) / 180;
      const p = pos[n];
      let y = 0;
      const out = [];
      for (let t = 0; t <= scene.ticks; t++) {
        const f = keyAt(a.face, t, 0);
        let want = y;
        if (typeof f === 'number') want = (f * Math.PI) / 180;
        else if (f === 'path') {
          // Along the ground velocity; standing still keeps the last heading.
          const q0 = p[Math.max(0, t - 1)], q1 = p[Math.min(scene.ticks, t + 1)];
          if (Math.hypot(q1[0] - q0[0], q1[1] - q0[1]) > 1e-3) want = Math.atan2(q1[0] - q0[0], q1[1] - q0[1]);
        } else if (typeof f === 'string') {
          if (!pos[f]) throw new Error('scene ' + scene.id + ': ' + n + ' faces unknown actor ' + f);
          const q = pos[f][t];
          want = Math.atan2(q[0] - p[t][0], q[1] - p[t][1]);
        }
        const d = wrap(want - y);
        y = t === 0 ? want : y + Math.max(-turn, Math.min(turn, d));
        out.push(y);
      }
      yaw[n] = out;
    }
    return { pos, lift, yaw };
  }

  // Clip layers at scene time t: [[clip, clip tick, weight], ...] with the crossfade from the
  // clip before. A clip row is [tick, clip, from = 0, fade = 3]; fade 0 cuts.
  function clipLayers(list, t) {
    let k = -1;
    for (let i = 0; i < list.length; i++) if (list[i][0] <= t) k = i;
    if (k < 0) return [];
    const [t0, clip, from = 0, fade = FADE] = list[k];
    const w = k > 0 && fade > 0 ? Math.min(1, (t - t0) / fade) : 1;
    const out = [[clip, from + t - t0, w]];
    if (w < 1) {
      const [p0, pc, pf = 0] = list[k - 1];
      out.push([pc, pf + t - p0, 1 - w]);
    }
    return out;
  }

  // Visibility windows: [t0, t1] or [[t0, t1], ...], visible for t0 <= t < t1.
  function shown(show, t) {
    if (!show) return true;
    const list = typeof show[0] === 'number' ? [show] : show;
    return list.some(([a, b]) => t >= a && t < b);
  }

  // The attack phases on the scene's bar: the scene's own spans, or the phases of one actor's
  // attack clips, shifted by the clip's start tick and cut where the next clip starts.
  SC.scenePhases = (scene) => {
    if (scene.spans) return scene.spans;
    const n = scene.phases;
    if (!n) return [];
    const a = scene.actors[n], rig = SC.RIGS[a.model];
    const out = [];
    a.clips.forEach(([t0, clip, from = 0], i) => {
      const c = rig && rig.clips[clip];
      if (!c || c.markers.hit == null) return;
      const stop = i + 1 < a.clips.length ? a.clips[i + 1][0] : scene.ticks;
      for (const s of SC.phaseSpans(c.markers)) {
        const f = Math.max(t0, t0 + s.from - from), to = Math.min(stop, t0 + s.to - from);
        if (to > f) out.push({ phase: s.phase, from: f, to, clip });
      }
    });
    return out;
  };

  // A 16 x 16 block-like texture: the base colour with a little noise, darker bottom and right edge.
  const boxTextures = {};
  function blockTexture(color) {
    if (boxTextures[color]) return boxTextures[color];
    const c = document.createElement('canvas');
    c.width = c.height = 16;
    const g = c.getContext('2d');
    const base = new T.Color(color);
    let s = color.length * 977;
    for (let y = 0; y < 16; y++)
      for (let x = 0; x < 16; x++) {
        s = (s * 1103515245 + 12345) >>> 0;
        g.fillStyle = '#' + base.clone().multiplyScalar(0.86 + ((s >>> 16) % 28) / 100).getHexString();
        g.fillRect(x, y, 1, 1);
      }
    g.fillStyle = 'rgba(0,0,0,0.22)';
    g.fillRect(0, 15, 16, 1);
    g.fillRect(15, 0, 1, 16);
    return (boxTextures[color] = c);
  }
  function boxProp(p, group) {
    const [w, h, d] = p.box;
    const canvas = blockTexture(p.color || '#6f6c66');
    // One 16-texel tile per block on every face, as a block would be.
    const mats = [[d, h], [d, h], [w, d], [w, d], [w, h], [w, h]].map(([u, v]) => {
      const t = new T.CanvasTexture(canvas);
      t.magFilter = t.minFilter = T.NearestFilter;
      t.generateMipmaps = false;
      t.encoding = T.sRGBEncoding;
      t.wrapS = t.wrapT = T.RepeatWrapping;
      t.repeat.set(u, v);
      return new T.MeshLambertMaterial({ map: t });
    });
    const mesh = new T.Mesh(new T.BoxGeometry(w, h, d), mats);
    mesh.position.set(p.at[0], (p.y || 0) + h / 2, p.at[1]);
    mesh.castShadow = mesh.receiveShadow = true;
    group.add(mesh);
    return (t) => (mesh.visible = shown(p.show, t));
  }

  class Sequencer {
    // stage: an SC.Stage; actors: name -> SC.actor for every actor and model prop of the scene.
    constructor(stage, scene, actors) {
      this.stage = stage;
      this.scene = scene;
      this.specs = withProps(scene);
      this.actors = actors;
      this.plan = plan(scene, this.specs);
      this.group = new T.Group();
      for (const a of Object.values(actors)) this.group.add(a.root);
      this.props = (scene.props || []).filter((p) => !p.model).map((p) => (p.box ? boxProp(p, this.group) : p.make(this.group, T)));
      stage.scene.add(this.group);
      const env = scene.env || null;
      stage.setEnv(env, env && env.lamp ? actors[env.lamp.actor] : null);
      this.t = 0;
    }
    apply(t) {
      this.t = Math.max(0, Math.min(this.scene.ticks, t));
      const i = Math.floor(this.t), f = this.t - i, j = Math.min(i + 1, this.scene.ticks);
      for (const [n, actor] of Object.entries(this.actors)) {
        const spec = this.specs[n];
        const p = this.plan.pos[n], h = this.plan.lift[n], y = this.plan.yaw[n];
        actor.root.position.set(p[i][0] + (p[j][0] - p[i][0]) * f, h[i] + (h[j] - h[i]) * f, p[i][1] + (p[j][1] - p[i][1]) * f);
        actor.root.rotation.y = y[i] + wrap(y[j] - y[i]) * f;
        actor.root.scale.setScalar(spec.size || 1);
        actor.root.visible = shown(spec.show, this.t);
        actor.setState(keyAt(spec.state, this.t, []));
        actor.pose(clipLayers(spec.clips || [], this.t));
      }
      for (const prop of this.props) prop(this.t);
    }
    dispose() {
      this.stage.scene.remove(this.group);
      this.stage.setEnv(null, null);
    }
  }
  SC.Sequencer = Sequencer;

  // Loads every actor and model prop of a scene onto a stage: a promise of its Sequencer.
  SC.playScene = (stage, scene) => {
    const specs = withProps(scene);
    const names = Object.keys(specs);
    return Promise.all(names.map((n) => SC.actor(specs[n].model, specs[n].hold))).then((list) => {
      const actors = {};
      names.forEach((n, i) => (actors[n] = list[i]));
      return new Sequencer(stage, scene, actors);
    });
  };

  // ------------------------------------------------------------------ timeline bar
  // A bar of `ticks` with coloured spans [{phase, from, to}], marks [[tick, label]] and a playhead.
  // The playhead stays inside the track for every tick, the last one too.
  SC.timeline = (ticks, spans, marks, onSeek) => {
    const bar = SC.el('div', { class: 'tl-bar', role: 'slider', 'aria-label': 'Шкала времени, тики', 'aria-valuemin': 0, 'aria-valuemax': ticks, tabindex: 0 });
    const at = (t) => (Math.max(0, Math.min(ticks, t)) / ticks) * 100;
    for (const s of spans) {
      bar.append(SC.el('div', {
        class: 'tl-span', title: SC.PHASES[s.phase].name + ': тики ' + s.from + '–' + s.to,
        style: `left:${at(s.from)}%;width:${at(s.to) - at(s.from)}%;background-color:${SC.PHASES[s.phase].color}`,
      }));
    }
    const step = ticks > 60 ? 20 : ticks > 24 ? 5 : 2;
    for (let t = 0; t <= ticks; t += step) {
      bar.append(SC.el('div', { class: 'tl-tick', style: `left:calc((100% - 1px) * ${at(t) / 100})` }, SC.el('span', {}, t)));
    }
    for (const [t, label] of marks || []) {
      bar.append(SC.el('div', { class: 'tl-mark', style: `left:calc((100% - 2px) * ${at(t) / 100})` }, SC.el('span', {}, label)));
    }
    const head = SC.el('div', { class: 'tl-head' });
    bar.append(head);
    const seek = (ev) => {
      const r = bar.getBoundingClientRect();
      onSeek(Math.max(0, Math.min(ticks, ((ev.clientX - r.left) / r.width) * ticks)));
    };
    let drag = false;
    bar.addEventListener('pointerdown', (ev) => {
      drag = true;
      bar.setPointerCapture(ev.pointerId);
      seek(ev);
    });
    bar.addEventListener('pointermove', (ev) => drag && seek(ev));
    bar.addEventListener('pointerup', () => (drag = false));
    bar.addEventListener('keydown', (ev) => {
      if (ev.key === 'ArrowLeft' || ev.key === 'ArrowRight') {
        onSeek(null, ev.key === 'ArrowLeft' ? -1 : 1);
        ev.preventDefault();
      }
    });
    return {
      el: bar,
      set(t) {
        // The head is 3 px wide: its left edge runs from 0 to the track width minus 3 px.
        head.style.left = `calc((100% - 3px) * ${at(t) / 100})`;
        bar.setAttribute('aria-valuenow', Math.floor(Math.max(0, Math.min(ticks, t))));
      },
    };
  };

  SC.legend = () =>
    SC.el('div', { class: 'legend-row tl-legend' },
      Object.values(SC.PHASES).map((p) => SC.el('span', {}, SC.el('i', { style: 'background:' + p.color }), p.name)));
})();
