'use strict';
// SoulsCraft bestiary showcase: the registry every other script fills in.
// Generated scripts (glb.js, data.js, rigs.js, items.js, mobs/list.js) call SC.glb, SC.data,
// SC.rig, SC.item, SC.equipment and SC.mobScripts; a mob script js/mobs/<id>.js calls SC.mob.
(function () {
  const SC = (window.SC = {
    DATA: null, // data/mobs.json
    GLB: {}, // model id -> base64 glb
    RIGS: {}, // mob id -> {sockets, texture, clips: {name: {ticks, loop, markers}}}
    ITEMS: {}, // item id -> {model: item model JSON, texture: data URL, display: its (inherited) display}
    EQUIP: {}, // armour set -> data URL of the 64x32 texture
    MOBS: {}, // mob id -> what its mob script registered (scenes, notes)
    ALTAR: null, // data/altar.json
    BLOCKS: null, // blocks.js: {blocks: [...], models: {id: {glb, bounds, glow}}, structures: {id: {name, placed}}}
    BLOCK_TEX: {}, // block texture id -> data URL
    EFFECT_ICONS: {}, // 'soulscraft:<effect>' -> data URL of its 18x18 mob effect icon
    TROPHIES: {}, // mob id -> assets/rigs/<mob>.trophy.json (head, diorama)
    TROPHY_DATA: null, // data/trophies.json: the diorama, the rejected variants, drops
    BOOK: null, // data/book.json: stages of the hunter's book
  });
  SC.TICK = 20;

  SC.data = (d) => (SC.DATA = d);
  SC.glb = (id, b64) => (SC.GLB[id] = b64);
  SC.rig = (id, r) => (SC.RIGS[id] = r);
  SC.item = (id, it) => (SC.ITEMS[id] = it);
  SC.equipment = (id, png) => (SC.EQUIP[id] = png);
  SC.altarData = (d) => (SC.ALTAR = d);
  SC.blocks = (d) => (SC.BLOCKS = d);
  SC.blockTexture = (id, png) => (SC.BLOCK_TEX[id] = png);
  SC.effectIcon = (id, png) => (SC.EFFECT_ICONS[id] = png);
  SC.trophy = (id, t) => (SC.TROPHIES[id] = t);
  SC.trophyData = (d) => (SC.TROPHY_DATA = d);
  SC.bookData = (d) => (SC.BOOK = d);
  // Extra blocks at the end of a mob page (js/trophies.js: its trophies): fn(mob, def, main).
  SC.MOB_SECTIONS = [];
  SC.mobSection = (fn) => SC.MOB_SECTIONS.push(fn);
  // Extra pages and nav sections (js/altar.js): SC.page(prefix, render) handles #prefix and
  // #prefix/..., SC.navSection(fn(active) -> [title, [[href, label, key], ...]]) adds to the nav.
  SC.PAGES = {};
  SC.NAV = [];
  SC.page = (prefix, render) => (SC.PAGES[prefix] = render);
  SC.navSection = (fn) => SC.NAV.push(fn);
  // A mob script that throws while it registers breaks only its own page: the error is kept and
  // shown there (app.js), the other pages do not see it.
  SC.MOB_ERRORS = {};
  SC.mob = (id, def) => (SC.MOBS[id] = def);
  SC.mobScript = (id, fn) => {
    try {
      fn();
    } catch (e) {
      SC.MOB_ERRORS[id] = String(e && e.stack ? e.stack : e);
      console.error(SC.MOB_ERRORS[id]);
    }
  };
  // mobs/list.js names the mob scripts; they load right here, in order, before app.js runs.
  SC.mobScripts = (list) => {
    for (const name of list) document.write('<script src="js/mobs/' + name + '.js"><\/script>');
  };

  SC.mobById = (id) => SC.DATA.mobs.find((m) => m.id === id);

  // Phases of an attack on its timeline, as the build writes them from data/mobs.json. Colours are
  // CSS tokens (soulscraft.css, --phase-*), for inline styles only.
  SC.PHASES = {
    windup: { name: 'Замах', color: 'var(--phase-windup)' },
    hold: { name: 'Пауза', color: 'var(--phase-hold)' },
    hit: { name: 'Удар', color: 'var(--phase-hit)' },
    recover: { name: 'Окно игрока', color: 'var(--phase-recover)' },
  };
  SC.phaseSpans = (m) => {
    if (!m || m.hit == null) return [];
    const spans = [{ phase: 'windup', from: m.windup, to: m.hold != null ? m.hold : m.hit }];
    if (m.hold != null) spans.push({ phase: 'hold', from: m.hold, to: m.hit });
    spans.push({ phase: 'hit', from: m.hit, to: m.recover });
    spans.push({ phase: 'recover', from: m.recover, to: m.end });
    return spans;
  };
  SC.phaseAt = (m, t) => {
    const s = SC.phaseSpans(m).find((p) => t >= p.from && t < p.to);
    return s ? s.phase : null;
  };

  // Names of clips for buttons, every clip of every model; mob data (attacks, moves) wins over these.
  SC.CLIP_NAMES = {
    idle: 'Покой', walk: 'Шаг', run: 'Бег', hurt: 'Урон', death: 'Смерть',
    block: 'Щит поднят', circle_strafe: 'Кружит', backstep: 'Отступ',
    dodge: 'Уворот', knocked: 'Отброшен', swing: 'Удар', jump: 'Прыжок',
    fly: 'Полёт', glide: 'Парит', circle: 'Кружит влево', circle_r: 'Кружит вправо', dive: 'Пике, крылья сложены',
    climb: 'Набор высоты после пике',
    hop: 'Прыжок-шаг', jump_air: 'В полёте', stick: 'Прилипание',
    swim: 'Плывёт', drift: 'Подкрадывается', release: 'Отпускает', retreat: 'Уходит',
    aim: 'Целится', crouch: 'Присед', charge_stop: 'Таран без стены', rage: 'Рёв ярости',
    wall: 'Стена', wall_hit: 'Удар в стену', wave: 'Волна', trophy: 'Поза трофея',
  };
  // Mob state flags of rig.json parts (part(..., state=...)), for the toggles of the model window.
  SC.STATE_NAMES = { rage: 'Ярость' };
  SC.clipName = (mob, clip) => {
    const a = mob && [...(mob.attacks || []), ...(mob.moves || [])].find((x) => x.clip === clip);
    return a ? a.name : SC.CLIP_NAMES[clip] || clip;
  };

  SC.el = (tag, attrs, ...kids) => {
    const e = document.createElement(tag);
    for (const [k, v] of Object.entries(attrs || {})) {
      if (k === 'class') e.className = v;
      else if (k === 'style') e.style.cssText = v;
      else if (k.startsWith('on')) e.addEventListener(k.slice(2), v);
      else if (v != null && v !== false) e.setAttribute(k, v === true ? '' : v);
    }
    for (const kid of kids.flat()) if (kid != null && kid !== false) e.append(kid.nodeType ? kid : String(kid));
    return e;
  };
  SC.num = (v) => String(v).replace('.', ',');
  SC.fail = (msg) => {
    const err = document.getElementById('err');
    if (err) err.textContent = msg;
    console.error(msg);
  };
})();
