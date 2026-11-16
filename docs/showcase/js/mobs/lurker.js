'use strict';
// Lurker: notes and behaviour scenes of its showcase page. Numbers come from data/mobs.json
// (js/data.js), phase ticks from its rig (js/rigs.js); the scenes below only place and time.
// Player at the origin; the lurker comes from -z (yaw 180 looks at -z, away from the player).
//
// Both scenes are under water, in the dark: the scene `env` gives the dark water colour, the fog
// and the dim light (glowing parts are never fogged), and `lift` the height above the sea floor.
SC.mobScript('lurker', function () {
  // Deep water below Y = 40: black-blue, a faint cold light from above.
  const DEEP = { background: '#03080f', fog: 0.05, light: 0.6, ambientColor: '#4f7290', sun: 0.12, sunColor: '#9cc6e6',
    ground: '#7d93ab' };

  // ------------------------------------------------------------------ scenes
  const SWORD = { socket_hand_r: 'rusty_sword' };
  // Heights: the lurker's mouth is 0.44 blocks above its origin, the player's chest 1.15 above
  // theirs, so in the grab the lurker floats 0.7 blocks above the player's feet.
  const BITE = 0.7;

  // (a) It creeps up while the player looks away, the lure goes dark, the lunge, the grab, three
  // hits to break free, and it leaves into the dark.
  const ambush = {
    id: 'ambush',
    title: 'Засада в темноте',
    intro: 'Глубина ниже Y = 40, света нет. Видна только приманка. Удильщик подплывает, пока игрок на него не смотрит, гасит приманку и бросается.',
    ticks: 300,
    phases: 'lurker', // the grab cuts the lunge short: its 20-tick window does not happen here
    env: DEEP,
    eye: [10.5, 3.4, -3.6],
    target: [0, 2.5, -3.6],
    actors: {
      lurker: {
        model: 'lurker',
        clips: [[0, 'idle'], [30, 'drift'], [64, 'idle'], [100, 'drift'], [140, 'attack_lunge'], [156, 'grab'],
          [189, 'release'], [203, 'retreat'], [260, 'idle']],
        pos: [[0, [0, -7.6]], [30, [0, -7.6]], [64, [0, -5.6]], [100, [0, -5.6]], [140, [0, -3.6]], [150, [0, -3.6]],
          [156, [0, -0.6]], [189, [0, -0.6]], [197, [0, -1.9]], [205, [0, -2.2]], [260, [0, -12]]],
        lift: [[0, 1.6], [150, 1.6], [156, 1.0 + BITE], [189, 0.35 + BITE], [203, 1.1], [260, 1.6]],
        face: [[0, 'player'], [203, 180]],
        turn: 14,
      },
      player: {
        model: '_player',
        hold: SWORD,
        clips: [[0, 'idle'], [156, 'hurt'], [164, 'swing'], [172, 'idle'], [174, 'swing'], [182, 'idle'], [184, 'swing'],
          [192, 'idle']],
        pos: [[0, [0, 0]]],
        lift: [[0, 1.0], [156, 1.0], [189, 0.35], [240, 0.9]],
        face: [[0, 0], [64, 'lurker'], [92, 0], [156, 'lurker']],
        turn: 12,
      },
    },
    captions: [
      [0, 'Темнота. Удильщик висит неподвижно в 8 блоках; видна только приманка.'],
      [30, 'Игрок смотрит в другую сторону. Удильщик подплывает: тело чёрное, огонёк приманки висит ровно.'],
      [64, 'Игрок оборачивается. Удильщик замирает: пока на него смотрят, он не приближается.'],
      [92, 'Игрок отвернулся. Удильщик подбирается на 3–4 блока.'],
      [140, 'Замах 10 тиков: пасть раскрывается, приманка гаснет. В темноте это видно только как пропавший огонёк.'],
      [150, 'Бросок: до 6 блоков, урон × 1,3.'],
      [156, 'Хватка: удильщик держит игрока в пасти до 40 тиков, мотает и тянет вниз.'],
      [169, 'Удар 1 из 3.'],
      [179, 'Удар 2 из 3.'],
      [189, 'Третий удар: удильщик разжимает пасть и отшатывается.'],
      [203, 'Уходит в темноту с погасшей приманкой.'],
      [260, 'Далеко в темноте приманка зажигается снова. Засада начинается сначала.'],
    ],
    marks: [[64, 'смотрит'], [144, 'гаснет'], [169, '1'], [179, '2'], [189, '3']],
  };

  // (b) The player carries a light and watches it: the tell is plain, the lunge misses, and the
  // 20-tick window after it is the time to hit.
  const light = {
    id: 'light',
    title: 'Со светом бросок мимо',
    intro: 'Игрок несёт свет и смотрит на удильщика. Замах видно целиком, уворот вбок оставляет бросок в пустой воде, после него окно 20 тиков.',
    ticks: 200,
    phases: 'lurker',
    env: Object.assign({}, DEEP, { fog: 0.06, lamp: { actor: 'player' } }),
    eye: [-9, 3.4, -1.7],
    target: [0.9, 2.5, -1.9],
    actors: {
      lurker: {
        model: 'lurker',
        clips: [[0, 'idle'], [40, 'attack_lunge'], [76, 'retreat'], [150, 'idle']],
        pos: [[0, [0, -5]], [50, [0, -5]], [56, [0, 0.6]], [76, [0, 1.3]], [150, [-1, -12]]],
        lift: [[0, 1.7], [76, 1.7], [150, 1.9]],
        face: [[0, 'player'], [46, null], [76, 180]],
        turn: 10,
      },
      player: {
        model: '_player',
        hold: SWORD,
        clips: [[0, 'idle'], [45, 'dodge'], [53, 'idle'], [56, 'walk'], [62, 'swing'], [70, 'swing'], [78, 'idle']],
        pos: [[0, [0, 0]], [45, [0, 0]], [52, [2.4, 0.3]], [56, [2.4, 0.3]], [62, [1.5, -0.6]]],
        lift: [[0, 1.0]],
        face: [[0, 'lurker']],
        turn: 18,
      },
    },
    captions: [
      [0, 'Игрок с фонарём смотрит на удильщика. В свете видно тело и пасть, и он не подплывает.'],
      [40, 'Он всё же бросается. Замах 10 тиков: пасть раскрывается, приманка гаснет. В свете это видно хорошо.'],
      [45, 'Игрок видит замах и уходит вбок.'],
      [50, 'Бросок на 6 блоков приходится в пустую воду.'],
      [56, 'Пасть захлопнулась впустую. Окно 20 тиков: удильщика проносит по инерции, он открыт.'],
      [62, 'Игрок подплывает и бьёт в окно.'],
      [76, 'Окно закрылось. Удильщик уходит в темноту.'],
      [150, 'Вдали приманка снова светится.'],
    ],
    marks: [[44, 'гаснет'], [45, 'уворот'], [56, 'мимо']],
  };

  SC.mob('lurker', {
    fields: [
      ['grab_max_ticks', '{attacks.grab.grab_max_ticks}', 'после попавшего броска хватка длится не дольше {attacks.grab.grab_max_ticks} тиков'],
      ['pulls_down', '{attacks.grab.pulls_down}', 'цель каждый тик уходит к пасти удильщика и на {attacks.grab.pull_down_per_tick} блока вниз'],
      ['damage', '{attacks.grab.damage|x}', 'урон хватки раз в {attacks.grab.grab_hit_every_ticks} тиков'],
      ['break_hits', '{attacks.grab.break_hits}', '{attacks.grab.break_hits} удара игрока по удильщику освобождают цель (считаются удары схваченного игрока); удильщик уходит в темноту (backoff)'],
    ],
    notes: [
      'Приманка — две части на конце стебля: светящаяся луковица lure (glow: в игре полная яркость, на витрине эмиссия) и тёмная lure_dark на том же месте, чуть меньше. «Погасить» приманку значит сжать светящуюся луковицу в ноль (масштаб части в клипе): остаётся тёмная. Переключение занимает один тик.',
      'Стебель — цепочка из трёх частей и колпачка. В покое он качается с запаздыванием, при броске его отбрасывает назад потоком.',
      'Бросок и хватка — два клипа: attack_lunge (замах 10, бросок 6, окно 20 тиков из mobs.json) и grab (40 тиков, по кругу). Освобождение после третьего удара — клип release, уход в темноту — retreat с тёмной приманкой, подкрадывание — drift. Клип walk повторяет swim: игра крутит клип движения по шагу.',
      'Сцены идут под водой: тёмный фон, туман и тусклый свет — это среда сцены (env), высота над дном — дорожка lift. Светящиеся части туман не прячет.',
    ],
    scenes: [ambush, light],
  });
});
