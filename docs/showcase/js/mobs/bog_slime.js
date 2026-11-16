'use strict';
// Bog slime: notes and behaviour scenes of its showcase page. Numbers come from data/mobs.json
// (js/data.js), phase ticks from its rig (js/rigs.js); the scenes below only place and time.
// Player at the origin; the slime comes from -z.
//
// Sizes 1, 2, 4 are the actor `size`; the jumps are `lift` hops (SC.hop); the children of the
// split appear with `show`; the landing half of attack_jump is a clip row started from its hit
// tick with no crossfade ([tick, clip, from, 0]). The shadow, the mud wave and the splashes are
// drawn props (`make`): in the game they are particles.
SC.mobScript('bog_slime', function () {
  const ID = 'bog_slime';

  // ------------------------------------------------------------------ drawn props
  const flat = (T, geo, color, opacity) => {
    const m = new T.Mesh(geo, new T.MeshBasicMaterial({ color, transparent: true, opacity, depthWrite: false }));
    m.rotation.x = -Math.PI / 2;
    return m;
  };

  // The shadow at the landing spot, darker while the slime comes down.
  const shadow = (p) => ({
    make(group, T) {
      const m = flat(T, new T.CircleGeometry(p.r, 40), 0x000000, 0.5);
      m.position.set(p.at[0], 0.012, p.at[1]);
      group.add(m);
      return (t) => {
        m.visible = t >= p.from && t < p.to;
        const u = Math.max(0, Math.min(1, (t - p.from) / (p.to - p.from)));
        m.material.opacity = 0.22 + 0.4 * u;
      };
    },
  });
  // The mud wave of the landing: a ring running out to the radius, and the danger zone.
  const wave = (p) => ({
    make(group, T) {
      const ring = flat(T, new T.RingGeometry(0.8, 1, 48), 0x5a4a2c, 0.9);
      const zone = flat(T, new T.CircleGeometry(p.r, 48), 0x6b5530, 0.22);
      const edge = flat(T, new T.RingGeometry(p.r - 0.04, p.r, 64), 0xc9a25a, 0.7);
      for (const m of [ring, zone, edge]) {
        m.position.set(p.at[0], 0.02, p.at[1]);
        group.add(m);
      }
      return (t) => {
        const u = (t - p.t0) / p.dur;
        ring.visible = u >= 0 && u <= 1;
        zone.visible = edge.visible = t >= p.t0 && t < p.t0 + p.dur + p.linger;
        if (ring.visible) {
          const r = 0.3 + (p.r - 0.3) * Math.sqrt(u);
          ring.scale.set(r, r, 1);
          ring.material.opacity = 0.95 - 0.5 * u;
        }
      };
    },
  });
  // Mud splashes flying out of a landing or a burst.
  const splash = (p) => ({
    make(group, T) {
      const n = p.n || 8, s = p.size || 1;
      const bits = [];
      for (let i = 0; i < n; i++) {
        const m = new T.Mesh(new T.BoxGeometry(0.09 * s, 0.09 * s, 0.09 * s),
          new T.MeshLambertMaterial({ color: i % 3 ? 0x3d3423 : 0x2b4a22 }));
        group.add(m);
        const a = (i / n) * Math.PI * 2 + 0.4, v = (0.55 + 0.35 * ((i * 7) % 3) / 2) * s;
        bits.push({ m, dx: Math.sin(a) * v, dz: Math.cos(a) * v, h: (0.35 + 0.25 * (i % 2)) * s });
      }
      return (t) => {
        const u = (t - p.t0) / (p.dur || 12);
        for (const b of bits) {
          b.m.visible = u >= 0 && u <= 1;
          b.m.position.set(p.at[0] + b.dx * u, 0.4 * s * (1 - u) * 0.3 + 4 * b.h * u * (1 - u), p.at[1] + b.dz * u);
        }
      };
    },
  });

  const mob = SC.mobById(ID);
  const sz = (s) => mob.sizes.find((x) => x.size === s);
  const jump = mob.attacks.find((a) => a.clip === 'attack_jump');
  const SWORD = { socket_hand_r: 'rusty_sword' };
  const nums = (s) => 'здоровье ' + sz(s).health + ', удар ' + sz(s).damage;

  // ------------------------------------------------------------------ (a) the high jump
  // Windup 12 ticks from t 30, takeoff at its tick 11 (t 41), 20 ticks in the air, landing at
  // t 61 = the hit marker of attack_jump, then its recover window from t 63.
  const W0 = 30, UP = W0 + 11, LAND = 61;
  const HIT_T = jump.windup, REC = jump.hit, WIN = jump.recover; // 12, 2, 15
  const highJump = {
    id: 'high_jump',
    title: 'Высокий прыжок: выйди из тени',
    intro: 'Слизь размера 4 сжимается 12 тиков, и на месте приземления появляется тень. Кто вышел из тени дальше чем на 2 блока, остаётся вне грязевой волны и бьёт в окно после приземления.',
    ticks: 112,
    eye: [7.2, 6.4, 9.8],
    target: [-0.8, 1.9, -2.6],
    // The windup runs to the hit marker, then the slime is in the air: the bar shows the attack's
    // own phases, not where the clip rows are cut.
    spans: [
      { phase: 'windup', from: W0, to: W0 + HIT_T, clip: 'attack_jump' },
      { phase: 'hit', from: LAND, to: LAND + REC, clip: 'attack_jump' },
      { phase: 'recover', from: LAND + REC, to: LAND + REC + WIN, clip: 'attack_jump' },
    ],
    actors: {
      slime: {
        model: ID,
        size: 4,
        clips: [[0, 'idle'], [6, 'hop'], [22, 'idle'], [W0, 'attack_jump'], [UP, 'jump_air'], [LAND, 'attack_jump', HIT_T, 0],
          [LAND + REC + WIN, 'idle']],
        pos: [[0, [0, -7]], [11, [0, -7]], [19, [0, -5.5]], [UP, [0, -5.5]], [LAND, [0, 0]]],
        lift: [SC.hop(11, 19, 0.8), SC.hop(UP, LAND, 3)],
        face: [[0, 'player'], [W0, null], [LAND + REC + WIN, 'player']],
        turn: 6,
      },
      player: {
        model: '_player',
        hold: SWORD,
        clips: [[0, 'idle'], [38, 'dodge'], [46, 'idle'], [LAND + 2, 'walk'], [LAND + 7, 'swing'], [LAND + 15, 'swing'],
          [LAND + 23, 'idle'], [88, 'walk'], [96, 'idle']],
        pos: [[0, [0, 0]], [38, [0, 0]], [45, [-3.3, 0.6]], [LAND + 2, [-3.3, 0.6]], [LAND + 7, [-1.8, 0.3]], [88, [-1.8, 0.3]],
          [96, [-3.2, 1.2]]],
        face: [[0, 'slime']],
        turn: 30,
      },
    },
    props: [
      shadow({ at: [0, 0], r: 1.05, from: W0, to: LAND }),
      wave({ at: [0, 0], r: jump.wave.radius_blocks, t0: LAND, dur: 6, linger: 10 }),
      splash({ at: [0, 0], t0: LAND, n: 10, size: 2 }),
    ],
    captions: [
      [0, 'Слизь размера 4 (2 блока) прыгает к игроку. Большие прыгают выше и реже.'],
      [W0, 'Замах высокого прыжка: 12 тиков слизь сжимается и расползается вширь. На месте приземления — тень.'],
      [38, 'Игрок видит тень и уходит из неё: волна бьёт на ' + jump.wave.radius_blocks + ' блока от центра.'],
      [UP, 'Прыжок. Высоту даёт физика слизи, клип только вытягивает её (jump_air).'],
      [LAND, 'Приземление: урон × ' + SC.num(jump.damage) + ' и грязевая волна радиусом 2, замедление 3 с. Игрок в 3 блоках — мимо.'],
      [LAND + REC, 'Окно ' + WIN + ' тиков: слизь растеклась и прилипла к земле. Игрок подходит и бьёт.'],
      [LAND + REC + WIN, 'Окно закрылось: слизь собирается обратно.'],
    ],
    marks: [[W0, 'тень'], [38, 'выход'], [LAND, 'приземление'], [LAND + 15, 'удар']],
  };

  // ------------------------------------------------------------------ (b) death and split
  // The burst is tick 10 of the death clip: here the game spawns the smaller slimes.
  const D1 = 28, B1 = D1 + 10, D2 = 108, B2 = D2 + 10;
  const child = (x0, z0, x1, z1, hopAt, x2, z2) => ({
    model: ID,
    size: 2,
    show: [B1, 999],
    clips: [[B1, 'hop'], [B1 + 16, 'idle'], [hopAt, 'hop'], [hopAt + 16, 'idle']],
    pos: [[B1, [x0, z0]], [B1 + 5, [x0, z0]], [B1 + 13, [x1, z1]], [hopAt + 5, [x1, z1]], [hopAt + 13, [x2, z2]]],
    lift: [SC.hop(B1 + 5, B1 + 13, 0.5), SC.hop(hopAt + 5, hopAt + 13, 0.7)],
    face: [[0, 'player']],
    turn: 8,
  });
  const tiny = (x0, z0, x1, z1) => ({
    model: ID,
    size: 1,
    show: [B2, 999],
    clips: [[B2, 'hop'], [B2 + 16, 'idle']],
    pos: [[B2, [x0, z0]], [B2 + 5, [x0, z0]], [B2 + 13, [x1, z1]]],
    lift: [SC.hop(B2 + 5, B2 + 13, 0.35)],
    face: [[0, 'player']],
    turn: 10,
  });
  const c3 = child(0, -2.6, 0, -3.5, 68, 0, -1.9);
  c3.clips = [[B1, 'hop'], [B1 + 16, 'idle'], [68, 'hop'], [84, 'idle'], [86, 'hop'], [100, 'stick'], [D2, 'death']];
  c3.pos.push([91, [0, -1.9]], [99, [0, -0.85]]);
  c3.lift.push(SC.hop(91, 99, 0.6));
  c3.show = [B1, D2 + 20];
  const split = {
    id: 'split',
    title: 'Деление: 4, 2, 1',
    intro: 'Убитая слизь лопается и делится на 2–3 слизи вдвое меньше. Они прыгают к игроку; слизь размера 2 делится ещё раз, на самые мелкие.',
    ticks: 140,
    eye: [8.2, 5.8, 3.4],
    target: [0, 0.6, -1.4],
    actors: {
      big: {
        model: ID,
        size: 4,
        show: [0, D1 + 20],
        clips: [[0, 'idle'], [12, 'hurt'], [22, 'idle'], [D1, 'death']],
        pos: [[0, [0, -2.2]]],
        face: [[0, 'player']],
      },
      c1: child(-0.5, -2.2, -1.5, -2.8, 60, -1.7, -1.0),
      c2: child(0.5, -2.2, 1.5, -2.8, 64, 1.7, -1.1),
      c3,
      t1: tiny(-0.25, -0.85, -0.7, -1.9),
      t2: tiny(0.25, -0.85, 0.4, -2.6),
      player: {
        model: '_player',
        hold: SWORD,
        clips: [[0, 'idle'], [8, 'swing'], [16, 'idle'], [24, 'swing'], [32, 'idle'], [62, 'walk'], [72, 'idle'], [104, 'swing'],
          [112, 'idle']],
        pos: [[0, [0, 0]], [62, [0, 0]], [72, [0, 0.15]]],
        face: [[0, 'big'], [56, 'c3']],
        turn: 20,
      },
    },
    props: [
      splash({ at: [0, -2.2], t0: B1, n: 10, size: 2 }),
      splash({ at: [0, -0.85], t0: B2, n: 8, size: 1 }),
    ],
    captions: [
      [0, 'Слизь размера 4: ' + nums(4) + '.'],
      [D1, 'Смерть: слизь оседает, раздувается и лопается.'],
      [B1, 'На разрыве она делится: 2–3 слизи размера 2 (' + nums(2) + ').'],
      [60, 'Новые слизи прыгают к игроку по очереди.'],
      [100, 'Касание: прилипание, замедление II на 2 с.'],
      [D2, 'Игрок добивает прилипшую слизь.'],
      [B2, 'Слизь размера 2 делится на слизи размера 1 (' + nums(1) + '). Эти уже не делятся.'],
    ],
    marks: [[B1, 'деление'], [100, 'прилипла'], [B2, 'деление']],
  };

  // ------------------------------------------------------------------ (c) the three sizes
  const hops = (times, h) => ({
    clips: [[0, 'idle']].concat(...times.map((t) => [[t, 'hop'], [t + 16, 'idle']])),
    lift: times.map((t) => SC.hop(t + 5, t + 13, h)),
  });
  const sizes = {
    id: 'sizes',
    title: 'Три размера рядом',
    intro: 'Одна модель на все размеры: игра масштабирует её по размеру, как ванильную слизь. Размер 1 — полблока, 2 — блок, 4 — два блока.',
    ticks: 96,
    eye: [0.4, 3.4, 8.6],
    target: [-0.3, 0.8, -1.5],
    actors: {
      s1: Object.assign({ model: ID, size: 1, pos: [[0, [-1.6, -1.5]]], face: [[0, 0]] }, hops([4, 28, 52, 76], 0.35)),
      s2: Object.assign({ model: ID, size: 2, pos: [[0, [-0.5, -1.5]]], face: [[0, 0]] }, hops([10, 46, 78], 0.6)),
      s4: Object.assign({ model: ID, size: 4, pos: [[0, [1.4, -1.6]]], face: [[0, 0]] }, hops([20, 68], 1.0)),
      player: { model: '_player', hold: SWORD, clips: [[0, 'idle']], pos: [[0, [-3.0, -1.0]]], face: [[0, 60]] },
    },
    captions: [
      [0, 'Размер 1: ' + nums(1) + '. Размер 2: ' + nums(2) + '. Размер 4: ' + nums(4) + '.'],
      [20, 'Чем больше слизь, тем выше и реже она прыгает.'],
    ],
    marks: [],
  };

  SC.mob(ID, {
    fields: [
      ['jump_velocity', '4 → {sizes.4.jump_velocity}; 2 → {sizes.2.jump_velocity}; 1 → {sizes.1.jump_velocity}', 'высота прыжка по размеру: большие прыгают выше'],
      ['decide_mult', '{sizes.4.decide_mult|x}', 'интервал решений размера 4: большие прыгают реже'],
      ['shadow_every_ticks', '{params.shadow_every_ticks}', 'пока слизь в воздухе, на месте приземления каждые {params.shadow_every_ticks} тика идут частицы ({params.shadow_particle})'],
      ['волна грязи', 'замедляет всех', 'и игрока, в которого попал сам прыжок'],
    ],
    view: { size: 4 }, // the model window and the index card show the size 4 slime (2 blocks)
    notes: [
      'Одна модель на все размеры. При размере 1 это внешний куб ванильной слизи, 8×8×8 пикселей; игра масштабирует модель по размеру 1, 2 и 4, как ванильный рендер слизи: полблока, блок, два блока. В окне модели выше — размер 4.',
      'Модель непрозрачная: полупрозрачности у моделей мода в Horizon нет. Нутро видно через «рот» — тёмную впадину спереди глубиной 2 пикселя: тёмная грязь, корни и кость.',
      'Сжатие и растяжение — слоями, как у магмового куба, а не масштабом частей: куб собран из трёх слоёв с тёмным ядром внутри. В прыжке слои расходятся и в щелях видно ядро; при сжатии слои входят друг в друга, а из-под нижнего выползает грязь, и слизь становится шире. Так тексели не растягиваются.',
      'Высокий прыжок в игре: клип attack_jump играет 12 тиков замаха до метки удара, в воздухе — jump_air, при приземлении — attack_jump с метки удара: урон, волна и окно 15 тиков. Высоту прыжка даёт физика слизи, клипы корень не поднимают; в сценах витрина поднимает слизь сама. Тень, волна и брызги — реквизит витрины, в игре это частицы.',
      'Смерть и деление: на 10-м тике клипа death слизь лопается; в этот момент игра спавнит слизи меньшего размера.',
    ],
    scenes: [highJump, split, sizes],
  });
});
