'use strict';
// #book: a mock of the hunter's book (docs/design/08-guide-book.md) in a vanilla-book style.
// The game window is drawn by the mod; this page shows what each stage of an entry opens, with
// the same data the game will use: data/mobs.json (SC.DATA), data/book.json (SC.BOOK, stages and
// thresholds), data/altar.json (SC.ALTAR), the mob's glb and rig (clips, phase markers).
(function () {
  const T = THREE;
  const el = SC.el;
  const B = () => SC.BOOK;
  const ENV = { land: 'на земле', air: 'в воздухе', water: 'в воде', land_water: 'на земле и в воде' };
  const isElite = (m) => !!m.elite;
  const killsFor = (m, s) => (isElite(m) ? s.elite_kills : s.kills);
  const pct = (v) => SC.num(Math.round(v * 1000) / 10) + '%';

  // The stage an entry is at after `kills` kills (met: the mob was seen).
  function stageOf(m, kills, met) {
    if (!met) return 0;
    let n = 1;
    for (const s of B().stages) if (s.n >= 2 && kills >= killsFor(m, s)) n = s.n;
    return n;
  }

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

  // ------------------------------------------------------------------ the enemy entry
  function enemyPages(m, n, view, fail) {
    const left = el('div', { class: 'bk-page' });
    const right = el('div', { class: 'bk-page' });
    const rig = SC.RIGS[m.id];
    // Left: title, the model, clip buttons and the phase bar (stage 3+).
    left.append(el('div', { class: 'bk-title' }, n ? m.name : '???'));
    left.append(el('div', { class: 'bk-pic' }, view.canvas));
    view.set(m, { silhouette: n === 0 }).catch((e) => fail(String(e && e.stack ? e.stack : e)));
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

    // Right: what the stage opens.
    if (n === 0) {
      right.append(el('div', { class: 'bk-h' }, '???'), el('p', {}, '?????? ??? ????? ?? ??????.'),
        el('p', { class: 'bk-hint' }, 'Запись закрыта. Встреть этого врага: он должен оказаться в 16 блоках и увидеть тебя.'));
      return [left, right];
    }
    right.append(el('div', { class: 'bk-h' }, 'Образ'), el('p', {}, m.look),
      el('p', {}, el('b', {}, 'Где: '), m.spawn + ', ' + ENV[m.env] + '.'));
    if (n >= 2) {
      right.append(el('div', { class: 'bk-h' }, 'Числа'),
        el('table', { class: 'bk-stats' }, el('tbody', {},
          el('tr', {}, el('td', {}, 'Здоровье'), el('td', {}, m.stats.health)),
          el('tr', {}, el('td', {}, 'Урон'), el('td', {}, m.stats.damage)),
          el('tr', {}, el('td', {}, 'Броня'), el('td', {}, m.stats.armor)))));
      right.append(el('div', { class: 'bk-h' }, 'Атаки'));
      if (n === 2) right.append(el('ul', { class: 'bk-list' }, m.attacks.map((a) => el('li', {}, a.name))));
    }
    if (n >= 3) {
      right.append(el('ul', { class: 'bk-list' }, m.attacks.map((a) => {
        const timed = a.windup + a.hit + a.recover > 0;
        return el('li', {}, el('button', { type: 'button', class: 'bk-link', onclick: () => pick(a.clip) }, a.name),
          timed ? ' — замах ' + a.windup + (a.hold ? ', пауза ' + a.hold : '') + ', удар ' + a.hit + ', окно ' + a.recover + ' т.' : '',
          el('div', { class: 'bk-small' }, 'Замах: ' + a.tell + (a.window_note ? '. Окно: ' + a.window_note : '') + '.'));
      })));
    }
    if (n >= 4) {
      const events = SC.DATA.events.filter((ev) => ev.waves.some((w) => w.mobs.some((x) => x.type === m.key)));
      const t = SC.TROPHIES[m.id];
      right.append(el('div', { class: 'bk-h' }, 'Тактика'), el('p', {}, m.question), el('ul', { class: 'bk-list' }, m.behavior.map((b) => el('li', {}, b))),
        el('div', { class: 'bk-h' }, 'Добыча'),
        el('ul', { class: 'bk-list' }, m.loot.map((l) => el('li', {}, l.name + ' — ' + pct(l.chance))),
          t ? el('li', {}, 'Голова — ' + pct(isElite(m) ? SC.TROPHY_DATA.drops.head.world_elite : SC.TROPHY_DATA.drops.head.world) + ', в ивенте чаще') : null),
        el('p', {}, el('b', {}, 'Сила: '), 'появляется от силы игрока P ' + SC.num(m.tier_p) + '.'),
        el('p', {}, el('b', {}, 'Ивенты: '), events.length ? events.map((e) => e.name).join(', ') : 'пока ни в одном ивенте.'));
    }
    return [left, right];
  }

  // ------------------------------------------------------------------ altar and events
  function altarPages(st) {
    const left = el('div', { class: 'bk-page' }, el('div', { class: 'bk-title' }, 'Алтарь'));
    const right = el('div', { class: 'bk-page' }, el('div', { class: 'bk-title' }, 'Ивенты'));
    const entry = (open, title, text) => el('div', { class: 'bk-entry' + (open ? '' : ' locked') },
      el('span', { class: 'bk-lock', 'aria-hidden': 'true' }, open ? '✦' : '✕'), el('b', {}, open ? title : '???'),
      el('div', { class: 'bk-small' }, open ? text : 'Запись закрыта'));
    const e = Object.fromEntries(B().altar_entries.map((x) => [x.id, x]));
    left.append(entry(st.touched, e.altar.name, 'Ядро, подставка, кольца; алтарь работает только под открытым небом. Открыто: ' + e.altar.when.toLowerCase() + '.'));
    for (const lv of SC.ALTAR.levels)
      left.append(entry(st.level >= lv.level, 'Уровень ' + lv.level + ' · ' + lv.size + '×' + lv.size, lv.unlocks + '.'));
    const held = SC.ALTAR.modifiers.slice(0, st.banners);
    left.append(el('div', { class: 'bk-h' }, 'Знамёна ' + held.length + ' из ' + SC.ALTAR.modifiers.length));
    left.append(el('div', { class: 'bk-banners' }, SC.ALTAR.modifiers.map((md, i) => {
      const open = i < st.banners;
      const it = SC.ITEMS[md.banner];
      return el('div', { class: 'bk-banner' + (open ? '' : ' locked'), title: open ? md.name + ': ' + md.text : '???' },
        it ? el('img', { src: it.texture, alt: open ? md.name : 'закрыто', width: 32, height: 32 }) : null);
    })));
    for (const ev of SC.DATA.events) {
      right.append(entry(st.event >= 1, ev.name, 'Передышка ' + ev.rest_seconds.join('–') + ' с, здоровье алтаря ' + ev.altar_health + '.'));
      if (st.event >= 2)
        right.append(el('ul', { class: 'bk-list' }, ev.waves.map((w) => el('li', {}, 'Волна ' + w.wave + ': ' +
          w.mobs.map((x) => (x.name || 'зомби') + ' × ' + x.count).join(', ')))),
          el('p', { class: 'bk-small' }, 'Награда: голова моба ивента, по одной за каждое знамя «сложнее», части брони; на алтаре 3-го уровня — диорама.'));
      else if (st.event === 1) right.append(el('p', { class: 'bk-hint' }, 'Волны и награда откроются после первой победы.'));
    }
    right.append(el('p', { class: 'bk-hint' }, e.events.when + '.'));
    return [left, right];
  }

  // ------------------------------------------------------------------ the page
  function page(sub, params) {
    const ui = SC.ui;
    ui.buildNav('book');
    const main = ui.main;
    main.append(el('h1', {}, 'Книга охотника'));
    main.append(el('p', { class: 'sum' }, 'Справочник в игре: каждый игрок видит только то, что сам встретил, убил или построил. Записи о враге открываются ступенями по числу убийств (' +
      B().source + '). Ниже — макет окна: в игре его рисует мод.'));
    const errBox = el('div', { class: 'sc-err', hidden: true });
    const fail = ui.panelError(errBox);
    const book = el('div', { class: 'bk-book', role: 'region', 'aria-label': 'Макет окна книги охотника' });
    const tabs = el('div', { class: 'bk-tabs', role: 'tablist' });
    const pageNo = el('div', { class: 'bk-pageno' });
    const spread = el('div', { class: 'bk-spread' });
    book.append(tabs, pageNo, spread);
    const bar = el('div', { class: 'bk-actionbar' });
    const hotbar = el('div', { class: 'bk-hotbar', 'aria-hidden': 'true' }, [...Array(9).keys()].map((i) =>
      el('div', { class: 'mc-slot' }, i === 0 && SC.ITEMS.hunter_book ? el('img', { src: SC.ITEMS.hunter_book.texture, alt: '' }) : null)));
    const hud = el('div', { class: 'bk-hud' }, el('div', { class: 'mc-mock-label' }, 'Сообщение над панелью предметов при новой ступени'), bar, hotbar);

    // Controls: section, mob, stage (and the kills that stage needs), altar and event progress.
    const ctl = el('div', { class: 'bk-ctl' });
    const mobRow = el('div', { class: 'sc-clips', role: 'group', 'aria-label': 'Моб' });
    const stageRow = el('div', { class: 'sc-clips', role: 'group', 'aria-label': 'Ступень записи' });
    const slider = el('input', { type: 'range', min: 0, max: B().stages.length - 1, step: 1, 'aria-label': 'Ступень записи' });
    const killsNote = el('div', { class: 'small' });
    const opens = el('table', { class: 'sc-moves bk-opens' });
    const altarCtl = el('div', { class: 'sc-clips' });
    ctl.append(el('div', { class: 'mc-mock-label' }, 'Управление макетом'), mobRow,
      el('div', { class: 'bk-slider' }, el('span', { class: 'sc-state-label' }, 'Ступень:'), slider, killsNote), stageRow, opens, altarCtl);
    main.append(el('div', { class: 'bk-wrap' }, el('div', {}, el('div', { class: 'mc-mock-label' }, 'Макет: окно книги в игре рисует мод, здесь — содержание и расклад'), book, hud), ctl), errBox);

    const view = modelView(fail);
    const state = {
      section: params.get('section') === 'altar' ? 'altar' : 'enemies',
      mob: SC.mobById(params.get('mob')) ? params.get('mob') : 'stalker',
      stage: params.has('stage') ? Math.max(0, Math.min(4, Number(params.get('stage')))) : 3,
      touched: true, level: 2, banners: 5, event: 1,
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
    function draw() {
      const m = SC.mobById(state.mob);
      tabs.textContent = '';
      for (const [key, label] of [['enemies', B().sections[0]], ['altar', B().sections[1] + ' и ' + B().sections[2].toLowerCase()]])
        tabs.append(el('button', { type: 'button', role: 'tab', class: 'bk-tab' + (state.section === key ? ' on' : ''), 'aria-selected': String(state.section === key),
          onclick: () => ((state.section = key), draw()) }, label));
      spread.textContent = '';
      const mobIndex = SC.DATA.mobs.indexOf(m);
      if (state.section === 'enemies') {
        pageNo.textContent = 'Страница ' + (mobIndex * 2 + 1) + ' из ' + SC.DATA.mobs.length * 2;
        spread.append(...enemyPages(m, state.stage, view, fail));
      } else {
        pageNo.textContent = 'Алтарь и ивенты';
        spread.append(...altarPages(state));
      }
      spread.append(el('div', { class: 'bk-arrows' },
        el('button', { type: 'button', class: 'bk-arrow', 'aria-label': 'Предыдущая запись', onclick: () => step(-1) }, '◀'),
        el('button', { type: 'button', class: 'bk-arrow', 'aria-label': 'Следующая запись', onclick: () => step(1) }, '▶')));
      // Controls
      slider.value = state.stage;
      const s = B().stages[state.stage];
      const k = killsFor(m, s);
      killsNote.textContent = s.name + (k == null ? ': моб ещё не встречен' : k === 0 ? ': встречен, убийств нет' : ': от ' + k + ' убийств' + (isElite(m) ? ' (элитный)' : ''));
      for (const b of stageRow.children) b.classList.toggle('on', Number(b.dataset.n) === state.stage);
      opens.textContent = '';
      opens.append(el('thead', {}, el('tr', {}, ['Ступень', 'Условие', 'Открывает'].map((h) => el('th', {}, h)))),
        el('tbody', {}, B().stages.map((x) => el('tr', { class: x.n <= state.stage ? 'open' : 'shut' },
          el('td', {}, x.n + '. ' + x.name), el('td', {}, x.n >= 2 ? (killsFor(m, x) + ' уб.') : x.when), el('td', {}, x.opens)))));
      if (state.stage >= 1) message(B().action_bar.replace('{mob}', m.name).replace('{stage}', s.name.toLowerCase()));
      else message('');
      for (const b of mobRow.querySelectorAll('button')) b.classList.toggle('on', b.dataset.key === state.mob);
    }
    function step(d) {
      if (state.section !== 'enemies') return;
      const list = SC.DATA.mobs;
      state.mob = list[(list.findIndex((x) => x.id === state.mob) + d + list.length) % list.length].id;
      draw();
    }
    for (const m of SC.DATA.mobs)
      mobRow.append(el('button', { type: 'button', 'data-key': m.id, onclick: () => ((state.mob = m.id), (state.section = 'enemies'), draw()) }, m.name));
    for (const s of B().stages)
      stageRow.append(el('button', { type: 'button', 'data-n': s.n, onclick: () => ((state.stage = s.n), draw()) }, s.n + '. ' + s.name));
    slider.oninput = () => {
      state.stage = Number(slider.value);
      draw();
    };
    const toggles = [
      ['Держал предмет призыва', () => state.touched, (v) => (state.touched = v)],
    ];
    altarCtl.append(el('span', { class: 'sc-state-label' }, 'Алтарь и ивенты:'));
    for (const [label, get, set] of toggles) {
      const b = el('button', { type: 'button', class: get() ? 'on' : '', 'aria-pressed': String(get()) }, label);
      b.onclick = () => {
        set(!get());
        b.classList.toggle('on', get());
        b.setAttribute('aria-pressed', String(get()));
        draw();
      };
      altarCtl.append(b);
    }
    const lv = el('select', { 'aria-label': 'Достроенный уровень алтаря' }, [0, 1, 2, 3, 4].map((n) => el('option', { value: n, selected: n === state.level }, n ? 'уровень ' + n : 'не строил')));
    lv.onchange = () => ((state.level = Number(lv.value)), draw());
    const bn = el('select', { 'aria-label': 'Сколько знамён держал' }, [...Array(SC.ALTAR.modifiers.length + 1).keys()].map((n) => el('option', { value: n, selected: n === state.banners }, 'знамён: ' + n)));
    bn.onchange = () => ((state.banners = Number(bn.value)), draw());
    const ev = el('select', { 'aria-label': 'Ивент' }, ['не пробовал', 'была попытка', 'победа'].map((t, n) => el('option', { value: n, selected: n === state.event }, 'ивент: ' + t)));
    ev.onchange = () => ((state.event = Number(ev.value)), draw());
    altarCtl.append(lv, bn, ev);
    draw();
  }
  SC.page('book', page);
})();
