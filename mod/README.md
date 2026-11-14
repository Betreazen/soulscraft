# Код мода SoulsCraft

Нативный бандл `soulscraft/` и контент-мод `content/` с одним id `soulscraft`. Движок Horizon на коммите `7e4e421` (тот же, что у RustCraft и Path-of-craft-tree, иначе общий стенд не соберёт). Прототипы и их итоги — [../prototypes/README.md](../prototypes/README.md).

## Что где

```text
mod/
  sync.ps1                    копии из data/ и assets/ в content/ и в крейт; руками копии не правятся
  content/                    контент-мод: только то, что пишет sync.ps1, плюс horizon-mod.toml
    data/soulscraft/mobs.json                     числа мобов для других модов (путь ниже)
    data/soulscraft/horizon/entity_type/*.json    семь типов на шаблоне minecraft:zombie
    data/soulscraft/horizon/item/*.json           оружие из гнёзд ригов: ржавый меч, серп, молот
    data/soulscraft/horizon/mob_effect/bleeding.json   общее кровотечение soulscraft:bleeding
    assets/soulscraft/rigs/*.rig.json             риги для клиента (читаются во время игры)
    assets/soulscraft/textures/entity/*.png       текстуры мобов
    assets/soulscraft/{items,models/item,textures/item}/   модели и текстуры оружия
    assets/soulscraft/lang/                       имена
  soulscraft/                 нативный бандл
    horizon-mixin.toml        хуки, модули, крейт; в шапке — места вызова хуков и ловушки
    rules/                    крейт soulscraft-rules без игры: вся логика, cargo test
    server.rs                 сервер: учёт мобов, статы от P, мозг → навигация и урон, заметки клиентам
    tools.rs                  /sc, арена, волны, стенд П5
    power.rs                  сила игрока P: снаряжение и сводка PCT `pct:summary` (кэш на тике уровня)
    selftest.rs               самотест стенда (soulscraft_selftest run)
    client.rs                 клиент: риги, позы по тикам, оружие в гнезде, подписи отладки
    altar.rs                  алтарь на сервере: ядра, уровни, высокие вещи, подставка, меню (очередь хука block_changed)
    altar_ui.rs               меню, виды алтаря для клиентов, `/sc altar`, `/sc event`
    event.rs                  ивент: фазы, участники, волны, мобы волны, вытягивание, проклятие, конец и награда
    event_path.rs             карта путей зоны из мира и пролом громилы
    event_ui.rs               боссбар и предупреждение, вид ивента, строки меню, снимок для самотеста
    altar_client.rs           клиент алтаря: горн над подставкой, призрак, лучи и точки волны, экран меню
    selftest_altar.rs         сценарии алтаря, ивента, обороны и места моба (`altar_`, `event_`, `defense_`, `fit_`)
    cover.rs                  укрытие от атакующих турелей RustCraft (роль из block entity турели)
    trophy.rs                 трофеи на сервере: block entity голов и диорам, поворот при установке, голова на стену, частицы диорам
    trophy_client.rs          отрисовка головы и фигурки диорамы из <моб>.trophy.json и рига моба
    spawn_boost.rs            учащённый спавн алтаря: сектор, его смещение, лишние проходы спавна и список спавна сектора
    selftest_trophy.rs        сценарии трофеев, награды, знамён рода и учащённого спавна (`trophy_`, `reward_`, `kin_`, `boost_`)
    selftest_turret.rs        совместные сценарии с турелями RustCraft (`run turret`)
    vanilla.rs                ванильные мобы (07-vanilla-mobs.md): принятие, рост Г2 на ванильной базе, метка PCT, Г4, ворота целей, приёмы
    selftest_vanilla.rs       сценарии ванильных мобов (`vanilla_`, `vtiming_`, `vskeleton_`; медленные — `run vslow`)
    book.rs                   книга охотника на сервере: прогресс в данных игрока, засчитывание убийств, строка над панелью, `/sc book`
    book_client.rs            окно книги: разделы, страницы, модель моба в клипах (`native_draw_scene`; ванильного — дерево частей и текстура его типа из движка, поза своя), шкалы фаз
    selftest_book.rs          сценарии книги (`book_`) на площадке алтаря, големы вместо игроков
    hook_*.rs                 тонкие хелперы: одна строка, вызов модуля
```

