'use strict';
// Revenant: notes and behaviour scenes of its showcase page. Numbers come from data/mobs.json
// (js/data.js), phase ticks from its rig (js/rigs.js); the scenes below only place and time.
// Player at the origin; the revenant comes from -z. Angles of SC.orbit: 0 = +z, 180 = -z.
SC.mobScript('revenant', function () {
  const AT = (deg, r) => [Math.sin((deg * Math.PI) / 180) * r, Math.cos((deg * Math.PI) / 180) * r];
  const SWORD = { socket_hand_r: 'rusty_sword' };

  // (a) The player keeps the shield up; the revenant circles for at most 3 s, then bashes.
  const shield = {
    id: 'shield',
    title: 'Ждёт, пока опустишь щит',
    intro: 'Игрок держит щит. Латник не бьёт в щит мечом: кружит и ждёт, а через 3 секунды бьёт щитом.',
    ticks: 180,
    phases: 'rev',
    eye: [-4.6, 6.4, 3.9],
    target: [-0.6, 0.4, -1.9],
    actors: {
      rev: {
        model: 'revenant',
        clips: [[0, 'walk'], [52, 'circle_strafe'], [112, 'shield_bash'], [130, 'block'], [146, 'backstep'], [158, 'block']],
        pos: [[0, [0, -8]], [52, AT(180, 2.6)], SC.orbit(52, 112, [0, 0], 2.6, 2.6, 180, 250), [118, AT(250, 1.5)],
          [146, AT(250, 1.5)], [156, AT(250, 3.2)]],
        face: [[0, 'player']],
        turn: 14,
      },
      player: {
        model: '_player',
        hold: SWORD,
        clips: [[0, 'block'], [120, 'knocked'], [134, 'idle'], [160, 'block']],
        pos: [[0, [0, 0]], [120, [0, 0]], [127, [1.5, 0.55]]],
        face: [[0, 'rev']],
        turn: 20,
      },
    },
    captions: [
      [0, 'Латник подходит со щитом. Игрок держит свой щит поднятым.'],
      [52, 'Игрок не опускает щит. Латник кружит на 2–3 блоках и ждёт, но не дольше 3 секунд (60 тиков).'],
      [112, '3 секунды прошли. Латник отводит щит к груди: замах 8 тиков…'],
      [120, '…и бьёт щитом: урон × 0,5 и отбрасывание. Щит игрока выбит.'],
      [122, 'Окно 8 тиков: пока латник бьёт, его собственный щит опущен.'],
      [146, 'Латник отступает на шаг и снова поднимает щит.'],
    ],
    marks: [[52, 'ждёт'], [112, '3 с']],
  };

  // (b) Delayed overhead: dodging when the sword freezes is too early; dodging on the release works.
  const delayed = {
    id: 'delayed',
    title: 'Задержанный удар: рано и вовремя',
    intro: 'Меч поднимается за 10 тиков и замирает ещё на 8. Кто уворачивается, как только меч замер, попадает под удар. Кто ждёт отпускания, уходит и бьёт в окно.',
    ticks: 170,
    phases: 'rev',
    eye: [1.4, 4.4, 6.6],   // behind the player, so the sidestep runs across the picture
    target: [1.4, 0.6, -1.2],
    actors: {
      rev: {
        model: 'revenant',
        clips: [[0, 'block'], [20, 'attack_overhead'], [56, 'block'], [100, 'block'], [110, 'attack_overhead'], [146, 'idle']],
        // Run 1: it steps after the player through the pause and still reaches the new spot.
        pos: [[0, [0, -2.2]], [30, [0, -2.2]], [38, [1.6, -1.5]], [94, [1.6, -1.5]], [102, [0, -2.2]]],
        // Run 1: it turns after the player all through the pause. Run 2: the release is already
        // under way when the player moves, so the swing goes where the player was.
        face: [[0, 'player'], [126, null], [146, 'player']],
        turn: 12,
      },
      player: {
        model: '_player',
        hold: SWORD,
        // Both dodges are a long sidestep, about 3 blocks: the first one is still caught, the
        // second one leaves the swing hitting empty ground where the player stood.
        clips: [[0, 'idle'], [30, 'dodge'], [38, 'hurt'], [50, 'idle'], [92, 'walk'], [102, 'idle'], [126, 'dodge'], [134, 'walk'],
          [140, 'swing'], [148, 'idle']],
        pos: [[0, [0, 0]], [30, [0, 0]], [37, [2.9, 0.4]], [92, [2.9, 0.4]], [102, [0, 0]], [126, [0, 0]], [133, [3, 0.4]],
          [140, [1.1, -1.5]]],
        face: [[0, 'rev']],
        turn: 30,
      },
    },
    captions: [
      [0, 'Первая попытка. Латник начинает задержанный удар сверху.'],
      [20, 'Замах: меч поднимается над головой.'],
      [30, 'Меч замер, и игрок сразу уходит вбок на 3 блока. Это рано: идёт пауза, латник доворачивается и шагает следом.'],
      [38, 'Удар в паузу попадает: × 1,3.'],
      [56, 'Ранний уворот наказан задержкой.'],
      [100, 'Вторая попытка. Тот же удар.'],
      [120, 'Меч замер. Игрок ждёт.'],
      [126, 'Меч пошёл вниз, и только теперь игрок уходит вбок на те же 3 блока. Латник уже не может довернуться.'],
      [128, 'Удар уходит в пустоту.'],
      [133, 'Окно 16 тиков: меч в земле, латник открыт. Игрок заходит сбоку и бьёт.'],
    ],
    marks: [[30, 'рано'], [126, 'вовремя']],
  };

  SC.mob('revenant', {
    fields: [
      ['hold_step_speed', '{params.hold_step_speed}', 'в паузе задержанного удара смотрит на игрока и идёт к нему примерно вполовину обычной скорости (это множитель пути, скорость идёт его квадратом): кто отскочил в паузе, того он догонит, кто отскочил в момент удара, от удара ушёл'],
      ['knockback', '{attacks.shield_bash.knockback}', 'удар щитом отбрасывает игрока (сила отбрасывания {params.knockback_strength})'],
      ['shield_disable_ticks', '{params.shield_disable_ticks}', 'удар щитом выбивает поднятый щит игрока на {params.shield_disable_ticks} тиков, как топор'],
      ['shield_front_reduction', '{params.shield_front_reduction}', 'удар спереди по латнику с поднятым щитом даёт не больше {params.shield_front_reduction|rest} урона, сзади — полный'],
    ],
    notes: [
      'Меч в правой руке — предмет soulscraft:rusty_sword, спрайт 16×16 с родителем item/handheld, как ванильный меч. Игра выдавливает спрайт на 1 пиксель в толщину, витрина делает то же самое. Ставится он так же, как в игре: гнездо socket_hand_r в точке (−1, 10, −2) от плеча руки с поворотами rotX(−90), rotY(180), затем display.thirdperson_righthand от item/handheld. Игрок в сценах держит тот же предмет той же цепочкой.',
      'Щит — часть модели латника на левом предплечье, а не предмет: он двигается вместе с рукой во всех клипах.',
    ],
    armor: { rusty_chestplate: 'rusty' },
    scenes: [shield, delayed],
  });
});
