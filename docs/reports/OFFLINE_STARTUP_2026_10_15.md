# Исправление автономного запуска

Дата: 2026-10-15. Статус: исправлено, автономный exe пересобран и обновлён по ярлыку.

## Причина и изменения

Предыдущая прямая команда cargo build --release не включала Tauri custom-protocol.
Профиль Rust release сам по себе не переключает Tauri в production: generate_context
получал dev=true и пустые встроенные assets при настроенном devUrl. Окно пыталось
открыть http://127.0.0.1:5173, хотя Vite не работал. Это локальный сервер разработки,
а не интернет-сервис CleanCore. Прежнее утверждение о готовности exe к автономному
запуску было ошибочным; проверка PE-версии этого не обнаруживала.

Причина проверена по установленным исходникам tauri-macros 2.7.2 (context.rs),
tauri-codegen 2.7.2 (context.rs) и tauri 2.12.3 (lib.rs).

- Cargo feature custom-protocol передаёт tauri/custom-protocol.
- Release main без этого флага останавливается compile_error.
- Packaging-тест проверяет production mode, встроенный index.html и наличие
  каждого указанного в нём JS/CSS-файла. Тест не запускает окно или Vite.
- Прямая команда сборки в DEVELOPMENT исправлена. Frontend, удаление и IPC не менялись.

## Проверки

| Проверка | Результат | Ограничения |
| --- | --- | --- |
| rustfmt --check main.rs и offline_assets.rs | Пройдена | Только затронутые Rust-файлы |
| cargo build --release --features custom-protocol --offline | Exit 0, 1 мин 33 сек | Повторная сборка после освобождения места |
| cargo test --test offline_assets --features custom-protocol --offline | 1 passed, exit 0 | После освобождения места; окно/Vite не запускались |
| cargo clippy -p cleancore --features custom-protocol --test offline_assets --bin cleancore --offline -- -D warnings | Exit 0 | Затронутые цели |
| git diff --check | Exit 0 | Проверка оформления |
| python scripts/check_docs.py | Exit 0 | 366 файлов, 230 локальных ссылок, 79 таблиц, 54 модуля |

Итоговый exe: target/release/cleancore.exe, 13 531 648 байт, 2026-10-15 23:14:49
по местному времени. FileVersion 0.0.4; UAC requireAdministrator сохранён.
SHA-256: `78FE2412235C237BE834C4E74A05DC289D038C9C6D7499EF25F7794F9A87C196`.
Desktop/CleanCore.lnk повторно проверен: указывает на этот файл.

При заполнении C: удалены только 16 объектных файлов текущей неудачной компиляции
tauri-c4cb58ba8853baf9.*.rcgu.o (113 641 468 байт). Старый compiler archive обратимо
сжат с 892 587 126 до 361 414 656 байт. Исходники и пользовательские данные не удалялись.

## Риски и остаток

Автоматическая проверка встроенных ресурсов не заменяет проверку нативного окна.
Версия остаётся 0.0.4. Программа требует установленного WebView2 Runtime, но её
анализ и очистка работают локально; dev-server и доступ к интернету им не нужны.
Открытые пункты очистки и выпуска остаются в прежнем плане.