Крейт `rules`:

| Модуль | Что делает |
| --- | --- |
| `data` | читает `data/mobs.json` и проверяет атаки |
| `attack` | фазы атаки по тикам: замах, пауза, удар, окно; тики ударов и рывков |
| `scale` | сила игрока P (Г1, веса из `mobs.json` `power`), рост моба от P (Г2), P группы (Г5) |
| `pct` | сводка Path-of-craft-tree `pct:summary`: разбор (строка JSON или составной тег), кэш с правилом «None не стирает известное» |
| `brain` | мозг моба: приказы движения, взгляд, начало атак, удары с проверкой дальности и дуги |
| `rig` | читает `rig.json`, позы частей по клипу и тику с интерполяцией, рамку модели в позе покоя (для `hitbox`) |
| `wire` | заметки сервер → клиент: клип, тик начала, флаги |
| `command` | разбор `/sc` и `soulscraft_selftest` |
| `geom` | векторы в осях мира Minecraft |
| `altar` | `data/altar.json`: что нужно каждому кольцу, уровень по полным кольцам, знамёна и их сумма (Г10), высокие вещи, поворот знамени |
| `flow` | карта путей зоны: клетки, поля расстояний по проходимым клеткам и с проломом, шаги, точки появления |
| `defense` | Г12: роли, перехват и возврат, замах вытягивания, потолок урона, проклятие, удары пролома по твёрдости, книга сломанных блоков |
| `event` | план ивента со знамёнами, фазы прогона, зона, награда (`data/trophies.json`) |
| `fit` | место для рамки моба и поиск ближайшего (раунд 9) |
| `cover` | укрытие: кто видит моба, ближайшее место вне линии огня |
| `view` | сообщения алтаря клиенту и кнопки меню серверу (JSON) |
| `trophy` | файлы трофеев (`<моб>.trophy.json`, копии в `data/trophy/`), какой блок чей, поворот головы и диорамы при установке, коробки частиц по `facing`, частота частиц, фаза клипа фигурки |
| `boost` | учащённый спавн: сектор и его смещение к алтарю, сила по уровню и знамёнам, лишние кандидаты-чанки, список спавна сектора, строка меню |
| `vanilla` | `data/mobs.json` `vanilla`: группы, приёмы, сигналы замаха, проверки; мозг ванильного моба (`VBrain`): когда он берёт моба и что приказывает |
| `book` | книга охотника: `data/book.json`, записи врагов из `mobs.json` (свои и `vanilla.groups`), прогресс игрока и его JSON, ступени (элитные быстрее), кому засчитать убийство, вес спавнера, отчёты машин RustCraft (`soulscraft-kill turret|guard`, старый `pct-machine kill`; `machine_kill`), что открывает предмет в руке, сообщение клиенту |
| `book_page` | страницы книги по ступеням: строки, клипы, шкалы фаз атак; у ванильной строки — числа каждого типа от движка (`mob_page_with`, `VanillaStats`; куб — по размеру) и рост по силе P; разделы «Алтарь» и «Ивенты» |

## Команды

```powershell
cargo test                         # крейт rules, секунды (из корня репозитория)
.\mod\sync.ps1                     # обновить копии после правки data/ или assets/
.\mod\sync.ps1 -Check              # упасть, если копия отстала (перед коммитом и stage)
```

Сборка и проверки идут на общем стенде `C:\Claude code projects\horizon-joint-dev` (его README):

```powershell
.\joint.ps1 stage -Mod soulscraft
.\joint.ps1 server                 # серверный движок со всеми модами стенда
.\joint.ps1 check -Filter timing   # самотест без клиента: фазы атак всех мобов
.\joint.ps1 build                  # сервер и клиент, для ботов и скриншотов
.\joint.ps1 logs -Mod soulscraft
```

Свой `dev.ps1` не нужен: стенд собирает три мода одним проходом на одном коммите.

## Где лежат числа во время игры

