'use strict';
// #book: a mock of the hunter's book (docs/design/08-guide-book.md) in a vanilla-book style.
// The game window is drawn by the mod; this page lays the lines out the way rules/src/book_page.rs makes
// them (three sections: Враги with SoulsCraft's mobs and the 11 vanilla groups, Алтарь with «Оборона
// алтаря», Ивенты) from the same data: data/mobs.json (SC.DATA), data/book.json (SC.BOOK, stages and
// thresholds), data/altar.json (SC.ALTAR), data/trophies.json (SC.TROPHY_DATA), the mob's glb and rig.
(function () {
  const T = THREE;
  const el = SC.el;
  const B = () => SC.BOOK;
  const isElite = (m) => !!m.elite;
  const killsFor = (m, s) => (isElite(m) ? s.elite_kills : s.kills);
  const pct = (v) => SC.num(Math.round(v * 1000) / 10) + '%';

  // ------------------------------------------------------------------ the model on the page
  // A stage on paper: the mob in idle, a silhouette below stage 1, attack clips from stage 3.
  function modelView(fail) {
    const canvas = el('canvas', { class: 'bk-canvas', 'aria-label': 'Модель моба на странице книги' });
    const stage = SC.ui.keep(new SC.Stage(canvas, { ground: 0, fov: 30, controls: false }));
    stage.renderer.setClearColor(new T.Color('#e8dbb7'), 1);
    const shadow = new T.MeshBasicMaterial({ color: '#140d07' }); // ink: the page shows a silhouette
    let actor = null, clip = 'idle', t = 0, mobId = null, silhouette = false, playing = true, opened = 0;
    const view = {
      canvas,
      onTick: null,
      set(m, opts) {
        silhouette = !!opts.silhouette;
        stage.scene.overrideMaterial = silhouette ? shadow : null;
        if (m.id === mobId) return Promise.resolve();
        const ticket = ++opened;
        return SC.actor(m.id).then((a) => {
          if (ticket !== opened) return;
          if (actor) stage.scene.remove(actor.root);
          actor = a;
          mobId = m.id;
          stage.add(a);
          a.root.scale.setScalar(SC.viewSize(m.id));
          a.root.rotation.y = -0.5;
          const pts = a.bounds('idle');
          stage.frame(pts, 1.15); // the clip stays what the page picked
        });
      },
      play(name, tick) {
        clip = name;
        t = tick || 0;
        playing = tick == null;
      },
      seek(v) {
        t = v;
        playing = false;
      },
      resume() {
        playing = true;
      },
      clip: () => clip,
      tick: () => t,
      ticks: () => (actor ? actor.ticks(clip) : 1),
    };
    SC.onFrame((dt) => {
      if (!actor) return;
      if (playing) {
        t += dt * SC.TICK;
        const n = actor.ticks(clip);
        if (actor.loops(clip)) t %= n;
        else if (t > n + 10) t = 0;
      }
      const shown = Math.max(0, Math.min(actor.ticks(clip), t));
      actor.pose([[clip, shown, 1]]);
      if (view.onTick) view.onTick(shown);
      stage.render();
    }, fail);
    return view;
  }


  // ------------------------------------------------------------------ lines, as book_page.rs makes them
  // Page lines are [tone, text]: 'head' (a heading), 'text', 'dim' (a hint or a locked line).
  const nm = (v) => SC.num(Math.round(v * 100) / 100);
  const lineEl = ([tone, text]) => (tone === 'head' ? el('div', { class: 'bk-h' }, text) : el('p', { class: tone === 'dim' ? 'bk-dim' : '' }, text));
  const scaled = (st, p) => ({ health: st.health * (1 + 0.9 * p), damage: st.damage * (1 + 0.6 * p), armor: st.armor + 6 * p });
  const statLine = (a) => 'здоровье ' + nm(a.health) + ' · урон ' + nm(a.damage) + ' · броня ' + nm(a.armor);
  const growLine = (a, b) => 'здоровье ' + nm(a.health) + ' → ' + nm(b.health) + ', урон ' + nm(a.damage) + ' → ' + nm(b.damage) + ', броня ' + nm(a.armor) + ' → ' + nm(b.armor);
  // A cube's own numbers at a size (book_page.rs cube_stats): health size², damage size (magma: +2), armour size × 3 for magma.
  const cube = (key, size) => ({ health: size * size, damage: size + (key === 'minecraft:magma_cube' ? 2 : 0), armor: key === 'minecraft:magma_cube' ? size * 3 : 0 });

  // The entries of the «Враги» section: SoulsCraft's mobs, then the changed vanilla groups.
  function entries() {
    const own = SC.DATA.mobs.map((m) => ({ id: m.id, name: m.name, elite: !!m.elite, vanilla: false, keys: [m.key, m.id], m }));
    const van = SC.DATA.vanilla.groups.map((g) => ({ id: g.id, name: g.name, elite: false, vanilla: true, keys: g.types, m: g }));
    return own.concat(van);
  }
  const typeNames = (e) => {
    const parts = e.name.split(', ');
    return e.keys.map((k, i) => {
      const n = parts.length === e.keys.length ? parts[i] : k.replace('minecraft:', '');
      return n.charAt(0).toUpperCase() + n.slice(1);
    });
  };

  // A changed vanilla mob's numbers, one line a type: the zombie row from the data, cubes by their sizes 4 and 2.
  function vanillaRows(e, grow) {
    const out = [];
    const z = SC.DATA.vanilla_zombie;
    e.keys.forEach((k, i) => {
      const forms = [];
      if (e.m.behavior === 'cube') for (const size of [4, 2]) forms.push([typeNames(e)[i] + ', размер ' + size, cube(k, size)]);
      else if (e.id === 'zombie' && k === 'minecraft:zombie') forms.push([typeNames(e)[i], { health: z.health, damage: z.damage, armor: z.armor }]);
      for (const [label, st] of forms) out.push(label + ': ' + (grow ? growLine(scaled(st, 0), scaled(st, 1)) : statLine(scaled(st, 0))));
    });
    return out;
  }

  const eventLinks = (e) => SC.DATA.events
    .map((ev) => [ev, ev.waves.filter((w) => w.mobs.some((x) => e.keys.includes(x.type))).map((w) => w.wave)])
    .filter(([, w]) => w.length).map(([ev, w]) => ev.name + ': волны ' + w.join(', '));

  // mob_page: the lines of an enemy's page at a stage (0 unknown .. 4 expert) and its kills.
  function mobLines(e, stage, kills) {
    if (stage === 0) return [['dim', '???'], ['dim', 'Встретьте этого врага, чтобы узнать о нём.']];
    const st = B().stages, m = e.m;
    const lines = [['dim', 'Ступень: ' + st[stage].name + ' · убито: ' + kills]];
    if (stage < 4) lines.push(['dim', 'До ступени «' + st[stage + 1].name + '»: ещё ' + Math.max(0, killsFor(e, st[stage + 1]) - kills) + ' убийств']);
    if (m.role) lines.push(['text', m.role]);
    if (m.look) lines.push(['text', m.look]);
    if (m.spawn) lines.push(['text', 'Где водится: ' + m.spawn]);
    if (stage >= 2) {
      lines.push(['head', 'Числа']);
      if (m.stats) lines.push(['text', 'Здоровье ' + nm(m.stats.health) + ' · урон ' + nm(m.stats.damage) + ' · броня ' + nm(m.stats.armor)]);
      else {
        const rows = vanillaRows(e, false);
        if (!rows.length) lines.push(['dim', 'Числа ванильные.']);
        for (const r of rows) lines.push(['text', r]);
        if (rows.length && rows.length < e.keys.length) lines.push(['dim', 'Макет показывает числа только части типов, остальные книга берёт у движка.']);
      }
      if (m.attacks.length) lines.push(['text', 'Атаки: ' + m.attacks.map((a) => a.name).join(', ')]);
    }
    if (stage >= 4) {
      if (Array.isArray(m.behavior) && m.behavior.length) lines.push(['head', 'Тактика'], ...m.behavior.map((b) => ['text', b]));
      if (!e.vanilla) {
        // The loot line: the armour pieces, the head, the soul shard and the horn (data/mobs.json soul_shard, horn_drop).
        const loot = m.loot.map((l) => l.name + ' — ' + pct(l.chance));
        loot.push('голова — ' + pct(SC.TROPHY_DATA.drops.head[e.elite ? 'world_elite' : 'world']));
        const sh = SC.DATA.soul_shard, horn = SC.DATA.horn_drop;
        const shn = e.elite ? sh.elite_count : sh.count;
        loot.push('осколок души' + (shn > 1 ? ' × ' + shn : '') + ' — ' + pct(e.elite ? sh.elite_chance : sh.chance));
        loot.push(SC.itemName(e.elite ? horn.elite_item : horn.item) + ' — ' + pct(e.elite ? horn.elite_chance : horn.chance));
        lines.push(['head', 'Добыча'], ['text', loot.join(', ')]);
      }
      if (m.stats) lines.push(['head', 'Рост по силе P'], ['text', 'От P = 0 до P = 1: ' + growLine(scaled(m.stats, 0), scaled(m.stats, 1))]);
      else {
        const rows = vanillaRows(e, true);
        if (rows.length) lines.push(['head', 'Рост по силе P'], ['dim', 'От P = 0 до P = 1:'], ...rows.map((r) => ['text', r]));
      }
      const ev = eventLinks(e);
      if (ev.length) lines.push(['head', 'Ивенты'], ...ev.map((t) => ['text', t]));
    }
    return lines;
  }

  // A phase strip of an attack, to scale, for the entries that have no model to play (vanilla).
  function stripEl(a) {
    const segs = [['windup', a.windup], ['hold', a.hold], ['hit', a.hit], ['recover', a.recover]].filter(([, v]) => v);
    const total = segs.reduce((n, [, v]) => n + v, 0);
    return el('div', { class: 'bk-strip-row' }, el('div', { class: 'bk-small' }, a.name + ' — ' + segs.map(([k, v]) => SC.PHASES[k].name.toLowerCase() + ' ' + v).join(', ') + ' т. Замах: ' + a.tell + '.'),
      el('div', { class: 'att-strip', role: 'img', 'aria-label': 'Фазы ' + a.name }, segs.map(([k, v]) => el('i', { style: '--c:' + SC.PHASES[k].color + ';width:' + (v / total) * 100 + '%' }))));
  }

  // ------------------------------------------------------------------ the enemy entry
  function enemyPages(e, n, view, fail) {
    const left = el('div', { class: 'bk-page' });
    const right = el('div', { class: 'bk-page' });
    const m = e.m;
    left.append(el('div', { class: 'bk-title' }, n ? e.name : '???'));
    const kills = n >= 2 ? killsFor(e, B().stages[n]) : 0;
    if (!e.vanilla) {
      const rig = SC.RIGS[e.id];
      left.append(el('div', { class: 'bk-pic' }, view.canvas));
      view.set(m, { silhouette: n === 0 }).catch((x) => fail(String(x && x.stack ? x.stack : x)));
      const clipRow = el('div', { class: 'bk-clips' });
      const bar = el('div', { class: 'bk-bar' });
      const read = el('div', { class: 'bk-small' });
      left.append(clipRow, bar, read);
      const clips = n >= 3 ? ['idle', 'walk', ...m.attacks.filter((a) => rig.clips[a.clip]).map((a) => a.clip)] : n === 2 ? ['idle', 'walk'] : ['idle'];
      const pick = (name) => {
        for (const b of clipRow.children) b.classList.toggle('on', b.dataset.clip === name);
        view.play(name);
        bar.textContent = '';
        read.textContent = '';
        view.onTick = null;
        const mk = rig.clips[name] ? rig.clips[name].markers : {};
        if (n >= 3 && mk.hit != null) {
          const tl = SC.timeline(rig.clips[name].ticks, SC.phaseSpans(mk), [[mk.hit, 'удар']], (v, step) => view.seek(v == null ? Math.round(view.tick()) + step : v));
          bar.append(tl.el);
          view.onTick = (t) => {
            tl.set(t);
            const ph = SC.phaseAt(mk, t);
            read.textContent = 'тик ' + Math.floor(t) + (ph ? ' · ' + SC.PHASES[ph].name : '');
          };
        }
      };
      if (n >= 1)
        for (const c of clips.filter((c) => rig.clips[c]))
          clipRow.append(el('button', { type: 'button', class: 'bk-btn', 'data-clip': c, onclick: () => pick(c) }, SC.clipName(m, c)));
      // From stage 3 the page opens on the first timed attack, its phase bar under the model.
      const first = n >= 3 ? m.attacks.find((a) => rig.clips[a.clip] && rig.clips[a.clip].markers.hit != null) : null;
      pick(first ? first.clip : 'idle');
    } else {
      // A changed vanilla mob keeps its own model, which the book does not draw: the attacks as strips.
      left.append(el('div', { class: 'bk-pic bk-nomodel' }, el('span', {}, n ? 'Ванильный моб: модель в книге не рисуется' : '???')));
      if (n >= 3) for (const a of m.attacks) left.append(stripEl(a));
    }
    right.append(...mobLines(e, n, kills).map(lineEl));
    return [left, right];
  }

  // ------------------------------------------------------------------ altar and events (book_page.rs altar_page, events_page)
  // The lines are split over the two pages at a heading near the middle.
  function spread(title, lines) {
    const left = el('div', { class: 'bk-page' }, el('div', { class: 'bk-title' }, title));
    const right = el('div', { class: 'bk-page' }, el('div', { class: 'bk-title' }, ' '));
    const heads = lines.map((l, i) => (l[0] === 'head' ? i : -1)).filter((i) => i > 0);
    const cut = heads.length ? heads.reduce((a, b) => (Math.abs(b - lines.length / 2) < Math.abs(a - lines.length / 2) ? b : a)) : lines.length;
    left.append(...lines.slice(0, cut).map(lineEl));
    right.append(...lines.slice(cut).map(lineEl));
    return [left, right];
  }
  const locked = (title, when) => [['dim', title + ': ???'], ...(when ? [['dim', '  откроется: ' + when]] : [])];

  function altarLines(st) {
    const e = Object.fromEntries(B().altar_entries.map((x) => [x.id, x]));
    const lines = [['head', e.altar.name], ...(st.base ? [['text', e.altar.text]] : locked('Алтарь', e.altar.when))];
    lines.push(['head', e.levels.name]);
    for (const lv of SC.ALTAR.levels) {
      if (st.level >= lv.level) {
        const corner = lv.corner ? ', углы: ' + SC.pieceWord(lv.corner) : '';
        lines.push(['text', 'Уровень ' + lv.level + ': кольцо ' + lv.size + '×' + lv.size + corner + '. Открывает: ' + lv.unlocks]);
      } else lines.push(...locked('Уровень ' + lv.level, lv.level === 1 ? e.levels.when : ''));
    }
    lines.push(['head', e.modifiers.name]);
    SC.ALTAR.modifiers.slice(0, st.banners).forEach((md) => lines.push(['text', md.name + ': ' + md.text]));
    const hidden = SC.ALTAR.modifiers.length - st.banners;
    if (hidden > 0) lines.push(['dim', 'Не найдено знамён: ' + hidden + '. Откроется: ' + e.modifiers.when]);
    lines.push(['head', e.defense.name], ...(st.defense ? [['text', e.defense.text]] : locked(e.defense.name, e.defense.when)));
    return lines;
  }

  function eventLines(st) {
    const when = B().altar_entries.find((x) => x.id === 'events').when;
    const d = SC.TROPHY_DATA.drops;
    const lines = [];
    for (const ev of SC.DATA.events) {
      if (st.event === 0) {
        lines.push(...locked('Ивент', when));
        continue;
      }
      lines.push(['head', ev.name], ['text', 'Алтарь уровня ' + ev.tier + ' и выше; призыв: ' + SC.itemName(ev.item) + ' на подставке.']);
      if (ev.target) lines.push(['text', 'Цель: ' + ev.target]);
      if (st.event < 2) {
        lines.push(['dim', 'Волны и награда откроются после первой победы.']);
        continue;
      }
      for (const w of ev.waves) lines.push(['text', 'Волна ' + w.wave + ': ' + w.mobs.map((x) => x.count + ' × ' + SC.waveName(x)).join(', ')]);
      lines.push(['text', 'Награда: ' + d.head.event + '; части брони — ' + pct(d.armor.event) + '; ' + d.diorama.event]);
      const t = SC.TROPHY_DATA.tiers.find((x) => x.tier === ev.tier);
      if (t) lines.push(['text', 'Награда ступени: осколки души × ' + t.shards + (t.item ? '; ' + SC.itemName(t.item) + ' — ' + pct(t.chance) + ', знамёна «сложнее» поднимают шанс' : '')]);
    }
    return lines;
  }
  // The name of a piece for the book (book_page.rs piece_name).
  SC.pieceWord = (id) => ({ altar_pillar: 'колонна', altar_brazier: 'жаровня', altar_obelisk: 'обелиск' })[id] || id;

  // ------------------------------------------------------------------ the page
  function page(sub, params) {
    const ui = SC.ui;
    ui.buildNav('book');
    const main = ui.main;
    main.append(el('h1', {}, 'Книга охотника'));
    main.append(el('p', { class: 'sum' }, 'Справочник в игре: каждый игрок видит только то, что сам встретил, убил или построил. Три раздела: «Враги» (мобы SoulsCraft и 11 групп ванильных), «Алтарь» (в том числе запись «Оборона алтаря») и «Ивенты». Записи о враге открываются ступенями по числу убийств (' +
      B().source + '). Ниже — макет окна: в игре его рисует мод, а строки составляет код правил (book_page.rs) из тех же данных.'));
    const errBox = el('div', { class: 'sc-err', hidden: true });
    const fail = ui.panelError(errBox);
    const book = el('div', { class: 'bk-book', role: 'region', 'aria-label': 'Макет окна книги охотника' });
    const tabs = el('div', { class: 'bk-tabs', role: 'tablist' });
    const pageNo = el('div', { class: 'bk-pageno' });
    const spreadBox = el('div', { class: 'bk-spread' });
    book.append(tabs, pageNo, spreadBox);
    const bar = el('div', { class: 'bk-actionbar' });
    const hotbar = el('div', { class: 'bk-hotbar', 'aria-hidden': 'true' }, [...Array(9).keys()].map((i) =>
      el('div', { class: 'mc-slot' }, i === 0 && SC.ITEMS.hunter_book ? el('img', { src: SC.ITEMS.hunter_book.texture, alt: '' }) : null)));
    const hud = el('div', { class: 'bk-hud' }, el('div', { class: 'mc-mock-label' }, 'Сообщение над панелью предметов при новой ступени'), bar, hotbar);

    // Controls: the entry, the stage (and the kills that stage needs), altar and event progress.
    const ctl = el('div', { class: 'bk-ctl' });
    const mobRow = el('div', { class: 'sc-clips', role: 'group', 'aria-label': 'Моб' });
    const vanRow = el('div', { class: 'sc-clips', role: 'group', 'aria-label': 'Ванильные мобы' });
    const stageRow = el('div', { class: 'sc-clips', role: 'group', 'aria-label': 'Ступень записи' });
    const slider = el('input', { type: 'range', min: 0, max: B().stages.length - 1, step: 1, 'aria-label': 'Ступень записи' });
    const killsNote = el('div', { class: 'small' });
    const opens = el('table', { class: 'sc-moves bk-opens' });
    const altarCtl = el('div', { class: 'sc-clips' });
    ctl.append(el('div', { class: 'mc-mock-label' }, 'Управление макетом'), mobRow, vanRow,
      el('div', { class: 'bk-slider' }, el('span', { class: 'sc-state-label' }, 'Ступень:'), slider, killsNote), stageRow, opens, altarCtl);
    main.append(el('div', { class: 'bk-wrap' }, el('div', {}, el('div', { class: 'mc-mock-label' }, 'Макет: окно книги в игре рисует мод, здесь — содержание и расклад'), book, hud), ctl), errBox);

    const view = modelView(fail);
    const list = entries();
    const want = params.get('mob');
    const sections = { enemies: 'Враги', altar: 'Алтарь', events: 'Ивенты' };
    const state = {
      section: sections[params.get('section')] ? params.get('section') : 'enemies',
      mob: list.some((x) => x.id === want) ? want : 'stalker',
      stage: params.has('stage') ? Math.max(0, Math.min(4, Number(params.get('stage')))) : 3,
      base: true, level: 2, banners: 5, defense: true, event: 1,
    };
    let lastMsg = '';
    function message(text) {
      if (text === lastMsg) return;
      lastMsg = text;
      bar.textContent = text;
      bar.classList.remove('show');
      void bar.offsetWidth; // restart the fade
      bar.classList.add('show');
    }
    const current = () => list.find((x) => x.id === state.mob);
    function draw() {
      const e = current();
      tabs.textContent = '';
      B().sections.forEach((label, i) => {
        const key = ['enemies', 'altar', 'events'][i];
        tabs.append(el('button', { type: 'button', role: 'tab', class: 'bk-tab' + (state.section === key ? ' on' : ''), 'aria-selected': String(state.section === key),
          onclick: () => ((state.section = key), draw()) }, label));
      });
      spreadBox.textContent = '';
      if (state.section === 'enemies') {
        pageNo.textContent = 'Страница ' + (list.indexOf(e) + 1) + ' из ' + list.length;
        spreadBox.append(...enemyPages(e, state.stage, view, fail));
      } else if (state.section === 'altar') {
        pageNo.textContent = 'Алтарь';
        spreadBox.append(...spread('Алтарь', altarLines(state)));
      } else {
        pageNo.textContent = 'Ивенты';
        spreadBox.append(...spread('Ивенты', eventLines(state)));
      }
      spreadBox.append(el('div', { class: 'bk-arrows' },
        el('button', { type: 'button', class: 'bk-arrow', 'aria-label': 'Предыдущая запись', onclick: () => step(-1) }, '◀'),
        el('button', { type: 'button', class: 'bk-arrow', 'aria-label': 'Следующая запись', onclick: () => step(1) }, '▶')));
      // Controls
      slider.value = state.stage;
      const s = B().stages[state.stage];
      const k = killsFor(e, s);
      killsNote.textContent = s.name + (k == null ? ': моб ещё не встречен' : k === 0 ? ': встречен, убийств нет' : ': от ' + k + ' убийств' + (e.elite ? ' (элитный)' : ''));
      for (const b of stageRow.children) b.classList.toggle('on', Number(b.dataset.n) === state.stage);
      opens.textContent = '';
      opens.append(el('thead', {}, el('tr', {}, ['Ступень', 'Условие', 'Открывает'].map((h) => el('th', {}, h)))),
        el('tbody', {}, B().stages.map((x) => el('tr', { class: x.n <= state.stage ? 'open' : 'shut' },
          el('td', {}, x.n + '. ' + x.name), el('td', {}, x.n >= 2 ? (killsFor(e, x) + ' уб.') : x.when), el('td', {}, x.opens)))));
      if (state.stage >= 1 && state.section === 'enemies') message(B().action_bar.replace('{mob}', e.name).replace('{stage}', s.name.toLowerCase()));
      else message('');
      for (const b of [...mobRow.querySelectorAll('button'), ...vanRow.querySelectorAll('button')]) b.classList.toggle('on', b.dataset.key === state.mob && state.section === 'enemies');
    }
    function step(d) {
      if (state.section !== 'enemies') return;
      state.mob = list[(list.indexOf(current()) + d + list.length) % list.length].id;
      draw();
    }
    const go = (id) => () => ((state.mob = id), (state.section = 'enemies'), draw());
    for (const x of list)
      (x.vanilla ? vanRow : mobRow).append(el('button', { type: 'button', 'data-key': x.id, onclick: go(x.id) }, x.vanilla ? x.name.split(', ')[0] : x.name));
    vanRow.prepend(el('span', { class: 'sc-state-label' }, 'Ванильные:'));
    for (const s of B().stages)
      stageRow.append(el('button', { type: 'button', 'data-n': s.n, onclick: () => ((state.stage = s.n), draw()) }, s.n + '. ' + s.name));
    slider.oninput = () => {
      state.stage = Number(slider.value);
      draw();
    };
    const toggle = (label, key) => {
      const b = el('button', { type: 'button', class: state[key] ? 'on' : '', 'aria-pressed': String(state[key]) }, label);
      b.onclick = () => {
        state[key] = !state[key];
        b.classList.toggle('on', state[key]);
        b.setAttribute('aria-pressed', String(state[key]));
        draw();
      };
      return b;
    };
    altarCtl.append(el('span', { class: 'sc-state-label' }, 'Алтарь и ивенты:'), toggle('Держал предмет призыва', 'base'), toggle('Видел старт ивента (оборона)', 'defense'));
    const lv = el('select', { 'aria-label': 'Достроенный уровень алтаря' }, [0, 1, 2, 3, 4].map((n) => el('option', { value: n, selected: n === state.level }, n ? 'уровень ' + n : 'не строил')));
    lv.onchange = () => ((state.level = Number(lv.value)), draw());
    const bn = el('select', { 'aria-label': 'Сколько знамён держал' }, [...Array(SC.ALTAR.modifiers.length + 1).keys()].map((n) => el('option', { value: n, selected: n === state.banners }, 'знамён: ' + n)));
    bn.onchange = () => ((state.banners = Number(bn.value)), draw());
    const ev = el('select', { 'aria-label': 'Ивенты' }, ['не пробовал', 'была попытка', 'победа'].map((t, n) => el('option', { value: n, selected: n === state.event }, 'ивенты: ' + t)));
    ev.onchange = () => ((state.event = Number(ev.value)), draw());
    altarCtl.append(lv, bn, ev);
    draw();
  }
  SC.page('book', page);
})();
