'use strict';
// #defense: the altar's defense (docs/design/06-altar.md, «Оборона алтаря», round 7; numbers are
// hypothesis Г12 in data/mobs.json events[].defense, read here from SC.DATA) and the rules an
// event holds a player to: one life, the hardcore spare, the outline of the zone.
(function () {
  const el = SC.el;
  const SVG = 'http://www.w3.org/2000/svg';
  const def = () => SC.DATA.events[0].defense;
  const sec = (ticks) => SC.num(Math.round((ticks / SC.TICK) * 100) / 100);

  const ROLES = {
    assault: ['Штурмовик', 'Идёт к алтарю; игрока бьёт только при перехвате.'],
    hunter: ['Охотник', 'Идёт к ближайшему участнику; к алтарю, если в зоне участников нет.'],
    ranged: ['Стрелок', 'Стреляет в ближайшую видимую цель: игрока или алтарь.'],
  };

  // Who has which role: own mobs by `wave_role` of data/mobs.json, vanilla groups by theirs.
  function rolesTable() {
    const by = { assault: [], hunter: [], ranged: [] };
    for (const m of SC.DATA.mobs) by[m.wave_role].push(m.name);
    for (const g of SC.DATA.vanilla.groups) by[g.wave_role].push(g.name.split(', ')[0].toLowerCase());
    by.assault.push('зомби-знаменосец');
    return el('div', { class: 'wide' }, el('table', { class: 'sc-moves' },
      el('thead', {}, el('tr', {}, ['Роль', 'Цель', 'Мобы'].map((h) => el('th', {}, h)))),
      el('tbody', {}, Object.keys(ROLES).map((k) => el('tr', {}, el('td', {}, el('b', {}, ROLES[k][0])), el('td', {}, ROLES[k][1]), el('td', {}, by[k].join(', ')))))),
      el('p', { class: 'small sc-cap' }, 'Роль — поле wave_role в данных. Состав ролей в волне меняет давление: штурмовики проверяют, успевает ли игрок к алтарю, охотники — выдержит ли он сам.'));
  }

  // A plan view: the altar in the middle, the player, the rings of interception and of return, the beam.
  function scheme() {
    const d = def();
    const k = 7; // px a block
    const s = document.createElementNS(SVG, 'svg');
    s.setAttribute('viewBox', '-130 -130 260 260');
    s.setAttribute('class', 'sc-scheme');
    s.setAttribute('role', 'img');
    s.setAttribute('aria-label', 'Схема: алтарь, игрок на пути волны, кольцо перехвата ' + d.intercept_blocks + ' блоков и кольцо возврата ' + d.return_blocks + ' блоков');
    const add = (tag, attrs, text) => {
      const n = document.createElementNS(SVG, tag);
      for (const [a, v] of Object.entries(attrs)) n.setAttribute(a, v);
      if (text) n.textContent = text;
      s.append(n);
      return n;
    };
    const px = { x: 55, y: 0 };
    add('rect', { x: -9, y: -9, width: 18, height: 18, class: 'sc-s-altar' });
    add('text', { x: 0, y: -14, class: 'sc-s-t', 'text-anchor': 'middle' }, 'алтарь');
    add('circle', { cx: px.x, cy: px.y, r: d.return_blocks * k * 0.55, class: 'sc-s-ring far' });
    add('circle', { cx: px.x, cy: px.y, r: d.intercept_blocks * k * 0.55, class: 'sc-s-ring near' });
    add('text', { x: px.x, y: px.y - d.intercept_blocks * k * 0.55 - 3, class: 'sc-s-t', 'text-anchor': 'middle' }, d.intercept_blocks + ' блоков: перехват');
    add('text', { x: px.x, y: px.y + d.return_blocks * k * 0.55 + 11, class: 'sc-s-t', 'text-anchor': 'middle' }, d.return_blocks + ' блоков: возврат к алтарю');
    add('circle', { cx: px.x, cy: px.y, r: 4, class: 'sc-s-player' });
    add('text', { x: px.x + 7, y: px.y + 3, class: 'sc-s-t' }, 'игрок');
    // a mob walking in, one the player stopped, one at the altar with a beam
    add('line', { x1: -100, y1: -62, x2: px.x - 22, y2: px.y - 12, class: 'sc-s-path' });
    add('circle', { cx: px.x - 22, cy: px.y - 12, r: 3.5, class: 'sc-s-mob' });
    add('circle', { cx: -34, cy: 26, r: 3.5, class: 'sc-s-mob' });
    add('line', { x1: -34, y1: 26, x2: -6, y2: 6, class: 'sc-s-beam' });
    add('text', { x: -124, y: -68, class: 'sc-s-t' }, 'волна идёт к алтарю');
    add('text', { x: -124, y: 52, class: 'sc-s-t' }, 'у алтаря: луч вытягивания');
    return s;
  }

  function section(title, ...kids) {
    return [el('h2', {}, title), ...kids];
  }

  function page() {
    const ui = SC.ui;
    ui.buildNav('defense');
    const main = ui.main, d = def(), b = d.breach;
    main.append(el('h1', {}, 'Оборона алтаря'));
    main.append(el('p', { class: 'sum' }, 'Волна идёт к алтарю, а игрок решает, где её остановить. Числа — гипотеза Г12, книга охотника пересказывает то же в записи «Оборона алтаря».'));
    const wrap = el('div', { class: 'cols' }, el('div', { class: 'text' },
      el('h2', {}, 'Роли и перехват'),
      el('p', {}, 'Цель волны — алтарь. Моб, которого игрок ударил или к которому подошёл ближе ' + d.intercept_blocks + ' блоков, переключается на игрока, если видит его или может к нему пройти: за сплошной стеной игрок моба не тянет. Игрок ушёл дальше ' + d.return_blocks +
        ' блоков или ' + d.return_unseen_seconds + ' секунд не показывался — моб возвращается к алтарю.')),
      el('aside', {}, scheme()));
    main.append(wrap, rolesTable());

    main.append(...section('Вытягивание силы',
      el('p', {}, 'У алтаря моб встаёт и вытягивает из него силу: замах ' + d.drain_windup_ticks + ' тиков (' + sec(d.drain_windup_ticks) + ' с), от моба к ядру тянется тонкий луч. Удар по мобу в замахе сбивает вытягивание, как любую атаку с замахом. Потом луч полный: алтарь теряет здоровье, ' + SC.num(d.drain_per_second) + ' в секунду от каждого моба.'),
      el('p', {}, 'Предел: алтарь теряет не больше ' + d.altar_damage_cap_per_second + ' здоровья в секунду, как бы плотно ни стояла волна. Есть секунды, чтобы добежать. Пока алтарь теряет здоровье, полоска ивента мигает и звучит сигнал.')));

    main.append(...section('Путь к алтарю',
      el('p', {}, 'Ядро раз в волну строит одну карту путей от алтаря на всю зону 3×3 чанка и обновляет её, только когда в зоне меняется блок (не чаще раза в секунду). Мобы идут по ней, а не ищут путь каждый сам. Есть путь, пусть длинный — идут по нему. Стены, коридоры, ловушки и турели RustCraft — законная оборона: лабиринт ведёт врагов через простреливаемые места мимо игрока. Турели мобы не трогают. Лестниц мобы не строят.')));

    main.append(...section('Проклятие',
      el('p', {}, 'Моб, который ' + d.curse_after_seconds + ' секунд не может дойти до алтаря, встаёт где стоит и тянет к ядру проклятие: луч сквозь блоки, слабее вытягивания вблизи (' + SC.num(d.curse_per_second) + ' здоровья в секунду) и виден издалека. Снять его можно только убив моба. Летуны (сорокопут) атакуют алтарь напрямую и проклятия не знают.')));

    const rows = b.hits_by_hardness.map(([h, n]) => ['твёрдость до ' + h, n + ' ' + (n === 1 ? 'удар' : n < 5 ? 'удара' : 'ударов')]);
    main.append(...section('Пролом громилы',
      el('p', {}, 'Громила — единственный моб, который ломает блоки. Стену между собой и алтарём по карте путей он бьёт ударом сверху с волной, замах тот же, что обычно. Ломает только в зоне и пока ивент идёт. Число ударов растёт с твёрдостью блока:'),
      el('table', { class: 'stats' }, el('tbody', {}, rows.map(([k, v]) => el('tr', {}, el('td', {}, k), el('td', {}, v))))),
      el('p', {}, el('b', {}, 'Никогда не ломает: '), 'блоки алтаря и знамёна, всё из пространства имён rustcraft:, блоки с содержимым (сундуки, печи, всё, что хранит данные), неразрушимые блоки (' +
        b.never_blocks.map((x) => x.replace('minecraft:', '')).join(', ') + ').'),
      el('p', {}, el('b', {}, 'Возврат стен. '), 'Ядро запоминает каждый сломанный блок и его состояние и ставит обратно в конце ивента, и при победе, и при провале. Если на этом месте игрок поставил своё, его блок остаётся. Сломанные громилой блоки добычи не дают: они вернутся.')));

    main.append(...section('Одна жизнь и хардкор',
      el('p', {}, el('b', {}, 'Одна жизнь. '), 'Смерть участника выводит его из ивента: добыча не пропадает, но играть дальше этот ивент он не будет.'),
      el('p', {}, el('b', {}, 'Страховка хардкора. '), 'Смерть в ивенте не уничтожает персонажа, даже на хардкорном сервере: когда здоровье участника кончается, ему остаётся 1, и смерти не происходит (нет выпадения вещей, нет экрана смерти, нет режима наблюдателя). На следующем тике игрок выбывает из ивента, лечится, получает сопротивление и регенерацию на 10 секунд и встаёт за краем зоны со своей стороны. Если пола под столбцом нет (чанк не загружен), остаётся на месте.')));

    main.append(...section('Зона и провал',
      el('p', {}, 'Зона — 3×3 чанка с чанком алтаря в центре, ' + SC.DATA.events[0].flow.below_blocks + ' блоков вниз и вверх. На старте ивента её контур виден несколько секунд: на границе встают столбики частиц. Сила врагов — средняя сила P всех участников в зоне на старте плюс знамёна.'),
      el('p', {}, el('b', {}, 'Провал: '), 'здоровье алтаря кончилось или в зоне ' + sec(SC.DATA.events[0].flow.participant_grace_ticks) + ' секунды нет живых участников (игрок может на миг выйти). Оставшиеся мобы волны исчезают.')));

    main.append(el('p', { class: 'sc-pager' }, el('a', { href: '#event' }, '← Ивенты'), el('a', { href: '#altar' }, 'Алтарь'), el('a', { href: '#vanilla' }, 'Ванильные мобы →')));
    main.append(el('p', { class: 'small sc-cap' }, 'Источник: ' + d.source));
  }
  SC.page('defense', page);
})();
