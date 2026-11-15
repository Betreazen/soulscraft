# Безопасность · Security

SoulsCraft — нативный мод Horizon. Его код компилируется прямо в игру и в сервер и работает с правами пользователя, без песочницы. Поэтому лаунчер просит подтвердить доверие к моду (`--trust-native-code`). Перед установкой прочитайте код или ставьте мод только из этого репозитория по тегу релиза.

Что мод делает за пределами игры:

- хранит прогресс книги охотника в данных игрока (`soulscraft:book`), а состояние алтарей — в данных их блоков, внутри мира;
- читает `config/soulscraft/mobs.json` в папке сервера, если он есть, и ничего туда не пишет;
- не открывает сетевых соединений: клиент и сервер обмениваются сообщениями через соединение самой игры.

## Как сообщить об уязвимости

Не открывайте публичный issue. Используйте [приватное сообщение об уязвимости](https://github.com/Betreazen/soulscraft/security/advisories/new) на GitHub. Ответ — в течение нескольких дней. Исправление выйдет в ближайшем релизе с благодарностью в списке изменений, если вы не против.

---

SoulsCraft is a native Horizon mod: it is compiled into the game and the server and runs with your rights, unsandboxed. It keeps its state inside the world (player data and block data), only reads `config/soulscraft/mobs.json` and opens no network connections of its own. Report vulnerabilities privately through [GitHub security advisories](https://github.com/Betreazen/soulscraft/security/advisories/new), not in public issues.
