'use strict';
// Command palette (cmdk-like): Ctrl+K, «/» or the search button in the top bar. Finds any mob,
// attack, behaviour scene, altar part, banner, block, trophy and book page and opens its address.
// Matching is fuzzy and Russian-aware: ё = е, words in any order, letters with gaps, and a query
// typed in the wrong keyboard layout («uhjvbkf» finds «громила»).
(function () {
  const el = SC.el;
  const MAX = 60;

  // ------------------------------------------------------------------ what can be found
  let INDEX = null;
  function buildIndex() {
    const out = [];
    const add = (group, title, sub, href, keys) => out.push({ group, title, sub: sub || '', href, keys: keys || '' });
    add('Страницы', 'Бестиарий: все мобы', 'Карточки семи мобов', '#bestiary', 'индекс главная');
    add('Страницы', 'Оборона алтаря', 'Роли, перехват, вытягивание, проклятие, пролом, одна жизнь', '#defense', 'оборона защита перехват проклятие пролом хардкор жизнь');
    add('Страницы', 'Ванильные мобы', '11 групп с новыми приёмами', '#vanilla', 'ванильные зомби скелет паук ведьма разбойник поборник хоглин фантом слизень');
    add('Страницы', 'Предметы и рецепты', 'Горны, осколки, клинок, молот, Сердце бездны, 24 рецепта', '#recipes', 'рецепты крафт горн осколок клинок молот сердце сундуки');
    for (const m of SC.DATA.mobs) add('Мобы', m.name, m.role + ' · ' + m.key, '#mob/' + m.id, m.id + ' ' + m.question);
    for (const m of SC.DATA.mobs) {
      const rig = SC.RIGS[m.id];
      const has = (clip) => SC.GLB[m.id] && rig && rig.clips && rig.clips[clip];
      for (const a of m.attacks || [])
        add('Атаки', a.name, 'Атака · ' + m.name, '#mob/' + m.id + (has(a.clip) ? '?clip=' + a.clip : ''), (a.tell || '') + ' ' + (a.clip || ''));
      for (const mv of m.moves || [])
        if (mv.clip && has(mv.clip)) add('Атаки', mv.name, 'Движение · ' + m.name, '#mob/' + m.id + '?clip=' + mv.clip, mv.note || '');
      const def = SC.MOBS[m.id];
      if (def && def.scenes && SC.GLB[m.id])
        for (const s of def.scenes) add('Сцены', s.title, 'Сцена поведения · ' + m.name, '#mob/' + m.id + '?scene=' + s.id, s.intro || '');
    }
    add('Страницы', 'Ивенты алтаря: все ступени', 'Четыре ступени', '#event', 'ивент волны ступени');
    for (const ev of SC.DATA.events || []) add('Страницы', ev.name, 'Ивент ступени ' + ev.tier + ': волны и награда', '#event/' + ev.id, 'ивент волны награда горн ' + ev.item);
    if (SC.ALTAR) {
      add('Алтарь', 'Алтарь призыва', 'Уровни, достройка, призыв, меню, знамёна', '#altar', 'алтарь');
      for (const [only, title] of [['levels', 'Уровни алтаря'], ['build', 'Достройка и призрак'], ['summon', 'Призыв: Гнилой горн'], ['menu', 'Меню алтаря'], ['boost', 'Учащённый спавн'], ['banners', 'Знамёна алтаря']])
        add('Алтарь', title, 'Алтарь призыва', '#altar?only=' + only, 'алтарь');
      for (const lv of SC.ALTAR.levels) add('Алтарь', 'Уровень ' + lv.level + ' · ' + lv.size + '×' + lv.size, 'Алтарь в сборе', '#altar?only=levels&level=' + lv.level, lv.unlocks || '');
      add('Алтарь', 'Блоки алтаря и предметы', 'Блоки, знамёна, горн, книга', '#altar/blocks', 'блоки предметы');
      for (const b of SC.ALTAR.modifiers) add('Знамёна', b.name, b.text, '#banner/' + b.id, b.short + ' ' + b.id);
    }
    if (SC.BLOCKS)
      for (const b of SC.BLOCKS.blocks.filter((x) => x.set === 'altar' && !x.id.startsWith('banner_')))
        add('Блоки', b.name, 'Блок · ' + b.role, '#altar/blocks', b.id);
    if (SC.TROPHY_DATA) {
      add('Трофеи', 'Трофеи: диорамы', 'Диорамы, зал, головы', '#trophies', 'трофей награда');
      for (const [only, title] of [['one', 'Диорама крупно'], ['all', 'Все семь диорам днём и ночью'], ['hall', 'Зал трофеев'], ['heads', 'Головы мобов'], ['drops', 'Откуда берутся трофеи'], ['data', 'Что получает мод']])
        add('Трофеи', title, 'Трофеи', '#trophies?only=' + only, 'трофей');
      for (const m of SC.DATA.mobs) if (SC.TROPHIES[m.id]) add('Трофеи', 'Диорама: ' + m.name, 'Трофей', '#trophies?only=one&mob=' + m.id, 'трофей ' + m.id);
    }
    if (SC.BOOK) {
      add('Книга', 'Книга охотника', 'Макет окна книги', '#book', 'книга справочник');
      add('Книга', 'Книга: алтарь и ивенты', 'Записи алтаря', '#book?section=altar', 'книга алтарь ивенты');
      for (const m of SC.DATA.mobs) add('Книга', 'Книга: ' + m.name, 'Запись о мобе', '#book?mob=' + m.id, 'книга ' + m.id);
    }
    return out;
  }

  // ------------------------------------------------------------------ fuzzy matching
  const norm = (s) => s.toLowerCase().replace(/ё/g, 'е');
  const EN = "`qwertyuiop[]asdfghjkl;'zxcvbnm,.";
  const RU = 'ёйцукенгшщзхъфывапролджэячсмитьбю';
  const swap = (s, a, b) => [...s].map((c) => (a.indexOf(c) >= 0 ? b[a.indexOf(c)] : c)).join('');
  // One query word in a text: [score, matched indices] or null. A substring wins, at a word
  // start more; else the letters in order with gaps (fewer and shorter gaps win).
  function word(q, text) {
    const at = text.indexOf(q);
    if (at >= 0) {
      const start = at === 0 || /[\s\-:·(«]/.test(text[at - 1]);
      return [100 + (start ? 60 : 0) - Math.min(at, 40), [...Array(q.length).keys()].map((k) => at + k)];
    }
    const hits = [];
    let i = 0, score = 40, prev = -2;
    for (const c of q) {
      i = text.indexOf(c, i);
      if (i < 0) return null;
      score += i === prev + 1 ? 4 : -Math.min(6, i - prev);
      hits.push(i);
      prev = i;
      i++;
    }
    return q.length >= 2 && score > 0 ? [score, hits] : null;
  }
  function match(words, item) {
    const title = norm(item.title), rest = norm(item.sub + ' ' + item.keys);
    let total = 0;
    const marks = [];
    for (const w of words) {
      const t = word(w, title), r = t ? null : word(w, rest);
      if (!t && !r) return null;
      if (t) {
        total += t[0];
        marks.push(...t[1]);
      } else total += r[0] * 0.5;
    }
    return { score: total, marks };
  }
  function search(query) {
    const q = norm(query.trim());
    if (!q) return INDEX.filter((x) => x.group === 'Страницы' || x.group === 'Мобы' || x.title === 'Алтарь призыва' || x.title === 'Трофеи: диорамы' || x.title === 'Книга охотника')
      .map((item) => ({ item, marks: [] }));
    const tries = [q, swap(q, EN, RU), swap(q, RU, EN)];
    const found = [];
    INDEX.forEach((item, n) => {
      let best = null;
      for (const t of tries) {
        const r = match(t.split(/\s+/).filter(Boolean), item);
        if (r && (!best || r.score > best.score)) best = r;
      }
      if (best) found.push({ item, marks: best.marks, score: best.score, n });
    });
    found.sort((a, b) => b.score - a.score || a.n - b.n);
    return found.slice(0, MAX);
  }

  // ------------------------------------------------------------------ the dialog
  // The shared .gd-cmdk overlay of style.css; the page behind is inert while it is open.
  let pal = null, input, list, results = [], active = 0, opener = null;
  const behind = () => [document.querySelector('.top'), document.querySelector('.layout'), document.getElementById('err'), document.querySelector('.skip')];
  function highlight(text, marks) {
    if (!marks.length) return [text];
    const set = new Set(marks);
    const out = [];
    let run = '', on = false;
    [...text].forEach((c, i) => {
      if (set.has(i) !== on) {
        if (run) out.push(on ? el('mark', {}, run) : run);
        run = '';
        on = !on;
      }
      run += c;
    });
    if (run) out.push(on ? el('mark', {}, run) : run);
    return out;
  }
  function render() {
    results = search(input.value);
    list.textContent = '';
    if (!results.length) {
      list.append(el('div', { class: 'gd-cmdk-empty' }, 'Ничего не нашлось по запросу «' + input.value.trim() + '». Попробуйте имя моба, атаку или «знамя».'));
      input.setAttribute('aria-activedescendant', '');
      return;
    }
    // Groups keep the order of their best result.
    const groups = [];
    for (const r of results) if (!groups.includes(r.item.group)) groups.push(r.item.group);
    const ordered = [];
    groups.forEach((g, gi) => {
      const box = el('div', { role: 'group', 'aria-labelledby': 'sc-pal-g' + gi }, el('div', { class: 'gd-cmdk-group', id: 'sc-pal-g' + gi, role: 'presentation' }, g));
      for (const r of results.filter((x) => x.item.group === g)) {
        const i = ordered.length;
        ordered.push(r);
        box.append(el('div', { class: 'gd-cmdk-item', role: 'option', id: 'sc-pal-o' + i, 'aria-selected': 'false',
          onclick: () => go(i), onpointermove: () => i !== active && select(i) },
        el('span', { class: 'gd-t' }, el('b', {}, highlight(r.item.title, r.marks)), el('small', {}, r.item.sub)),
        el('span', { class: 'gd-k' }, r.item.href.split('?')[0])));
      }
      list.append(box);
    });
    results = ordered;
    select(0);
  }
  function select(i) {
    if (!results.length) return;
    active = (i + results.length) % results.length;
    for (const o of list.querySelectorAll('[role=option]')) o.setAttribute('aria-selected', String(o.id === 'sc-pal-o' + active));
    const cur = document.getElementById('sc-pal-o' + active);
    input.setAttribute('aria-activedescendant', cur.id);
    cur.scrollIntoView({ block: 'nearest' });
  }
  function go(i) {
    const r = results[i];
    if (!r) return;
    close(true);
    if (location.hash === r.item.href) window.scrollTo(0, 0);
    else location.hash = r.item.href;
    document.getElementById('main').focus({ preventScroll: true }); // the page that opens, not the button
  }
  function make() {
    INDEX = buildIndex();
    input = el('input', { type: 'text', role: 'combobox', 'aria-expanded': 'true', 'aria-controls': 'sc-pal-list', 'aria-autocomplete': 'list',
      'aria-label': 'Поиск по витрине', placeholder: 'Моб, атака, знамя, трофей…', autocomplete: 'off', spellcheck: 'false' });
    list = el('div', { class: 'gd-cmdk-list', id: 'sc-pal-list', role: 'listbox', 'aria-label': 'Результаты' });
    const kbd = (k) => el('span', { class: 'gd-kbd' }, k);
    const bar = el('div', { class: 'gd-cmdk-bar' }, input, kbd('Esc'));
    bar.prepend(document.querySelector('#search-open .gd-glass').cloneNode(true));
    pal = el('div', { class: 'gd-cmdk', hidden: true },
      el('div', { class: 'gd-cmdk-box', role: 'dialog', 'aria-modal': 'true', 'aria-label': 'Поиск по витрине' }, bar, list,
        el('div', { class: 'gd-cmdk-foot', 'aria-hidden': 'true' },
          el('span', {}, kbd('↑'), kbd('↓'), ' выбрать'), el('span', {}, kbd('Enter'), ' открыть'), el('span', {}, kbd('Esc'), ' закрыть'))));
    input.addEventListener('input', render);
    input.addEventListener('keydown', (e) => {
      if (e.key === 'ArrowDown' || (e.key === 'n' && e.ctrlKey)) select(active + 1);
      else if (e.key === 'ArrowUp' || (e.key === 'p' && e.ctrlKey)) select(active - 1);
      else if (e.key === 'PageDown') select(Math.min(results.length - 1, active + 8));
      else if (e.key === 'PageUp') select(Math.max(0, active - 8));
      else if (e.key === 'Enter') go(active);
      else if (e.key === 'Escape') close();
      else if (e.key === 'Tab') input.focus(); // the input is the only stop in the dialog
      else return;
      e.preventDefault();
    });
    pal.addEventListener('click', (e) => e.target === pal && close()); // the scrim
    document.body.append(pal);
  }
  function open() {
    if (!SC.DATA) return;
    if (!pal) make();
    if (!pal.hidden) return input.select();
    opener = document.activeElement;
    input.value = '';
    render();
    pal.hidden = false;
    for (const x of behind()) if (x) x.inert = true;
    document.getElementById('search-open').setAttribute('aria-expanded', 'true');
    input.focus();
  }
  function close(quiet) {
    if (!pal || pal.hidden) return;
    pal.hidden = true;
    for (const x of behind()) if (x) x.inert = false;
    document.getElementById('search-open').setAttribute('aria-expanded', 'false');
    if (!quiet && opener && opener.isConnected && opener !== document.body) opener.focus();
  }
  SC.palette = { open, close, search: (q) => (INDEX || (INDEX = buildIndex()), search(q).map((r) => r.item)) };

  document.getElementById('search-open').addEventListener('click', open);
  document.addEventListener('keydown', (e) => {
    const typing = e.target && (e.target.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(e.target.tagName));
    const isOpen = pal && !pal.hidden;
    if ((e.key === 'k' || e.key === 'K' || e.code === 'KeyK') && (e.ctrlKey || e.metaKey) && !e.altKey) {
      e.preventDefault();
      if (isOpen) close();
      else open();
    } else if (e.key === '/' && !typing && !e.ctrlKey && !e.metaKey && !isOpen && !document.querySelector('.gd-sheet.gd-open')) {
      e.preventDefault();
      open();
    }
  });
})();
