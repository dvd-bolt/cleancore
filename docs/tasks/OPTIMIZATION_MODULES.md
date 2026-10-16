# Модульный план реализации системы оптимизации CleanCore (OPT)

Версия: 1.0.0  
Статус: Реализовано (100%)  
Базовая спецификация: [docs/OPTIMIZATION_SPEC.md](../OPTIMIZATION_SPEC.md)  
Связанные документы: [ROADMAP.md](../ROADMAP.md), [TASKS.md](../TASKS.md), [ARCHITECTURE.md](../ARCHITECTURE.md), [API_CONTRACTS.md](../API_CONTRACTS.md), [SECURITY.md](../SECURITY.md)

---

## Общая матрица модулей

| Модуль | Подмодули | Ответственный крейт / Слой | Зависимости | Приоритет | Статус |
| --- | --- | --- | --- | --- | --- |
| **OPT-01** | 01.1, 01.2, 01.3 | `cleaner-system`, `src/components` | Существующий `cleaner-system` | **P0** | **Готово (100%)** |
| **OPT-02** | 02.1, 02.2, 02.3 | `cleaner-disk`, `src-tauri`, `src/pages/DiskPage` | MFT Snapshot из Этапа 5 | **P0** | **Готово (100%)** |
| **OPT-03** | 03.1, 03.2, 03.3 | `cleaner-disk`, `src-tauri`, `src/pages/DiskPage` | Known Folders, `cleaner-disk` | **P1** | **Готово (100%)** |
| **OPT-04** | 04.1, 04.2, 04.3 | `cleaner-cleanup`, `cleaner-service`, UI | Win32 Shell, `cleaner-execution` | **P1** | **Готово (100%)** |
| **OPT-05** | 05.1, 05.2, 05.3, 05.4 | `cleaner-service`, `cleaner-storage`, `ServicePage` | `powercfg`, SQLite Journal | **P2** | **Готово (100%)** |
| **OPT-06** | 06.1, 06.2, 06.3 | `cleaner-system`, `src-tauri`, `OverviewPage` | OPT-01, OPT-02, OPT-05 | **P2** | **Готово (100%)** |

---

## Спецификация модулей

### Модуль OPT-01: Телеметрия батареи и сетевого питания (Battery & AC Awareness)

#### OPT-01.1. Win32 `GetSystemPowerStatus` адаптер
* **Крейт:** `cleaner-system`
* **Файлы:** `crates/cleaner-system/src/power.rs`, `crates/cleaner-system/src/lib.rs`
* **Цель:** Получение реального статуса питания устройства через `windows-sys::Win32::System::Power::GetSystemPowerStatus`.
* **Контракт DTO:**
  ```rust
  pub struct PowerStatusSnapshot {
      pub is_battery_present: bool,
      pub is_ac_connected: bool,
      pub battery_percent: Option<f64>,
      pub is_charging: bool,
      pub power_save_active: bool,
  }
  ```
* **Границы и запреты:** Не опрашивать API чаще, чем раз в 2 секунды. Не вызывать WMI. При десктопном ПК без батареи возвращать `is_battery_present = false`, `battery_percent = None`.
* **Приёмка:**
  - `cargo test -p cleaner-system`: тест на десериализацию и проверку структуры.
  - На машине с батареей процент совпадает с системным треем Windows.

#### OPT-01.2. Индикатор батареи и питания в UI
* **Слой:** Frontend
* **Файлы:** `src/components/Header.tsx`, `src/components/Footer.tsx`, `src/types.ts`
* **Цель:** Отображение ненавязчивого индикатора заряда батареи и источника питания рядом с системными метриками.
* **Приёмка:** При питании от сети отображается значок `Сеть (AC)`, при работе от батареи — `Батарея X%`. При падении заряда ниже 20% цвет текста меняется на предупреждающий `text-warn`.

#### OPT-01.3. Power-Aware троттлинг фоновых задач
* **Крейт:** `cleaner-disk`, `cleaner-cleanup`
* **Цель:** Автоматическое ограничение параллелизма ресурсоёмких задач (MFT traversal, хеширование SHA-256 дубликатов) при работе от батареи.
* **Поведение:**
  - От сети (AC): параллелизм `num_cpus` (полная скорость).
  - От батареи: ограничение пула воркеров до 1–2 потоков для предотвращения просадок FPS и перегрева.
  - Батарея < 20%: блокировка запуска новых глубоких сканирований с сообщением пользователю.
* **Приёмка:** Тест проверяет, что при флаге `is_ac_connected = false` размер рабочего пула задач не превышает 2.

---

### Модуль OPT-02: Крупные файлы (Space Hogs) из MFT-индекса

