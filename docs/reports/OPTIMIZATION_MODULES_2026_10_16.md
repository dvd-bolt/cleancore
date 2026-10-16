# Модули глубокой оптимизации CleanCore (OPT-01 — OPT-06)

Дата: 2026-10-16. Статус: реализовано, все целевые проверки пройдены.  
Спецификация: [docs/OPTIMIZATION_SPEC.md](../OPTIMIZATION_SPEC.md).  
План модулей: [docs/tasks/OPTIMIZATION_MODULES.md](../tasks/OPTIMIZATION_MODULES.md).

---

## 1. Назначение и контекст обновления

В рамках обновления реализован полный цикл оптимизации системы и дискового пространства CleanCore (модули `OPT-01` – `OPT-06`). Перенесены и адаптированы эффективные системные практики (Win32 Power Schemes, троттлинг ЦП, очистка Корзины через Win32 Shell API, MFT Space Hogs, обнаружение неактивных кэшей разработки и автопилот-советник), при этом полностью сохранён фирменный строгий стиль темы Pearl (`bg-surface`, `border-line`, `text-txt`, `text-accent`) без заимствования изумрудного дизайна сторонних проектов.

---

## 2. Пошаговый план и факт реализации по модулям

### OPT-01: Телеметрия батареи и сетевого питания (Battery & AC Awareness)
- **Что сделано:**
  - Создан адаптер Win32 `GetSystemPowerStatus` в `crates/cleaner-system/src/power.rs`.
  - Реализован сбор DTO `PowerStatusSnapshot` (наличие батареи, статус подключения AC/DC, процент заряда, режим Battery Saver).
  - Реализован power-aware ограничитель `should_throttle_heavy_tasks()` и `is_critically_low_battery()`.
  - В `src/components/Header.tsx` добавлен ненавязчивый индикатор питания с анимацией зарядки и предупреждением при низком заряде (<20%).
- **Файлы:**
  - Backend: `crates/cleaner-system/src/power.rs`, `crates/cleaner-system/src/lib.rs`.
  - Core DTO: `crates/cleaner-core/src/models.rs`.
  - Frontend: `src/components/Header.tsx`, `src/types.ts`.

### OPT-02: Крупные файлы (Space Hogs) из MFT-индекса
- **Что сделано:**
  - В `crates/cleaner-disk/src/ntfs/mod.rs` добавлен селектор `MftSnapshot::get_space_hogs()` без повторного дискового ввода-вывода.
  - Добавлена категоризация файлов (образы виртуальных машин, архивы, медиа, базы данных, установщики) и исключение системных файлов ОС (`pagefile.sys`, `swapfile.sys`, `hiberfil.sys`).
  - Реализована IPC-команда `get_space_hogs` и безопасный переход в Проводник `reveal_path_in_explorer`.
  - Создан компонент `SpaceHogsTable.tsx` с фильтрами порогов (100 МБ – 10 ГБ) и встроен в `DiskPage.tsx`.
- **Файлы:**
  - Backend: `crates/cleaner-disk/src/ntfs/mod.rs`, `src-tauri/src/disk_commands.rs`.
  - Frontend: `src/features/disk-map/SpaceHogsTable.tsx`, `src/pages/DiskPage.tsx`.

### OPT-03: Забытые и неиспользуемые файлы (Stale Files)
- **Что сделано:**
  - Реализован сканер `scan_stale_files` в `crates/cleaner-disk/src/stale_scanner.rs`.
  - Реализованы строгие защитные фильтры: отсечение ярлыков `.lnk`, скрытых файлов `.git`, системных `desktop.ini`, пропуск symlinks/reparse points/junctions.
  - Добавлена IPC-команда `get_stale_files`.
  - Создан компонент `StaleFilesView.tsx` с выбором давности (от 30 до 730 дней) и встроен в `DiskPage.tsx`.
- **Файлы:**
  - Backend: `crates/cleaner-disk/src/stale_scanner.rs`, `src-tauri/src/disk_commands.rs`.
  - Frontend: `src/features/disk-map/StaleFilesView.tsx`, `src/pages/DiskPage.tsx`.