- Сервер берёт `data/mobs.json`, вкомпилированный в крейт (`rules/data/mobs.json`, копия от `sync.ps1`).
- `/sc reload` читает `config/soulscraft/mobs.json` в папке сервера. Туда его кладёт `prototypes/tune.ps1`. Если файл есть при старте, сервер берёт его.
- Другие моды (турели RustCraft: `turret_effects`, `effects.bleeding`) читают копию из контент-мода: **`horizon-mods/soulscraft/data/soulscraft/mobs.json`** от папки сервера или instance.

## Хуки

| Хук | Где вызывается (SRC/crates/horizon-client/src) | Что делает мод | Ловушка `mods().is_empty()` |
| --- | --- | --- | --- |
| `level_tick_post` | `game/levels.rs:559-561` | весь сервер: мобы, задачи команд, самотест | нет проверки |
| `native_register_goals` (по `target`) | `mobs/ai.rs:434`, функция `ai.rs:464` | у типов `soulscraft:` убирает цели зомби, оставляет `Float`; снимает горение на солнце | именованного хука нет; диспетчер `native_client_hooks.rs:123-127` доходит только до trait-модов, поэтому инъекция в саму функцию |
| `register_commands` | `game/native_server.rs:1328` | `/sc …` (оператор 2), `soulscraft_selftest` (4) | нет |
| `perform_command` | `game/native_server.rs:1500` | `/sc` игрока; состояние игрока подменено, поэтому изменения мира идут в очередь до тика уровня | нет |
| `perform_console_command` | `game/native_hooks.rs:4636` | консоль и самотест | нет |
| `living_incoming_damage` v1 | `game/native_hooks.rs:9707` | щит латника режет удар спереди на 80% | v1 вызывается всегда; v3 (с атакующим) ждёт trait-мод (`9676-9680`), поэтому атакующий берётся из `causing` или ближайший игрок |
| `should_apply_effect_tick_this_tick`, `apply_effect_tick` | `game/potions.rs:1136-1139` | урон `soulscraft:bleeding` раз в `period_ticks`; в хуке игра только для чтения, удар ставится в очередь на следующий тик | нет |
| `net_from_server` | `game/native_net.rs:162` | заметки о клипах | нет |
| `mob_submit` | `game/native_mob_draw.rs:115` из `game/creatures.rs:7649` | рисует мобов SoulsCraft из ригов вместо зомби | нет |
| `native_world_primitives` (по `target`) | `game/world_primitives.rs:43` | подписи фаз над мобами при `/sc debug on`; контуры призрака алтаря, точки волны, лучи вытягивания и проклятия | в каталоге хука нет |

Хуки второй итерации (алтарь и ивент, [../prototypes/p6-altar.md](../prototypes/p6-altar.md)):

| Хук | Где вызывается (SRC/crates/horizon-client/src) | Что делает мод | Ловушка `mods().is_empty()` |
| --- | --- | --- | --- |
| `block_changed` (у `GameLevel`) | `game.rs:1388-1392` (`observe_block_changed`, каждый `Level.setBlock` авторитетного сервера) | ставит в очередь изменения блоков SoulsCraft и блоков в зонах алтарей; уровень только для чтения, хук зовётся и внутри наших `setblock`, поэтому берёт только замок своей очереди | нет |
| `block_entity` | при установке и загрузке блока, все стороны (API 15) | block entity у ядра и подставки | нет |
| `block_entity_ticks`, `block_entity_tick` | `native_hooks.rs:11815-11819`, тик из `tick_mod_block_entity` (`:12073-12096`) | тикающее ядро сообщает, где оно: так ядро в загруженном чанке находится без сканирования | нет |
| `block_entity_update_tag` | `game/lan.rs:1428` | клиентам — предмет подставки | нет |
| `right_click_block` | `native_hooks.rs:8307` (API 21), состояние игрока подменено | подставка берёт и отдаёт предмет (предметы игрока меняются сразу, мир — в тике уровня), ядро пустой рукой открывает меню | нет |
| `destroy_block` | `native_hooks.rs:8280` (API 14) | высокую вещь, сломанную в творческом режиме, остальные части не роняют | нет |
| `net_from_client` | `native_hooks.rs:8397` (API 40) | кнопки меню алтаря | нет |
| `block_entity_submit`, `block_entity_render_bounding_box` | `game/native_block_entity_draw.rs:487, 501` | предмет над подставкой; маленькие крутящиеся копии призрака у ядра, рамка ядра ±4 блока | нет |
| `tick` (окно) | `native_hooks.rs:2380` | открывает меню, которое прислал сервер, отправляет кнопки; на сервере выходит сразу (`remote.is_none()`) | нет |
| `screen_render_post`, `screen_mouse_button_pressed_pre` | `native_hooks.rs:3797, 3825` (API 78, 80) | меню алтаря на своём экране мода (`native_mod_screen_open`), как окно дерева PCT | нет |