#### OPT-02.1. Селектор крупных файлов в MFT движке
* **Крейт:** `cleaner-disk`
* **Файлы:** `crates/cleaner-disk/src/ntfs/space_hogs.rs`, `crates/cleaner-disk/src/ntfs/mod.rs`
* **Цель:** Мгновенный сбор среза Top-100 или Top-500 самых тяжёлых файлов тома из уже построенного дерева `MftSnapshot` без повторного дискового ввода-вывода.
* **Критерии фильтрации:**
  - Порог по размеру: конфигурируемый (по умолчанию `>= 500 МБ`).
  - Классификация по расширениям:
    - *Образы и контейнеры:* `.iso`, `.vhd`, `.vhdx`, `.vmdk`, `.qcow2`.
    - *Архивы:* `.zip`, `.7z`, `.rar`, `.tar`, `.gz`, `.bak`.
    - *Медиа:* `.mkv`, `.mp4`, `.mov`, `.avi`.
    - *Установщики:* `.exe`, `.msi`.
  - Исключение системных заблокированных файлов Windows (`pagefile.sys`, `swapfile.sys`, `hiberfil.sys`, `System Volume Information`) из списка удаляемых.
* **Приёмка:** Тест `ntfs::tests::space_hogs_extracts_sorted_entries_without_io`: тест на синтетическом MFT-дереве подтверждает корректную сортировку по убыванию размера и фильтрацию системных файлов.

#### OPT-02.2. IPC-команда и DTO
* **Слой:** Tauri Backend
* **Файлы:** `src-tauri/src/disk_commands.rs`, `src-tauri/capabilities/default.json`
* **Команда:** `get_space_hogs(min_size_mb: u64, category_filter: Option<String>) -> Result<Vec<LargeFileEntry>, String>`
* **Приёмка:** Автоматический тест в `src-tauri/src/ipc_tests.rs`.

#### OPT-02.3. Интерфейс крупных файлов в `DiskPage`
* **Слой:** Frontend
* **Файлы:** `src/pages/DiskPage.tsx`, `src/features/disk-map/SpaceHogsTable.tsx`
* **Цель:** Таблица с сортировкой по размеру, фильтрами по категориям, кнопками «Показать в Проводнике» (`Reveal`) и «Подготовить к удалению».
* **Приёмка:** Vitest UI-тест проверяет рендеринг таблицы, переключение фильтров и вызов IPC.

---

### Модуль OPT-03: Забытые и неиспользуемые файлы (Stale Files)

#### OPT-03.1. Сканер заброшенных файлов
* **Крейт:** `cleaner-disk`
* **Файлы:** `crates/cleaner-disk/src/stale_scanner.rs`
* **Цель:** Сканирование каталогов `Downloads` (`FOLDERID_Downloads`) и `Desktop` с оценкой времени неактивности по `LastAccessTime` / `LastWriteTime`.
* **Пороги давности:** 90 дней (3 мес), 180 дней (6 мес), 365 дней (1 год).
* **Приёмка:** Unit-тест с фикстурами файлов разных дат подтверждает попадание в выборку только объектов старше заданного порога.

#### OPT-03.2. Защитные фильтры
* **Цель:** Исключение ярлыков `.lnk`, скрытых файлов `.git`, системных `desktop.ini`, а также файлов с признаком облачной синхронизации (OneDrive/cloud sparse).
* **Приёмка:** Тест подтверждает, что `.lnk` и скрытые служебные файлы не предлагаются к удалению.

#### OPT-03.3. Пользовательский интерфейс Stale Files
* **Слой:** Frontend
* **Цель:** Просмотр сгруппированных по давности файлов в пользовательских каталогах с возможностью пакетного выбора и подтверждения удаления.
* **Приёмка:** Vitest-тест сценария выбора файлов и проверки блокировки повторного клика.

---

### Модуль OPT-04: Кэши разработки и Корзина

#### OPT-04.1. Детектор кэшей разработчиков
* **Крейт:** `cleaner-cleanup`
* **Цель:** Поиск неактивных каталогов `node_modules`, `.next`, `target/`, `.venv` в выбранных пользователем рабочих каталогах.
* **Критерий неактивности:** Дата последней модификации манифеста (`package.json`, `Cargo.toml`, `requirements.txt`) старше 60 дней.
* **Приёмка:** Тест на синтетической структуре папок проекта подтверждает нахождение `node_modules` и пропуск недавно изменявшихся репозиториев.

