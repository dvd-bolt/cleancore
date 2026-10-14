---
name: stage5-isolated
description: Режим строгой изоляции разработки Этапа 5 (Анализ диска и дубликатов). Гарантирует, что агент работает исключительно в границах модулей 5.1–5.6 и крейта cleaner-disk, не трогая, не откатывая и не изменяя код этапов 1–4, которые параллельно разрабатываются/проверяются второй нейросетью.
---

# Скилл: stage5-isolated (Изолированная разработка Этапа 5)

## 🎯 Назначение и контекст
Пользователь ведёт разработку проекта CleanCore параллельно в двух разных нейросетях:
- **Вторая нейросеть** занимается ревизией, проверкой и доработкой предыдущих этапов (Этапы 1, 2, 3 и 4: мониторинг, процессы, сканер пользовательских temp-файлов, подтверждение и журнал очистки).
- **Данный агент** назначен строго на **Этап 5: Анализ диска и дубликатов — v0.2 (модули 5.1–5.6)** в ветке `feature/stage5-disk-analysis`.

Чтобы результаты работы двух нейросетей не конфликтовали, данный скилл устанавливает **строгие границы изоляции** (boundary rules).

---

## 🔒 Белый список (Разрешённая зона ответственности)

Агент имеет право создавать, редактировать и развивать **ТОЛЬКО** следующие файлы и компоненты:

1. **Крейт анализа диска:**
   - `crates/cleaner-disk/**` (модели `models.rs`, обход дерева `analyzer.rs`, трёхфазный поиск `duplicates.rs`, 10-минутный буфер метрик `history.rs`, фоновые задачи `tasks.rs`, тесты и документация).

2. **IPC бэкенда для Этапа 5:**
   - `src-tauri/src/disk_commands.rs` (команды анализа структуры, дубликатов и истории ресурсов).

3. **Frontend интерфейс для Этапа 5:**
   - `src/pages/DiskPage.tsx` (Studio Pearl интерфейс карты диска, категорий, топа файлов/папок, инспектора дубликатов и графиков 10-минутной истории).

4. **Минимально необходимые точки подключения (Integrations only):**
   - `src-tauri/Cargo.toml` (подключение зависимости `cleaner-disk`).
   - `src-tauri/src/state.rs` (добавление поля `pub disk_manager: Arc<DiskManager>`).
   - `src-tauri/src/lib.rs` (регистрация `pub mod disk_commands;`, вызов `state.disk_manager.history.record_snapshot` и регистрация команд диска в `invoke_handler!`).
   - `src-tauri/build.rs` и `src-tauri/capabilities/default.json` (регистрация разрешений команд диска).
   - `src/types.ts` (добавление интерфейсов диска и `"disk"` в `ActivePage`).
   - `src/lib/ipc.ts` (добавление функций `fetchQuickDirectories`, `startDiskAnalysis`, `getDiskAnalysisProgress`, `getDiskAnalysisReport`, `startDuplicateScan`, `getDuplicateScanProgress`, `getDuplicateReport`, `cancelDiskTask`, `fetchResourceHistory`).
   - `src/components/Header.tsx` (добавление пункта навигации «Диск»).
   - `src/App.tsx` (подключение страницы `DiskPage` для `activePage === "disk"`).
   - Документация этапа: отметки в `docs/TASKS.md`, `docs/STATUS.md`, `CHANGELOG.md` для Этапа 5.

---

## 🚫 Чёрный список (Строго запрещено модифицировать)

Агенту **КАТЕГОРИЧЕСКИ ЗАПРЕЩАЕТСЯ**:

- ❌ Трогать, редактировать или рефакторить крейты этапов 1–4:
  - `crates/cleaner-cleanup/**`
  - `crates/cleaner-execution/**`
  - `crates/cleaner-system/**` (за исключением минимального сохранения компилируемости при сборке воркспейса)
  - `crates/cleaner-storage/**`
- ❌ Модифицировать чужие страницы интерфейса:
  - `src/pages/CleanupPage.tsx`
  - `src/features/cleanup-*/**`
  - `src/pages/OverviewPage.tsx`
  - `src/pages/ProcessesPage.tsx`
  - `src/pages/JournalPage.tsx`
  - `src/pages/SettingsPage.tsx`
- ❌ Выполнять команды сброса состояния чужих файлов:
  - `git checkout -- <чужие файлы>`
  - `git restore <чужие файлы>`
  - `git reset --hard`
  Любые незакоммиченные изменения второй нейросети должны бережно сохраняться!

---

## 🧪 Правила тестирования и проверок

1. **Таргетированные тесты:** Всегда запускать проверку своего крейта в приоритетном порядке:
   ```powershell
   cargo test -p cleaner-disk
   cargo clippy -p cleaner-disk --all-targets -- -D warnings
   ```
2. **Изолированные тесты фронтенда:**
   ```powershell
   npm test src/lib/ipc.test.ts
   npm run build
   ```
3. **Общий чекинг:** Проверять общую компилируемость воркспейса (`cargo check --workspace`), не ломая существующие интерфейсы.

---

## 🛡️ Безопасность Этапа 5
- Поиск дубликатов — строго инспекционный (режим чтения). Никакого автоматического удаления файлов!
- Защита от reparse points/junctions/symlinks.
- Исключение Windows Hard links из дубликатов.
- Работа строго без прав администратора.