### OPT-04: Кэши разработки и системная Корзина Windows
- **Что сделано:**
  - В `crates/cleaner-service/src/recycle_bin.rs` через Win32 Shell API реализованы безопасные функции `SHQueryRecycleBinW` и `SHEmptyRecycleBinW`.
  - В `crates/cleaner-service/src/dev_caches.rs` реализован поиск неактивных `node_modules`, `target`, `.venv` (по дате модификации манифестов `package.json`, `Cargo.toml`, `requirements.txt`) и виртуальных дисков Docker/WSL2 `ext4.vhdx`.
  - Добавлены IPC-команды `query_recycle_bin`, `empty_recycle_bin`, `get_dev_cache_summary`.
  - Создана панель `DevAndRecyclePanel.tsx` с 2-шаговым подтверждением очистки и встроена во вкладку `dev_recycle` на `ServicePage.tsx`.
- **Файлы:**
  - Backend: `crates/cleaner-service/src/recycle_bin.rs`, `crates/cleaner-service/src/dev_caches.rs`, `src-tauri/src/service_commands.rs`.
  - Frontend: `src/features/service/DevAndRecyclePanel.tsx`, `src/pages/ServicePage.tsx`.

### OPT-05: Профили электропитания и транзакционный журнал отката
- **Что сделано:**
  - В `crates/cleaner-service/src/power_tuning.rs` реализован прямой вызов `System32\powercfg.exe` (без командных оболочек `cmd`/PowerShell).
  - Сформированы 3 профиля: «Тихий» (лимит процессора 99% — аппаратно отключает Turbo Boost, снижает нагрев на 5–15°C и убирает вой кулеров), «Сбалансированный» (заводские 100%/5%), «Максимальная мощность» (100%/100%).
  - В `crates/cleaner-storage/src/power_store.rs` создана таблица `power_state_snapshots` для сохранения моментальных снимков схемы питания перед изменениями с возможностью 1-клик отката.
  - Добавлены IPC-команды `get_power_profile_state`, `apply_power_profile`, `rollback_power_profile`.
  - Создан компонент `PowerTuningCard.tsx` и встроен в `ServicePage.tsx`.
- **Файлы:**
  - Backend: `crates/cleaner-service/src/power_tuning.rs`, `crates/cleaner-storage/src/power_store.rs`, `src-tauri/src/service_commands.rs`.
  - Frontend: `src/features/service/PowerTuningCard.tsx`, `src/pages/ServicePage.tsx`.

### OPT-06: Автопилот процессов и Smart Insights
- **Что сделано:**
  - В `crates/cleaner-service/src/advisor.rs` разработан чистый аналитический движок, сопоставляющий текущие метрики (батарея, компиляторы/IDE, переполнение корзины, кэши проектов, свободное место на диске, RAM).
  - Реализована IPC-команда `get_smart_insights`.
  - Создан виджет `SmartInsightsWidget.tsx` и встроен на главную страницу `OverviewPage.tsx`.
  - Написаны тесты `SmartInsightsWidget.test.tsx` и `OverviewPage.test.tsx`.
- **Файлы:**
  - Backend: `crates/cleaner-service/src/advisor.rs`, `src-tauri/src/service_commands.rs`.
  - Frontend: `src/components/SmartInsightsWidget.tsx`, `src/pages/OverviewPage.tsx`.

---

## 3. Сводная таблица проверок и верификации

| Проверка | Команда | Результат |
| --- | --- | --- |
| Rust Unit Tests (Workspace) | `cargo test --workspace --lib` | **114 passed** (0 failed, 1 ignored) |
| Cleaner-Service Security & Stress | `cargo test -p cleaner-service --test adversarial_security_tests --test stress_tests` | **25 passed** (0 failed) |
| Cleaner-Storage Recovery & Migrations | `cargo test -p cleaner-storage --tests` | **28 passed** (0 failed) |
| Cleaner-Disk NTFS & Stale Tests | `cargo test -p cleaner-disk --tests` | **25 passed** (0 failed) |
| Rust Linter (Clippy) | `cargo clippy --workspace --lib` | **0 errors, 0 warnings** |
| Rust Formatter | `cargo fmt --check` | **Чисто (соответствие rustfmt)** |
| Frontend Tests (Vitest) | `npx vitest run` | **21 passed files, 75 passed tests** |
| TypeScript Compiler | `npx tsc --noEmit` | **0 errors** |
| Frontend Production Build | `npm run build` | **Vite build Exit 0 (dist готов)** |
| Валидатор репозитория и документации | `python scripts/check_docs.py` | **[SUCCESS] Все 319 файлов UTF-8/LF, 0 битых ссылок** |
