// Writes docs/showcase/js/extra.js: what the handbook needs and the Blender build (assets/models/build.py)
// does not embed. Run from anywhere: node docs/showcase/tools/embed-extra.mjs
//   data/loot.json                              -> chests, shards in chests, drops of vanilla zombies
//   mod/content/data/soulscraft/recipe/*.json   -> the 24 recipes as a 3x3 grid
//   mod/content/data/soulscraft/horizon/item/   -> numbers of the items (damage, durability, health)
//   mod/content/assets/soulscraft/lang/ru_ru.json -> names of items, blocks, mobs
// js/extra.js registers it as SC.extra({...}) like js/data.js registers SC.data (js/core.js).
import { readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..', '..', '..');
const read = (...p) => JSON.parse(readFileSync(join(root, ...p), 'utf8'));
const listJson = (...p) => readdirSync(join(root, ...p)).filter((f) => f.endsWith('.json')).sort();

// Vanilla names for the ingredients the recipes use (the lang file holds only the mod's own).
const VANILLA_NAMES = {
  'minecraft:deepslate_bricks': 'Глубинносланцевые кирпичи', 'minecraft:diamond': 'Алмаз', 'minecraft:rotten_flesh': 'Гнилая плоть',
  'minecraft:iron_ingot': 'Железный слиток', 'minecraft:bone': 'Кость', 'minecraft:gold_ingot': 'Золотой слиток',
  'minecraft:netherite_scrap': 'Незеритовый лом', 'minecraft:ender_pearl': 'Жемчуг Края', 'minecraft:soul_campfire': 'Костёр душ',
  'minecraft:crying_obsidian': 'Плачущий обсидиан', 'minecraft:amethyst_shard': 'Аметистовый осколок', 'minecraft:white_banner': 'Белое знамя',
  'minecraft:clock': 'Часы', 'minecraft:sugar': 'Сахар', 'minecraft:iron_sword': 'Железный меч', 'minecraft:feather': 'Перо',
  'minecraft:slime_ball': 'Шарик слизи', 'minecraft:skeleton_skull': 'Череп скелета', 'minecraft:cod': 'Сырая треска',
  'minecraft:golden_apple': 'Золотое яблоко', 'minecraft:shield': 'Щит', 'minecraft:campfire': 'Костёр', 'minecraft:anvil': 'Наковальня',
  'minecraft:book': 'Книга', 'minecraft:blaze_rod': 'Огненный стержень', '#soulscraft:heads': 'Любая голова моба SoulsCraft',
  'minecraft:zombie': 'Зомби', 'minecraft:husk': 'Кадавр', 'minecraft:drowned': 'Утопленник', 'minecraft:zombie_villager': 'Зомби-житель',
};
// Chest tables of loot.json by the structure they belong to.
const CHEST_NAMES = {
  'minecraft:chests/simple_dungeon': 'Подземелье', 'minecraft:chests/abandoned_mineshaft': 'Заброшенная шахта',
  'minecraft:chests/desert_pyramid': 'Пустынный храм', 'minecraft:chests/jungle_temple': 'Храм в джунглях',
  'minecraft:chests/pillager_outpost': 'Застава разбойников', 'minecraft:chests/woodland_mansion': 'Лесной особняк',
  'minecraft:chests/stronghold_corridor': 'Крепость, коридор', 'minecraft:chests/bastion_other': 'Бастион, обычные комнаты',
  'minecraft:chests/bastion_bridge': 'Бастион, мост', 'minecraft:chests/bastion_treasure': 'Бастион, сокровищница',
  'minecraft:chests/ancient_city': 'Древний город', 'minecraft:chests/end_city_treasure': 'Город Края',
};

// A recipe as a 3x3 grid: nine cells (an item id, a '#tag' or null), left to right, top to bottom.
// A shaped pattern keeps its place in the grid, as the game puts it (rows pad to the right, the
// grid pads at the bottom); a shapeless recipe is its ingredients in a row, for any order.
function recipe(file) {
  const r = read('mod', 'content', 'data', 'soulscraft', 'recipe', file);
  const out = { id: file.replace(/\.json$/, ''), result: r.result.id, count: r.result.count, shaped: r.type.endsWith('shaped') };
  if (out.shaped) {
    const width = Math.max(...r.pattern.map((row) => row.length));
    out.width = width;
    out.height = r.pattern.length;
    out.cells = r.pattern.flatMap((row) => row.padEnd(width, ' ').split('').map((c) => (c === ' ' ? null : r.key[c])));
  } else {
    out.cells = [...r.ingredients];
  }
  return out;
}

const recipes = listJson('mod', 'content', 'data', 'soulscraft', 'recipe').map(recipe);
const items = {};
for (const f of listJson('mod', 'content', 'data', 'soulscraft', 'horizon', 'item')) {
  const it = read('mod', 'content', 'data', 'soulscraft', 'horizon', 'item', f);
  const c = it.components || {};
  items[f.replace(/\.json$/, '')] = {
    template: it.template, max_stack_size: it.max_stack_size, rarity: it.rarity,
    max_damage: c['minecraft:max_damage'] ?? null,
    attributes: (c['minecraft:attribute_modifiers'] || []).map((a) => ({ type: a.type.replace('minecraft:', ''), amount: a.amount, slot: a.slot })),
    repairable: c['minecraft:repairable'] ? c['minecraft:repairable'].items : null,
  };
}
const used = new Set(recipes.flatMap((r) => r.cells.filter(Boolean)));
const data = {
  loot: read('data', 'loot.json'),
  chest_names: CHEST_NAMES,
  recipes,
  items,
  lang: read('mod', 'content', 'assets', 'soulscraft', 'lang', 'ru_ru.json'),
  vanilla_names: VANILLA_NAMES,
};
const missing = [...used].filter((id) => !id.startsWith('soulscraft:') && !VANILLA_NAMES[id]);
if (missing.length) throw new Error('no Russian name for: ' + missing.join(', '));
const lootTables = new Set([...data.loot.chests.map((c) => c.table)]);
for (const t of lootTables) if (!CHEST_NAMES[t]) throw new Error('no name for the chest table ' + t);

const body = "'use strict';\n// Generated by docs/showcase/tools/embed-extra.mjs. Do not edit by hand.\nSC.extra(" + JSON.stringify(data) + ');\n';
writeFileSync(join(root, 'docs', 'showcase', 'js', 'extra.js'), body);
console.log('js/extra.js: ' + recipes.length + ' recipes, ' + Object.keys(items).length + ' items, ' + data.loot.chests.length + ' chest entries, ' + body.length + ' bytes');
