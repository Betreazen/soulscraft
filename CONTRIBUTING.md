# Как помочь проекту

Спасибо за интерес к SoulsCraft. Сообщения об ошибках, идеи по балансу и пулл-реквесты принимаются на русском и на английском.

## Ошибки и идеи

- **Ошибка:** откройте [issue по шаблону](https://github.com/Betreazen/soulscraft/issues/new/choose). Укажите версию мода, коммит Horizon и шаги. Строки журнала с `soulscraft` очень помогают.
- **Баланс:** числа атак и мобов — гипотезы, их проверяет плейтест ([docs/design/03-hypotheses.md](docs/design/03-hypotheses.md)). Напишите, какой замах вы успеваете прочитать, а какой нет, и сколько длится бой. `/sc debug on` подписывает фазы атак над мобами.

## Код

Нужны Rust (stable, edition 2024), для моделей — Blender 5.2, для сборки и проверки в игре — [лаунчер Horizon](https://github.com/horizon-rust/horizon) 0.2.96 или новее и сборка Horizon `8f83e5c` (Nightly 2026-11-21).

```powershell
cd mod/soulscraft
cargo horizon-mod check         # один раз: SDK сборки 8f83e5c в .horizon/
cargo test --lib                # правила без игры
cd ../..
.\mod\sync.ps1                  # разложить data/ и assets/ по пакету мода
.\mod\sync.ps1 -Check           # перед коммитом: копии не отстали
horizon-launcher native-build mod/soulscraft --version nightly-2026-11-21-8f83e5c --trust-native-code
```

Сборка, выделенный сервер под Linux, самотест и ловушки Mod API — в [mod/README.md](mod/README.md).

Правила проекта:

- **Код правил не зависит от игры.** Всё, что можно проверить тестом, живёт в крейте мода (`mod/soulscraft/src`, кроме `modules/` и `mixins/`) без `Game`, а модули с `Game` только передают события. Новое правило приходит с тестом.
- **Числа в данных.** Атаки, мобы, ивент и алтарь задают `data/*.json`, а не код. Копии в пакете мода (`mod/soulscraft/assets`, `mod/soulscraft/data`) пишет только `mod/sync.ps1`.
- **Атаки считаются в тиках.** Замах, удар и восстановление задаются в тиках, а не в секундах, и проверяются самотестом `vtiming_*`.
- **Модели строятся кодом.** Мобы, блоки и трофеи собираются скриптами в Blender ([assets/models/README.md](assets/models/README.md)); руками `.blend` и `.png` не правятся.
- **Проверка в игре.** Самотест в мире (`soulscraft_selftest run`): в одиночной игре и на выделенном сервере Linux ([tools/linux-selftest.sh](tools/linux-selftest.sh)). Боты: `prototypes/check-final.ps1 -Version <сборка клиента> -Export <экспорт сервера>` гоняет сцены на сервере Linux в `horizon-linux-dev` ([mod/README.md](mod/README.md)); остальные скрипты `prototypes/` писались для старого стенда на Horizon `7e4e421` ([prototypes/README.md](prototypes/README.md)).
- Исходники модулей мода (`mod/soulscraft/src/modules`) начинаются с `//`, а не с `//!`: их вставляют внутрь модуля игры. Хелпер миксина (`mod/soulscraft/src/mixins`) — ровно одна закрытая функция, без строк `use`.

Устройство кода и места вызова хуков Horizon описаны в [mod/README.md](mod/README.md), решения по дизайну — в [docs/design/](docs/design/).
