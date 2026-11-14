'use strict';
// Pages of the bestiary: #bestiary (index of all mobs), #mob/<id>, #event; more pages come from
// SC.page() (js/altar.js: #altar, #altar/blocks, #banner/<id>), more nav sections from SC.navSection().
(function () {
  const el = SC.el;
  const main = document.getElementById('main');
  const nav = document.getElementById('nav');
  let stages = [];

  const ENV = { land: 'земля', air: 'воздух', water: 'вода', land_water: 'земля и вода' };
  const pct = (v) => SC.num(Math.round(v * 1000) / 10) + '%';
  const sec = (ticks) => SC.num(Math.round((ticks / SC.TICK) * 100) / 100) + ' с';

  function clear() {
    SC.stopFrames();
    for (const s of stages) s.dispose();
    stages = [];
    main.textContent = '';
  }

  function buildNav(active) {
    // the grip of the nav sheet on phones (ui.js) stays
    for (const c of [...nav.children]) if (!c.classList.contains('gd-grip')) c.remove();
    nav.append(el('h3', {}, 'Бестиарий'));
    const ul = el('ul');
    ul.append(el('li', {}, el('a', { href: '#bestiary', class: active === 'bestiary' ? 'on' : '' }, el('span', { class: 'nm' }, 'Все мобы'))));
    for (const m of SC.DATA.mobs) {
      const ready = !!SC.GLB[m.id];
      ul.append(el('li', {}, el('a', { href: '#mob/' + m.id, class: active === m.id ? 'on' : '' },
        el('span', { class: 'nm' }, m.name), el('span', { class: 'sc-dot' + (ready ? ' ready' : ''), title: ready ? 'модель готова' : 'модель в работе' }))));
    }
    nav.append(ul);
    nav.append(el('h3', {}, 'Ивенты'));
    nav.append(el('ul', {}, el('li', {}, el('a', { href: '#event', class: active === 'event' ? 'on' : '' }, el('span', { class: 'nm' }, 'Тестовый ивент')))));
    for (const section of SC.NAV) {
      const [title, links] = section(active);
      nav.append(el('h3', {}, title));
      nav.append(el('ul', {}, links.map(([href, label, key]) =>
        el('li', {}, el('a', { href, class: active === key ? 'on' : '' }, el('span', { class: 'nm' }, label))))));
    }
  }

  // ------------------------------------------------------------------ index
  function bestiary() {
    buildNav('bestiary');
    main.append(el('h1', {}, 'Бестиарий первого среза'));
    main.append(el('p', { class: 'sum' }, 'Семь мобов, и каждый задаёт игроку свой вопрос. Замах пропорционален урону, после сильной атаки моб открыт. Числа — гипотезы до плейтестов.'));
    const grid = el('div', { class: 'sc-grid' });
    for (const m of SC.DATA.mobs) {
      const ready = !!SC.GLB[m.id];
      const pic = el('div', { class: 'sc-thumb' + (ready ? '' : ' wip') }, ready ? '' : el('span', {}, 'модель в работе'));
      if (ready)
        SC.snapshot(m.id).then((url) => pic.append(el('img', { src: url, alt: 'Модель: ' + m.name })))
          .catch((e) => {
            console.error(e);
            pic.append(el('span', { class: 'sc-err' }, 'Ошибка картинки: ' + String(e)));
          });
      grid.append(el('a', { class: 'sc-card gd-glow', href: '#mob/' + m.id },
        pic,
        el('div', { class: 'sc-card-body' },
          el('b', {}, m.name),
          el('code', {}, m.key),
          el('div', { class: 'sc-role' }, m.role + ' · ' + ENV[m.env] + ' · от P ' + SC.num(m.tier_p)),
          el('div', { class: 'sc-q' }, m.question),
          el('div', { class: 'sc-nums' }, 'здоровье ' + m.stats.health + ' · удар ' + m.stats.damage + ' · броня ' + m.stats.armor))));
    }
    main.append(grid);
  }

  // ------------------------------------------------------------------ mob page
  function statsTable(m) {
    const rows = [['Id', m.key], ['Среда', ENV[m.env]], ['Ступень силы игрока', 'от ' + SC.num(m.tier_p)], ['Где', m.spawn],
      ['Здоровье', m.stats.health], ['Удар', m.stats.damage + (m.damage_note ? ' (' + m.damage_note + ')' : '')], ['Броня', m.stats.armor]];
    if (m.stats.speed != null) rows.push(['Скорость', SC.num(m.stats.speed)]);
    if (m.stats.knockback_resistance != null) rows.push(['Сопротивление отбрасыванию', SC.num(m.stats.knockback_resistance)]);
    if (m.height_px) rows.push(['Рост', m.height_px + ' пикселей']);
    const z = SC.DATA.vanilla_zombie;
    return el('table', { class: 'stats' }, el('tbody', {}, rows.map(([k, v]) => el('tr', {}, el('td', {}, k), el('td', {}, v)))),
      el('caption', { class: 'sc-cap' }, 'Ванильный зомби: здоровье ' + z.health + ', удар ' + z.damage + ', броня ' + z.armor + ', скорость ' + SC.num(z.speed) + '.'));
  }

  // Effects an attack puts on the player (mobs.json attacks[].effects): name, and for the
  // effects SoulsCraft owns, the effect id whose icon the build paints (SC.EFFECT_ICONS).
  const EFFECTS = { bleed: ['кровотечение', 'bleeding'], slowness: ['замедление', null], poison: ['отравление', null] };

  function effectTags(a) {
    return (a.effects || []).map((e) => {
      const [name, own] = EFFECTS[e.id] || [e.id, null];
      const def = own && SC.DATA.effects && SC.DATA.effects[own];
      const icon = def && SC.EFFECT_ICONS[def.id];
      const title = def ? def.id + ': урон ' + SC.num(def.damage) + ' раз в ' + def.period_ticks + ' тиков' : 'ванильный эффект';
      return el('div', { class: 'sc-effect', title },
        icon ? el('img', { src: icon, width: 18, height: 18, alt: '' }) : null, name + ' ' + SC.num(e.seconds) + ' с');
    });
  }

  // One attack in a bottom sheet: its phases as tiles and a strip to scale, damage, effects, what
  // the player sees, and a link that opens the clip in the model window. On a phone the moveset
  // table keeps only the name and damage; the rest of a row lives here.
  function attackSheet(m, a) {
    const timed = a.windup + a.hold + a.hit + a.recover > 0;
    const phases = [['windup', a.windup], ['hold', a.hold], ['hit', a.hit], ['recover', a.recover]].filter(([, v]) => v);
    const total = phases.reduce((n, [, v]) => n + v, 0);
    const body = [];
    if (timed) {
      body.push(el('h4', {}, 'Фазы, тики'),
        el('dl', { class: 'att-phases' }, phases.map(([k, v]) => el('div', { style: '--c:' + SC.PHASES[k].color },
          el('dt', {}, SC.PHASES[k].name), el('dd', {}, v + ' ', el('small', {}, SC.num(Math.round((v / SC.TICK) * 100) / 100) + ' с'))))),
        el('div', { class: 'att-strip', role: 'img', 'aria-label': 'Фазы в масштабе: ' + phases.map(([k, v]) => SC.PHASES[k].name + ' ' + v).join(', ') },
          phases.map(([k, v]) => el('i', { style: '--c:' + SC.PHASES[k].color + ';width:' + (v / total) * 100 + '%' }))));
      if (a.window_note) body.push(el('p', { class: 'small' }, 'Окно игрока: ' + a.window_note));
    }
    body.push(el('h4', {}, 'Урон'), el('p', {}, a.damage ? '× ' + SC.num(a.damage) + ' от удара моба (' + m.stats.damage + ')' + (a.knockback ? ', отбрасывание' : '') : '—'), effectTags(a));
    if (a.tell) body.push(el('h4', {}, 'Что видно'), el('p', {}, a.tell));
    const rig = SC.RIGS[m.id];
    if (a.clip && SC.GLB[m.id] && rig && rig.clips[a.clip])
      body.push(el('a', { class: 'att-go', href: '#mob/' + m.id + '?clip=' + a.clip }, 'Открыть клип в окне модели →'));
    SC.sheet(a.name + ' · ' + m.name, body);
  }

  function movesetTable(m) {
    const head = el('tr', {}, ['Атака', 'Замах', 'Пауза', 'Удар', 'Окно игрока', 'Урон', 'Что видно'].map((h, i) => el('th', { class: 'c' + i }, h)));
    const rows = m.attacks.map((a) => {
      const timed = a.windup + a.hold + a.hit + a.recover > 0;
      const t = (v) => (timed ? v + ' т' : '—');
      return el('tr', {},
        el('td', {}, el('button', { type: 'button', class: 'sc-att', 'aria-haspopup': 'dialog', title: 'Фазы атаки подробно', onclick: () => attackSheet(m, a) }, a.name)), el('td', {}, t(a.windup)), el('td', {}, a.hold ? a.hold + ' т' : '—'), el('td', {}, t(a.hit)),
        el('td', {}, timed ? a.recover + ' т' : '—', a.window_note ? el('div', { class: 'small' }, a.window_note) : null),
        el('td', {}, a.damage ? '× ' + SC.num(a.damage) + (a.knockback ? ', отбрасывание' : '') : '—', effectTags(a)),
        el('td', {}, a.tell));
    });
    return el('div', { class: 'wide' }, el('table', { class: 'sc-moves sc-moveset' }, el('thead', {}, head), el('tbody', {}, rows)),
      el('p', { class: 'small sc-cap' }, 'т — тики, 20 в секунду. Фаза удара (2 тика, у бросков дольше) — наше предположение: в бестиарии её нет.'));
  }

  function placeholder(m) {
    return el('div', { class: 'scene' }, el('div', { class: 'sc-stage wip' }, el('span', {}, 'Модель «' + m.name + '» в работе')),
      el('div', { class: 'caption' }, 'Модель, клипы и сцены появятся, когда в assets/models/mobs/ будет файл ' + m.id + '.py.'));
  }

  // An error inside one panel stays in that panel: a red line under its window, the console too.
  function panelError(box) {
    return (msg) => {
      console.error(msg);
      box.textContent = msg;
      box.hidden = false;
    };
  }

  // The 3D window with clip buttons, state toggles, controls and the phase bar.
  function viewerPanel(m) {
    const rig = SC.RIGS[m.id];
    const canvas = el('canvas', { 'aria-label': 'Модель «' + m.name + '», клип выбирается кнопками ниже' });
    const stageBox = el('div', { class: 'sc-stage' }, canvas);
    const clipsRow = el('div', { class: 'sc-clips', role: 'group', 'aria-label': 'Клипы' });
    const barBox = el('div', { class: 'sc-tl' });
    const readout = el('div', { class: 'sc-readout', 'aria-live': 'off' });
    const play = el('button', { type: 'button', class: 'reset', title: 'Пауза или ход' }, 'Пауза');
    const speeds = [1, 0.5, 0.25];
    let speedIdx = 0;
    const speed = el('button', { type: 'button', class: 'reset', title: 'Скорость' }, '1×');
    const back = el('button', { type: 'button', title: 'Тик назад' }, '−1');
    const fwd = el('button', { type: 'button', title: 'Тик вперёд' }, '+1');
    const link = el('button', { type: 'button', class: 'reset', title: 'Скопировать ссылку на этот клип и тик' }, 'Ссылка');
    const errBox = el('div', { class: 'sc-err', hidden: true });
    const statesRow = el('div', { class: 'sc-clips sc-states', role: 'group', 'aria-label': 'Состояния моба' });
    const panel = el('div', { class: 'scene' }, stageBox, errBox, clipsRow, statesRow, barBox, SC.legend(),
      el('div', { class: 'controls' }, play, speed, back, fwd, link, readout));
    const fail = panelError(errBox);
    const ro = SC.readout(readout);
    const stage = new SC.Stage(canvas, {});
    stages.push(stage);
    let actor = null, clip = 'idle', t = 0, playing = true, bar = null;
    const flags = new Set((params.get('state') || '').split(',').filter(Boolean));
    const ticks = () => (actor ? actor.ticks(clip) : 1);
    // The tick shown: a clip that does not loop holds its last tick through the pause at the end.
    const shownTick = () => Math.max(0, Math.min(ticks(), t));
    const markers = () => (rig && rig.clips[clip] ? rig.clips[clip].markers : {});
    function select(name) {
      clip = name;
      t = 0;
      for (const b of clipsRow.children) b.classList.toggle('on', b.dataset.clip === name);
      barBox.textContent = '';
      const mk = markers();
      const marks = [];
      if (mk.hit != null) marks.push([mk.hit, 'удар']);
      bar = SC.timeline(ticks(), SC.phaseSpans(mk), marks, (v, step) => {
        playing = false;
        play.textContent = 'Ход';
        t = v == null ? Math.max(0, Math.min(ticks(), Math.round(t) + step)) : v;
      });
      barBox.append(bar.el);
    }
    function show() {
      const now = shownTick();
      actor.setState([...flags]);
      actor.pose([[clip, now, 1]]);
      bar.set(now);
      const ph = SC.phaseAt(markers(), now);
      ro.set(Math.floor(now), ticks(), ph, ph ? SC.PHASES[ph].name : '');
    }
    play.onclick = () => {
      playing = !playing;
      play.textContent = playing ? 'Пауза' : 'Ход';
    };
    speed.onclick = () => {
      speedIdx = (speedIdx + 1) % speeds.length;
      speed.textContent = SC.num(speeds[speedIdx]) + '×';
    };
    back.onclick = () => {
      playing = false;
      play.textContent = 'Ход';
      t = Math.max(0, Math.floor(shownTick()) - 1);
    };
    link.onclick = () => {
      const hash = SC.hashWith({ clip, ct: Math.floor(shownTick()), state: flags.size ? [...flags].join(',') : null, scene: null, t: null, only: null });
      SC.copyLink(hash, m.name + ': ' + SC.clipName(m, clip) + ', тик ' + Math.floor(shownTick()));
    };
    fwd.onclick = () => {
      playing = false;
      play.textContent = 'Ход';
      t = Math.min(ticks(), Math.floor(shownTick()) + 1);
    };
    SC.actor(m.id).then((a) => {
      actor = stage.add(a);
      a.root.scale.setScalar(SC.viewSize(m.id));
      // Frame the model by its own bounds: the idle stance and all of the clip on show, again
      // when the canvas gets its real size.
      const idle = a.bounds('idle'), seen = {};
      const reframe = () => stage.frame(idle.concat(seen[clip] || (seen[clip] = a.bounds(clip))));
      stage.onResize = reframe;
      for (const flag of new Set(Object.values((rig && rig.states) || {}).map((f) => f.replace(/^!/, '')))) {
        const b = el('button', { type: 'button', 'aria-pressed': String(flags.has(flag)), class: flags.has(flag) ? 'on' : '' },
          SC.STATE_NAMES[flag] || flag);
        b.onclick = () => {
          if (flags.has(flag)) flags.delete(flag);
          else flags.add(flag);
          b.classList.toggle('on', flags.has(flag));
          b.setAttribute('aria-pressed', String(flags.has(flag)));
        };
        statesRow.append(el('span', { class: 'sc-state-label' }, 'Состояние:'), b);
      }
      if (!statesRow.children.length) statesRow.hidden = true;
      const order = rig ? Object.keys(rig.clips) : Object.keys(a.clips);
      for (const name of order) {
        clipsRow.append(el('button', { type: 'button', 'data-clip': name, onclick: () => (select(name), reframe()) }, SC.clipName(m, name)));
      }
      select(order.includes(params.get('clip')) ? params.get('clip') : order.includes('idle') ? 'idle' : order[0]);
      reframe();
      if (params.has('ct')) {
        t = Number(params.get('ct'));
        playing = false;
        play.textContent = 'Ход';
      }
      SC.onFrame((dt) => {
        if (playing) {
          t += dt * SC.TICK * speeds[speedIdx];
          // Attacks pause on their last tick for half a second before they repeat.
          const n = ticks(), loop = actor.loops(clip);
          if (loop) t %= n;
          else if (t > n + 10) t = 0;
        }
        show();
        stage.render();
      }, fail);
    }).catch((e) => fail(String(e && e.stack ? e.stack : e)));
    return panel;
  }

  // Behaviour scenes of a mob script (or of any page: m may be {} and def {scenes, ground, legend}).
  // def.ground: size of the stage's ground in blocks (14 by default); def.legend === false hides
  // the attack phase legend. A scene with `camera` keys moves the camera itself (SC.cameraAt).
  function scenesPanel(m, def) {
    const tabs = el('div', { class: 'sc-clips', role: 'tablist', 'aria-label': 'Сцены' });
    const canvas = el('canvas', { 'aria-label': 'Сцена поведения; что происходит, написано под ней' });
    const caption = el('div', { class: 'caption', 'aria-live': 'polite' });
    const barBox = el('div', { class: 'sc-tl' });
    const play = el('button', { type: 'button', class: 'reset' }, 'Пауза');
    const restart = el('button', { type: 'button', class: 'reset' }, 'Сначала');
    const slow = el('button', { type: 'button', class: 'reset', title: 'Скорость' }, '1×');
    const link = el('button', { type: 'button', class: 'reset', title: 'Скопировать ссылку на эту сцену и тик' }, 'Ссылка');
    const readout = el('div', { class: 'sc-readout' });
    const ro = SC.readout(readout);
    const intro = el('p', { class: 'sc-intro' });
    const errBox = el('div', { class: 'sc-err', hidden: true });
    const panel = el('div', { class: 'scene' }, tabs, intro, el('div', { class: 'sc-stage' }, canvas), errBox, caption, barBox,
      def.legend === false ? null : SC.legend(), el('div', { class: 'controls' }, play, restart, slow, link, readout));
    const fail = panelError(errBox);
    const stage = new SC.Stage(canvas, { eye: [5.2, 4.2, 6.4], target: [0, 0.6, -1.2], ground: def.ground || 14 });
    stages.push(stage);
    // clock runs on past the end of the scene for a second and a half, then starts over.
    let seq = null, playing = true, speed = 1, bar = null, stop = null, clock = 0, opened = 0, current = null;
    link.onclick = () => {
      if (!current) return;
      const tick = Math.floor(Math.min(clock, current.ticks));
      SC.copyLink(SC.hashWith({ scene: current.id, t: tick, clip: null, ct: null }), current.title + ', тик ' + tick);
    };
    play.onclick = () => {
      playing = !playing;
      play.textContent = playing ? 'Пауза' : 'Ход';
    };
    restart.onclick = () => (clock = 0);
    slow.onclick = () => {
      speed = speed === 1 ? 0.5 : speed === 0.5 ? 0.25 : 1;
      slow.textContent = SC.num(speed) + '×';
    };
    function open(scene) {
      current = scene;
      for (const b of tabs.children) b.classList.toggle('on', b.dataset.id === scene.id);
      intro.textContent = scene.intro || '';
      errBox.hidden = true;
      if (stop) stop();
      if (seq) seq.dispose();
      seq = null;
      if (scene.eye) stage.camera.position.set(...scene.eye);
      if (scene.target) stage.controls.target.set(...scene.target);
      stage.controls.enabled = !scene.camera; // a scene that flies its camera owns it
      stage.controls.update();
      const ticket = ++opened; // a slower load of a scene left behind must not show up
      SC.playScene(stage, scene).then((s) => {
        if (ticket !== opened) return s.dispose();
        seq = s;
        barBox.textContent = '';
        bar = SC.timeline(scene.ticks, SC.scenePhases(scene), scene.marks, (v, step) => {
          playing = false;
          play.textContent = 'Ход';
          clock = Math.max(0, Math.min(scene.ticks, v == null ? Math.round(clock) + step : v));
        });
        barBox.append(bar.el);
        clock = 0;
        if (params.get('scene') === scene.id && params.has('t')) {
          clock = Number(params.get('t'));
          playing = false;
          play.textContent = 'Ход';
        }
        stop = SC.onFrame((dt) => {
          if (playing) {
            clock += dt * SC.TICK * speed;
            if (clock > scene.ticks + 30) clock = 0;
          }
          const now = Math.min(clock, scene.ticks);
          seq.apply(now);
          if (scene.camera) {
            const cam = SC.cameraAt(scene, now);
            stage.camera.position.set(...cam.eye);
            stage.controls.target.set(...cam.target);
            stage.controls.update();
          }
          bar.set(now);
          let text = '';
          for (const [ct, c] of scene.captions) if (ct <= now) text = c;
          caption.textContent = text;
          const ph = SC.scenePhases(scene).find((p) => now >= p.from && now < p.to);
          ro.set(Math.floor(now), scene.ticks, ph ? ph.phase : null, ph ? SC.clipName(m, ph.clip) + ': ' + SC.PHASES[ph.phase].name.toLowerCase() : '');
          if (def.onTick) def.onTick(scene, now);
          stage.render();
        }, fail);
      }).catch((e) => fail(String(e && e.stack ? e.stack : e)));
    }
    for (const s of def.scenes) tabs.append(el('button', { type: 'button', role: 'tab', 'data-id': s.id, onclick: () => open(s) }, s.title));
    open(def.scenes.find((s) => s.id === params.get('scene')) || def.scenes[0]);
    return panel;
  }

  function itemStage(id) {
    // Pages are built after SC.itemsReady(), so the item texture is decoded already.
    const canvas = el('canvas', { 'aria-label': 'Модель предмета' });
    const box = el('div', { class: 'sc-item' }, canvas);
    const stage = new SC.Stage(canvas, { controls: false, eye: [0, 0.2, 3.0], target: [0, 0, 0], ground: 0, fov: 30 });
    stages.push(stage);
    const mesh = SC.itemMesh(id);
    mesh.position.set(-0.5, -0.5, -0.5);
    const spin = new THREE.Group();
    spin.add(mesh);
    stage.scene.add(spin);
    SC.onFrame((dt) => {
      spin.rotation.y += dt * 0.8;
      spin.rotation.x = 0.35;
      stage.render();
    });
    return box;
  }

  function lootPanel(m, def) {
    if (!m.loot.length) return el('p', {}, 'Добычи-декорации нет.');
    const wrap = el('div', { class: 'sc-loot' });
    for (const l of m.loot) {
      const id = l.item.split(':')[1];
      const it = SC.ITEMS[id];
      const card = el('div', { class: 'recipe' }, el('div', { class: 'rname' }, l.name + ' · ' + pct(l.chance)));
      if (!it) card.append(el('p', { class: 'small' }, 'Модель в работе.'));
      else if (it.model.elements) card.append(itemStage(id), el('p', { class: 'small' }, 'Модель предмета ' + l.item + ' любой формы: на голове игрока и стойки рисуется по display.head.'));
      else card.append(el('img', { class: 'sc-px', src: it.texture, alt: 'Значок: ' + l.name, width: 64, height: 64 }));
      const set = def && def.armor && def.armor[id];
      if (set && SC.EQUIP[set])
        card.append(el('img', { class: 'sc-px sc-equip', src: SC.EQUIP[set], alt: 'Текстура брони 64×32', width: 256, height: 128 }),
          el('p', { class: 'small' }, 'Ванильная форма: текстура 64×32 слоя humanoid, assets/soulscraft/equipment/' + set + '.json.'));
      wrap.append(card);
    }
    return wrap;
  }

  function mobPage(id) {
    const m = SC.mobById(id);
    if (!m) return bestiary();
    buildNav(id);
    const def = SC.MOBS[id];
    if (SC.MOB_ERRORS[id]) main.append(el('div', { class: 'sc-err' }, 'Скрипт js/mobs/' + id + '.js упал: ' + SC.MOB_ERRORS[id]));
    if (params.get('only') === 'scenes' && def && def.scenes && SC.GLB[id]) return main.append(scenesPanel(m, def));
    main.append(el('div', { class: 'head' }, el('div', {}, el('h1', {}, m.name), el('code', {}, m.key))));
    main.append(el('p', { class: 'sum' }, m.question));
    main.append(el('h2', {}, 'Модель и клипы'));
    main.append(SC.GLB[id] ? viewerPanel(m) : placeholder(m));
    main.append(el('div', { class: 'cols' },
      el('div', { class: 'text' }, el('h2', {}, 'Образ'), el('p', {}, m.look), el('h2', {}, 'Поведение'),
        el('ul', {}, m.behavior.map((b) => el('li', {}, b))), def && def.notes ? def.notes.map((n) => el('p', {}, n)) : null),
      el('aside', {}, el('h2', {}, 'Числа'), statsTable(m))));
    main.append(el('h2', {}, 'Мувсет'), movesetTable(m));
    if (m.moves && m.moves.length)
      main.append(el('table', { class: 'sc-moves' }, el('tbody', {}, m.moves.map((mv) => el('tr', {}, el('td', {}, mv.name), el('td', {}, mv.note))))));
    if (def && def.scenes && SC.GLB[id]) {
      main.append(el('h2', {}, 'Сцены поведения'));
      main.append(scenesPanel(m, def));
    }
    main.append(el('h2', {}, 'Добыча-декорация'), lootPanel(m, def));
    for (const section of SC.MOB_SECTIONS) {
      try {
        section(m, def, main);
      } catch (e) {
        main.append(el('div', { class: 'sc-err' }, String(e && e.stack ? e.stack : e)));
      }
    }
  }

  function eventPage() {
    buildNav('event');
    for (const ev of SC.DATA.events) {
      main.append(el('h1', {}, ev.name));
      main.append(el('p', { class: 'sum' }, 'Передышка ' + ev.rest_seconds.join('–') + ' с. Здоровье алтаря ' + ev.altar_health + '. Цель: ' + ev.target + '.'));
      main.append(el('table', { class: 'sc-moves' },
        el('thead', {}, el('tr', {}, el('th', {}, 'Волна'), el('th', {}, 'Мобы'))),
        el('tbody', {}, ev.waves.map((w) => el('tr', {}, el('td', {}, w.wave),
          el('td', {}, w.mobs.map((x) => (x.name || 'зомби') + ' × ' + x.count + (x.armored ? ', из них в броне ' + x.armored : '') + (x.health_mult ? ', здоровье × ' + x.health_mult : '')).join('; ')))))));
      main.append(el('p', { class: 'small sc-cap' }, 'Источник: ' + ev.source + '.'));
    }
  }

  // #mob/<id>?clip=<name>&ct=<tick>&scene=<id>&t=<tick> opens a clip or a scene paused there;
  // only=scenes shows the scene panel alone (screenshots, a link to one moment).
  let params = new URLSearchParams();
  function route() {
    clear();
    const [h, q] = location.hash.replace(/^#/, '').split('?');
    params = new URLSearchParams(q || '');
    const page = SC.PAGES[h.split('/')[0]];
    if (page) page(h.split('/').slice(1).join('/'), params);
    else if (h.startsWith('mob/')) mobPage(h.slice(4));
    else if (h === 'event') eventPage();
    else bestiary();
    window.scrollTo(0, 0);
  }

  // What extra pages (js/altar.js) share with the bestiary pages.
  SC.ui = {
    main, buildNav, scenesPanel, panelError, itemStage,
    keep: (stage) => (stages.push(stage), stage), // disposed when the page changes
  };

  if (!SC.DATA) SC.fail('Нет данных: js/data.js не загрузился. Пересоберите модели (assets/models/README.md).');
  else
    SC.itemsReady().then(() => {
      window.addEventListener('hashchange', route);
      route();
    });
})();
