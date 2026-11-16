'use strict';
// Stalker: notes and behaviour scenes of its showcase page. Numbers come from data/mobs.json
// (js/data.js), phase ticks from its rig (js/rigs.js); the scenes below only place and time.
// Player at the origin. Angles of SC.orbit and AT: 0 = +z, 180 = -z.
SC.mobScript('stalker', function () {
  const AT = (deg, r) => [Math.sin((deg * Math.PI) / 180) * r, Math.cos((deg * Math.PI) / 180) * r];
  const SWORD = { socket_hand_r: 'rusty_sword' };

  // (a) Circling at 4-6 blocks: while the player loses track of it, it works round to the flank
  // and the back, cuts twice and hops back; once the player keeps it in front, it only circles.
  const flank = {
    id: 'flank',
    title: 'Заходит сбоку и со спины',
    intro: 'Ловчий не идёт в лоб. Он кружит на 4–6 блоках и бьёт, когда игрок упустил его из виду. Кто держит ловчего перед собой, удара не получает.',
    ticks: 210,
    phases: 'stalker',
    eye: [0.8, 14.5, 10.6],
    target: [0, 0, 0.3],
    actors: {
      stalker: {
        model: 'stalker',
        clips: [[0, 'walk'], [24, 'circle_strafe'], [96, 'walk'], [106, 'attack_double'], [124, 'backstep'],
          [134, 'circle_strafe']],
        pos: [[0, AT(180, 11)], [24, AT(180, 5)], SC.orbit(24, 96, [0, 0], 5, 5, 180, 330), [106, AT(330, 1.6)],
          [124, AT(330, 1.6)], [132, AT(330, 4.4)], SC.orbit(134, 210, [0, 0], 4.6, 4.8, 330, 420)],
        face: [[0, 'player']],
        turn: 24,
      },
      player: {
        model: '_player',
        hold: SWORD,
        clips: [[0, 'idle'], [112, 'hurt'], [122, 'idle']],
        pos: [[0, [0, 0]]],
        // It loses track of the stalker after the approach and turns to it only once it is cut.
        face: [[0, 'stalker'], [30, 196], [116, 'stalker']],
        turn: 7,
      },
    },
    captions: [
      [0, 'Ловчий выходит из леса и не идёт в лоб.'],
      [24, 'Кружит на 5 блоках и смещается вбок. Игрок смотрит туда, где ловчий был, и теряет его из виду.'],
      [96, 'Ловчий сбоку и за спиной игрока. Короткий подход…'],
      [106, '…двойной взмах: замах 6 тиков, оба серпа расходятся в стороны.'],
      [112, 'Два удара с разницей в 3 тика, × 0,6 каждый, кровотечение на 3 секунды.'],
      [124, 'После удара ловчий отскакивает назад, а не стоит в окне.'],
      [136, 'Теперь игрок держит его перед собой и поворачивается следом. Ловчий кружит дальше: спины нет, удара нет.'],
    ],
    marks: [[24, 'кружит'], [106, 'сбоку'], [124, 'отскок']],
  };

  // (b) The lunge-dash: a long crouch, a straight dash of up to 6 blocks; the player steps aside
  // on the release, the stalker lands stuck for 15 ticks, facing away, and takes two hits.
  const lunge = {
    id: 'lunge',
    title: 'Выпад-рывок и 15 тиков в спину',
    intro: 'Присед со серпами за спиной длится 12 тиков: его видно. Рывок идёт по прямой. Кто ушёл вбок в момент рывка, получает ловчего спиной к себе на 15 тиков.',
    ticks: 170,
    phases: 'stalker',
    eye: [-7.4, 7.2, 3.2],    // from the side and above: the dash runs across, the sidestep goes away
    target: [0.9, 0.2, -2.2],
    actors: {
      stalker: {
        model: 'stalker',
        clips: [[0, 'circle_strafe'], [20, 'attack_lunge'], [53, 'backstep'], [63, 'circle_strafe']],
        pos: [SC.orbit(0, 20, [0, 0], 6, 5.6, 200, 180), [32, AT(180, 5.6)], [38, [0, 0.4]], [53, [0, 0.4]],
          [62, [-1.6, 3.2]], SC.orbit(63, 170, [0, 0], 4.6, 5, 330, 250)],
        // The dash keeps its heading; stuck, it turns round only late and slowly.
        face: [[0, 'player'], [30, null], [47, 'player']],
        turn: 9,
      },
      player: {
        model: '_player',
        hold: SWORD,
        clips: [[0, 'idle'], [31, 'dodge'], [39, 'walk'], [44, 'swing'], [52, 'swing'], [60, 'idle']],
        pos: [[0, [0, 0]], [31, [0, 0]], [38, [2.4, 0.2]], [39, [2.4, 0.2]], [44, [0.9, -0.7]]],
        face: [[0, 'stalker']],
        turn: 30,
      },
    },
    captions: [
      [0, 'Ловчий кружит на 6 блоках перед игроком.'],
      [20, 'Приседает, серпы назад: замах 12 тиков. Это знак выпада.'],
      [31, 'Игрок уходит вбок в момент рывка.'],
      [32, 'Рывок по прямой до 6 блоков, × 1,2. Свернуть он не может и пролетает мимо.'],
      [38, 'Приземлился, серпы в земле: застрял на 15 тиков и стоит спиной к игроку.'],
      [44, 'Окно: игрок заходит сзади и бьёт дважды.'],
      [47, 'Ловчий медленно разворачивается…'],
      [53, '…и отскакивает, чтобы снова кружить.'],
    ],
    marks: [[20, 'присед'], [38, 'застрял']],
  };

  SC.mob('stalker', {
    fields: [
      ['circle_radius_blocks', '{params.circle_radius_blocks}', 'кружит на этом расстоянии и выбирает точку сбоку или сзади, больше 90° от взгляда игрока (на {params.flank_angle_degrees}° от него)'],
      ['flank_max_ticks', '{params.flank_max_ticks}', 'бьёт, когда игрок потерял его из вида, или через {params.flank_max_ticks} тиков кружения'],
      ['lunge_more_often_if_player_at_wall', '{params.lunge_more_often_if_player_at_wall}', 'игрока со спиной у стены атакует чаще'],
      ['flee_below_health', '{params.flee_below_health}', 'убегает при здоровье ниже {params.flee_below_health|pct}, возвращается через {params.return_after_ticks} тиков (return_after_ticks)'],
    ],
    notes: [
      'Два серпа — один предмет soulscraft:stalker_sickle, спрайт 16×16 с родителем item/handheld, как ванильный меч: рукоять по диагонали снизу слева, клинок изгибается полумесяцем вправо, остриё смотрит вниз. Игра выдавливает спрайт на 1 пиксель. Правый серп стоит в гнезде socket_hand_r с display.thirdperson_righthand, левый в socket_hand_l с display.thirdperson_lefthand (зеркало). Гнёзда сдвинуты под длинную тонкую руку ловчего: низ кисти, передняя грань.',
      'Левая рука в игре не проверена: рисует ли Horizon предмет в socket_hand_l моба мода через thirdperson_lefthand так же, как правый, покажет прототип. Витрина и Blender повторяют цепочку игры.',
      'Сутулость — поворот покоя тела на 24°, поэтому все клипы начинаются из наклона. Голова, руки и юбка плаща из шкур наклонены обратно, капюшон смотрит вперёд и чуть вниз: лицо в тени, видны только глаза.',
      'Капюшон-добыча — модель предмета soulscraft:stalker_hood из тех же кубов, что носит моб: открытая оболочка вокруг головы 8×8×8, козырёк, щёки и ступенчатый острый верх. На игроке лицо остаётся в тени капюшона.',
    ],
    scenes: [flank, lunge],
  });
});
