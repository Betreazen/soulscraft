'use strict';
// The altar pages: #altar (levels, building with the ghost, the summon, the menu mock),
// #altar/blocks (blocks and items), #banner/<id> (one page per modifier banner).
// Data: SC.ALTAR (data/altar.json), SC.BLOCKS (blocks.js from build_blocks.py), block models
// from blocks_altar.glb and the level glb exported by Blender.
(function () {
  const T = THREE;
  const el = SC.el;
  const COLORS = { ghost: 0xe8f4ff, socket: 0xf0c040, wrong: 0xff3a30, zone: 0x59d3c8, chunk: 0x2f8f88 };
  const KIN = () => SC.ALTAR.kin_names;
  const mods = () => SC.ALTAR.modifiers;
  const modById = (id) => mods().find((m) => m.id === id);
  const levelName = (n) => 'Уровень ' + n;
  // The recipe of a block or item from the recipe files (js/extra.js): «по форме: …» or «без формы: …».
  const recipeLine = (id) => SC.recipeText(id) || 'нет рецепта';

  // ------------------------------------------------------------------ nav
  SC.navSection((active) => ['Алтарь', [
    ['#altar', 'Алтарь призыва', 'altar'],
    ['#altar/blocks', 'Блоки и предметы', 'altar/blocks'],
    ...mods().map((m) => ['#banner/' + m.id, m.name, 'banner/' + m.id]),
  ]]);

  function start(active, title, sum) {
    const ui = SC.ui;
    ui.buildNav(active);
    ui.main.append(el('h1', {}, title));
    if (sum) ui.main.append(el('p', { class: 'sum' }, sum));
    return ui.main;
  }
  // Waits for the block glb, then builds; an error stays inside the page.
  function whenReady(box, build) {
    SC.blocksReady().then(build).catch((e) => {
      const msg = String(e && e.stack ? e.stack : e);
      console.error(msg);
      box.append(el('div', { class: 'sc-err' }, msg));
    });
  }

  // ------------------------------------------------------------------ the altar as placed data
  const struct = (n) => SC.BLOCKS.structures['altar_level_' + n];
  const placedOf = (n, banners) => struct(n).placed.filter((p) => banners !== false || p.kind !== 'banner');
  // Pieces of a structure: placements grouped by `group` (a pillar's two segments are one piece).
  function pieces(placed) {
    const out = new Map();
    for (const p of placed) {
      if (!out.has(p.group)) out.set(p.group, { group: p.group, kind: p.kind, ring: p.ring, block: p.block, parts: [] });
      out.get(p.group).parts.push(p);
    }
    return [...out.values()];
  }
  function pieceBox(piece) {
    const lo = [Infinity, Infinity, Infinity], hi = [-Infinity, -Infinity, -Infinity];
    for (const p of piece.parts) {
      const [a, b] = SC.blockBox(p.model, p.drop);
      for (let k = 0; k < 3; k++) {
        lo[k] = Math.min(lo[k], a[k] + p.at[k]);
        hi[k] = Math.max(hi[k], b[k] + p.at[k]);
      }
    }
    return [lo, hi];
  }
  function pieceGroup(piece, swap) {
    const g = new T.Group();
    for (const p of piece.parts) g.add(SC.placeBlock(swap ? Object.assign({}, p, { model: swap(p.model) }) : p));
    return g;
  }
  // Text of what a ring needs: «16 плит, 2 гнезда, 4 колонны».
  function ringNeeds(n) {
    const ring = pieces(placedOf(n, false)).filter((p) => p.ring === n);
    const count = (k) => ring.filter((p) => p.kind === k).length;
    const lv = SC.ALTAR.levels[n - 1];
    const corner = lv.corner ? SC.BLOCKS.blocks.find((b) => b.id === lv.corner).name.toLowerCase() : null;
    const parts = [];
    if (count('core')) parts.push('ядро и подставка');
    parts.push(count('slab') + ' плит');
    if (count('socket')) parts.push(count('socket') + ' ' + (count('socket') === 1 ? 'гнездо' : 'гнезда'));
    if (corner) parts.push(count('corner') + ' × ' + corner);
    return parts.join(', ');
  }

  // ------------------------------------------------------------------ level viewer
  function levelViewer() {
    const canvas = el('canvas', { 'aria-label': 'Алтарь в сборе; уровень выбирается кнопками ниже' });
    const errBox = el('div', { class: 'sc-err', hidden: true });
    const levels = el('div', { class: 'sc-clips', role: 'group', 'aria-label': 'Уровень алтаря' });
    const opts = el('div', { class: 'sc-clips sc-states', role: 'group', 'aria-label': 'Показ' });
    const note = el('p', { class: 'sc-intro' });
    const panel = el('div', { class: 'scene' }, el('div', { class: 'sc-stage' }, canvas), errBox, levels, opts, note);
    const fail = SC.ui.panelError(errBox);
    const stage = SC.ui.keep(new SC.Stage(canvas, { ground: 13 }));
    let level = 4, banners = true, lit = false, root = null, horn = null, opened = 0;
    const toggle = (label, get, set) => {
      const b = el('button', { type: 'button', 'aria-pressed': String(get()), class: get() ? 'on' : '' }, label);
      b.onclick = () => {
        set(!get());
        b.classList.toggle('on', get());
        b.setAttribute('aria-pressed', String(get()));
        show(level, false);
      };
      return b;
    };
    opts.append(el('span', { class: 'sc-state-label' }, 'Показать:'),
      toggle('Знамёна на гнёздах', () => banners, (v) => (banners = v)),
      toggle('Ивент идёт: жаровни горят, горн на подставке', () => lit, (v) => (lit = v)));
    function show(n, reframe) {
      level = n;
      for (const b of levels.children) b.classList.toggle('on', Number(b.dataset.level) === n);
      const ticket = ++opened;
      SC.loadModel('altar_level_' + n).then((gltf) => {
        if (ticket !== opened) return;
        if (root) stage.scene.remove(root);
        root = new T.Group();
        // The level as Blender exported it; banner and brazier nodes switched by the toggles.
        const model = gltf.scene.clone(true);
        model.traverse((o) => {
          const name = o.userData.name || o.name || '';
          if (o.isMesh) o.castShadow = o.receiveShadow = true;
          if (name.includes(':banner_') && !banners) o.visible = false;
          if (name.includes(':altar_brazier') && lit) o.visible = false;
        });
        root.add(model);
        if (lit) {
          for (const p of struct(n).placed) if (p.block === 'altar_brazier') root.add(SC.placeBlock(Object.assign({}, p, { model: 'altar_brazier_lit' })));
          horn = SC.pedestalItem('rotten_horn');
          const ped = struct(n).placed.find((p) => p.kind === 'pedestal');
          horn.position.set(ped.at[0], ped.at[1], ped.at[2]);
          root.add(horn);
        } else horn = null;
        stage.scene.add(root);
        stage.setEnv(lit ? { background: '#232836', light: 0.55, sun: 0.45, sunColor: '#c8d0ff', ground: '#9aa0aa' } : null);
        if (reframe !== false) {
          const box = new T.Box3().setFromObject(root);
          const pts = [];
          for (let i = 0; i < 8; i++) pts.push(new T.Vector3(i & 1 ? box.max.x : box.min.x, i & 2 ? box.max.y : box.min.y, i & 4 ? box.max.z : box.min.z));
          stage.frame(pts, 1.08);
        }
        const lv = SC.ALTAR.levels[n - 1];
        note.textContent = levelName(n) + ', ' + lv.size + '×' + lv.size + '. Кольцо: ' + ringNeeds(n) + '. Открывает: ' + lv.unlocks.toLowerCase() + '.';
      }).catch((e) => fail(String(e && e.stack ? e.stack : e)));
    }
    for (const lv of SC.ALTAR.levels)
      levels.append(el('button', { type: 'button', 'data-level': lv.level, onclick: () => show(lv.level) }, levelName(lv.level) + ' · ' + lv.size + '×' + lv.size));
    let t = 0;
    SC.onFrame((dt) => {
      t += dt * SC.TICK;
      if (horn) horn.update(t);
      stage.render();
    }, fail);
    show(SC.altarParams.level || level);
    return panel;
  }

  function levelsTable() {
    const head = el('tr', {}, ['Уровень', 'Размер', 'Что достроить', 'Гнёзд всего', 'Учащённый спавн', 'Что открывает'].map((h) => el('th', {}, h)));
    let sockets = 0;
    const rows = SC.ALTAR.levels.map((lv) => {
      sockets += lv.sockets.length;
      return el('tr', {}, el('td', {}, lv.level), el('td', {}, lv.size + '×' + lv.size), el('td', {}, ringNeeds(lv.level)),
        el('td', {}, sockets), el('td', {}, '×' + (1 + SC.ALTAR.spawn_boost.extra_passes[lv.level - 1]) + ' попыток'), el('td', {}, lv.unlocks));
    });
    return el('div', { class: 'wide' }, el('table', { class: 'sc-moves' }, el('thead', {}, head), el('tbody', {}, rows)),
      el('p', { class: 'small sc-cap' }, 'Гнёзда стоят на клетках кольца вместо плит (предложение): уровень 2 — запад и восток, 3 — север и юг, 4 — четыре клетки «вертушкой». Число гнёзд — на весь алтарь этого уровня.'));
  }

  // ------------------------------------------------------------------ scene: building with the ghost
  // Rings 2..4 are built block by block after level 1. While a ring is missing, the core shows
  // its ghost the way the game will draw it: a line box per missing piece with a half-size copy
  // of the block turning in it, sockets as a gold box without a copy; a block that does not
  // belong where it stands gets a red box. Every tick of the schedule comes from the data.
  const STEP = 3, PAUSE = 26;
  function buildSchedule() {
    const all = pieces(placedOf(4, true));
    const events = []; // {piece, at}: the piece is placed at tick `at`
    const ghosts = []; // {piece, from, to}
    const wrong = []; // {piece (a wrong one), from, to}
    const captions = [[0, 'Уровень 1 готов: ядро, подставка и 8 плит. Игрок держит блок алтаря, и ядро показывает, чего не хватает до уровня 2.']];
    const marks = [];
    let t = 30;
    for (let n = 2; n <= 4; n++) {
      const ring = all.filter((p) => p.ring === n && p.kind !== 'banner');
      // Floor first, sweeping round the ring; then the corner pieces.
      const ang = (p) => Math.atan2(p.parts[0].at[2], p.parts[0].at[0]);
      const floor = ring.filter((p) => p.kind === 'slab' || p.kind === 'socket').sort((a, b) => ang(a) - ang(b));
      const corners = ring.filter((p) => p.kind === 'corner').sort((a, b) => ang(a) - ang(b));
      const ghostFrom = t - 24;
      for (const p of floor) {
        events.push({ piece: p, at: t });
        ghosts.push({ piece: p, from: ghostFrom, to: t });
        t += STEP;
      }
      corners.forEach((p, i) => {
        if (n === 3 && i === 1) {
          // A pillar where a brazier belongs: outlined red until it is broken and replaced.
          const bad = { group: 'wrong', kind: 'corner', ring: 3, block: 'altar_pillar', parts: [] };
          const base = p.parts[0];
          ['bottom', 'top'].forEach((part, k) => bad.parts.push({ block: 'altar_pillar', model: 'altar_pillar_' + part, at: [base.at[0], base.at[1] + k, base.at[2]], rot: 0, drop: base.drop }));
          wrong.push({ piece: bad, from: t, to: t + 44 });
          captions.push([t, 'Колонна поставлена туда, где третьему кольцу нужна жаровня: ядро обводит её красным, призрак жаровни остаётся.']);
          marks.push([t, 'ошибка']);
          t += 44;
          captions.push([t, 'Колонну сломали, на её место встала жаровня.']);
        }
        events.push({ piece: p, at: t });
        ghosts.push({ piece: p, from: ghostFrom, to: t });
        t += STEP * 2;
      });
      captions.push([ghostFrom, 'Призрак кольца ' + n + ': контуры недостающих блоков с маленькой вращающейся копией, гнёзда — жёлтым контуром без копии. Уровень ' + n + ': ' + ringNeeds(n) + '.']);
      t += 6;
      captions.push([t, 'Кольцо ' + n + ' на месте: алтарь ' + levelName(n).toLowerCase() + '. Знамёна в гнёзда ставятся по желанию, уровень от них не зависит.']);
      marks.push([t, 'ур. ' + n]);
      for (const b of all.filter((p) => p.ring === n && p.kind === 'banner')) {
        t += 8;
        events.push({ piece: b, at: t });
      }
      t += PAUSE;
    }
    captions.sort((a, b) => a[0] - b[0]);
    return { events, ghosts, wrong, captions, marks, ticks: t + 20 };
  }

  // The ghost of a piece: a line box and, unless it is a socket, a half-size copy turning in it.
  function ghostOf(piece, color, copy) {
    const [lo, hi] = pieceBox(piece);
    const g = new T.Group();
    g.add(SC.lineBox(lo, hi, color));
    if (copy) {
      const c = new T.Group();
      const model = pieceGroup(piece);
      const box = new T.Box3().setFromObject(model);
      const centre = box.getCenter(new T.Vector3());
      model.position.sub(centre);
      c.add(model);
      c.scale.setScalar(0.5);
      c.position.set((lo[0] + hi[0]) / 2, (lo[1] + hi[1]) / 2, (lo[2] + hi[2]) / 2);
      g.add(c);
      g.spin = c;
    }
    return g;
  }

  function buildScene() {
    const s = buildSchedule();
    const level1 = pieces(placedOf(1, true));
    return {
      id: 'build', title: 'Достройка: уровень 1 → 4', ticks: s.ticks,
      intro: 'Как ядро показывает, чего не хватает до следующего уровня. Полупрозрачности у моделей мода нет, поэтому призрак — контур линиями и маленькая копия блока.',
      eye: [8.6, 8.2, 10.2], target: [0, 0.4, 0],
      env: { background: '#5d6370' },
      props: [{
        make: (group) => {
          const items = [];
          for (const p of level1) {
            const g = pieceGroup(p);
            group.add(g);
          }
          for (const e of s.events) {
            const g = pieceGroup(e.piece);
            group.add(g);
            items.push((t) => {
              g.visible = t >= e.at;
              // Drops in from a little above, as a placed block pops in.
              const k = Math.max(0, Math.min(1, (t - e.at) / 2));
              g.position.y = (1 - k) * 0.25;
            });
          }
          for (const gh of s.ghosts) {
            const socket = gh.piece.kind === 'socket';
            const g = ghostOf(gh.piece, socket ? COLORS.socket : COLORS.ghost, !socket);
            group.add(g);
            items.push((t) => {
              g.visible = t >= gh.from && t < gh.to;
              if (g.spin) g.spin.rotation.y = (t * 3 * Math.PI) / 180;
            });
          }
          for (const w of s.wrong) {
            const g = pieceGroup(w.piece);
            const [lo, hi] = pieceBox(w.piece);
            const box = SC.lineBox(lo, hi, COLORS.wrong);
            group.add(g, box);
            items.push((t) => {
              g.visible = box.visible = t >= w.from && t < w.to;
              box.material.color.setHex(Math.floor(t / 4) % 2 ? COLORS.wrong : 0xff8a80);
            });
          }
          return (t) => items.forEach((f) => f(t));
        },
      }],
      actors: {},
      captions: s.captions,
      marks: s.marks,
    };
  }

  // ------------------------------------------------------------------ scene: the Rotting Horn
  // A level 3 altar. The player walks up, puts the horn on the pedestal; the event starts: the
  // horn is spent, the braziers light, the 3x3-chunk zone is outlined for a moment, zombies
  // climb out of the ground and walk on the altar.
  const ZONE = { lo: -24.5, hi: 23.5, chunks: [-8.5, 7.5] }; // the core in the middle of its chunk
  function summonScene() {
    const PLACE = 64, ON = PLACE + 6, START = PLACE + 34, ZONE_ON = START + 14, ZONE_OFF = START + 90, RISE = START + 96;
    const placed = placedOf(3, true);
    const ped = placed.find((p) => p.kind === 'pedestal');
    const spots = [[-7, -6], [8, -4], [6, 8], [-8, 5], [1, -9]];
    const actors = {
      player: {
        model: '_player',
        clips: [[0, 'walk'], [PLACE - 4, 'idle'], [PLACE, 'place'], [PLACE + 20, 'idle'], [START + 2, 'hurt'], [START + 12, 'idle'], [RISE + 30, 'block']],
        pos: [[0, [1.5, 11]], [PLACE - 4, [0, 1.75]]],
        lift: [[0, 0], [48, 0], [50, 0.5]],  // steps onto the slabs at z 3.5
        face: [[0, 'path'], [PLACE - 4, 180], [RISE + 20, 140]],
        turn: 14,
      },
    };
    spots.forEach(([x, z], i) => {
      const r = Math.hypot(x, z);
      const edge = 4.3 / Math.max(Math.abs(x), Math.abs(z)); // just outside the 7 x 7 altar
      const stop = [x * edge, z * edge];
      const t0 = RISE + i * 6, walk = t0 + 20, arrive = walk + Math.round(r * (1 - edge) * 9);
      actors['z' + i] = {
        model: '_zombie',
        clips: [[t0, 'rise', 0, 0], [walk, 'walk'], [arrive, 'swing'], [arrive + 12, 'idle']],
        pos: [[0, [x, z]], [walk, [x, z]], [arrive, stop]],
        lift: [[0, -2], [t0, -2], [t0 + 14, 0]],
        face: [[0, (Math.atan2(-x, -z) * 180) / Math.PI], [walk, 'path'], [arrive, (Math.atan2(-x, -z) * 180) / Math.PI]],
        show: [t0, 100000],
        turn: 10,
      };
    });
    return {
      id: 'summon', title: 'Гнилой горн на подставке', ticks: RISE + 120,
      intro: 'Алтарь 3-го уровня. Игрок кладёт Гнилой горн на подставку — ивент начинается: горн тратится, жаровни загораются, на миг видна зона 3×3 чанка, из земли лезут зомби.',
      env: { background: '#262b38', fog: 0.012, fogColor: '#262b38', light: 0.8, sun: 0.6, sunColor: '#c8d0ff', ground: '#8f969f' },
      camera: [
        [0, [7, 5.5, 15], [0, 0.8, 2]], [PLACE - 6, [-5, 4.6, 6.2], [0, 1.2, 0.4]], [START + 6, [-5, 4.6, 6.2], [0, 1.2, 0.4]],
        [ZONE_ON + 20, [34, 40, 44], [0, 0, 0]], [ZONE_OFF - 10, [34, 40, 44], [0, 0, 0]], [RISE + 10, [11, 8.5, 14], [0, 0.5, 0]],
        [RISE + 120, [10, 7.5, 13], [0, 0.5, 0]],
      ],
      props: [{
        make: (group) => {
          const items = [];
          for (const p of placed) {
            const g = SC.placeBlock(p);
            group.add(g);
            if (p.block === 'altar_brazier') {
              const lit = SC.placeBlock(Object.assign({}, p, { model: 'altar_brazier_lit' }));
              const light = new T.PointLight(0x5fdccf, 0, 7, 1.6);
              light.position.set(p.at[0], p.at[1] + 1, p.at[2]);
              group.add(lit, light);
              items.push((t) => {
                const on = t >= START + 4;
                g.visible = !on;
                lit.visible = on;
                light.intensity = on ? Math.min(1.6, (t - START - 4) / 6) : 0;
              });
            }
          }
          const horn = SC.pedestalItem('rotten_horn');
          horn.position.set(ped.at[0], ped.at[1], ped.at[2]);
          const flash = new T.PointLight(0x9cf5e4, 0, 9, 1.5);
          flash.position.set(ped.at[0], ped.at[1] + 1.2, ped.at[2]);
          group.add(horn, flash);
          items.push((t) => {
            horn.visible = t >= ON && t < START + 14;
            horn.update(t);
            // Spent: it rises half a block and shrinks away.
            const k = Math.max(0, Math.min(1, (t - START) / 14));
            horn.position.y = ped.at[1] + k * 0.5;
            horn.scale.setScalar(1 - k);
            flash.intensity = t >= START && t < START + 20 ? 3 * (1 - (t - START) / 20) : 0;
          });
          // The event zone: 3 x 3 chunks round the altar's chunk, chunk lines inside, posts.
          const zone = new T.Group();
          const mat = new T.LineBasicMaterial({ color: COLORS.zone, fog: false });
          const inner = new T.LineBasicMaterial({ color: COLORS.chunk, fog: false });
          const seg = (pts, m) => zone.add(new T.LineSegments(new T.BufferGeometry().setFromPoints(pts.map((p) => new T.Vector3(...p))), m));
          const { lo, hi, chunks } = ZONE;
          const y = 0.03;
          seg([[lo, y, lo], [hi, y, lo], [hi, y, lo], [hi, y, hi], [hi, y, hi], [lo, y, hi], [lo, y, hi], [lo, y, lo]], mat);
          const grid = [];
          for (const c of chunks) grid.push([c, y, lo], [c, y, hi], [lo, y, c], [hi, y, c]);
          seg(grid, inner);
          const posts = [];
          for (const x of [lo, ...chunks, hi]) for (const z of [lo, hi]) posts.push([x, 0, z], [x, 4, z], [z, 0, x], [z, 4, x]);
          seg(posts, mat);
          group.add(zone);
          items.push((t) => (zone.visible = t >= ZONE_ON && t < ZONE_OFF));
          return (t) => items.forEach((f) => f(t));
        },
      }],
      actors,
      captions: [
        [0, 'Алтарь 3-го уровня стоит под открытым небом. Игрок несёт Гнилой горн.'],
        [PLACE, 'Правый клик горном по подставке: горн ложится на подставку и висит над ней, медленно вращаясь.'],
        [START, 'Уровень алтаря не ниже ступени горна — ивент начинается. Горн тратится.'],
        [START + 6, 'Жаровни третьего кольца загораются душевным огнём.'],
        [ZONE_ON, 'Зона ивента — 3×3 чанка с чанком алтаря в центре. Её контур виден несколько секунд.'],
        [RISE, 'Волна 1: зомби вылезают из земли в зоне и идут к алтарю.'],
      ],
      marks: [[PLACE, 'горн'], [START, 'старт'], [RISE, 'волна']],
    };
  }

  // ------------------------------------------------------------------ menu mock
  // The altar menu as the mod draws it (mod/soulscraft/altar_client.rs, rules/src/view.rs menu_lines):
  // a dark panel with a title, a list of text lines and three buttons under it. The lines below are
  // built the way the server builds them (altar_ui.rs menu_of, rules/src/altar.rs Effects::lines,
  // boost.rs menu_line, event_ui.rs menu_lines); the controls under the mock only set the state.
  const trimNum = (v) => String(Math.round(v * 100) / 100); // the game writes numbers with a dot
  const signed = (v) => (v >= 0 ? '+' : '−') + trimNum(Math.abs(v));
  const BLOCK_WORD = { core: 'ядро', pedestal: 'подставка', slab: 'плита', socket: 'гнездо', altar_pillar: 'колонна', altar_brazier: 'жаровня', altar_obelisk: 'обелиск' };

  // Effects::lines of the banners standing.
  function effectLines(counts) {
    const n = (id) => counts[id] || 0;
    const sum = (key) => mods().reduce((a, m) => a + n(m.id) * (m.effect[key] || 0), 0);
    const out = [];
    if (Math.abs(sum('power')) > 1e-9) out.push('сила врагов P ' + signed(sum('power')));
    if (Math.abs(sum('wave_size')) > 1e-9) out.push('врагов в волне ' + signed(sum('wave_size') * 100) + '%');
    if (Math.abs(sum('rest_seconds')) > 1e-9) out.push('передышка ' + signed(sum('rest_seconds')) + ' с');
    if (Math.abs(sum('altar_health')) > 1e-9) out.push('здоровье алтаря ' + signed(sum('altar_health') * 100) + '%');
    const kins = mods().filter((m) => m.effect.kin && n(m.id));
    if (kins.length) {
      const share = Math.min(1, kins.reduce((a, m) => a + n(m.id) * m.effect.kin_share, 0));
      out.push('род: ' + kins.map((m) => KIN()[m.effect.kin] + (n(m.id) > 1 ? ' ×' + n(m.id) : '')).join(', ') + ' — ' + trimNum(share * 100) + '% волны и чаще в спавне');
    }
    const more = mods().reduce((a, m) => a + (m.reward === 'more' ? n(m.id) : 0), 0);
    const less = mods().reduce((a, m) => a + (m.reward === 'less' ? n(m.id) : 0), 0);
    if (more || less) out.push('трофеи: сложнее ' + more + ', проще ' + less + ' (голов ' + (more >= less ? '+' + (more - less) : '−' + (less - more)) + ')');
    return out;
  }

  // The boost's line (boost.rs menu_line): passes by level, the calm and horde banners scale them,
  // the sector starts start_chunks away and comes one chunk closer every drift_ticks.
  function boostLine(state, level, counts) {
    const b = SC.ALTAR.spawn_boost;
    if (!state.boost) return 'Учащённый спавн выключен';
    const wave = mods().reduce((a, m) => a + (counts[m.id] || 0) * (m.effect.wave_size || 0), 0);
    const passes = level ? Math.max(0, Math.round(b.extra_passes[level - 1] * (1 + wave))) : 0;
    if (!passes) return 'Учащённый спавн: нет (алтарь не достроен или затишье гасит его)';
    const side = 2 * b.sector_radius_chunks + 1;
    const d = b.start_chunks - Math.min(b.start_chunks, Math.floor(state.age / (b.drift_ticks / 1200)));
    const place = d === 0 ? 'вокруг алтаря' : 'в ' + d + ' чанк. от алтаря, на чанк ближе каждые ' + b.drift_ticks / 1200 + ' мин';
    return 'Учащённый спавн: ×' + (1 + passes) + ' попыток в секторе ' + side + '×' + side + ' чанков ' + place;
  }

  // The lines the client draws, top to bottom (view.rs menu_lines).
  function menuLines(state, counts, names) {
    const lv = state.sky ? state.level : 0;
    const out = [['Уровень алтаря: ' + lv]];
    if (!state.sky) out.push(['Над ядром нет неба: уровень не засчитан', 'warn']);
    else if (lv >= 4) out.push(['Алтарь достроен']);
    else {
      out.push(['До уровня ' + (lv + 1) + ':']);
      for (const l of ringList(lv + 1)) out.push(['  ' + l]);
    }
    if (!names.length) out.push(['Знамён нет']);
    else {
      out.push(['Знамёна: ' + names.join(', ')]);
      for (const l of effectLines(counts)) out.push(['  ' + l]);
    }
    out.push(['Учащённый спавн: ' + (state.boost ? 'включён' : 'выключен')]);
    const ev = SC.DATA.events.find((e) => e.id === state.event);
    if (state.running && lv && ev && ev.tier <= lv) {
      const health = Math.round(ev.altar_health * (1 + 0.5 * (counts.resilience || 0)));
      out.push(['Ивент «' + ev.name + '»: волна 2 из ' + ev.waves.length + ' (бой)'],
        ['Здоровье алтаря: ' + Math.round(health * 0.62) + ' / ' + health], ['Участники: Lumen, Sova, Kestrel']);
      return out;
    }
    if (!lv) out.push([state.sky ? 'Ивенты недоступны: достройте первое кольцо' : 'Ивенты недоступны: над ядром крыша']);
    else {
      out.push([SC.ALTAR.levels[lv - 1].unlocks]);
      for (const e of SC.DATA.events) if (e.tier <= lv) out.push(['Ивент «' + e.name + '»: положите ' + SC.itemName(e.item) + ' на подставку']);
    }
    out.push([boostLine(state, lv, counts)]);
    return out;
  }

  // «плита ×16»: what ring n still needs, a line per kind of block, from the placed structure.
  function ringList(n) {
    const placed = pieces(placedOf(n, false)).filter((p) => (n === 1 ? p.ring <= 1 : p.ring === n));
    const out = [], seen = new Map();
    for (const p of placed) {
      const word = BLOCK_WORD[p.kind === 'corner' ? SC.ALTAR.levels[n - 1].corner : p.kind] || p.kind;
      seen.set(word, (seen.get(word) || 0) + 1);
    }
    for (const [w, k] of seen) out.push(w + ' ×' + k);
    return out;
  }

  function menuMock() {
    const state = { level: 3, banners: ['weight', 'weight', 'mercy', 'kin_undead'], boost: true, running: false, sky: true, event: 'grave_night', age: 0 };
    const gui = el('div', { class: 'mc-gui mc-menu', role: 'img', 'aria-label': 'Макет меню алтаря: список строк и три кнопки' });
    const ctl = el('div', { class: 'mc-ctl' });
    const sockets = (lv) => SC.ALTAR.levels.slice(0, lv).reduce((a, l) => a + l.sockets.length, 0);
    function draw() {
      gui.textContent = '';
      const lv = state.sky ? state.level : 0;
      const counts = {}, names = [];
      for (let i = 0; i < sockets(lv); i++) {
        const id = state.banners[i];
        if (!id) continue;
        counts[id] = (counts[id] || 0) + 1;
        names.push(modById(id).name);
      }
      gui.append(el('div', { class: 'mc-title' }, 'Алтарь призыва'),
        ...menuLines(state, counts, names).map(([text, tone]) => el('div', { class: 'mc-line' + (tone === 'warn' ? ' warn' : ''), style: 'padding-left:' + (text.length - text.trimStart().length) * 0.5 + 'em' }, text.trimStart() || ' ')),
        el('div', { class: 'mc-btns' },
          el('span', { class: 'mc-gbtn' + (state.boost ? ' on' : '') }, 'Учащ. спавн: вкл'),
          el('span', { class: 'mc-gbtn' + (state.boost ? '' : ' on') }, 'Учащ. спавн: выкл'),
          el('span', { class: 'mc-gbtn' }, 'Показать призрак')));
    }
    const redraw = (fn) => () => (fn(), draw());
    const lvRow = el('div', { class: 'sc-clips' }, el('span', { class: 'sc-state-label' }, 'Уровень:'));
    for (const lv of SC.ALTAR.levels)
      lvRow.append(el('button', { type: 'button', 'data-level': lv.level, onclick: () => {
        state.level = lv.level;
        for (const b of lvRow.querySelectorAll('button')) b.classList.toggle('on', Number(b.dataset.level) === lv.level);
        draw();
      } }, String(lv.level)));
    for (const b of lvRow.querySelectorAll('button')) b.classList.toggle('on', Number(b.dataset.level) === state.level);
    const flags = el('div', { class: 'sc-clips' });
    for (const [key, label] of [['running', 'Идёт ивент'], ['boost', 'Учащённый спавн включён'], ['sky', 'Над ядром небо']]) {
      const b = el('button', { type: 'button', 'aria-pressed': String(state[key]), class: state[key] ? 'on' : '' }, label);
      b.onclick = () => {
        state[key] = !state[key];
        b.classList.toggle('on', state[key]);
        b.setAttribute('aria-pressed', String(state[key]));
        draw();
      };
      flags.append(b);
    }
    const evSel = el('select', { 'aria-label': 'Ивент, который идёт' }, SC.DATA.events.map((e) => el('option', { value: e.id, selected: e.id === state.event }, e.tier + '. ' + e.name)));
    evSel.onchange = redraw(() => (state.event = evSel.value));
    const ageSel = el('select', { 'aria-label': 'Сколько стоит алтарь' }, [[0, 'алтарь только что построен'], [10, 'стоит 10 минут'], [30, 'стоит 30 минут и больше']]
      .map(([v, t]) => el('option', { value: v, selected: v === state.age }, t)));
    ageSel.onchange = redraw(() => (state.age = Number(ageSel.value)));
    flags.append(evSel, ageSel);
    const pick = el('div', { class: 'sc-clips' }, el('span', { class: 'sc-state-label' }, 'Знамёна в гнёздах (по порядку):'));
    for (let i = 0; i < 8; i++) {
      const sel = el('select', { 'aria-label': 'Гнездо ' + (i + 1) }, el('option', { value: '' }, '—'),
        mods().map((m) => el('option', { value: m.id, selected: state.banners[i] === m.id }, m.short)));
      sel.onchange = () => {
        state.banners[i] = sel.value || null;
        draw();
      };
      pick.append(sel);
    }
    ctl.append(lvRow, flags, pick);
    draw();
    return el('div', { class: 'mc-wrap' }, el('div', { class: 'mc-mock-label' }, 'Макет: меню в игре рисует мод, здесь строки и кнопки в том виде, как их строит сервер'), gui, ctl,
      el('p', { class: 'small sc-cap' }, 'Во время ивента вместо строк о горнах и спавне идут три строки ивента. Над алтарём панель ивента пишет «Волна 2 из ' + SC.DATA.events[1].waves.length + ' · ' + SC.DATA.events[1].name + '».'));
  }

  // ------------------------------------------------------------------ spawn boost
  // data/altar.json spawn_boost (hypotheses of iteration 3): passes by level, the sector, the drift.
  function boostSection() {
    const b = SC.ALTAR.spawn_boost, side = 2 * b.sector_radius_chunks + 1;
    const head = el('tr', {}, ['Уровень', 'Попыток спавна', 'Доп. проходов'].map((h) => el('th', {}, h)));
    const rows = SC.ALTAR.levels.map((lv, i) => el('tr', {}, el('td', {}, lv.level), el('td', {}, '×' + (1 + b.extra_passes[i])), el('td', {}, b.extra_passes[i])));
    return [el('h2', {}, 'Учащённый спавн'),
      el('p', {}, 'Пока алтарь стоит под открытым небом и кнопка в меню включена, ванильный спавн пробует сектор вокруг алтаря чаще: каждый чанк сектора, где спавн и так возможен, проходится ещё несколько раз по ванильным правилам и лимитам. Кроме того, в списке спавна сектора появляются мобы других биомов и мобы SoulsCraft; знамёна рода поднимают вес своего рода.'),
      el('div', { class: 'wide' }, el('table', { class: 'sc-moves' }, el('thead', {}, head), el('tbody', {}, rows),
        el('caption', { class: 'sc-cap' }, 'Знамя орды увеличивает число проходов на 25%, знамя затишья уменьшает на 25%: при нуле проходов спавн не учащается.'))),
      el('table', { class: 'stats' }, el('tbody', {},
        el('tr', {}, el('td', {}, 'Сектор'), el('td', {}, side + '×' + side + ' чанков (радиус ' + b.sector_radius_chunks + ')')),
        el('tr', {}, el('td', {}, 'Где он начинает'), el('td', {}, b.start_chunks + ' чанков от алтаря, в одном из восьми направлений, которое задаёт место ядра')),
        el('tr', {}, el('td', {}, 'Шаг к алтарю'), el('td', {}, 'на чанк ближе каждые ' + b.drift_ticks / 1200 + ' минут: через ' + (b.start_chunks * b.drift_ticks) / 1200 + ' минут сектор уже вокруг алтаря')),
        el('tr', {}, el('td', {}, 'Чужие мобы'), el('td', {}, b.foreign.map((f) => SC.waveName({ type: f.type }) + ' (вес ' + f.weight + ')').join(', '))),
        el('tr', {}, el('td', {}, 'Мобы SoulsCraft'), el('td', {}, 'вес ' + b.soulscraft_weight + ' у каждого, элитных и водных нет')))),
      el('p', { class: 'small sc-cap' }, 'Источник: ' + b.source + '.')];
  }

  // ------------------------------------------------------------------ pages
  SC.altarParams = {};
  function altarPage(params) {
    const main = start('altar', 'Алтарь призыва', 'Мультиблок до 9×9: ядро с подставкой в центре и кольца вокруг. Чем больше колец достроено, тем выше уровень, больше гнёзд для знамён и старше ивенты. Пока алтарь стоит под открытым небом, он учащает спавн врагов вокруг.');
    SC.altarParams.level = Number(params.get('level')) || 0;
    const box = el('div');
    main.append(box);
    whenReady(box, () => {
      // only=levels|build|summon|menu shows that part alone (screenshots, a link to one moment).
      const only = params.get('only');
      const parts = {
        levels: () => [el('h2', {}, 'Уровни'), levelViewer(), levelsTable()],
        build: () => [el('h2', {}, 'Достройка и призрак'), SC.ui.scenesPanel({}, { scenes: [buildScene()], ground: 22, legend: false })],
        summon: () => [el('h2', {}, 'Призыв: Гнилой горн'), SC.ui.scenesPanel({}, { scenes: [summonScene()], ground: 64, legend: false })],
        menu: () => [el('h2', {}, 'Меню алтаря'), el('p', {}, 'Правый клик по ядру без предмета в руке. Список строк и три кнопки; кнопки нажимает любой игрок.'), menuMock()],
        boost: () => boostSection(),
        banners: () => [el('h2', {}, 'Знамёна'), bannerGrid(), el('p', {}, el('a', { href: '#altar/blocks' }, 'Все блоки алтаря и предметы →'))],
      };
      for (const [key, make] of Object.entries(parts)) if (!only || only === key) box.append(...make());
    });
  }

  function bannerGrid() {
    const grid = el('div', { class: 'sc-banners' });
    for (const m of mods()) {
      const it = SC.ITEMS[m.banner];
      grid.append(el('a', { class: 'sc-banner-card gd-glow', href: '#banner/' + m.id },
        it ? el('img', { class: 'sc-px', src: it.texture, alt: '', width: 64, height: 64 }) : null,
        el('b', {}, m.name), el('span', { class: 'small' }, m.text), el('span', { class: 'sc-reward ' + m.reward }, rewardText(m.reward))));
    }
    return grid;
  }
  const rewardText = (r) => ({ more: 'награда больше', less: 'награда меньше', same: 'награда без изменений' })[r];

  // A still picture of a block piece (all segments), framed by its box, three-quarter view.
  let snapStage = null;
  function pieceSnapshot(object) {
    if (!snapStage) {
      const c = document.createElement('canvas');
      c.width = c.height = 240;
      snapStage = new SC.Stage(c, { snapshot: true, controls: false, fov: 30, ground: 0 });
      snapStage.renderer.setSize(240, 240, false);
      snapStage.camera.aspect = 1;
      snapStage.camera.updateProjectionMatrix();
    }
    snapStage.scene.add(object);
    const box = new T.Box3().setFromObject(object);
    const pts = [];
    for (let i = 0; i < 8; i++) pts.push(new T.Vector3(i & 1 ? box.max.x : box.min.x, i & 2 ? box.max.y : box.min.y, i & 4 ? box.max.z : box.min.z));
    snapStage.frame(pts, 1.12);
    snapStage.renderer.render(snapStage.scene, snapStage.camera);
    const url = snapStage.canvas.toDataURL('image/png');
    snapStage.scene.remove(object);
    return url;
  }
  SC.pieceSnapshot = pieceSnapshot; // js/recipes.js draws the blocks of the recipes' results with it

  function blocksPage() {
    const main = start('altar/blocks', 'Блоки алтаря и предметы', 'Модели блоков — ванильные JSON-модели из элементов, по блоку на клетку; колонна и обелиск — несколько блоков друг на друге, как дверь. Рецепты — в разделе «Предметы и рецепты».');
    const box = el('div');
    main.append(box);
    whenReady(box, () => {
      const grid = el('div', { class: 'sc-grid' });
      for (const b of SC.BLOCKS.blocks.filter((x) => x.set === 'altar' && !x.id.startsWith('banner_'))) {
        const state = b.states.lit ? { lit: true } : {};
        const pic = pieceSnapshot(SC.blockPiece(b.id, state, 0));
        const facts = [b.part ? 'высота ' + b.part.length + ' (' + b.part.join(', ') + ')' : 'один блок'];
        if (b.states.lit) facts.push('состояния: не горит, горит');
        if (b.on_slab) facts.push('стоит на плите: модель на 8 пикселей ниже');
        grid.append(el('div', { class: 'sc-card' },
          el('div', { class: 'sc-thumb' }, el('img', { src: pic, alt: 'Блок: ' + b.name })),
          el('div', { class: 'sc-card-body' }, el('b', {}, b.name), el('code', {}, 'soulscraft:' + b.id),
            el('div', { class: 'sc-q' }, b.role), el('div', { class: 'sc-role' }, facts.join(' · ')),
            el('div', { class: 'sc-nums' }, recipeLine(b.id)))));
      }
      box.append(el('h2', {}, 'Блоки'), grid);
      box.append(el('h2', {}, 'Знамёна'), el('p', {}, 'Двенадцать знамён, каждое ставится на гнездо модификатора. Блок высотой 2, смотрит наружу от алтаря. Все собираются из белого знамени и двух осколков души плюс один предмет.'), bannerGrid());
      const items = [
        ['rotten_horn', 'Гнилой горн', 'Горн первой ступени: на подставке алтаря запускает тестовый ивент с зомби. Тратится при старте.', 'Где взять: сундуки подземелий и шахт, редко с ванильных зомби, крафт: ' + recipeLine('rotten_horn')],
        ['hunter_book', 'Книга охотника', 'Справочник в игре: открывает сведения об алтарях и врагах по мере встреч и убийств (08-guide-book.md).', 'Крафт: ' + recipeLine('hunter_book')],
      ];
      const loot = el('div', { class: 'sc-loot' });
      for (const [id, name, text, where] of items) {
        const it = SC.ITEMS[id];
        loot.append(el('div', { class: 'recipe' }, el('div', { class: 'rname' }, name + ' · soulscraft:' + id),
          el('img', { class: 'sc-px', src: it.texture, alt: 'Значок: ' + name, width: 96, height: 96 }), el('p', {}, text), el('p', { class: 'small' }, where)));
      }
      box.append(el('h2', {}, 'Предметы'), loot);
    });
  }

  function bannerPage(id) {
    const m = modById(id);
    if (!m) return altarPage(new URLSearchParams());
    const main = start('banner/' + id, m.name, m.text + '.');
    const box = el('div');
    main.append(box);
    whenReady(box, () => {
      // The cloth as painted: banner_<id>_top over _bottom (16 x 32), cut to the cloth's shape
      // (columns 2..13, rows 3..23, then the stepped point), as altar_banners.py lays it out.
      const canvas = el('canvas', { class: 'sc-px sc-cloth', width: 12, height: 25, 'aria-label': 'Полотнище знамени: ' + m.emblem });
      const g = canvas.getContext('2d');
      const inCloth = (x, y) => (y >= 3 && y <= 23 && x >= 2 && x <= 13) || (y >= 24 && y <= 25 && x >= 4 && x <= 11) || (y >= 26 && y <= 27 && x >= 6 && x <= 9);
      const full = document.createElement('canvas');
      full.width = 16;
      full.height = 32;
      const fg = full.getContext('2d');
      let loaded = 0;
      [['banner_' + id + '_top', 0], ['banner_' + id + '_bottom', 16]].forEach(([tex, y]) => {
        const img = new Image();
        img.onload = () => {
          fg.drawImage(img, 0, y);
          if (++loaded < 2) return;
          for (let yy = 3; yy < 28; yy++)
            for (let xx = 2; xx < 14; xx++) if (inCloth(xx, yy)) g.drawImage(full, xx, yy, 1, 1, xx - 2, yy - 3, 1, 1);
        };
        img.src = SC.BLOCK_TEX[tex];
      });
      // The block, turning.
      const view = el('canvas', { 'aria-label': 'Знамя на гнезде, вращается' });
      const stage = SC.ui.keep(new SC.Stage(view, { controls: false, fov: 30, ground: 0 }));
      const spin = new T.Group();
      const socket = SC.placeBlock({ model: 'altar_socket', at: [0, 0, 0], rot: 0 });
      const piece = SC.blockPiece(m.banner, {}, 0);
      piece.children.forEach((c) => (c.children[0].position.y -= SC.BLOCKS.slab_drop / 16));
      piece.position.y = 1;
      spin.add(socket, piece);
      stage.scene.add(spin);
      const pts = [new T.Vector3(-0.7, 0, -0.7), new T.Vector3(0.7, 2.6, 0.7)];
      stage.frame([pts[0], pts[1], new T.Vector3(-0.7, 2.6, 0.7), new T.Vector3(0.7, 0, -0.7)], 1.15);
      let t = 0;
      SC.onFrame((dt) => {
        t += dt;
        spin.rotation.y = Math.PI + Math.sin(t * 0.6) * 1.1;
        stage.render();
      });
      const opp = m.opposite ? modById(m.opposite) : null;
      const rows = [['Id блока', 'soulscraft:' + m.banner], ['Эмблема', m.emblem], ['Действие', m.text],
        ['Числа', Object.entries(m.effect).map(([k, v]) => k + ' = ' + (typeof v === 'number' ? SC.num(v) : KIN()[v] || v)).join(', ') + ' (гипотеза Г10)'],
        ['Награда', rewardText(m.reward) + (m.reward === 'more' ? ': + 1 голова за победу' : m.reward === 'less' ? ': трофеев меньше' : '')],
        ['Складывается', opp ? 'одинаковые складываются; с «' + opp.name + '» гасят друг друга' : 'одинаковые складываются'],
        ['Где стоит', 'на гнезде модификатора; гнёзд 2 на уровне 2, 4 на уровне 3, 8 на уровне 4'],
        ['Рецепт', recipeLine(m.banner)]];
      box.append(el('div', { class: 'sc-banner-page' },
        el('div', { class: 'sc-banner-pics' }, canvas, el('div', { class: 'sc-item sc-banner-view' }, view),
          el('img', { class: 'sc-px', src: SC.ITEMS[m.banner].texture, alt: 'Значок предмета', width: 64, height: 64 })),
        el('table', { class: 'stats' }, el('tbody', {}, rows.map(([k, v]) => el('tr', {}, el('td', {}, k),
          el('td', {}, k === 'Складывается' && opp ? [v.split('«')[0], el('a', { href: '#banner/' + opp.id }, opp.name), v.split('»')[1]] : v)))))));
      const list = mods();
      const i = list.indexOf(m);
      box.append(el('p', { class: 'sc-pager' },
        el('a', { href: '#banner/' + list[(i + list.length - 1) % list.length].id }, '← ' + list[(i + list.length - 1) % list.length].name),
        el('a', { href: '#altar' }, 'Алтарь'),
        el('a', { href: '#banner/' + list[(i + 1) % list.length].id }, list[(i + 1) % list.length].name + ' →')));
    });
  }

  SC.page('altar', (sub, params) => (sub === 'blocks' ? blocksPage() : altarPage(params)));
  SC.page('banner', (sub) => bannerPage(sub));
})();