#### OPT-04.2. Детектор виртуальных дисков WSL/Docker
* **Крейт:** `cleaner-service`
* **Цель:** Проверка реального размера `ext4.vhdx` для Docker и WSL2. Если файл занимает более 20 ГБ, CleanCore информирует пользователя и предоставляет инструкцию по сжатию.
* **Приёмка:** Тест корректного определения путей к WSL/Docker образам в LocalAppData.

#### OPT-04.3. Очистка системной Корзины
* **Крейт:** `cleaner-service`
* **Цель:** Подсчёт занятого места в Корзине через `SHQueryRecycleBinW` и штатная очистка через `SHEmptyRecycleBinW` с флагами `SHERB_NOCONFIRMATION | SHERB_NOPROGRESSUI`.
* **Приёмка:** Тест на фикстуре проверяет корректность вызова Win32 API и обработку возвращаемых кодов `HRESULT`.

---

### Модуль OPT-05: Профили электропитания и транзакционный откат

#### OPT-05.1. Адаптер `powercfg` / Win32 Power API
* **Крейт:** `cleaner-service`
* **Файлы:** `crates/cleaner-service/src/power_tuning.rs`
* **Цель:** Чтение активной схемы (`powercfg /getactivescheme`), создание временного дубликата (`powercfg /duplicatescheme`), чтение/запись параметров процессора `PROCTHROTTLEMIN` и `PROCTHROTTLEMAX`.
* **Границы и запреты:** Не изменять заводские системные схемы Windows напрямую. Работать только через созданную схему-дубликат CleanCore.

#### OPT-05.2. Снимок и журнал отката в SQLite
* **Крейт:** `cleaner-storage`
* **Файлы:** `crates/cleaner-storage/src/power_journal.rs`
* **Таблица:**
  ```sql
  CREATE TABLE IF NOT EXISTS power_state_snapshots (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      created_at_utc TEXT NOT NULL,
      original_scheme_guid TEXT NOT NULL,
      original_scheme_name TEXT NOT NULL,
      original_cpu_min_ac INTEGER,
      original_cpu_max_ac INTEGER,
      original_cpu_min_dc INTEGER,
      original_cpu_max_dc INTEGER,
      is_restored INTEGER NOT NULL DEFAULT 0
  );
  ```
* **Приёмка:** Интеграционный тест: создание снимка → симуляция аварии → восстановление исходной схемы при следующем старте.

#### OPT-05.3. Профили производительности
* **Профили:**
  - *Quiet (Тихий):* Max CPU = 99% (отключение Turbo Boost), Min CPU = 5%.
  - *Balanced (Сбалансированный):* заводские параметры Windows.
  - *Performance (Производительный):* Min CPU = 100%, Max CPU = 100% (только при питании от сети).
* **Приёмка:** Проверка обратного считывания значений после переключения.

#### OPT-05.4. Карточка питания в `ServicePage`
* **Слой:** Frontend
* **Файлы:** `src/pages/ServicePage.tsx`, `src/features/service/PowerTuningCard.tsx`
* **Цель:** Переключатель профилей с подтверждением, индикатор активной схемы и кнопка «Вернуть заводские настройки».

---

### Модуль OPT-06: Автопилот процессов и Smart Insights

#### OPT-06.1. Process Watcher для тяжелых задач
* **Крейт:** `cleaner-system`
* **Цель:** Фоновое отслеживание запуска заранее настроенных приложений (игры, IDE, 3D-редакторы) с проверкой PID и времени старта.

#### OPT-06.2. Сессионное управление режимом
* **Цель:** При запуске целевого exe и наличии питания от сети (AC) CleanCore временно активирует профиль Performance. После завершения последнего отслеживаемого процесса система автоматически возвращается в исходный профиль.
* **Приёмка:** Тест симуляции жизненного цикла процесса с проверкой двух переключений схемы.

#### OPT-06.3. Виджет «Быстрые находки» на `OverviewPage`
* **Слой:** Frontend
* **Файлы:** `src/pages/OverviewPage.tsx`, `src/components/SmartInsightsWidget.tsx`
* **Цель:** Компактный блок из 2–3 карточек с максимальным потенциалом освобождения места (Space Hogs, забытые файлы, Корзина) с прямым переходом к действию.
* **Приёмка:** Vitest-тест отображения карточек рекомендаций и клика по кнопке перехода.

---

## Протокол изолированной разработки и приёмки

1. Каждый модуль реализуется в рамках своего крейта без модификации параллельных модулей.
2. Обязательные проверки для каждого модуля:
   - `cargo test -p <crate-name>`
   - `cargo clippy -p <crate-name> -- -D warnings`
   - `npx vitest run` (для UI компонентов)
   - `npx tsc --noEmit`
   - `python scripts/check_docs.py`