Хуки третьей итерации (трофеи, знамёна, учащённый спавн; [../prototypes/p7-rewards.md](../prototypes/p7-rewards.md), [../prototypes/p4-spawn.md](../prototypes/p4-spawn.md)):

| Хук | Где вызывается (SRC/crates/horizon-client/src) | Что делает мод | Ловушка `mods().is_empty()` |
| --- | --- | --- | --- |
| `block_entity`, `block_entity_ticks`, `block_entity_tick` (те же, что у алтаря) | см. выше | block entity у голов (`<моб>_head_stand` на полу, `<моб>_head_mount` на стене; не `*_head`: такому имени движок сам отдаёт ванильный block entity черепа, `horizon-client-render/src/containers.rs:265-279, 389`) и диорам (`<моб>_trophy`); тикают только диорамы и сообщают, где они | нет |
| `state_for_placement` | `game.rs:1659-1669` (`before_state_for_placement`, каждый `BlockItem.place` авторитетного сервера, API 76) | голова на полу поворачивается к игроку (16 положений, как `SkullBlock`), диорама — табличкой к игроку | нет, только `native_authoritative` |
| `right_click_block` (теперь с `face`) | `native_hooks.rs:8307` (API 21) | предмет `<моб>_head` по боковой грани блока — `<моб>_head_mount` лицом от стены (на пол его ставит `place_block` — `<моб>_head_stand`) (ставится в тике уровня, из стопки уходит одна, в творческом — нет) | нет |
| `block_entity_submit` | `native_block_entity_draw.rs:501` | голова (из `<моб>.trophy.json`, в покое, `keep()`) и фигурка диорамы (риг моба, клип `trophy` по `game_time + partial`, без `keep()`), светящиеся кубы на полном свете | нет |
| уровень (`level_tick_post`) | см. выше | частицы диорам: каждый тик по каждой строке файла трофея с шансом `1 / every` одна частица своего типа `soulscraft:*` в случайной точке коробки, повёрнутой по `facing` (`GameLevel::native_send_particles` через `Game::with_level`, `native_hooks.rs:9928-9963`, `game.rs:4716`) | нет |
| `spawning_chunks`, `RETURN` (по имени функции) | `game/natural_spawning.rs:739-748`, из `spawn_natural` `:764` | чанки сектора учащённого спавна среди кандидатов повторяются `passes` раз | — |
| `spawn_hooks`, `RETURN` (по имени функции) | `game/natural_spawning.rs:457-459`, из `mobs/spawn_selection.rs:715, 1312` | истина в измерении с активным сектором: так спавн спрашивает `potential_spawns` без trait-мода | сама функция и есть ловушка: `!mods().is_empty()` |
| `potential_spawns` (API 388) | `native_hooks.rs:6335-6356` из `natural_spawning.rs:460-472` | позиция монстров в секторе: монстры других биомов и мобы SoulsCraft, знамя рода поднимает веса своей группы | вызывается только при `spawn_hooks()`, его включает инъекция выше |

Хуки четвёртой итерации (книга охотника, [../prototypes/p9-book.md](../prototypes/p9-book.md)):

