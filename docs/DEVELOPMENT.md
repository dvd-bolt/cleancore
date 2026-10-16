# Подготовка и ежедневная разработка

## Текущее состояние

Приложение реализовано частично; аудит этапов 1–7 и критерии выпуска — в [отчёте](reports/STAGES1_7.md) и [RELEASE](RELEASE.md). Ближайшие задачи — в [TASKS](TASKS.md), контекст — в [STATUS](STATUS.md).

## Окружение

Во время исследования 2026-10-13 обнаружены Windows 11 Pro x64 (26200), Rust/Cargo 1.95.0 с MSVC target, rustfmt/Clippy, Node 24.15.0, npm 11.12.1, pnpm 11.25.0, Git 2.49.0, Visual Studio 2022 с C++ tools, SDK 10.0.22621.0 и WebView2 Runtime. Это историческая проверка, а не гарантия следующей сборки.

Перед работой проверить:

```powershell
git --version
rustc --version
cargo --version
rustup show
node --version
npx pnpm --version # либо corepack pnpm --version без elevation
```

Toolchain Rust закреплён в `rust-toolchain.toml` (1.95.0, MSVC target, rustfmt, clippy).
По отдельности подтвердить C++ tools, Windows SDK и WebView2. Отсутствие `cl.exe` в обычном PATH не доказывает отсутствие Visual Studio toolchain. Целевую Windows 10 проверять в отдельной среде.

## Выбранные соглашения

- Frontend-менеджер пакетов — pnpm; не создавать параллельные npm/yarn lock-файлы.
- Закрепить точную версию Rust stable и pnpm при создании проекта.
- Cargo.lock и pnpm-lock.yaml коммитятся после появления зависимостей.
- UTF-8, LF и отступы определены `.editorconfig`.
- Основной exe требует UAC по решению 006; shell из UI не используется. Cargo unit tests и frontend checks не требуют повышения.

## Планируемые команды

После создания scripts этапа 1:

```powershell
pnpm dev
pnpm tauri dev
pnpm tauri build
```

`pnpm dev` — только frontend, без фиктивных метрик: показать недоступность Tauri backend. `pnpm tauri dev` — приложение. Команды существуют в package.json; браузерный режим не выполняет системных действий.

Для прямой сборки автономного exe сначала выполните `pnpm build`, затем
`cargo build -p cleancore --release --bin cleancore --features custom-protocol --offline`.
Флаг `custom-protocol` встраивает frontend из dist и отключает devUrl. Без него
release-бинарник намеренно не компилируется. Tauri CLI включает этот флаг при build.
Проверка встроенного HTML, JS и CSS без запуска окна:
`cargo test -p cleancore --test offline_assets --features custom-protocol --offline`.

Проверки — в [TESTING](TESTING.md). Windows exe требует права администратора по прямому запросу пользователя (решение 006); не обходить UAC.

## Рабочий цикл

1. Прочитать правила, ТЗ, ROADMAP, TASKS и STATUS.
2. Выбрать модуль в границах текущего запроса, проверить зависимости.
3. Зафиксировать цель и критерии в [задаче](templates/TASK.md).
4. Сделать изменение, выполнить применимые проверки.
5. Обновить задачи, статус и CHANGELOG без копирования всей переписки.
6. Если Git уже создан, подготовить небольшой коммит; если публикация не авторизована, не выдумывать `origin`.

## Как продолжать в следующем чате

Пример задания: «Прочитай AGENTS.md, TECHNICAL_SPEC.md, docs/ROADMAP.md, docs/TASKS.md и docs/STATUS.md. Выполни только модуль <ID>. Не расширяй задачу на другие этапы. Проверь результат и обнови статус».

## Диагностика

Зафиксировать точную команду, выходной код, OS/build и воспроизводимые шаги. Проверить манифесты, lock-файлы и системные компоненты. Не обходить Access Denied изменением ACL/защит; повышенный запуск основного exe описан в решении 006. Перед публикацией логов убрать секреты и персональные пути.

## Runtime помощника

python scripts/prepare_assistant_runtime.py подготавливает закреплённый CPU runtime
в игнорируемом runtime/llama-b11429. Нужна сеть при первой подготовке; повторный запуск
с корректными hashes работает из готовых файлов. Веса скрипт не скачивает.
Обычный cargo test не требует DLL. Дополнительная проверка настоящего процесса:
cargo test -p cleaner-assistant real_runtime_rejects_truncated_weights -- --ignored.
Это тест отказа, а не тест качества модели.
