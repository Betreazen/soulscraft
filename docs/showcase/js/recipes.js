'use strict';
// #recipes: the four horns and where to get them, soul shards, the tier items with their numbers
// and all 24 recipes as a 3x3 grid. Data: SC.EXTRA (js/extra.js from tools/embed-extra.mjs: loot.json,
// the recipe files, the item files, the lang file), SC.DATA (horn_drop, soul_shard), SC.TROPHY_DATA.
(function () {
  const el = SC.el;
  const E = () => SC.EXTRA;
  const pct = (v) => SC.num(Math.round(v * 1000) / 10) + '%';
  const sid = (id) => id.replace('soulscraft:', '');
  const HORNS = ['rotten_horn', 'grave_horn', 'storm_horn', 'abyss_horn'];
  const SPRITE_OF = { brute_maul: 'brute_maul_sprite' }; // the maul's own model is a solid; its slot icon is the sprite
  const blockIcons = {}; // altar block id -> data URL, drawn once the block models are loaded

  // A picture of a soulscraft id: the item sprite, or the block turned three-quarters.
  function icon(id, size) {
    const n = sid(id);
    const it = SC.ITEMS[SPRITE_OF[n] || n];
    const src = blockIcons[n] || (it && !it.model.elements ? it.texture : null);
    return src ? el('img', { class: 'sc-px', src, alt: '', width: size, height: size }) : null;
  }
  const nameOf = (id) => (id[0] === '#' ? SC.EXTRA.vanilla_names[id] : SC.itemName(id));

  function slot(id, count) {
    const mine = id && id.startsWith('soulscraft:');
    const pic = mine ? icon(id, 40) : null;
    return el('div', { class: 'sc-slot' + (id ? '' : ' empty') + (id && !pic ? ' txt' : ''), title: id ? nameOf(id) : '' },
      id ? pic || el('span', {}, nameOf(id)) : null, count > 1 ? el('i', {}, count) : null);
  }

  // The grid of a recipe: a shaped pattern keeps its shape in the middle of the 3x3, a shapeless one
  // is its ingredients from the top left (the order does not matter in the game).
  function craftGrid(r) {
    const cells = Array(9).fill(null);
    if (r.shaped) {
      const x0 = Math.floor((3 - r.width) / 2), y0 = Math.floor((3 - r.height) / 2);
      r.cells.forEach((c, i) => (cells[(y0 + Math.floor(i / r.width)) * 3 + x0 + (i % r.width)] = c));
    } else r.cells.forEach((c, i) => (cells[i] = c));
    return el('div', { class: 'sc-craft', role: 'img', 'aria-label': 'Сетка 3×3: ' + r.cells.filter(Boolean).map(nameOf).join(', ') }, cells.map((c) => slot(c)));
  }

  function legend(r) {
    const count = new Map();
    for (const c of r.cells.filter(Boolean)) count.set(c, (count.get(c) || 0) + 1);
    return [...count].map(([id, n]) => nameOf(id) + (n > 1 ? ' × ' + n : '')).join(', ');
  }

  function recipeCard(r) {
    return el('div', { class: 'recipe sc-recipe', id: 'r-' + r.id },
      el('div', { class: 'rname' }, SC.itemName(r.result) + (r.count > 1 ? ' × ' + r.count : '')),
      el('div', { class: 'sc-craftrow' }, craftGrid(r), el('span', { class: 'sc-arrow', 'aria-hidden': 'true' }, '→'), slot(r.result, r.count)),
      el('p', { class: 'small' }, (r.shaped ? 'По форме' + (r.width * r.height < 9 ? ' (можно двигать по сетке)' : '') : 'Без формы, в любом порядке') + ': ' + legend(r) + '.'));
  }

  const recipeOf = (item) => E().recipes.find((r) => r.result === 'soulscraft:' + item);
  const link = (item) => el('a', { href: '#recipes', onclick: (e) => {
    e.preventDefault();
    const t = document.getElementById('r-' + item);
    if (t) t.scrollIntoView({ behavior: 'smooth', block: 'center' });
  } }, 'рецепт ниже');

  // «по форме: …» for js/altar.js (the blocks page, the banner pages).
  SC.recipeText = (n) => {
    const r = recipeOf(n);
    return r ? (r.shaped ? 'по форме' : 'без формы') + ': ' + legend(r) + (r.count > 1 ? ' → ' + r.count + ' шт.' : '') : null;
  };

  // ------------------------------------------------------------------ horns
  function whereTo(item) {
    const id = 'soulscraft:' + item, loot = E().loot, rows = [];
    const chests = loot.chests.filter((c) => c.item === id);
    if (chests.length)
      rows.push(['Сундуки', chests.map((c) => E().chest_names[c.table] + ' — ' + pct(c.chance)).join('; ')]);
    for (const k of loot.killed_by_player.filter((x) => x.item === id))
      rows.push(['Ванильные зомби', k.tables.map((t) => SC.waveName({ type: t.replace('minecraft:entities/', 'minecraft:') })).join(', ') + ', убитые игроком — ' + pct(k.chance)]);
    const h = SC.DATA.horn_drop;
    if (h.item === id) rows.push(['Мобы SoulsCraft', 'обычный моб — ' + pct(h.chance) + ' за засчитанное убийство']);
    if (h.elite_item === id) rows.push(['Элитный моб (громила)', pct(h.elite_chance) + ' за засчитанное убийство']);
    rows.push(['Крафт', recipeOf(item) ? link(item) : '—']);
    return rows;
  }

  function hornCard(item, i) {
    const ev = SC.DATA.events.find((e) => e.item === 'soulscraft:' + item);
    return el('div', { class: 'recipe sc-horn' },
      el('div', { class: 'rname' }, 'Ступень ' + (i + 1) + ' · ' + SC.itemName('soulscraft:' + item)),
      el('div', { class: 'sc-craftrow' }, icon('soulscraft:' + item, 64), el('p', { class: 'small' }, 'Запускает ивент «' + ev.name + '» на алтаре уровня ' + (i + 1) + ' и выше. Тратится при старте. Id: soulscraft:' + item + '.')),
      el('table', { class: 'stats' }, el('tbody', {}, whereTo(item).map(([k, v]) => el('tr', {}, el('td', {}, k), el('td', {}, v))))));
  }

  function hornsSection() {
    const chestNote = E().loot.chest_shards;
    return [el('h2', {}, 'Горны'),
      el('p', {}, 'Четыре горна, по одному на ступень ивента. Горн можно найти или собрать: найденный пропускает крафт, но не уровень алтаря. Горн бури требует любую голову моба SoulsCraft, горн бездны — голову громилы. Все шансы — гипотезы до плейтестов с людьми.'),
      el('div', { class: 'sc-loot' }, HORNS.map(hornCard)),
      el('p', { class: 'small' }, 'В тех же сундуках с шансом ' + pct(chestNote.chance) + ' лежат осколки души, от ' + chestNote.count[0] + ' до ' + chestNote.count[1] + ' штук.')];
  }

  // ------------------------------------------------------------------ shards
  function shardSection() {
    const s = SC.DATA.soul_shard, c = E().loot.chest_shards, tiers = SC.TROPHY_DATA.tiers;
    const uses = E().recipes.filter((r) => r.cells.includes('soulscraft:soul_shard')).length;
    const rows = [
      ['Моб SoulsCraft', pct(s.chance) + ', ' + s.count + ' шт. за засчитанное убийство; элитный — ' + pct(s.elite_chance) + ', ' + s.elite_count + ' шт.'],
      ['Сундуки структур', pct(c.chance) + ', от ' + c.count[0] + ' до ' + c.count[1] + ' шт.'],
      ['Победа в ивенте', tiers.map((t) => 'ступень ' + t.tier + ' — ' + t.shards).join(', ') + ' шт. всегда'],
      ['Нужен для', uses + ' рецептов (блоки алтаря, знамёна, горны) и починки клинка и молота на наковальне'],
    ];
    return [el('h2', {}, 'Осколок души'),
      el('div', { class: 'sc-craftrow' }, icon('soulscraft:soul_shard', 64), el('p', {}, 'Валюта алтаря: из них собирают блоки, знамёна и горны. Падает с мобов SoulsCraft, из сундуков и за победу в ивентах.')),
      el('table', { class: 'stats' }, el('tbody', {}, rows.map(([k, v]) => el('tr', {}, el('td', {}, k), el('td', {}, v)))))];
  }

  // ------------------------------------------------------------------ tier items
  const f = (v) => SC.num(Math.round(v * 100) / 100);
  function tierItems() {
    const attr = (it, type) => (it.attributes.find((a) => a.type === type) || {}).amount;
    const rows = [];
    for (const [n, kind, special] of [
      ['revenant_blade', 'меч', '+2 к броне, пока в главной руке'],
      ['brute_maul', 'топор', '+1 к отбрасыванию; топор сбивает щит'],
      ['abyss_heart', 'талисман', '+4 к здоровью (два сердца), пока лежит во второй руке'],
    ]) {
      const it = E().items[n], tr = SC.TROPHY_DATA.tiers.find((t) => 'soulscraft:' + n === t.item);
      const dmg = attr(it, 'attack_damage'), spd = attr(it, 'attack_speed');
      rows.push(el('tr', {},
        el('td', {}, el('div', { class: 'sc-craftrow' }, icon('soulscraft:' + n, 40), el('b', {}, SC.itemName('soulscraft:' + n)))),
        el('td', {}, kind), el('td', {}, 'ступень ' + tr.tier + ', ' + pct(tr.chance)),
        el('td', {}, dmg == null ? '—' : f(1 + dmg)), el('td', {}, spd == null ? '—' : f(4 + spd)), el('td', {}, it.max_damage == null ? '—' : it.max_damage),
        el('td', {}, special + (it.repairable ? '; чинится осколком души' : ''))));
    }
    return [el('h2', {}, 'Предметы ступеней'),
      el('p', {}, 'Награда за победу в ивенте ступени 2–4 с шансом; умножается на знамёна «сложнее». Нигде больше не добыть, своего рецепта нет.'),
      el('div', { class: 'wide' }, el('table', { class: 'sc-moves' },
        el('thead', {}, el('tr', {}, ['Предмет', 'Основа', 'Выдаёт', 'Урон', 'Ударов в с', 'Прочность', 'Особенность'].map((h) => el('th', {}, h)))),
        el('tbody', {}, rows), el('caption', { class: 'sc-cap' }, 'Ориентир — ванильные мечи и топоры: алмазный меч 7 урона и 1,6 удара в секунду, незеритовый 8; алмазный топор 9 урона и 1 удар в секунду. Сердце бездны действует через сервер мода: он дважды в секунду смотрит вторую руку и надевает или снимает прибавку здоровья.')))];
  }

  // ------------------------------------------------------------------ all recipes
  function recipesSection() {
    const groups = [['Блоки алтаря', (r) => r.id.startsWith('altar_')], ['Знамёна', (r) => r.id.startsWith('banner_')],
      ['Горны и книга', (r) => !r.id.startsWith('altar_') && !r.id.startsWith('banner_')]];
    const out = [el('h2', {}, E().recipes.length + ' рецептов'),
      el('p', {}, 'Сетка 3×3, как в верстаке. Предметы SoulsCraft нарисованы, ванильные подписаны. Все знамёна собираются из белого знамени и двух осколков души, различается четвёртый предмет.')];
    for (const [title, test] of groups) out.push(el('h3', {}, title), el('div', { class: 'sc-loot' }, E().recipes.filter(test).map(recipeCard)));
    return out;
  }

  function page() {
    const ui = SC.ui;
    ui.buildNav('recipes');
    const main = ui.main;
    main.append(el('h1', {}, 'Предметы и рецепты'));
    main.append(el('p', { class: 'sum' }, 'Что нужно, чтобы запустить ивент: горн своей ступени, алтарь, осколки души. Где найти горны, чем платить за блоки алтаря и что получить за победу.'));
    const box = el('div');
    main.append(box);
    // The altar blocks of the results are drawn from their models, so they wait for the glb.
    SC.blocksReady().then(() => {
      for (const r of E().recipes) {
        const n = sid(r.result);
        if (n.startsWith('altar_') && !blockIcons[n]) blockIcons[n] = SC.pieceSnapshot(SC.blockPiece(n, {}, 0));
      }
      box.append(...hornsSection(), ...shardSection(), ...tierItems(), ...recipesSection());
      box.append(el('p', { class: 'sc-pager' }, el('a', { href: '#vanilla' }, '← Ванильные мобы'), el('a', { href: '#altar/blocks' }, 'Блоки алтаря'), el('a', { href: '#event' }, 'Ивенты →')));
    }).catch((e) => {
      const msg = String(e && e.stack ? e.stack : e);
      console.error(msg);
      box.append(el('div', { class: 'sc-err' }, msg));
    });
  }
  SC.page('recipes', page);
})();
