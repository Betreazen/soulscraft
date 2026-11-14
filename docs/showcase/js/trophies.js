'use strict';
// Trophies (docs/design/09-trophies.md): #trophies (one diorama up close with its particles,
// glow and light, all seven by day and by night, the trophy hall flythrough, the heads, drops,
// what the mod gets, the rejected variants) and a «Трофей» block at the end of every mob page.
//
// Everything is drawn from assets/rigs/<mob>.trophy.json (SC.TROPHIES) the way the mod's
// block entity will draw it, with the mob's own glb:
//   block group  at the block's bottom centre, turned by the blockstate (turn = -facing_y, or
//                -22.5 * rotation for a floor head)
//   inner group  position offset / 16, rotation y = yaw, scale: the mob's draw frame (the glTF
//                frame: feet at the origin, +z its front)
//   figure       the actor in the clip `trophy` at (time + phase) % ticks, state flags on; a head
//                is the rig's head parts taken out of the actor, each at its rest place
//   particles    the diorama's emitters: a particle of an own type (SC.BLOCKS.particles: sprite
//                and horizon:behaviour) is born in the box with chance 1/every each tick and
//                moves as the engine moves it (velocity + jitter, gravity, friction, size and
//                alpha curves over its lifetime). Sprites are blended, as the engine blends
//                particles; there is no translucent geometry.
//   light        the block's light level as a warm point light in the block; glowing elements
//                and glow parts are emissive (full-bright in the game)
(function () {
  const T = THREE;
  const el = SC.el;
  const rad = (d) => (d * Math.PI) / 180;
  const ORIGIN = 1.501; // blocks from the feet to the model origin
  const FACING = { north: 0, east: 90, south: 180, west: 270 };
  const mobs = () => SC.DATA.mobs.filter((m) => SC.TROPHIES[m.id]);
  const mobName = (id) => SC.mobById(id).name;

  SC.navSection(() => ['Награды', [
    ['#trophies', 'Трофеи: диорамы', 'trophies'],
    ['#book', 'Книга охотника', 'book'],
  ]]);

  // ------------------------------------------------------------------ light
  // The three lights of the viewer. env goes to Stage.setEnv; lamp scales the block's light;
  // a particle takes the light of its block in the game, so it is as bright as the light there.
  const LIGHTS = {
    day: { name: 'День', note: 'Обычный свет витрины.', env: null, lamp: 0.5, floor: 1 },
    night: { name: 'Ночь', note: 'Тёмно-синее небо, слабая луна: видно, что светится сама диорама.', env: { background: '#0b0f1c', light: 0.2, ambientColor: '#7083b8', sun: 0.1, sunColor: '#aab8ff', ground: '#30343e' }, lamp: 2.2, floor: 0.32 },
    cave: { name: 'Тёмная пещера', note: 'Почти без света: светятся только диорама и частицы.', env: { background: '#030303', light: 0.03, sun: 0, ground: '#1c1c1c' }, lamp: 2.6, floor: 0.05 },
  };
  const brightness = (mode, level) => (mode === 'day' ? 1 : Math.min(1, Math.max(LIGHTS[mode].floor, 1.15 * Math.pow(level / 15, 0.7))));

  // ------------------------------------------------------------------ building trophies
  // A part node at a game-frame place: top parts carry the model origin, children are local.
  function setGamePose(node, pivot, rot, top) {
    node.position.set(pivot[0] / 16, (top ? ORIGIN * 16 : 0) / 16 - pivot[1] / 16, -pivot[2] / 16);
    node.rotation.set(rot[0], -rot[1], -rot[2], 'ZYX');
    node.scale.set(1, 1, 1);
  }
  // A flat label on the plaque: the name and a made-up kill count, as the mod will write it.
  function plaqueMesh(plaque, text) {
    const c = document.createElement('canvas');
    const w = plaque.to[0] - plaque.from[0], h = plaque.to[1] - plaque.from[1];
    c.width = 256;
    c.height = Math.round((256 * h) / w);
    const g = c.getContext('2d');
    g.fillStyle = '#2a1c0b';
    g.textAlign = 'center';
    g.textBaseline = 'middle';
    let size = Math.round(c.height * 0.6);
    g.font = '700 ' + size + 'px Georgia, serif';
    while (g.measureText(text).width > c.width * 0.86 && size > 8) g.font = '700 ' + --size + 'px Georgia, serif';
    g.fillText(text, c.width / 2, c.height / 2 + 1);
    const tex = new T.CanvasTexture(c);
    tex.encoding = T.sRGBEncoding;
    const mat = new T.MeshLambertMaterial({ map: tex, transparent: true, polygonOffset: true, polygonOffsetFactor: -2 });
    const mesh = new T.Mesh(new T.PlaneGeometry(w / 16, h / 16), mat);
    // On the plaque's north face (block px of the north-facing model), a hair in front of it.
    mesh.position.set((plaque.from[0] + plaque.to[0]) / 32 - 0.5, (plaque.from[1] + plaque.to[1]) / 32, plaque.from[2] / 16 - 0.5 - 0.002);
    mesh.rotation.y = Math.PI;
    return mesh;
  }
  // The plaque says the name and the owner's kills (from the hunter's book): a sample number.
  const plaqueText = (id) => mobName(id) + ' · ' + (7 + (id.length * 5) % 17);

  // A head on the floor (rotation 0..15) or on a wall (facing): {group, actor}.
  SC.trophyHead = (id, where, r) =>
    SC.actor(id).then((actor) => {
      const h = SC.TROPHIES[id].head, fit = h[where];
      const nodes = {};
      for (const p of h.parts) {
        nodes[p.name] = actor.node(p.name);
        if (!nodes[p.name]) throw new Error(id + ': no part ' + p.name + ' in the glb');
      }
      const draw = new T.Group();
      for (const p of h.parts) {
        if (p.parent === null) draw.add(nodes[p.name]);
        setGamePose(nodes[p.name], p.pivot, p.rotation, p.parent === null);
      }
      // Children of the head parts that are not in the head (the lurker's tail and fins) go.
      for (const p of h.parts)
        for (const c of [...nodes[p.name].children]) {
          const name = (c.userData.name || '').split(':')[1];
          if (name && !name.startsWith('socket_') && !nodes[name]) nodes[p.name].remove(c);
        }
      const inner = new T.Group();
      inner.position.set(fit.offset[0] / 16, fit.offset[1] / 16, fit.offset[2] / 16);
      inner.rotation.y = rad(fit.yaw);
      inner.scale.setScalar(fit.scale);
      inner.add(draw);
      const group = new T.Group();
      group.rotation.y = where === 'floor' ? rad(-22.5 * (r || 0)) : rad(-FACING[r || 'north']);
      group.add(inner);
      draw.traverse((o) => {
        if (o.isMesh) o.castShadow = o.receiveShadow = true;
      });
      return { group, actor };
    });

  // ------------------------------------------------------------------ particles
  const partTex = {};
  function particleTexture(pid) {
    if (partTex[pid]) return partTex[pid];
    const info = SC.BLOCKS.particles[pid];
    if (!info) throw new Error('no particle type ' + pid);
    const tex = new T.Texture();
    const img = new Image();
    img.onload = () => {
      tex.image = img;
      tex.needsUpdate = true;
    };
    img.src = 'data:image/png;base64,' + info.png;
    tex.encoding = T.sRGBEncoding;
    tex.magFilter = tex.minFilter = T.NearestFilter;
    tex.generateMipmaps = false;
    return (partTex[pid] = tex);
  }
  // A small seeded generator: the same diorama spawns the same particles on every load.
  function seeded(seed) {
    let s = seed >>> 0;
    return () => {
      s = (s + 0x6d2b79f5) >>> 0;
      let t = s;
      t = Math.imul(t ^ (t >>> 15), t | 1);
      t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
      return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
    };
  }
  const hash = (s) => [...s].reduce((h, c) => (Math.imul(h, 31) + c.charCodeAt(0)) >>> 0, 7);
  const lerp = (a, b, k) => a + (b - a) * k;

  // The particles of one diorama in a group in block px of the north-facing model (the group is
  // the block's: the facing turns them with it). tick() is one game tick.
  class Particles {
    constructor(emitters, seed) {
      this.group = new T.Group();
      this.group.position.set(-0.5, 0, -0.5);
      this.group.scale.setScalar(1 / 16);
      this.rand = seeded(seed);
      this.emitters = emitters.map((e) => {
        const pid = e.type.split(':')[1];
        return { e, b: SC.BLOCKS.particles[pid].behaviour, tex: particleTexture(pid) };
      });
      this.live = [];
      this.pool = [];
      this.bright = 1;
    }
    spawn(em) {
      const r = this.rand, b = em.b, box = em.e.box;
      const s = this.pool.pop() || new T.Sprite(new T.SpriteMaterial({ transparent: true, depthWrite: false, fog: false }));
      s.material.map = em.tex;
      s.material.color.setScalar(this.bright);
      s.position.set(lerp(box.from[0], box.to[0], r()), lerp(box.from[1], box.to[1], r()), lerp(box.from[2], box.to[2], r()));
      const v = b.velocity || [0, 0, 0], j = b.velocity_jitter || [0, 0, 0];
      const p = {
        s, b, age: 0, life: b.lifetime || 20 + Math.floor(r() * 20),
        v: new T.Vector3(...v.map((c, k) => (c + (r() * 2 - 1) * j[k]) * 16)),
      };
      this.group.add(s);
      this.live.push(p);
      this.draw(p);
    }
    draw(p) {
      const k = Math.min(1, p.age / p.life), b = p.b;
      const size = b.size ? lerp(b.size[0], b.size[1], k) : 0.05;
      p.s.scale.setScalar(Math.max(0.001, size * 2 * 16));
      p.s.material.opacity = b.alpha ? lerp(b.alpha[0], b.alpha[1], k) : 1;
    }
    tick() {
      for (const em of this.emitters) if (this.rand() < 1 / em.e.every) this.spawn(em);
      const keep = [];
      for (const p of this.live) {
        p.age += 1;
        if (p.age > p.life) {
          this.group.remove(p.s);
          this.pool.push(p.s);
          continue;
        }
        p.v.y -= 0.04 * (p.b.gravity || 0) * 16;
        p.s.position.add(p.v);
        p.v.multiplyScalar(p.b.friction != null ? p.b.friction : 0.98);
        this.draw(p);
        keep.push(p);
      }
      this.live = keep;
    }
    run(n) {
      for (let i = 0; i < n; i++) this.tick();
    }
    setBright(v) {
      this.bright = v;
      for (const p of this.live) p.s.material.color.setScalar(v);
    }
  }

  // ------------------------------------------------------------------ the diorama
  const WARM = 0xffdcae;
  // One diorama: {group, actor, d, particles, lamp, setTime(tick), step(ticks), setLight(mode)}.
  // phase: ticks the loop is shifted by (the mod takes it from the block's position).
  SC.trophyDiorama = (id, facing, phase) =>
    Promise.all([SC.actor(id), SC.blocksReady()]).then(([actor]) => {
      const t = SC.TROPHIES[id], d = t.diorama;
      const group = new T.Group();
      group.rotation.y = rad(-FACING[facing || 'south']);
      group.add(SC.placeBlock({ model: d.block.split(':')[1], at: [0, 0, 0], rot: 0 }));
      group.add(plaqueMesh(d.plaque, plaqueText(id)));
      const inner = new T.Group();
      inner.position.set(d.offset[0] / 16, d.offset[1] / 16, d.offset[2] / 16);
      inner.rotation.y = rad(d.yaw);
      inner.scale.setScalar(d.scale);
      inner.add(actor.root);
      group.add(inner);
      actor.setState(d.flags);
      const lamp = new T.PointLight(WARM, 0, 2.6, 1.6);
      lamp.position.set(0, 0.8, -0.25);
      group.add(lamp);
      const particles = new Particles(d.particles, hash(id) + (phase || 0));
      group.add(particles.group);
      const out = { id, group, actor, d, lamp, particles, mode: 'day' };
      out.setTime = (tick) => actor.pose([[d.clip, tick + (phase || 0), 1]]);
      out.step = (n) => particles.run(n);
      out.setLight = (mode) => {
        out.mode = mode;
        lamp.intensity = (d.light / 15) * LIGHTS[mode].lamp;
        particles.setBright(brightness(mode, d.light));
      };
      out.setTime(0);
      out.setLight('day');
      return out;
    });

  // A stone block (a wall behind a wall head) in a cell.
  function wallBlock(at, texId) {
    const g = new T.Group();
    const tex = blockTex(texId || 'altar_stone', 1, 1);
    const mesh = new T.Mesh(new T.BoxGeometry(1, 1, 1), new T.MeshLambertMaterial({ map: tex }));
    mesh.position.set(at[0], at[1] + 0.5, at[2]);
    mesh.castShadow = mesh.receiveShadow = true;
    g.add(mesh);
    return g;
  }
  // A block texture (data URL from items.js), nearest filtered, repeated u x v times.
  const texCache = {}, texImages = {};
  // The images of block textures, decoded once: a still picture must not catch one half loaded.
  function texReady(ids) {
    return Promise.all(ids.map((id) => texImages[id] || (texImages[id] = new Promise((done, fail) => {
      const img = new Image();
      img.onload = () => done(img);
      img.onerror = () => fail(new Error('block texture ' + id));
      img.src = SC.BLOCK_TEX[id];
    }))));
  }
  function blockTex(id, u, v) {
    const key = id + ':' + u + ':' + v;
    if (texCache[key]) return texCache[key];
    const tex = new T.Texture();
    texReady([id]).then(([img]) => {
      tex.image = img;
      tex.needsUpdate = true;
    });
    tex.encoding = T.sRGBEncoding;
    tex.magFilter = tex.minFilter = T.NearestFilter;
    tex.generateMipmaps = false;
    tex.wrapS = tex.wrapT = T.RepeatWrapping;
    tex.repeat.set(u, v);
    return (texCache[key] = tex);
  }
  // A box of blocks with a block texture, one tile per block on every face.
  function texturedBox(size, at, texId) {
    const [w, h, d] = size;
    const mats = [[d, h], [d, h], [w, d], [w, d], [w, h], [w, h]].map(([u, v]) => new T.MeshLambertMaterial({ map: blockTex(texId, u, v) }));
    const mesh = new T.Mesh(new T.BoxGeometry(w, h, d), mats);
    mesh.position.set(at[0], at[1] + h / 2, at[2]);
    mesh.receiveShadow = mesh.castShadow = true;
    return mesh;
  }
  // Frame points of the cell [lo, hi] (blocks), for Stage.frame.
  const boxPts = (lo, hi) => {
    const pts = [];
    for (let i = 0; i < 8; i++) pts.push(new T.Vector3(i & 1 ? hi[0] : lo[0], i & 2 ? hi[1] : lo[1], i & 4 ? hi[2] : lo[2]));
    return pts;
  };

  function whenReady(box, build) {
    Promise.all([SC.blocksReady(), texReady(['altar_stone', 'dio_wood'])]).then(build).catch((e) => {
      const msg = String(e && e.stack ? e.stack : e);
      console.error(msg);
      box.append(el('div', { class: 'sc-err' }, msg));
    });
  }
  const fmt = (v) => SC.num(Math.round(v * 100) / 100);
  function buttons(row, items, current, onPick) {
    row.textContent = '';
    for (const [key, label] of items) {
      const b = el('button', { type: 'button', 'data-key': key, class: key === current ? 'on' : '', 'aria-pressed': String(key === current) }, label);
      b.onclick = () => {
        for (const x of row.querySelectorAll('button')) {
          x.classList.toggle('on', x === b);
          x.setAttribute('aria-pressed', String(x === b));
        }
        onPick(key);
      };
      row.append(b);
    }
  }
  const sec = (ticks) => SC.num(Math.round((ticks / SC.TICK) * 10) / 10) + ' с';
  const particleName = (type) => SC.BLOCKS.particles[type.split(':')[1]].name;
  // The emitters of a diorama as a list: what, how often, how long one lives.
  function particleList(d) {
    return el('ul', { class: 'sc-plist' }, d.particles.map((e) => {
      const b = SC.BLOCKS.particles[e.type.split(':')[1]].behaviour;
      return el('li', {}, el('b', {}, particleName(e.type)), ' (' + e.type + '): в среднем раз в ' + e.every + ' тиков, живёт ' + sec(b.lifetime));
    }));
  }

  // ------------------------------------------------------------------ one diorama up close
  // A large viewer: the diorama on its stage, its figure breathing in the trophy loop, its
  // particles, the block light and the glow under day, night and dark-cave light. fixed: the mob
  // of a mob page (no mob buttons). Address: mob, light=day|night|cave, facing, and t=<tick> on
  // #trophies or dt=<tick> on a mob page (frozen at that tick).
  function dioramaPanel(params, fixed) {
    const canvas = el('canvas', { 'aria-label': 'Диорама моба крупно; свет и моб выбираются кнопками ниже' });
    const errBox = el('div', { class: 'sc-err', hidden: true });
    const mRow = fixed ? null : el('div', { class: 'sc-clips', role: 'group', 'aria-label': 'Моб' });
    const lRow = el('div', { class: 'sc-clips', role: 'group', 'aria-label': 'Свет' });
    const pause = el('button', { type: 'button', class: 'reset' }, 'Пауза');
    const turn = el('button', { type: 'button', class: 'reset' }, 'Повернуть блок');
    const fx = el('button', { type: 'button', class: 'reset', 'aria-pressed': 'true' }, 'Частицы: вкл');
    const link = el('button', { type: 'button', class: 'reset', title: 'Скопировать ссылку на эту диораму, свет и тик' }, 'Ссылка');
    const readout = el('div', { class: 'sc-readout' });
    const ro = SC.readout(readout);
    const text = el('div', { class: 'sc-intro' });
    const panel = el('div', { class: 'scene' }, el('div', { class: 'sc-stage tall' }, canvas), errBox, mRow, lRow,
      el('div', { class: 'controls' }, pause, turn, fx, link, readout), text);
    const fail = SC.ui.panelError(errBox);
    const stage = SC.ui.keep(new SC.Stage(canvas, { ground: 7, fov: 30 }));
    let mob = fixed || (SC.TROPHIES[params.get('mob')] ? params.get('mob') : 'revenant');
    let mode = LIGHTS[params.get('light')] ? params.get('light') : 'day';
    let facing = FACING[params.get('facing')] != null ? params.get('facing') : 'south';
    const tp = fixed ? 'dt' : 't';
    let playing = !params.has(tp), time = Number(params.get(tp)) || 0, done = 0, fxOn = true;
    let dio = null, root = null, opened = 0;
    const faces = ['south', 'west', 'north', 'east'];
    function light() {
      stage.setEnv(LIGHTS[mode].env, null);
      if (dio) dio.setLight(mode);
    }
    function show() {
      const ticket = ++opened;
      SC.trophyDiorama(mob, facing).then((o) => {
        if (ticket !== opened) return;
        if (root) stage.scene.remove(root);
        root = o.group;
        stage.scene.add(root);
        dio = o;
        o.particles.group.visible = fxOn;
        o.step(120 + Math.floor(time)); // particles already in the air when the page opens
        done = Math.floor(time);
        o.setTime(time);
        light();
        stage.frame(boxPts([-0.55, 0, -0.55], [0.55, 1.02, 0.55]), 0.94);
        const d = o.d;
        text.textContent = '';
        text.append(el('p', {}, el('b', {}, mobName(mob) + '. '), d.text + '.'),
          el('p', { class: 'small' }, 'Блок ' + d.block + ', свет ' + d.light + ', масштаб фигурки ' + fmt(d.scale) +
            ', поза — клип trophy (' + sec(d.ticks) + ' по кругу)' + (d.flags.length ? ', флаги: ' + d.flags.join(', ') : '') + '. Частицы:'),
          particleList(d));
      }).catch((e) => fail(String(e && e.stack ? e.stack : e)));
    }
    pause.onclick = () => {
      playing = !playing;
      pause.textContent = playing ? 'Пауза' : 'Ход';
    };
    if (!playing) pause.textContent = 'Ход';
    turn.onclick = () => {
      facing = faces[(faces.indexOf(facing) + 1) % 4];
      show();
    };
    fx.onclick = () => {
      fxOn = !fxOn;
      fx.textContent = 'Частицы: ' + (fxOn ? 'вкл' : 'выкл');
      fx.setAttribute('aria-pressed', String(fxOn));
      if (dio) dio.particles.group.visible = fxOn;
    };
    link.onclick = () => {
      const tick = dio ? Math.floor(time) % dio.d.ticks : Math.floor(time);
      const set = { light: mode, facing, [tp]: tick };
      if (!fixed) Object.assign(set, { mob, only: 'one' });
      SC.copyLink(SC.hashWith(set), mobName(mob) + ': ' + LIGHTS[mode].name.toLowerCase() + ', тик ' + tick);
    };
    buttons(lRow, Object.entries(LIGHTS).map(([k, v]) => [k, v.name]), mode, (k) => {
      mode = k;
      light();
      SC.toast('Свет: ' + LIGHTS[k].name.toLowerCase(), { desc: LIGHTS[k].note });
    });
    if (mRow) buttons(mRow, mobs().map((m) => [m.id, m.name]), mob, (k) => ((mob = k), show()));
    show();
    SC.onFrame((dt) => {
      if (playing) time += dt * SC.TICK;
      if (dio) {
        const whole = Math.floor(time);
        if (whole > done) dio.step(Math.min(60, whole - done));
        done = whole;
        dio.setTime(time);
        ro.set(Math.floor(time) % dio.d.ticks, dio.d.ticks, null, '', 'частиц в воздухе: ' + dio.particles.live.length + ' · свет блока ' + dio.d.light + ' · facing ' + facing);
      }
      stage.render();
    }, fail);
    return panel;
  }

  // ------------------------------------------------------------------ all seven, day and night
  let snapStage = null;
  function snapshot(object, lo, hi, env) {
    if (!snapStage) {
      const c = document.createElement('canvas');
      c.width = c.height = 260;
      snapStage = new SC.Stage(c, { snapshot: true, controls: false, fov: 30, ground: 0 });
      snapStage.renderer.setSize(260, 260, false);
      snapStage.camera.aspect = 1;
      snapStage.camera.updateProjectionMatrix();
    }
    snapStage.setEnv(env, null);
    snapStage.scene.add(object);
    snapStage.frame(boxPts(lo, hi), 1.02);
    snapStage.renderer.render(snapStage.scene, snapStage.camera);
    const url = snapStage.canvas.toDataURL('image/png');
    snapStage.scene.remove(object);
    return url;
  }
  function galleryTable() {
    const head = el('tr', {}, ['Моб', 'День', 'Ночь', 'Сцена'].map((h) => el('th', {}, h)));
    const body = el('tbody');
    const table = el('table', { class: 'sc-moves sc-matrix' }, el('thead', {}, head), body);
    const jobs = [];
    for (const m of mobs()) {
      const day = el('td', {}), night = el('td', {});
      const d = SC.TROPHIES[m.id].diorama;
      body.append(el('tr', {}, el('td', {}, el('a', { href: '#mob/' + m.id }, m.name)), day, night,
        el('td', { class: 'sc-left' }, d.text + '. Свет ' + d.light + ', масштаб ' + fmt(d.scale) + '. Частицы: ' + d.particles.map((e) => particleName(e.type).toLowerCase()).join(', ') + '.')));
      jobs.push(() => SC.trophyDiorama(m.id, 'south').then((o) => {
        o.step(120);
        o.setLight('day');
        day.append(el('img', { src: snapshot(o.group, [-0.5, 0, -0.5], [0.5, 1, 0.5], null), alt: m.name + ': диорама днём', width: 170, height: 170 }));
        o.setLight('night');
        night.append(el('img', { src: snapshot(o.group, [-0.5, 0, -0.5], [0.5, 1, 0.5], LIGHTS.night.env), alt: m.name + ': диорама ночью', width: 170, height: 170 }));
      }));
    }
    // One at a time: the snapshot stage is shared.
    jobs.reduce((p, job) => p.then(job), Promise.resolve()).catch((e) => {
      console.error(e);
      table.after(el('div', { class: 'sc-err' }, String(e && e.stack ? e.stack : e)));
    });
    return el('div', { class: 'wide' }, table);
  }

  // ------------------------------------------------------------------ the trophy hall
  // A long dim hall: three dioramas on stone pedestals along each side wall facing the aisle,
  // the brute at the end; each mob's head on the wall above its diorama. The dioramas light
  // themselves (block light, glow) and breathe, their particles drift. The camera walks the
  // aisle and stops at each.
  const HALL = {
    left: [['revenant', -1.5], ['stalker', -4.5], ['marksman', -7.5]],
    right: [['shrike', -1.5], ['lurker', -4.5], ['bog_slime', -7.5]],
    end: ['brute', -10],
  };
  function hallScene(prepared) {
    const T_END = 560;
    const stops = [];
    let t = 60;
    const visit = (eye, target) => {
      stops.push([t, eye, target], [t + 28, eye, target]);
      t += 60;
    };
    for (let i = 0; i < 3; i++) {
      const zl = HALL.left[i][1], zr = HALL.right[i][1];
      visit([-0.7, 1.85, zl + 1.0], [-3, 1.4, zl]);
      visit([0.7, 1.85, zr + 1.0], [3, 1.4, zr]);
    }
    visit([0, 1.9, -7.4], [0, 1.45, -10]);
    const wide = [[0, 2.7, 4.6], [0, 1.3, -6]];
    const order = [HALL.left[0], HALL.right[0], HALL.left[1], HALL.right[1], HALL.left[2], HALL.right[2], HALL.end];
    const captions = [[0, 'Зал трофеев: семь диорам на постаментах, над каждой — голова того же моба. Зал почти тёмный: светят сами диорамы.']];
    order.forEach(([id], i) => captions.push([stops[i * 2][0], mobName(id) + ': ' + SC.TROPHIES[id].diorama.text + '.']));
    captions.push([t + 10, 'Каждая диорама — один блок; моба рисует мод его же ригом, частицы пускает блок.']);
    return {
      id: 'hall', title: 'Зал трофеев', ticks: T_END,
      intro: 'Пролёт камеры по залу трофеев: слева латник, ловчий и арбалетчик, справа сорокопут, удильщик и слизь, в конце громила. Свет зала тусклый, чтобы читались свечение и частицы.',
      env: { background: '#0e0c0b', light: 0.28, ambientColor: '#9c8a78', sun: 0.14, sunColor: '#ffd9b0', ground: '#3a332c' },
      camera: [[0, ...wide], [40, ...wide], ...stops, [t + 40, ...wide], [T_END, ...wide]],
      props: [{
        make: (group) => {
          group.add(texturedBox([9, 0.25, 15], [0, -0.25, -3.5], 'dio_wood'));
          group.add(texturedBox([1, 5, 15], [-4, 0, -3.5], 'altar_stone'), texturedBox([1, 5, 15], [4, 0, -3.5], 'altar_stone'));
          group.add(texturedBox([9, 5, 1], [0, 0, -11], 'altar_stone'));
          for (const z of [-3, -6, -9]) {
            const sconce = new T.PointLight(0xffb070, 0.55, 4.5, 1.8);
            sconce.position.set(z === -6 ? 3.3 : -3.3, 3.4, z);
            group.add(sconce);
          }
          const place = (o, x, z) => {
            group.add(texturedBox([1, 1, 1], [x, 0, z], 'altar_stone'));
            o.dio.group.position.set(x, 1, z);
            o.head.group.position.set(x, 2, z);
            group.add(o.dio.group, o.head.group);
          };
          for (const o of prepared) {
            const left = HALL.left.find((r) => r[0] === o.id), right = HALL.right.find((r) => r[0] === o.id);
            if (left) place(o, -3, left[1]);
            else if (right) place(o, 3, right[1]);
            else place(o, 0, HALL.end[1]);
            o.dio.setLight('night');
            o.dio.step(120);
          }
          let last = 0;
          return (now) => {
            const whole = Math.floor(now);
            if (whole > last && whole - last < 60) for (const o of prepared) o.dio.step(whole - last);
            last = whole;
            for (const o of prepared) o.dio.setTime(now);
          };
        },
      }],
      actors: {},
      captions,
      marks: order.map(([id], i) => [stops[i * 2][0], mobName(id).split(/[ -]/)[0].toLowerCase()]),
    };
  }
  function hallPanel(box) {
    const note = el('p', { class: 'sc-intro' }, 'Загрузка диорам…');
    box.append(note);
    const facingOf = (id) => (HALL.left.some((r) => r[0] === id) ? 'east' : HALL.right.some((r) => r[0] === id) ? 'west' : 'south');
    Promise.all(mobs().map((m, i) => Promise.all([SC.trophyDiorama(m.id, facingOf(m.id), i * 11), SC.trophyHead(m.id, 'wall', facingOf(m.id))])
      .then(([dio, head]) => ({ id: m.id, dio, head })))).then((prepared) => {
      note.remove();
      box.append(SC.ui.scenesPanel({}, { scenes: [hallScene(prepared)], ground: 30, legend: false }));
    }).catch((e) => box.append(el('div', { class: 'sc-err' }, String(e && e.stack ? e.stack : e))));
  }

  // ------------------------------------------------------------------ heads
  // One mob's head: on the floor in all 16 rotations (a ring), on the four sides of a pillar.
  function headsPanel(params) {
    const canvas = el('canvas', { 'aria-label': 'Голова моба на полу в 16 поворотах и на стене с четырёх сторон' });
    const errBox = el('div', { class: 'sc-err', hidden: true });
    const mRow = el('div', { class: 'sc-clips', role: 'group', 'aria-label': 'Моб' });
    const icons = el('div', { class: 'sc-icons' });
    const panel = el('div', { class: 'scene' }, el('div', { class: 'sc-stage' }, canvas), errBox, mRow, icons);
    const fail = SC.ui.panelError(errBox);
    const stage = SC.ui.keep(new SC.Stage(canvas, { ground: 11, fov: 28 }));
    let root = null, opened = 0;
    function show(id) {
      const ticket = ++opened;
      const jobs = [];
      for (let r = 0; r < 16; r++) jobs.push(SC.trophyHead(id, 'floor', r));
      for (const f of ['north', 'east', 'south', 'west']) jobs.push(SC.trophyHead(id, 'wall', f));
      Promise.all(jobs).then((heads) => {
        if (ticket !== opened) return;
        if (root) stage.scene.remove(root);
        root = new T.Group();
        heads.slice(0, 16).forEach((h, r) => {
          const a = rad(-22.5 * r) + Math.PI; // rotation r sits where it looks out of the ring
          h.group.position.set(Math.sin(a) * 4, 0, Math.cos(a) * 4);
          root.add(h.group);
        });
        // The pillar: two stone blocks, a head on each side at the upper one.
        root.add(wallBlock([0, 0, 0]), wallBlock([0, 1, 0]));
        const side = { north: [0, 1, -1], east: [1, 1, 0], south: [0, 1, 1], west: [-1, 1, 0] };
        heads.slice(16).forEach((h, i) => {
          const f = ['north', 'east', 'south', 'west'][i];
          h.group.position.set(...side[f]);
          root.add(h.group);
        });
        stage.scene.add(root);
        stage.frame(boxPts([-4.6, 0, -4.6], [4.6, 2, 4.6]), 1.0);
      }).catch((e) => fail(String(e && e.stack ? e.stack : e)));
      icons.textContent = '';
      const it = SC.ITEMS[id + '_head'];
      if (it)
        icons.append(el('img', { class: 'sc-px', src: it.texture, alt: 'Значок предмета: голова', width: 64, height: 64 }),
          el('span', { class: 'small' }, 'Значок предмета ' + 'soulscraft:' + id + '_head: лицевая грань головы с текстуры моба (запасной вариант; лучше — мод рисует голову в слоте, как ваниль).'));
    }
    const first = SC.TROPHIES[params.get('mob')] ? params.get('mob') : 'brute';
    buttons(mRow, mobs().map((m) => [m.id, m.name]), first, show);
    show(first);
    SC.onFrame(() => stage.render(), fail);
    return panel;
  }

  // ------------------------------------------------------------------ tables
  function dropsTable() {
    const d = SC.TROPHY_DATA.drops;
    const pc = (v) => SC.num(Math.round(v * 1000) / 10) + '%';
    const rows = [
      ['Голова', d.head.event, pc(d.head.world) + ' с обычного моба, ' + pc(d.head.world_elite) + ' с элитного'],
      ['Часть брони', pc(d.armor.event) + ' за победу на каждую часть брони мобов ивента', d.armor.world],
      ['Диорама', d.diorama.event, 'нет'],
    ];
    return el('table', { class: 'sc-moves' }, el('thead', {}, el('tr', {}, ['Трофей', 'Ивент', 'Обычный мир'].map((h) => el('th', {}, h)))),
      el('tbody', {}, rows.map((r) => el('tr', {}, r.map((c) => el('td', {}, c))))));
  }
  function exportTable() {
    const head = el('tr', {}, ['Моб', 'Блок', 'Голова', 'Фигурка: масштаб, сдвиг, поворот', 'Свет', 'Частицы'].map((h) => el('th', {}, h)));
    const rows = mobs().map((m) => {
      const t = SC.TROPHIES[m.id], d = t.diorama;
      return el('tr', {}, el('td', {}, m.name), el('td', {}, d.block), el('td', {}, fmt(t.head.floor.scale)),
        el('td', {}, fmt(d.scale) + ' · [' + d.offset.map(fmt).join('; ') + '] · ' + SC.num(d.yaw) + '°' + (d.flags.length ? ' · ' + d.flags.join(', ') : '')),
        el('td', {}, String(d.light)),
        el('td', {}, d.particles.map((e) => e.type.split(':')[1] + ' /' + e.every).join(', ')));
    });
    return el('div', { class: 'wide' }, el('table', { class: 'sc-moves' }, el('thead', {}, head), el('tbody', {}, rows)),
      el('p', { class: 'small sc-cap' }, 'Масштаб — доля размера моба в мире; фигурка вписана в коробку своей сцены на всех тиках клипа trophy, один сдвиг на весь клип. «/6» — частица рождается в среднем раз в 6 тиков.'));
  }
  function particleTypesTable() {
    const head = el('tr', {}, ['Тип', 'Спрайт', 'Скорость, блоков за тик', 'Живёт', 'Размер', 'Прозрачность'].map((h) => el('th', {}, h)));
    const rows = Object.entries(SC.BLOCKS.particles).map(([pid, p]) => {
      const b = p.behaviour;
      return el('tr', {}, el('td', {}, el('b', {}, p.name), ' soulscraft:' + pid),
        el('td', {}, el('img', { class: 'sc-px sc-sprite', src: 'data:image/png;base64,' + p.png, alt: p.name, width: 32, height: 32 })),
        el('td', {}, '[' + b.velocity.map((v) => SC.num(v)).join('; ') + ']' + (b.velocity_jitter ? ' ± [' + b.velocity_jitter.map((v) => SC.num(v)).join('; ') + ']' : '')),
        el('td', {}, b.lifetime + ' тиков'), el('td', {}, SC.num(b.size[0]) + ' → ' + SC.num(b.size[1])), el('td', {}, SC.num(b.alpha[0]) + ' → ' + SC.num(b.alpha[1])));
    });
    return el('div', { class: 'wide' }, el('table', { class: 'sc-moves' }, el('thead', {}, head), el('tbody', {}, rows)),
      el('p', { class: 'small sc-cap' }, 'Свои типы частиц мода (API 103 движка): assets/soulscraft/particles/<тип>.json со спрайтом и horizon:behaviour. Размер — половина ширины в блоках.'));
  }

  // ------------------------------------------------------------------ page
  function page(sub, params) {
    const ui = SC.ui;
    ui.buildNav('trophies');
    const main = ui.main;
    main.append(el('h1', {}, 'Трофеи: диорамы'));
    main.append(el('p', { class: 'sum' }, SC.TROPHY_DATA.diorama.text + ' ' + SC.TROPHY_DATA.diorama.item +
      '. Награда за победу на алтаре 3-го уровня и выше. Ещё трофеи — головы мобов и части их брони.'));
    const box = el('div');
    main.append(box);
    whenReady(box, () => {
      const only = params.get('only');
      const parts = {
        one: () => [el('h2', {}, 'Диорама крупно'),
          el('p', {}, 'Моб стоит в своей трофейной позе и медленно дышит (клип trophy, 4 секунды по кругу). Свет: день, ночь и тёмная пещера — ночью видно, что светится сама диорама: свет блока, светящиеся детали, глаза и приманки. Частицы — туман, искры, снег, пузыри — пускает блок.'),
          dioramaPanel(params)],
        all: () => [el('h2', {}, 'Все семь'), el('p', { class: 'small' }, 'Каждая диорама днём и ночью, с частицами в воздухе. Один масштаб кадра.'), galleryTable()],
        hall: () => {
          const b = el('div');
          hallPanel(b);
          return [el('h2', {}, 'Зал трофеев'), b];
        },
        heads: () => [el('h2', {}, 'Головы'),
          el('p', {}, 'Блок головы на полу поворачивается на 16 направлений (состояние rotation 0..15), на стене — на 4 (facing). Головы — те же кубы и текстура, что у моба; рога громилы и приманка удильщика выходят за ванильные 8 пикселей.'),
          headsPanel(params)],
        drops: () => [el('h2', {}, 'Откуда берутся'), dropsTable(),
          el('p', { class: 'small sc-cap' }, 'Числа — предложения (09-trophies.md). Диорама видна в пределах ' + SC.TROPHY_DATA.diorama.view_blocks +
            ' блоков (ограничение отрисовки block entity), частицы — в пределах ' + SC.TROPHY_DATA.diorama.particles_blocks + '.')],
        data: () => [el('h2', {}, 'Что получает мод'),
          el('p', {}, 'Файл assets/rigs/<моб>.trophy.json (версия 2): голова как части рига и раздел diorama — блок, клип позы, масштаб, сдвиг, поворот, флаги состояния, свет блока, табличка и частицы. Цепочка отрисовки фигурки:'),
          el('pre', { class: 'sc-chain' }, 'translate(0.5, 0, 0.5)\nrotate_about(y, turn)        диорама и голова на стене: -facing; голова на полу: -22,5 × rotation\ntranslate(offset / 16)\nrotate_about(y, yaw)          180 + поворот сцены: фигурка смотрит на табличку\nscale(scale)\nscale(1, -1, -1); translate(0, -1.501, 0)   кадр модели, как у моба\nчасти рига в клипе trophy на тике (game_time + фаза) % ticks + partial'),
          el('p', {}, 'Блок диорамы — обычная модель из элементов: подставка и сцена со светящимися элементами. Фигурку рисует block entity; частицы — его серверный тик (send_particles), в среднем раз в every тиков, в коробке box. Исследование движка — docs/research/trophy-effects.md.'),
          exportTable(), particleTypesTable()],
        rejected: () => [el('h3', {}, 'Отвергнутые варианты'), el('p', { class: 'small' }, SC.TROPHY_DATA.rejected.text)],
      };
      for (const [k, make] of Object.entries(parts)) if (!only || only === k) box.append(...make());
    });
  }
  SC.page('trophies', page);

  // ------------------------------------------------------------------ mob page block
  SC.mobSection((m, def, main) => {
    const t = SC.TROPHIES[m.id];
    if (!t) return;
    const params = new URLSearchParams(location.hash.split('?')[1] || '');
    const armour = m.loot.length
      ? el('ul', {}, m.loot.map((l) => el('li', {}, l.name + ' (' + l.item + '): ' + SC.num(Math.round(l.chance * 1000) / 10) + '% в мире, ' +
        SC.num(SC.TROPHY_DATA.drops.armor.event * 100) + '% за победу в ивенте')))
      : el('p', {}, 'Частей брони у этого моба нет.');
    const icon = SC.ITEMS[m.id + '_head'];
    const box = el('div', { id: 'trophy' });
    main.append(el('h2', {}, 'Трофей: диорама'), box,
      el('div', { class: 'cols' },
        el('div', { class: 'text' }, el('h3', {}, 'Броня'), armour,
          el('p', { class: 'small' }, el('a', { href: '#trophies?mob=' + m.id }, 'Все диорамы и зал трофеев →'))),
        el('aside', {}, el('h3', {}, 'Голова'), icon ? el('img', { class: 'sc-px', src: icon.texture, alt: 'Значок: голова', width: 64, height: 64 }) : null,
          el('p', { class: 'small' }, 'soulscraft:' + m.id + '_head, на стене soulscraft:' + m.id + '_wall_head. Масштаб ' + fmt(t.head.floor.scale) + '.'))));
    whenReady(box, () => box.append(dioramaPanel(params, m.id)));
  });
})();
