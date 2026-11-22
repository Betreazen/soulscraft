# Код мода SoulsCraft

С версии 0.8.0 SoulsCraft — один пакет Mod API Horizon на сборке Nightly 2026-11-21 (`8f83e5c`), лаунчер 0.2.96 и новее. Лаунчер собирает только мод, игра берётся готовой из SDK этой сборки. Прежний формат — нативный бандл `horizon-mixin.toml` и отдельный контент-мод на Horizon `7e4e421` — новые сборки не загружают. Он описан в истории: [../docs/devlog.md](../docs/devlog.md) и [../prototypes/README.md](../prototypes/README.md).

> [!IMPORTANT]
> Два нативных мода в одной сборке пока не линкуются ([horizon#19616](https://github.com/horizon-rust/horizon/issues/19616)): каждый нативный мод тянет свои копии `serde` и `log`. Поэтому общий стенд с RustCraft и Path-of-craft-tree (`horizon-joint-dev`) для 0.8.0 не работает: SoulsCraft собирается и проверяется отдельно. Зависимости ради этого не меняем, решение за владельцем.

## Что где

```text
mod/
  sync.ps1                       копии из ../data и ../assets в пакет; руками копии не правятся
  soulscraft/                    пакет мода: только то, что берёт упаковщик Horizon
    horizon-mod.toml             манифест: id, side = both, capabilities, [native] (сборка Horizon и
                                 horizon-mixins), 33 модуля [[native.module]], 18 миксинов [[native.mixin]]
    Cargo.toml, Cargo.lock       крейт soulscraft-rules (lib soulscraft_rules); Cargo.lock сделан из Cargo.lock игры
    src/lib.rs                   модули правил, Hooks и Part мода: регистрации, хуки без игры, «ворота» миксинов
    src/bridge.rs                очереди между крейтом и кодом в игре (сеть, тики block entity, книга, урон)
    src/loot.rs                  изменения таблиц лута (data/loot.json)
    src/<модуль>.rs              правила без игры (таблица ниже), у каждого свои тесты
    src/modules/*.rs             код в игре: модули [[native.module]], компилируются внутри crate::game
                                 как game::soulscraft_<имя> и видят закрытые поля Game
    src/mixins/*.rs              хелперы [[native.mixin]]: по одной закрытой функции, без строк use
    assets/soulscraft/           риги, модели, текстуры, звуки, имена (пишет sync.ps1)
    data/soulscraft/             типы мобов, блоки, предметы, эффект, таблицы лута, рецепты, теги (пишет sync.ps1)
    data/soulscraft/*.json       mobs, altar, trophies, book, loot: крейт вкомпилирует их через include_str!
    .horizon/                    SDK сборки, его скачивает cargo horizon-mod; в git не идёт
```

Модули в игре:

| Модуль | Что делает |
| --- | --- |
| `compat`, `compat_entities`, `compat_world`, `compat_client` | старые вызовы `Game::native_*` (7e4e421) под именами `game.sc_<старое имя>(…)` поверх функций 8f83e5c: мозг мобов (навигация, взгляд, удар), сущности, блоки, игроки, команды, сеть, рисование. Подписи старые, поэтому остальной код почти не менялся. Подписи над мобами и над ядром — `compat_client::draw_label` |
| `goals` | новый моб SoulsCraft: цели зомби убираются (остаётся `Float`), горение на солнце снимается, своя рамка и флаг нежити |
| `server` | сервер: учёт мобов, статы от P, мозг → навигация и урон, заметки клиентам |
| `tools` | `/sc`, арена, волны, стенд П5; разбор типизированной команды (`on_mod_command`) |
| `power` | сила игрока P: снаряжение и сводка PCT `pct:summary` (кэш на тике уровня) |
| `client` | клиент: риги, позы по тикам, оружие в гнезде, подписи отладки |
| `altar`, `altar_ui`, `altar_client` | алтарь на сервере, его меню и виды для клиентов, клиент алтаря (горн над подставкой, призрак, лучи, точки волны, экран меню) |
| `event`, `event_path`, `event_ui` | ивент: фазы, участники, волны, вытягивание, проклятие, награда; карта путей и пролом; боссбар и строки меню |
| `cover` | укрытие от атакующих турелей RustCraft |
| `trophy`, `trophy_client` | трофеи: головы на стену, частицы диорам; отрисовка головы и фигурки диорамы |
| `spawn_boost`, `natural` | учащённый спавн у алтаря; естественный спавн по биомам, спавнер удильщика, осколки души |
| `vanilla` | ванильные мобы (07-vanilla-mobs.md): принятие, рост Г2, ворота целей, приёмы |
| `book`, `book_client` | книга охотника на сервере и её окно |
| `relic`, `cost`, `find` | Сердце бездны в левой руке; время мода в тике (`/sc cost`); моб по id через индекс тика |
| `selftest*` | сценарии самотеста `soulscraft_selftest run [фильтр]` |

Правила без игры (`src/*.rs`):

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
| `trophy` | файлы трофеев (`<моб>.trophy.json`, копии в пакете: `assets/soulscraft/rigs/<моб>.trophy.json`), какой блок чей, поворот головы и диорамы при установке, коробки частиц по `facing`, частота частиц, фаза клипа фигурки |
| `boost` | учащённый спавн: сектор и его смещение к алтарю, сила по уровню и знамёнам, лишние кандидаты-чанки, список спавна сектора, строка меню |
| `vanilla` | `data/mobs.json` `vanilla`: группы, приёмы, сигналы замаха, проверки; мозг ванильного моба (`VBrain`): когда он берёт моба и что приказывает |
| `book` | книга охотника: `data/book.json`, записи врагов из `mobs.json` (свои и `vanilla.groups`), прогресс игрока и его JSON, ступени (элитные быстрее), кому засчитать убийство, вес спавнера, отчёты машин RustCraft (`soulscraft-kill turret|guard`, старый `pct-machine kill`; `machine_kill`), что открывает предмет в руке, сообщение клиенту |
| `book_page` | страницы книги по ступеням: строки, клипы, шкалы фаз атак; у ванильной строки — числа каждого типа от движка (`mob_page_with`, `VanillaStats`; куб — по размеру) и рост по силе P; разделы «Алтарь» и «Ивенты» |

## Сборка и проверка

```powershell
cd mod/soulscraft
cargo horizon-mod check           # один раз после клонирования: SDK 8f83e5c в .horizon/
cargo test --lib                  # правила, лут и регистрации без игры, секунды
cd ../..
.\mod\sync.ps1                    # обновить копии после правки data/ или assets/
.\mod\sync.ps1 -Check             # упасть, если копия отстала (перед коммитом)
```

Сборка для Windows-клиента и одиночной игры — изолированным лаунчером `C:\Claude code projects\horizon-sdk-dev\hl.ps1` (свои папки данных, без самообновления, под потолком CPU 87%, замок сборки):

```powershell
& 'C:\Claude code projects\horizon-sdk-dev\hl.ps1' native-build "C:\Claude code projects\soulscraft-modapi\mod\soulscraft" `
    --version nightly-2026-11-21-8f83e5c --trust-native-code
```

`native-build` собирает мод и выбирает сборку для экземпляра. Дальше одиночная игра: запустить этот экземпляр, создать мир с читами и ввести в чат `/soulscraft_selftest run [фильтр]`. Итог пишется в `selftest/soulscraft.json` в папке игры, строки `soulscraft: selftest PASS|FAIL` — в журнал. Первая сборка — несколько минут, пересборка после правки модуля — около минуты.

Выделенный сервер собирается только под Linux x86-64 (SDK сервера есть только для Linux). Это делает лаунчер 0.2.96 в Docker через `C:\Claude code projects\horizon-linux-dev\hlx.ps1`:

```powershell
$hlx = 'C:\Claude code projects\horizon-linux-dev\hlx.ps1'
& $hlx -Sync "C:\Claude code projects\soulscraft-modapi\mod\soulscraft" native-build /hz/src/soulscraft `
    --server-output /hz/exports/soulscraft-N --version nightly-2026-11-21-8f83e5c --trust-native-code
& $hlx -Sync "C:\Claude code projects\soulscraft-modapi\tools" -SyncAs soulscraft-tools `
    -Sh "bash /hz/src/soulscraft-tools/linux-selftest.sh soulscraft-N [фильтр]"
```

[`../tools/linux-selftest.sh`](../tools/linux-selftest.sh) поднимает сервер `/hz/servers/soulscraft` на экспорте, запускает самотест из консоли, ждёт `selftest/soulscraft.json` с `"done":true`, печатает итог и останавливает сервер. Папка `--server-output` не должна существовать заранее: каждой сборке — свой номер.

Боты гоняют сцены 0.7.x на том же сервере: `prototypes/check-final.ps1 -Version native-8f83e5c-<ключ> -Export soulscraft-N [-Only сцена,…]`.
- Сервер запускают `horizon-server --dir /hz/servers/soulscraft start` и после прогона останавливают `stop`. Порт `127.0.0.1:25591` общий, поэтому перед запуском проверьте, что другие серверы в `/hz/servers` стоят.
- Боты — клиенты изолированной установки `horizon-sdk-dev` с этой сборкой, без звука и под ограничением CPU в 87 %. Консоль и журнал сервера идут через `hlx.ps1` (функция `Use-ModApiBench` в `prototypes/lib/bot.ps1`).
- Сцена записи LetsCam пропускается, пока LetsCam нельзя собрать вместе с SoulsCraft ([horizon#19616](https://github.com/horizon-rust/horizon/issues/19616)).

## Ловушки перехода на Mod API

Всё, на чём мы споткнулись при переходе с бандла 7e4e421 на 8f83e5c, и как обошли.

- **Пакет — плоский Cargo-проект.** Упаковщик (`horizon-mod-package`, `source_file`) берёт только `Cargo.toml`, `Cargo.lock`, `build.rs`, `src/**/*.rs|toml`, `assets/**`, `data/**` и README/LICENSE. Вложенные крейты (`rules/`, `loot/`) он отвергает: `unexpected package path: loot/Cargo.toml`. Поэтому правила и лут — модули одного крейта, а JSON, который крейт вкомпилирует, лежит в `data/` пакета.
- **Крейт мода зовётся по-старому.** Пакет `soulscraft-rules`: код в игре всегда звал правила `soulscraft_rules::…`, а в патч-крейте игры имя мода — это имя его крейта. Крейт мода не может звать модули в игре; модули могут звать крейт. То, что крейт слышит без игры (пакеты сети, тик block entity, клик книгой), он кладёт в очередь `bridge.rs`, а модули забирают её на тике уровня или клиента.
- **`Cargo.lock` — из игры.** `horizon-launcher native-lock` берёт самые новые версии, и сборка их отвергает. Lock сделан из `Cargo.lock` игры той же сборки и прорежен `cargo metadata`.
- **Ворота миксинов.** Игра зовёт точку `native_*` только если какой-то мод подписан на её хук. HEAD-миксин на точке без метода в `Hooks`/Part молчит, поэтому у каждого такого миксина в `lib.rs` есть пустой метод.
- **`inline(always)` не берёт миксин.** `native_block_entity`, `native_state_for_placement`, `native_block_changed`: первые два стали методами Part в `lib.rs`, третий — миксин на его единственном вызывающем (`<GameLevel as LevelAccess>::observe_block_changed`).
- **Тип привязки RETURN — текст цели.** У `add_planned` это `HashMap<u32, u32>`, как в её сигнатуре; другой путь к тому же типу не подходит.
- **Вызов по значению.** `native_apply_effect_tick` и `native_living_incoming_damage` получают `call` по значению: миксин может взять только `&call` и ничего через него не изменит. Урон щита латника поэтому считает `lib.rs` (`living_incoming_damage`) функцией, которую тик уровня кладёт в `bridge`.
- **В блоках `block`, а не `place_block`.** Предмет головы ставит свою подставку через поле `block` файла предмета.
- **Два `serde_json`.** Значения SDK (`LootTableLoadEvent::table`) — `serde_json` игры, правила собраны со своим. Таблица лута переходит через текст, а то, что уходит в правила из модулей, — через `soulscraft_rules::serde_json`.
- **Ресурсы мода не лежат в дереве ClientResources.** На 8f83e5c ресурсы мода накладываются на паки, но в папку, которую читает `ClientResources`, не копируются: латник рисовался зомби. Риги и файлы трофеев вкомпилированы в крейт (`rig::embedded_text`, `trophy::embedded_text`), файл из пака по-прежнему главнее.
- **Атрибуты — базы, а не кэш `attrs`.** `m.attrs` — только кэш баз атрибутов (`mobs/finalize.rs`). Сохранение пишет базы, загрузка обрезает здоровье по сохранённому максимуму, а броню при собранной живой карте атрибутов игра читает только из неё. Статы от P и броня ставятся через `finalize::set_base`; иначе после выгрузки чанка латник возвращался с 30 здоровья, а броня ванильного зомби не росла.
- **Тег нежити не видит типы мода.** Проверка игры (`game/items/potions.rs`, `mob_can_be_affected`) ищет тип в теге как `minecraft:<путь>`, поэтому `soulscraft:revenant` в `#minecraft:undead` никогда не находится. Отказ нежити от отравления и регенерации даёт хук `mob_effect_applicable` в `lib.rs`.
- **Контекст установки требует настоящий предмет.** Хук `state_for_placement` получает контекст с предметом в руке; неизвестный предмет (например, блок головы, у которого нет своего предмета) игра отвергает, и хук не спрашивается.
- **Регистрация целей.** Мобов строит `horizon_client_lib`, его функции миксин не берут, а `EntityPart::register_goals` только добавляет цели. Поэтому новый моб подгоняется в `entity_join_level` (сервер) и обходом списка мобов на тиках уровня и клиента (копии у клиентов).
- **Текста в мире нет в world geometry.** `submit_custom_geometry` рисует только линии и четырёхугольники. Подписи фаз и «Алтарь hp/max» рисуются как таблички имён в рисовании самого моба и ядра (`compat_client::draw_label`); теперь их закрывают стены.
- **Пауза пустого сервера.** Выделенный сервер без игроков засыпает, и самотест из консоли стоит. Для самотеста: `horizon-server --dir <папка> config set properties.pause-when-empty-seconds=0`.
- **Сеть.** `mod_net` нет. Пять каналов объявлены в `register` (`play_to_client`: `soulscraft:state`, `altar`, `menu`, `book`; `play_to_server`: `soulscraft:press`), байты прежние, без обёртки. В повторе LetsCam пакеты приходят обычными custom payload: их ловит хук `client_packet`.
- **Block entity старых миров.** Тип block entity объявлен для каждого блока с block entity (ядро, подставка, 21 трофей), путь типа = путь блока. Без объявления игра молча выбрасывает сохранённые данные при загрузке чанка.

## Где лежат числа во время игры

- Сервер берёт `data/mobs.json`, вкомпилированный в крейт (`soulscraft/data/soulscraft/mobs.json`, копия от `sync.ps1`).
- `/sc reload` читает `config/soulscraft/mobs.json` в папке сервера. Туда его кладёт `prototypes/tune.ps1`. Если файл есть при старте, сервер берёт его.
- Другие моды (турели RustCraft: `turret_effects`, `effects.bleeding`) читали копию из контент-мода `horizon-mods/soulscraft/data/soulscraft/mobs.json`. Теперь копия лежит в пакете по тому же пути `data/soulscraft/mobs.json`; как её найдёт другой мод на новом маршруте, пока не проверено.

## Функции игры

Хук — метод `Hooks` или Part в `lib.rs`. Миксин — строка `[[native.mixin]]`, хелпер в `src/mixins/`. Пути — относительно `crates/horizon-client/src/` исходников 8f83e5c; `nh::` — `horizon_client::game::native_hooks::`.

| Старая точка (7e4e421) | Что делает мод | Маршрут на 8f83e5c |
| --- | --- | --- |
| `level_tick_post` | весь сервер: мобы, задачи команд, самотест, очереди `bridge` | ворота `level_tick_post`; HEAD `nh::base::Game::native_level_tick_post`; хук теперь зовётся и на клиенте, хелпер работает только на сервере |
| `register_goals` (по имени функции) | новый моб SoulsCraft: цели, рамка, флаги | ворота `entity_join_level`; HEAD `nh::base::Game::native_entity_join_level`; копии у клиента — `goals::fit_all` на тике клиента |
| `register_commands` | `/sc` (уровень 2), `sc book`, `soulscraft_selftest` и `soulscraft-kill` (4) | хук `register_commands` в `lib.rs`, без игры |
| `perform_command`, `perform_console_command` | команды игрока и консоли | ворота `run_mod_command`; HEAD `nh::base::Game::native_run_mod_command`; строка собирается из `CommandInvocation` (`bridge::command_line`) |
| `living_incoming_damage` | щит латника режет удар спереди | хук `living_incoming_damage` в `lib.rs`, функция из `bridge` |
| `should_apply_effect_tick_this_tick` | период кровотечения | хук в `lib.rs`, период из `bridge` |
| `apply_effect_tick` | удар кровотечения в очередь | ворота; HEAD `nh::base::Game::native_apply_effect_tick` (`&call`) |
| `net_from_server`, `net_from_client` | заметки клипов, виды и меню алтаря, книга; кнопки меню | объявленные пакеты, обработчики в `lib.rs` → `bridge` → тик клиента / уровня; повтор — хук `client_packet` |
| `mob_submit` | моб SoulsCraft из рига вместо зомби, подписи фаз | ворота `render_living_pre`; HEAD `nh::render_living_pre::Game::native_render_living_pre` |
| `native_world_primitives` (по имени функции) | контуры призрака, точки волны, лучи, столбики подписей | ворота `submit_custom_geometry`; HEAD `nh::submit_custom_geometry::Game::native_submit_custom_geometry` |
| `block_changed` | очередь изменений блоков алтаря | HEAD `game::level_access::<GameLevel as LevelAccess>::observe_block_changed` |
| `block_entity`, `block_entity_ticks`, `block_entity_tick`, `block_entity_update_tag`, `block_entity_render_bounding_box` | block entity ядра, подставки и трофеев: тикают, что видит клиент, рамка ядра | объявленные типы и `BlockEntityPart` в `lib.rs`, без игры; тики идут в `bridge` |
| `block_entity_submit` | предмет на подставке, призрак, головы, диорамы, фигурка книги, «Алтарь hp/max» | ворота Part; HEAD `nh::base::Game::native_block_entity_submit` |
| `right_click_block` | голова на стену, подставка и ядро | ворота; HEAD `nh::base::Game::native_right_click_block`; только сервер и главная рука, отмена кончает клик как `Success` |
| `destroy_block` | высокая вещь в творческом режиме не роняет остальное | ворота `break_block`; HEAD `nh::base::Game::native_break_block` |
| `tick` (окно) | меню алтаря и окно книги, очередь пакетов | ворота `client_tick_post`; HEAD `nh::client_tick_post::Game::native_client_tick_post` |
| `screen_render_post`, `screen_mouse_button_pressed_pre` | меню алтаря и окно книги | ворота; HEAD на `nh::base::Game::native_screen_render_post` и `…::native_screen_mouse_button_pressed_pre` |
| `state_for_placement` | поворот головы и диорамы к игроку | `BlockPart` `Placement` в `lib.rs` |
| `use_item` | клик книгой | `ItemPart` на `soulscraft:hunter_book` в `lib.rs` → `bridge` |
| `potential_spawns` | мобы SoulsCraft биома и сектора алтаря | ворота; HEAD `nh::native_potential_spawns::Game::native_potential_spawns` |
| `spawning_chunks` (RETURN) | чанки сектора повторяются | RETURN `game::natural_spawning::Game::spawning_chunks` |
| `spawn_hooks` (RETURN) | включал `potential_spawns` без trait-мода | не нужен: у мода есть `Hooks`, и `!mods().is_empty()` истинно |
| `mob_death_canceled` | смерть моба для книги и сервера | HEAD `nh::living_death::Game::mob_death_canceled` (до проверки подписки, ворот не нужно) |
| `add_planned` (RETURN) | мобы спавнера | RETURN `game::spawner::Game::add_planned` |
| `guest_die` | страховка хардкора | HEAD `game::lan_actions::respawn::Game::guest_die` |
| `native_loot_table_load` | горны и осколки в сундуках, Гнилой горн | хук `loot_table_load` в `lib.rs` |
| — | отказ нежити от отравления и регенерации | хук `mob_effect_applicable` в `lib.rs` |

## Как устроен моб

1. Тип `soulscraft:<моб>` копирует зомби (единственный подходящий шаблон: `content.rs:414-431`). Базовые атрибуты — из `mobs.json`. Размер типа мода в JSON не задаётся: строка типа в `entity-types.json` — строка шаблона (`mod_content.rs:829-833`), а в файле типа неизвестные поля запрещены (`content.rs:436-460`: `template`, `item`, `damage`, `attributes`, `deny_unknown_fields`). Поэтому рамку (`hitbox` из `mobs.json`) ставит модуль `goals` в копию типа каждого моба (`Arc::make_mut`): на сервере при входе моба в уровень, у клиента на его тике; рамка моба — ширина и высота его типа (`mobs/living.rs:489`), по ним же путь (`mobs/path.rs:430-432`).
2. Сила P — метка сущности `soulscraft.p=0.40`: она сохраняется с мобом. Без метки P считается по игрокам рядом (Г5) из брони, оружия и сводки PCT `pct:summary` (Г1; без сводки `T = 0`), или берётся из `/sc p`. Сводки читаются на тике уровня раз в 20 тиков в кэш (`power.rs`): внутри действия игрока данные других игроков недоступны (`native_place`).
3. При первом появлении в памяти (спавн, загрузка чанка, перезапуск) сервер ставит атрибуты от P базами атрибутов (`finalize::set_base`: здоровье, скорость, урон, сопротивление отбрасыванию, броня), их пишет сохранение; броня ещё и в карту брони команды `/attribute`, которую игра читает, пока живая карта атрибутов не собрана (`mobs/armor.rs`).
4. Каждый тик мозг из крейта получает «чувства» (позиции, щит игрока, стена за ним, крыша) и отдаёт приказ, взгляд, события фаз и удары. Решения — раз в `20 − 10·P` тиков (Г8), фазы атаки — каждый тик.
5. Удар — `sc_hurt_entity` (compat) с типом `minecraft:mob_attack` от моба: броня, щит игрока, неуязвимость и хуки урона работают как у ванили. Болт стрелка — стрела со скоростью 3 и уроном удар/3.
6. Клиент получает «клип и тик начала» только при смене и раз в 40 тиков всем; позу считает сам по тикам. Клиент только пишет заметки в свою таблицу (`client.rs`, `on_net`) и ничего не шлёт серверу и не ждёт ответа, поэтому в повторе записи LetsCam (заметки приходят обычными пакетами custom payload в исходное время, без сервера, и их ловит хук `client_packet`) клипы и подписи показываются так же: начало клипа пересчитывается от `now` каждой пачки.

## Как устроен ванильный моб

Срез 8, [../prototypes/p8-vanilla.md](../prototypes/p8-vanilla.md).

1. Тип ванили из `data/mobs.json` `vanilla.groups` принимается, когда тик уровня видит его впервые (после мобов SoulsCraft). Моб волны ивента (метка `soulscraft.wave` или список ивента) остаётся ивенту.
2. P — как у своих мобов (метка `soulscraft.p=`, иначе игроки рядом). Рост Г2 — модификаторы `soulscraft:power` к здоровью, урону и скорости на ванильной базе (`mobs/finalize.rs`: `add_modifier`, `AddMultipliedBase`), броня — база атрибута и карта брони типа (как у своих мобов). Здоровье сохраняет долю максимума (`scale::kept_health`: моб, которому другой мод поставил мало здоровья, так и остаётся; свежий полный — полон). Метка PCT `pct.xpNNN` — от ванильного здоровья. Флаги нежити, вид и теги не трогаются.
3. Спавнер в `spawner_radius_blocks` при первом принятии — метка `soulscraft.spawner`: рост есть, мозга нет (Г4).
4. Остальным в цели добавляются две цели-ворота с приоритетом раньше всех (`SoulscraftGate` — `MOVE | LOOK | JUMP`, `SoulscraftLeapGate` — `JUMP`), их не прерывают. Ворота срабатывают, пока мозг держит моба: так ванильные цели движения и атаки встают, и во время замаха ничего не бьёт. Ворота встают со следующего полного тика целей, поэтому при приказе «стоять» или «рывок» сервер каждый тик ещё и останавливает путь (`sc_mob_stop`), ставит управление движением в `Wait` и обнуляет `zza`/`xxa`; куб в замахе теряет и горизонтальную скорость своего прыжка. Флаг `goals.disabled` не подошёл: `riders::drive` снимает его каждый тик (`mobs/riders.rs:381-396`, из `server_ai_step`, `ai.rs:1959`).
5. Мозг (`soulscraft_rules::vanilla::VBrain`) получает цель ванили (`m.ai.target`: провокация PCT её меняет — мозг идёт за ней), отдаёт приказ, взгляд, позу `aggressive`, удары, сигнал замаха и вопросы серверу (высокая точка, укрытие от взгляда цели). Удар — `sc_mob_attack` (ванильный удар моба с его эффектами: голод кадавра, иссушение) с множителем приёма; снаряды — `sc_summon` трезубца, взрывного зелья или стрелы (id снаряда движок не отдаёт: бросок засчитан, если снаряд найден на месте командой `execute if entity`). `/sc force <приём>` берёт ближайшего моба SoulsCraft, а если его нет — ближайшего принятого ванильного с этим приёмом.
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