| Хук | Где вызывается (SRC/crates/horizon-client/src) | Что делает мод | Ловушка `mods().is_empty()` |
| --- | --- | --- | --- |
| `use_item` (API 13) | `game.rs:6971` | правый клик книгой: забирает клик, сервер шлёт игроку прогресс с «открыть» | нет |
| `mob_death_canceled`, `HEAD` (по имени функции) | `game/creatures.rs:1585`, из `mob_died` до `:1626` | смерть моба книги: убийца из последнего урона и `last_hurt_by_player`, метка спавнера | именованный `living_death` за `mods().is_empty()` (`:1586`), поэтому инъекция в функцию |
| `add_planned`, `RETURN` (по имени функции) | `game/spawner.rs:145` | id мобов спавнера (их же читает PCT) | — |
| `perform_console_command` (тот же) | `game/native_server.rs:779` для `native_run_mod_command` | отчёты машин RustCraft: своя команда `soulscraft-kill` (регистрируется в `register_commands`, уровень 4) и `pct-machine kill` | нет |
| `block_entity_submit` (тот же) | `game/native_scene.rs:155-166` из `native_draw_scene` | фигурка моба в окне книги (block entity `soulscraft:book_figure` на невидимом блоке сцены) | нет |
| `tick`, `screen_render_post`, `screen_mouse_button_pressed_pre`, `net_from_server` (те же) | см. выше | окно книги (экран `soulscraft_book`) и сообщение `soulscraft:book` | нет |

Trait-мод не нужен и в третьей итерации: `native_potential_spawns` молчит не из-за хука, а из-за одной функции `spawn_hooks`, и бандл сам возвращает из неё истину там, где стоит активный алтарь. Охраняемые хуки движка (40 мест в 20 файлах) остаются выключенными, как у PCT и RustCraft. Подробно и цена — [../prototypes/p4-spawn.md](../prototypes/p4-spawn.md).

## Как устроен моб

1. Тип `soulscraft:<моб>` копирует зомби (единственный подходящий шаблон: `content.rs:414-431`). Базовые атрибуты — из `mobs.json`. Размер типа мода в JSON не задаётся: строка типа в `entity-types.json` — строка шаблона (`mod_content.rs:829-833`), а в файле типа неизвестные поля запрещены (`content.rs:436-460`: `template`, `item`, `damage`, `attributes`, `deny_unknown_fields`). Поэтому рамку (`hitbox` из `mobs.json`) ставит хук `register_goals` в копию типа каждого моба (`Arc::make_mut`), у сервера и у клиентов; рамка моба — ширина и высота его типа (`mobs/living.rs:489`), по ним же путь (`mobs/path.rs:430-432`).
2. Сила P — метка сущности `soulscraft.p=0.40`: она сохраняется с мобом. Без метки P считается по игрокам рядом (Г5) из брони, оружия и сводки PCT `pct:summary` (Г1; без сводки `T = 0`), или берётся из `/sc p`. Сводки читаются на тике уровня раз в 20 тиков в кэш (`power.rs`): внутри действия игрока данные других игроков недоступны (`native_place`).
3. При первом появлении в памяти (спавн, загрузка чанка, перезапуск) сервер ставит атрибуты от P: здоровье, скорость, урон и сопротивление отбрасыванию прямо в моба (`mobs.rs:1017-1027`), броню — командой `/attribute` (она ведёт свою карту брони, `mobs/armor.rs:43-63`).
4. Каждый тик мозг из крейта получает «чувства» (позиции, щит игрока, стена за ним, крыша) и отдаёт приказ, взгляд, события фаз и удары. Решения — раз в `20 − 10·P` тиков (Г8), фазы атаки — каждый тик.
5. Удар — `native_hurt_entity` с типом `minecraft:mob_attack` от моба: броня, щит игрока, неуязвимость и хуки урона работают как у ванили. Болт стрелка — стрела со скоростью 3 и уроном удар/3.
6. Клиент получает «клип и тик начала» только при смене и раз в 40 тиков всем; позу считает сам по тикам. Клиент только пишет заметки в свою таблицу (`client.rs`, `on_net`) и ничего не шлёт серверу и не ждёт ответа, поэтому в повторе записи LetsCam (заметки приходят в `net_from_server` в исходное время, без сервера) клипы и подписи показываются так же: начало клипа пересчитывается от `now` каждой пачки.

## Как устроен ванильный моб

