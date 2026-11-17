'use strict';
// Shrike: notes and behaviour scenes of its showcase page. Numbers come from data/mobs.json
// (js/data.js), phase ticks from its rig (js/rigs.js); the scenes below only place and time.
// Player at the origin. Angles of the arcs: 0 = +z, growing angles turn left, which is the way
// the clip `circle` banks (`circle_r` is its mirror).
//
// `flight()` below builds a flight path out of arcs, lines and hovers in 3D and hands it to the
// scene as the core fields: `pos` (ground position), `lift` (height) and `face` (a target or
// 'path', the heading along the path).
SC.mobScript('shrike', function () {
  // ---------------------------------------------------------------- flight paths
  const RAD = Math.PI / 180;
  const ease = {
    linear: (u) => u,
    smooth: (u) => u * u * (3 - 2 * u),
    brake: (u) => 1 - (1 - u) * (1 - u),
    drop: (u) => u * u,
  };

  // A path of segments from a start point [x, z, y]: arc around a centre, straight line, or
  // hover in place. `face` of a segment: an actor name, a yaw in degrees, or none (along the path).
  function flight(start, t0) {
    const segs = [];
    let end = start, endT = t0 || 0;
    const push = (t1, fn, face) => {
      segs.push({ t0: endT, t1, fn, face });
      end = fn(1);
      endT = t1;
    };
    const api = {
      arc(t1, c, r0, r1, a0, a1, h0, h1, face) {
        const p0 = [c[0] + Math.sin(a0 * RAD) * r0, c[1] + Math.cos(a0 * RAD) * r0, h0];
        const d = Math.hypot(p0[0] - end[0], p0[1] - end[1], p0[2] - end[2]);
        if (d > 0.05 && segs.length) throw new Error('shrike scene: arc starts away from the path end by ' + d.toFixed(2));
        push(t1, (u) => {
          const a = (a0 + (a1 - a0) * u) * RAD, r = r0 + (r1 - r0) * u;
          return [c[0] + Math.sin(a) * r, c[1] + Math.cos(a) * r, h0 + (h1 - h0) * u];
        }, face);
        return api;
      },
      to(t1, p, how, face) {
        const p0 = end, e = ease[how || 'smooth'];
        push(t1, (u) => p0.map((v, k) => v + (p[k] - v) * e(u)), face);
        return api;
      },
      // Into an arc: flies a line to where the arc would start.
      toArc(t1, c, r, a, h, how) {
        return api.to(t1, [c[0] + Math.sin(a * RAD) * r, c[1] + Math.cos(a * RAD) * r, h], how);
      },
      hold(t1, face) {
        const p = end;
        push(t1, () => p, face);
        return api;
      },
      end: () => end,
      // pos and lift keys on every tick, face keys where a segment starts.
      build(ticks) {
        const at = (t) => {
          let s = segs[0];
          for (const g of segs) if (g.t0 <= t) s = g;
          const u = s.t1 > s.t0 ? Math.max(0, Math.min(1, (t - s.t0) / (s.t1 - s.t0))) : 1;
          return s.fn(u);
        };
        const pos = [], lift = [];
        for (let t = 0; t <= ticks; t++) {
          const p = at(t);
          pos.push([t, [p[0], p[1]]]);
          lift.push([t, p[2]]);
        }
        const face = segs.map((s) => [s.t0, s.face != null ? s.face : 'path']);
        return { pos, lift, face };
      },
    };
    return api;
  }
  const ARC = (c, r, a, h) => [c[0] + Math.sin(a * RAD) * r, c[1] + Math.cos(a * RAD) * r, h];
  const O = [0, 0];
  const SWORD = { socket_hand_r: 'rusty_sword' };

  // ---------------------------------------------------------------- (a) a pair takes turns
  // Bird a circles at 8 blocks, bird b at 8.5 on the other side. a dives first (tick 50), b second
  // (tick 140). Each time the player steps aside on the fold and hits the bird while it climbs.
  const A_HIT = 64, B_HIT = 154; // ticks of the dive strike: attack start + windup 14
  const a = flight(ARC(O, 3.5, 100, 8))
    .arc(50, O, 3.5, 3.5, 100, 300, 8, 8)
    .to(54, ARC(O, 3.5, 310, 8.2), 'brake', 'player')  // brakes
    .hold(61, 'player')                                // the screech: frozen in the air
    .to(A_HIT + 1, [0.15, 0.1, 0.25], 'drop')           // folded, the stoop onto where the player was
    .hold(A_HIT + 2)
    .to(A_HIT + 32, [-4.2, 3.0, 1.6], 'linear')          // the slow climb just above the ground
    .toArc(140, O, 3.5, 200, 8, 'smooth')
    .arc(240, O, 3.5, 3.5, 200, 560, 8, 8)
    .build(240);
  const b = flight(ARC(O, 4, 280, 8.5))
    .arc(140, O, 4, 4, 280, 840, 8.5, 8.5)
    .to(144, ARC(O, 4, 850, 8.7), 'brake', 'player')
    .hold(151, 'player')
    .to(B_HIT + 1, [-1.15, 0.95, 0.25], 'drop')
    .hold(B_HIT + 2)
    .to(B_HIT + 32, [3.0, -2.4, 1.6], 'linear')
    .toArc(222, O, 4, 100, 8.5, 'smooth')
    .arc(240, O, 4, 4, 100, 172, 8.5, 8.5)
    .build(240);

  const pair = {
    id: 'pair',
    title: 'Пара: пикируют по очереди',
    intro: 'Два сорокопута кружат в 8–9 блоках над игроком и пикируют по очереди. Крик и замершая в воздухе птица — замах 14 тиков. Игрок уходит вбок, когда крылья сложились, и бьёт птицу, пока она 30 тиков набирает высоту низко над землёй.',
    ticks: 240,
    phases: 'a',
    eye: [11, 1.0, 13.5],
    target: [-0.4, 4.3, 0],
    actors: {
      a: { model: 'shrike', clips: [[0, 'circle'], [50, 'attack_dive'], [83, 'hurt'], [93, 'fly'], [140, 'circle']],
        pos: a.pos, lift: a.lift, face: a.face, turn: 30 },
      b: { model: 'shrike', clips: [[0, 'circle'], [140, 'attack_dive'], [169, 'hurt'], [179, 'fly'], [222, 'circle']],
        pos: b.pos, lift: b.lift, face: b.face, turn: 30 },
      player: {
        model: '_player',
        hold: SWORD,
        clips: [[0, 'idle'], [58, 'dodge'], [66, 'idle'], [70, 'walk'], [80, 'swing'], [88, 'idle'],
          [148, 'dodge'], [156, 'idle'], [160, 'walk'], [166, 'swing'], [174, 'idle']],
        pos: [[0, [0, 0]], [58, [0, 0]], [65, [2.3, 0.4]], [70, [2.3, 0.4]], [80, [-1.2, 1.0]],
          [148, [-1.2, 1.0]], [155, [-0.2, 2.6]], [160, [-0.2, 2.6]], [166, [0.0, 0.5]]],
        face: [[0, 'a'], [100, 'b']],
        turn: 24,
      },
    },
    captions: [
      [0, 'Пара кружит в 8–9 блоках над игроком: обе в крене, по дуге влево.'],
      [50, 'Первая тормозит и кричит. Голова вверх, клюв раскрыт, крылья вскинуты: птица замерла в воздухе.'],
      [58, 'Игрок уходит вбок на 2 блока.'],
      [61, 'Крылья сложены, птица срывается в пике — туда, где игрок стоял.'],
      [64, 'Удар в пустую землю. Попади он — урон × 1,2 и замедление на 3 секунды.'],
      [66, 'Окно 30 тиков: птица выходит из пике и тяжело набирает высоту низко над землёй.'],
      [80, 'Игрок догоняет и бьёт.'],
      [96, 'Окно закрылось: птица уходит наверх, к паре.'],
      [140, 'Теперь пикирует вторая. Первая кружит и ждёт.'],
      [148, 'Крик, замах — игрок снова уходит вбок.'],
      [156, 'Промах, вторая птица открыта на 30 тиков.'],
      [166, 'Удар в окно.'],
      [186, 'Обе снова наверху. Вопрос моба: поймаешь ли окно после пике?'],
    ],
    marks: [[50, 'крик'], [140, 'крик 2'], [B_HIT + 2, 'окно 2']],
  };

  // ---------------------------------------------------------------- (b) under a roof
  // The player stands under a lean-to. The shrike circles, comes lower to look, hovers, circles
  // again; it never dives at a player under a roof. The player steps out, and the cry comes at once.
  const R_HIT = 190; // attack start 176 + windup 14
  const OUT = [3.2, -1.2];
  const w = flight(ARC(O, 4.5, 0, 8))
    .arc(60, O, 4.5, 4.5, 0, 240, 8, 8)
    .arc(110, O, 4.5, 3.6, 240, 440, 8, 6)             // lower and tighter: watching the roof
    .to(118, ARC(O, 3.6, 458, 5.6), 'brake')
    .hold(142, 'player')                               // hovers at the edge of the roof
    .to(150, ARC(O, 3.9, 470, 6.4), 'smooth')
    .arc(176, O, 3.9, 4.5, 470, 560, 6.4, 8)
    .to(180, ARC(O, 4.5, 572, 8.2), 'brake', 'player')
    .hold(187, 'player')
    .to(R_HIT + 1, [OUT[0], OUT[1], 0.25], 'drop')
    .hold(R_HIT + 2)
    .to(R_HIT + 32, [OUT[0] + 3.6, OUT[1] - 3.0, 1.6], 'linear')
    .build(230);

  const roof = {
    id: 'roof',
    title: 'Под крышей: ждёт',
    intro: 'Игрок стоит под навесом. Сорокопут не пикирует на игрока под крышей: кружит, спускается посмотреть, зависает и ждёт. Стоит выйти — сразу крик.',
    ticks: 230,
    phases: 'shrike',
    eye: [11, 1.0, 13.5],
    target: [0.6, 4.1, 0],
    props: [{ model: '_shelter', at: [0, 0] }],
    actors: {
      shrike: { model: 'shrike', clips: [[0, 'circle'], [118, 'idle'], [142, 'fly'], [150, 'circle'], [176, 'attack_dive']],
        pos: w.pos, lift: w.lift, face: w.face, turn: 30 },
      player: {
        model: '_player',
        hold: SWORD,
        clips: [[0, 'idle'], [162, 'walk'], [172, 'idle'], [184, 'walk'], [192, 'idle']],
        pos: [[0, [0, 0.3]], [162, [0, 0.3]], [172, OUT], [184, OUT], [192, [0.2, 0.3]]],
        face: [[0, 'shrike']],
        turn: 24,
      },
    },
    captions: [
      [0, 'Игрок под навесом. Сорокопут кружит в 8 блоках над ним.'],
      [60, 'Спускается ниже и сужает круг: смотрит, но не пикирует.'],
      [118, 'Зависает у края крыши. Под крышей игрок для него недосягаем: птица ждёт.'],
      [150, 'Снова набирает высоту и кружит.'],
      [162, 'Игрок выходит из-под навеса.'],
      [176, 'Сразу крик: птица замерла, крылья вскинуты, замах 14 тиков.'],
      [184, 'Игрок возвращается под крышу.'],
      [190, 'Пике приходится в пустое место.'],
      [192, 'Птица открыта 30 тиков, а игрок снова под крышей: на следующем круге сорокопут опять будет ждать.'],
    ],
    marks: [[118, 'ждёт'], [162, 'вышел'], [176, 'крик']],
  };

  SC.mob('shrike', {
    fields: [
      ['group_size', '{params.group_size}', 'в мире появляется парой или тройкой'],
      ['очередь пике', 'цель в радиусе слежения 24 блока', 'все сорокопуты с одной целью пикируют по одному; остальные кружат'],
      ['точка пике', 'в конце замаха', 'цель фиксируется, когда складываются крылья, и дальше не меняется; уворот — в этот момент'],
      ['waits_if_player_under_roof', '{params.waits_if_player_under_roof}', 'игрока под крышей не атакует, ждёт'],
      ['смерть', 'падает', 'у убитого сорокопута снимается NoGravity, тело падает'],
    ],
    notes: [
      'Модель на 24 пикселя в размахе: перья — ступенчатые тонкие кубы, как у фантома и попугая, поэтому рваный край крыльев — это геометрия, а не прозрачные пиксели. Снизу, откуда его видит игрок, читаются широкие крылья с бледным костяным пятном на маховых, длинный хвост и клюв-крюк.',
      'Крен и наклон пике — в самих клипах: у кадра отрисовки моба в игре есть только поворот по body_yaw. Кружит влево клип circle, вправо — circle_r. Покой — зависание на месте.',
      'Пике: 14 тиков замаха (крик, затем крылья складываются), 2 тика удара, 30 тиков окна. Если птица падает дольше, чем длится конец замаха, мод держит клип dive (сложенные крылья) до касания. Клип climb — те же 30 тиков набора высоты отдельно.',
      'Смерть: птица кувыркается и ложится на спину. Чтобы тело упало с высоты, мод снимает с моба no_gravity.',
    ],
    scenes: [pair, roof],
  });
});
