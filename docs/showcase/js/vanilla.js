'use strict';
// #vanilla: the changed vanilla mobs, 11 groups of data/mobs.json `vanilla.groups` (docs/design/07-vanilla-mobs.md):
// what changes, how the windup reads, the attacks with their ticks, the growth with the player's power P.
// Numbers come from SC.DATA; the prose is from 07-vanilla-mobs.md, «Статус в коде».
(function () {
  const el = SC.el;
  const ROLE = { assault: 'штурмовик', hunter: 'охотник', ranged: 'стрелок' };

  // What each group does and how its windup reads (07-vanilla-mobs.md).
  const TEXT = {
    zombie: ['Выпад с замахом, иногда шаг назад после ванильного удара. Группа из трёх и больше на одну цель: ближний идёт прямо, остальные заходят с боков по очереди. Утопленник с трезубцем держит бой сам: держит дистанцию и бросает трезубец с замахом.',
      'Руки вперёд и рык; утопленник поднимает трезубец над головой.'],
    skeleton: ['Отходит, если цель ближе 4 блоков (и стреляет снова после выстрела). Раз в 100 тиков ищет точку на 2 блока и больше выше цели и идёт туда. После трёх выстрелов пауза 20 тиков.',
      'Ванильное натяжение лука.'],
    spider: ['Прыжок с замахом (ванильный прыжок рядом выключен). Ждёт на уступе выше игрока, пока тот не подойдёт под него, и прыгает вниз.',
      'Шипение и облачко у лап.'],
    witch: ['В 16 блоках держит бой сама: дистанция 6–10, бросок зелья с замахом (замедление издали, яд, вред вблизи). Раненая ниже половины здоровья уходит за укрытие от взгляда цели, ванильное питьё лечит её там.',
      'Смех и искры.'],
    pillager: ['В 20 блоках держит бой сам: дистанция 8–14, выстрел с замахом, перезарядка на пути в укрытие от взгляда цели. Высоту не ищет.',
      'Щелчок арбалета, поза атаки.'],
    vindicator: ['Задержанный удар топором сверху: замах, пауза, удар. В паузе не поворачивается за игроком.',
      'Топор поднят, искры.'],
    piglin_brute: ['В 4 блоках каждый удар с замахом. Промах открывает его на 15 тиков, попадание — на 4.',
      'Злой рык, поза атаки.'],
    hoglin: ['Таран: замах, бросок на 5 блоков. После промаха 20 тиков стоит, не поворачиваясь.',
      'Злой рык, облачко у копыт.'],
    wither_skeleton: ['Серия из двух ударов, между ними пауза; после серии отходит на 3 блока.',
      'Стук костей, поза атаки.'],
    slime: ['Большой прыжок, начиная с размера 2: замах сжатием, приземление бьёт по площади, радиус растёт с размером. Тень приземления не рисуется: это приём топяной слизи.',
      'Хлюпанье, слизь или пламя у ног.'],
    phantom: ['Короткий крик за 10 тиков до пике: пике держится 10 тиков после решения, фантом ещё кружит. В отличие от сорокопута, под крышей не ждёт.',
      'Высокий крик.'],
  };
  const UNTOUCHED = {
    'minecraft:creeper': 'Крипер', 'minecraft:silverfish': 'Чешуйница', 'minecraft:endermite': 'Эндермит', 'minecraft:blaze': 'Ифрит', 'minecraft:ghast': 'Гаст',
    'minecraft:evoker': 'Вызыватель', 'minecraft:ravager': 'Разоритель', 'minecraft:breeze': 'Вихрь', 'minecraft:guardian': 'Страж',
    'minecraft:elder_guardian': 'Древний страж', 'minecraft:creaking': 'Скрипень', 'minecraft:shulker': 'Шалкер', 'minecraft:ender_dragon': 'Эндер-дракон',
    'minecraft:wither': 'Иссушитель', 'minecraft:enderman': 'Эндермен', 'minecraft:zombified_piglin': 'Зомбифицированный пиглин', 'minecraft:piglin': 'Пиглин',
  };
  const typeLabels = (g) => {
    const parts = g.name.split(', ');
    return g.types.map((t, i) => (parts.length === g.types.length ? parts[i] : t.replace('minecraft:', '')) + ' (' + t + ')');
  };

  function attackTable(g) {
    if (!g.attacks.length) return el('p', { class: 'small' }, 'Своих приёмов с замахом нет: поведение — в числах группы ниже, выстрелы и натяжение ванильные.');
    const head = el('tr', {}, ['Приём', 'Замах', 'Пауза', 'Удар', 'Окно', 'Урон', 'Что видно', 'Звук'].map((h) => el('th', {}, h)));
    const rows = g.attacks.map((a) => el('tr', {}, el('td', {}, a.name + (a.only ? ' (' + a.only.map((t) => t.replace('minecraft:', '')).join(', ') + ')' : '')),
      el('td', {}, a.windup + ' т'), el('td', {}, a.hold ? a.hold + ' т' : '—'), el('td', {}, a.hit + ' т'), el('td', {}, a.recover ? a.recover + ' т' : '—'),
      el('td', {}, a.damage ? '× ' + SC.num(a.damage) : '—'), el('td', {}, a.tell),
      el('td', {}, a.cue ? el('code', {}, a.cue.sound.replace('{name}', '<тип>')) : '—')));
    return el('div', { class: 'wide' }, el('table', { class: 'sc-moves' }, el('thead', {}, head), el('tbody', {}, rows)));
  }

  function paramsList(g) {
    const rows = Object.entries(g.params).map(([k, v]) => el('tr', {}, el('td', {}, el('code', {}, k)), el('td', {}, Array.isArray(v) ? v.map(SC.num).join('–') : typeof v === 'number' ? SC.num(v) : String(v))));
    return el('details', {}, el('summary', {}, 'Числа группы (' + rows.length + ')'), el('table', { class: 'stats' }, el('tbody', {}, rows)));
  }

  function card(g) {
    const [what, reads] = TEXT[g.id] || ['', ''];
    return el('section', { class: 'sc-vgroup', id: 'v-' + g.id },
      el('h3', {}, g.name),
      el('p', { class: 'small' }, 'Типы: ' + typeLabels(g).join(', ') + '. Роль в волне: ' + ROLE[g.wave_role] + '.'),
      el('p', {}, el('b', {}, 'Что меняется. '), what),
      el('p', {}, el('b', {}, 'Как читается замах. '), reads),
      attackTable(g), paramsList(g));
  }

  function growth() {
    const z = SC.DATA.vanilla_zombie, f = (v) => SC.num(Math.round(v * 100) / 100);
    return [
      el('p', {}, 'Каждый ванильный моб из групп растёт с силой игрока P (от 0 до 1) по тем же правилам, что и свои мобы: здоровье × (1 + 0,9 · P), урон × (1 + 0,6 · P), броня + 6 · P, скорость × (1 + 0,1 · P), а интервал решений мозга 20 − 10 · P тиков. Для зомби: здоровье ' + z.health + ' → ' + f(z.health * 1.9) + ', урон ' + z.damage + ' → ' + f(z.damage * 1.6) +
        ', броня ' + z.armor + ' → ' + f(z.armor + 6) + '. Книга охотника на ступени «Знаток» показывает этот рост для каждого типа.'),
      el('p', {}, 'Мобы из спавнеров (в ' + SC.DATA.vanilla.spawner_radius_blocks + ' блоках от спавнера) растут по силе P, но приёмов не получают. Мобы волн ивента — по правилам ивента: ванильный моб волны, не зомби, берёт числа, приёмы и роль своей группы (скелет — стрелок).'),
      el('p', { class: 'small' }, 'Приёмы по тикам — гипотеза Г6 и Г13, рост — Г2, числа группы — гипотезы среза. У скелета приёма с замахом нет, у него правила дистанции и паузы (числа группы).'),
    ];
  }

  function page() {
    const ui = SC.ui;
    ui.buildNav('vanilla');
    const main = ui.main, groups = SC.DATA.vanilla.groups;
    main.append(el('h1', {}, 'Ванильные мобы'));
    main.append(el('p', { class: 'sum' }, groups.length + ' групп получают приёмы или правила; остальные не меняются (список ниже). Моб остаётся со своей моделью и целями: мозг SoulsCraft берёт его только на время приёма.'));
    main.append(el('h2', {}, 'Рост от силы P'), ...growth());
    main.append(el('h2', {}, 'Группы'), el('p', { class: 'small' }, groups.map((g, i) => [i ? ' · ' : '', el('a', { href: '#vanilla', onclick: (e) => {
      e.preventDefault();
      document.getElementById('v-' + g.id).scrollIntoView({ behavior: 'smooth', block: 'start' });
    } }, g.name.split(', ')[0])])));
    for (const g of groups) main.append(card(g));
    main.append(el('h2', {}, 'Не трогаем'), el('p', {}, 'Эти мобы уже читаются или не враги: ' + SC.DATA.vanilla.untouched.map((t) => UNTOUCHED[t] || t).join(', ') + '. Нейтральные, боссы и крипер вне изменений.'));
    main.append(el('p', { class: 'sc-pager' }, el('a', { href: '#defense' }, '← Оборона алтаря'), el('a', { href: '#bestiary' }, 'Бестиарий'), el('a', { href: '#recipes' }, 'Предметы и рецепты →')));
    main.append(el('p', { class: 'small sc-cap' }, 'Источник: ' + SC.DATA.vanilla.source));
  }
  SC.page('vanilla', page);
})();