Срез 8, [../prototypes/p8-vanilla.md](../prototypes/p8-vanilla.md).

1. Тип ванили из `data/mobs.json` `vanilla.groups` принимается, когда тик уровня видит его впервые (после мобов SoulsCraft). Моб волны ивента (метка `soulscraft.wave` или список ивента) остаётся ивенту.
2. P — как у своих мобов (метка `soulscraft.p=`, иначе игроки рядом). Рост Г2 — модификаторы `soulscraft:power` к здоровью, урону и скорости на ванильной базе (`mobs/finalize.rs`: `add_modifier`, `AddMultipliedBase`), броня — карта брони типа (как у своих мобов). Здоровье сохраняет долю максимума (`scale::kept_health`: моб, которому другой мод поставил мало здоровья, так и остаётся; свежий полный — полон). Метка PCT `pct.xpNNN` — от ванильного здоровья. Флаги нежити, вид и теги не трогаются.
3. Спавнер в `spawner_radius_blocks` при первом принятии — метка `soulscraft.spawner`: рост есть, мозга нет (Г4).
4. Остальным в цели добавляются две цели-ворота с приоритетом раньше всех (`SoulscraftGate` — `MOVE | LOOK | JUMP`, `SoulscraftLeapGate` — `JUMP`), их не прерывают. Ворота срабатывают, пока мозг держит моба: так ванильные цели движения и атаки встают, и во время замаха ничего не бьёт. Ворота встают со следующего полного тика целей, поэтому при приказе «стоять» или «рывок» сервер каждый тик ещё и останавливает путь (`native_mob_stop`), ставит управление движением в `Wait` и обнуляет `zza`/`xxa`; куб в замахе теряет и горизонтальную скорость своего прыжка. Флаг `goals.disabled` не подошёл: `riders::drive` снимает его каждый тик (`mobs/riders.rs:381-396`, из `server_ai_step`, `ai.rs:1959`).
5. Мозг (`soulscraft_rules::vanilla::VBrain`) получает цель ванили (`m.ai.target`: провокация PCT её меняет — мозг идёт за ней), отдаёт приказ, взгляд, позу `aggressive`, удары, сигнал замаха и вопросы серверу (высокая точка, укрытие от взгляда цели). Удар — `native_mob_attack` (ванильный удар моба с его эффектами: голод кадавра, иссушение) с множителем приёма; снаряды — `native_summon` трезубца, взрывного зелья или стрелы (id снаряда движок не отдаёт: бросок засчитан, если снаряд найден на месте командой `execute if entity`). `/sc force <приём>` берёт ближайшего моба SoulsCraft, а если его нет — ближайшего принятого ванильного с этим приёмом.
6. Фантом не берётся: мозг только держит его ванильное пике (`m.extra.flag`, `mobs/phantom.rs`) 10 тиков после решения и кричит.

## Строки лога

```text
soulscraft: adopt mob=revenant id=12 p=0.40 size=0 fresh=true health=42.6 tick=4500
soulscraft: phase mob=revenant id=12 attack=overhead phase=hold t=10 tick=4567
soulscraft: strike mob=revenant id=12 attack=overhead t=18 tick=4575 hit=true dmg=5.20 target=sc_a
soulscraft: mode mob=revenant id=12 mode=circle tick=4600
soulscraft: blocked mob=revenant id=12 dmg=6.00->1.20
soulscraft: death mob=revenant id=12 tick=4700
soulscraft: selftest PASS timing_revenant_overhead
soulscraft: vadopt mob=minecraft:zombie id=40 group=zombie p=0.00 spawner=false health=20.0 tick=900
soulscraft: vphase mob=minecraft:zombie id=40 attack=lunge phase=hit t=10 tick=950
soulscraft: vstrike mob=minecraft:zombie id=40 attack=lunge t=10 tick=950 hit=true target=sc_a
soulscraft: vmode mob=minecraft:skeleton id=41 mode=retreat tick=960
soulscraft: book credit mob=revenant id=12 players=["sc_a"] weight=1 how=kill
soulscraft: book player=sc_a entry=revenant stage=Убит
```

Их разбирает `prototypes/lib/log.ps1`.
