# Контракты API

Фактическая регистрация команд находится в src-tauri/src/lib.rs, build.rs и capabilities.
Типы frontend описаны вручную; изменения контракта проверяются тестами сериализации и IPC.

## Общий контракт

### Выбор папки в интерфейсе

`choose_analysis_folder` принимает только инжектированное локальное окно Tauri,
не принимает пользовательский путь и возвращает `string | null`. Отмена не
меняет текущий выбор UI. Выбор не запускает анализ или удаление; путь проверяется
обычными командами анализа при отдельном запуске. Доступ ограничен существующей
capability локального окна main; новых зависимостей и shell-выполнения нет.

- Аргументы команд валидирует Rust backend.
- DTO frontend/backend согласуются проверками сериализации. Внутренние типы сторонних библиотек в публичный контракт не попадают.
- Время снимка — UTC с явной единицей/форматом. Длительность вычисления скоростей измеряется монотонными часами.
- Значения размеров — байты; проценты и скорости имеют указанную шкалу. Для целых значений, не представимых точно в JavaScript, используется десятичная строка.
- Отсутствующие данные не превращаются в ноль. Поле имеет доступность и причину: нет замера, отказ доступа, ошибка источника или устаревание.
- Ошибка содержит стабильный код и безопасное сообщение; внутренний stack trace или полный персональный путь не выдаётся без необходимости.

## Группы операций

| Планируемая операция | Вход | Результат | Этап |
| --- | --- | --- | --- |
| Получить метрики | Без произвольных системных аргументов | Снимок CPU/RAM/дисков/сети и доступность полей | 2 |
| Получить процессы | Проверенные фильтры/сортировка при необходимости | Процессы, доступность полей, время снимка | 2 |
| Читать/сохранять настройки | Разрешённые настройки, валидный интервал | Проверенные настройки | 2 |
| Начать scan | Идентификатор встроенной категории | Идентификатор задачи | 3 |
| Получить состояние задачи | Идентификатор задачи | Состояние, прогресс, ошибки | 3 |
| Запросить отмену | Идентификатор задачи | Подтверждение запроса, не обещание отката | 3 |
| Читать preview | Идентификатор preview и параметры порции | Кандидаты, исключения, версия правил | 3 |
| Исполнить выбор | Preview ID, выбранные object ID, привязанное подтверждение | Идентификатор операции | 4 |
| Читать/очистить журнал | Проверенные фильтры либо явный запрос очистки | Записи или результат очистки истории | 4 |
| Экспортировать журнал | Запрос экспорта и штатный выбор файла | Результат сохранения | 6 |

Названия Rust-функций и wire-поля закрепляются при реализации группы; изменение после выпуска требует версии контракта либо миграции совместимости.

## Модели

| Модель | Минимальные сведения |
| --- | --- |
| MetricSnapshot | Время, значения, доступность, интервал измерения |
| ProcessSnapshot | Время, PID, имя, CPU, RAM, доступный путь; идентичность процесса учитывается при смене PID |
| TaskStatus | ID, вид, состояние, обработанные объекты, известный общий объём, отмена |
| CleanupPreview | ID, категория, версия правила, время scan, кандидаты и причины пропуска |
| CleanupCandidate | Object ID, отображаемый путь, размер, категория, риск/ограничения |
| CleanupOutcome | Operation ID, per-object результаты, сводка и состояние журнала |
| AuditRecord | Время, операция, категория, объект/безопасный идентификатор, действие, исход, код причины |

## Жизненный цикл

Задача: выполняется → завершена / отменена / ошибка. Прогресс может быть неопределённым; неизвестный общий объём не заменяется фиктивным процентом.

Preview не расширяется после подтверждения. Изменение правил, набора или объектов требует нового preview либо безопасного пропуска. Повторное исполнение контролируется backend. Неизвестный preview отклоняется.

Исход объекта: обработан / пропущен / ошибка / неизвестный результат. Отменённая операция может содержать уже обработанные объекты.

## Разрешения

Команды разрешены только необходимому локальному окну. Проверить ограничения собственных команд Tauri, а не только permissions плагинов. Очистка не принимает произвольные shell-строки и новый список путей.

## Проверки

Некорректные аргументы, неизвестные ID, повторные запросы, устаревшие preview, частичная доступность, точность чисел и разрыв UI/backend покрываются тестами при появлении контрактов.

## Реализованные контракты этапа 4

- `get_cleanup_availability()` → исполнение разрешено/заблокировано, постоянство журнала, причина.
- `get_execution_preview(previewId)` → указанный неизменяемый execution-preview либо null. Без ID не подставляется последний глобальный снимок.
- `prepare_cleanup_confirmation(previewId, selectedIds)` → одноразовое подтверждение точного набора с TTL 120 секунд. Новый выбор аннулирует прежний токен.
- `start_confirmed_cleanup(confirmationId, acknowledged)` → зарегистрированный Running outcome с operation_id, затем spawn_blocking worker. Без true, постоянного журнала и допущенного адаптера отказ.
- `get_cleanup_operation(operationId)` → прогресс/результат или null, включая известный исход из SQLite.
- `cancel_cleanup(operationId)` → принят ли запрос остановки следующих объектов.
- `get_cleanup_journal(before, limit)` → cursor-page; IPC limit 1..25, UI порциями по 10.

Wire DTO — `cleaner-core/src/cleanup.rs`; frontend — `src/features/cleanup-execution/types.ts`. Байты — десятичные строки, временные метки — Unix milliseconds (UTC). ID не превышают 128 байт; объекты берутся только из backend preview, пути через IPC не принимаются. Preview живёт 10 минут, максимум 1000 объектов и 8 активных снимков. Публикация TrustedCandidate доступна только Rust-сканеру.

Сканер захватывает TrustedCandidate во время scan и публикует только полный снимок. `get_cleanup_preview(previewId, offset, limit, query)` возвращает страницу, `execution_preview_id`, `complete`, `cancelled`, `diagnostics`, `unfiltered_candidates`. Поиск применяется до пагинации по всему снимку. Limit 1..100; ID до 128 байт; query до 512 байт. `size_bytes` и `total_bytes` — десятичные строки. Object IDs совпадают в анализе и подтверждении. Partial/Cancelled/Failed не разрешают исполнение. Задача сохраняется 10 минут, одновременно работает один scan; максимум 8 записей. Контракт объединения — [STAGE4_INTEGRATION](STAGE4_INTEGRATION.md).

## Контракты аудита этапов 1–7

- MetricSnapshot дополнен disks и network_interfaces: массивы FieldValue для всех
  доступных накопителей и отдельных сетевых интерфейсов. Существующие disk/network — агрегаты для карточек.
- ResourceHistory: массивы number | null, согласованные с timestamps_utc. null не означает ноль.
  Окно ограничено монотонными 600 секундами и 1201 точкой.
- DiskManager: одна активная задача, сохранён только текущий результат. Превышение лимитов
  сканирования (32 уровня, 100000 объектов) — ошибка, не доказательство полного анализа.
- generate_csv_report(config) возвращает текст CSV; непустой target_path отвергается.
- save_csv_report(config) и export_cleanup_journal() открывают системный диалог, создают
  новый файл и возвращают путь либо null при отмене. Цель записи из webview не принимается.
  Экспорт журнала ограничен последними 200 операциями и 10000 объектами.
- get_storage_resilience_status() показывает пути, постоянство SQLite/settings.json,
  версию схемы и сведения о восстановлении; ошибки версии не подставляются нулём.

Разрешения новых экспортных команд доступны только локальному main.

## Локальный помощник (этап 8)

Команды разрешены только локальному main:

- assistant_status() → enabled, runtime_present, model?, task?. Наличие exe не означает
  проверенную целостность: SHA-256 проверяются непосредственно перед запуском.
- assistant_choose_model() → ModelInfo?; путь выбирает нативный диалог, frontend его не задаёт.
- assistant_enable(modelId, licenseSource, accepted) привязывает согласие к выбранному ID;
  источник 4..512 байт, accepted обязателен. Модель не включается без выбора.
- assistant_prepare_context(selection) → PreparedContext: metrics bool, preview_id?,
  journal_ids (не более 5). Backend извлекает только агрегаты из существующих ID,
  дополнительные поля запрещены. Пути, имена процессов и содержимое файлов не включаются.
- assistant_ask(contextId, question) → task ID; только текущая сводка до 120 секунд,
  вопрос 1..1024 байт, один запрос. Контекст не принимается напрямую из webview.
- assistant_cancel() отменяет запрос; assistant_disable() также выключает помощника,
  очищает выбранную модель/контекст. Ответы и лицензия не сохраняются между запусками.

Task state: running/completed/cancelled/failed, answer?, error?, recommendations.
Рекомендации — только processes/disk/cleanup/journal. Текст модели не управляет переходами
или командами приложения. Никаких IPC для выбора exe/аргументов runtime нет.

## Исправления 2026-10-15

DiskAnalysisReport дополнен complete и diagnostics. Недоступный дочерний каталог/
ограничение обхода дают частичный отчёт, не потерю всех данных. Ошибка самого корня
по-прежнему fatal. get_quick_directories включает локальные fixed/removable диски.

CleanupConfirmation: irreversible bool (по умолчанию false для старых данных).
CleanupItemStatus дополнен Deleted; CleanupOutcome.deleted_bytes — десятичная строка,
по умолчанию "0" для старых записей. Recycled/recycled_bytes сохранены.
Production BoundDelete — явно подтверждаемое удаление без Корзины. Partial с пропусками
может иметь execution_preview_id только если нет отмены/fatal/ресурсных лимитов;
выбор требует дополнительного согласия. Исполнение остаётся по ID и проверенному handle.

set_startup_enabled(itemId, expectedCommand, enabled, acknowledged) доступна local main.
Поддерживает только HKCU_Run_* и StartupFolderUser_* из свежего backend перечня.
Фронтенд не задаёт путь ключа или binary данные. Без acknowledged отказ; неизвестный
формат/нет прав/изменённая команда дают ошибку без shell/elevation.

## MFT-анализ (решение 006)

DiskAnalysisReport: engine (`ntfs-mft`/`directory`), engine_note (причина fallback или null), elapsed_ms, allocated_bytes/volume_used_bytes (null при обычном обходе), warnings (особенности учёта, отдельно от diagnostics). complete относится к поддерживаемому unnamed DATA учёту, не всему содержимому тома/потокам. Hard links считаются один раз.

get_disk_analysis_report(reportId, path?) с path возвращает подпапку из привязанного к отчёту текущего MFT-индекса; filesystem scan не запускается. Чужой том, stale ID или отсутствие индекса возвращают REPORT_NOT_FOUND. Подготовка отчёта выполняется spawn_blocking.

## Treemap API

get_disk_treemap(reportId,path,sizeMode: logical|allocated) → TreemapData {root,extensions,empty_files,empty_file_count,parent_path,indexed}. TreemapNode {id,name,path,size_bytes,logical_bytes,allocated_bytes,is_dir,is_aggregate,file_count,extension,children}; ID — строка полного file reference. Размер дерева <=12000, глубина <=24; агрегаты сохраняют сумму, не являются отдельными файлами. 128 строк extensions, `*` — суммарные прочие типы без highlight. Empty files — первые 100 в текущей папке; total отдельно.

reveal_disk_entry(reportId,entryId) → void. Путь разрешается из текущего индекса, передача собственного pathname невозможна. Shell32 выбирает объект в Проводнике, файл не запускается. Stale report/entry или исчезнувший объект дают типизированную ошибку. Оба command разрешены только local main.


## Расширение очистки 0.0.4

- get_cleanup_rules() возвращает 11 фиксированных CleanupRule с preview_only и последствиями. get_cleanup_profile_context() показывает LocalAppData текущей учётной записи.
- start_cleanup_scan(categoryId, projectPath?) принимает путь только для project-rust-target-v1/project-node-modules-v1: backend проверяет манифест, Known Folder boundary и фиксированный дочерний кеш. Для других категорий projectPath отвергается. Это выбор области анализа, не delete(path).
- resume_cleanup_scan(taskId) продолжает backend-курсор, отзывает старый execution-preview/confirmations и отклоняется при активной очистке. Внутренние порции с прогрессом продолжаются автоматически; готовая партия требует явного перехода.
- CleanupPreviewPage дополнен resumable и batch_ready. Список ограничен 1000 кандидатами на партию; total_bytes относится к партии, processed_items — к обходу. Отмена/fatal/неразрешённые лимиты не публикуют execution-preview.
- prepare_disk_file_cleanup(reportId, entryId) разрешает индексированный файл/небольшую папку только в заданных пользовательских Known Folders на DRIVE_FIXED. MFT reference и полный 64-битный номер тома сверяются с открытым объектом, готовится свежий точный список. Индекс без номера тома не авторизует действие. Удерживаемые объекты повторно сверяются по FILE_ID_INFO (128-битный ID и номер тома).
- prepare_duplicate_cleanup(reportId, groupId, keeperIndex, selectedIndices) не принимает путей и не позволяет выбрать сохраняемый оригинал. Исполнитель повторно хеширует обе копии и удерживает оригинал.
- invalidate_disk_report(reportId) отзывает только соответствующий сохранённый отчёт; после изменения требуется новый анализ. Не изменяет файловую систему.
- get_cleanup_preferences()/save_cleanup_preferences(preferences): excluded_paths и protected_duplicate_roots, до 100 полных локальных путей каждого вида, без масок/traversal. Сохранение отзывает подтверждения и запрещено при активном исполнении. Перед каждым действием адаптер проверяет актуальную защиту.
- clear_duplicate_hash_cache() удаляет только записи внутреннего кеша. Кеш анализа не является разрешением изменения файлов.

Все команды доступны только локальному main, имеют отдельные capabilities. Новые категории не открывают shell, автоматическое удаление или обход Windows ACL. Решение 007 уточняет прежнюю привязку только к user-temp-v1.


### Возможные остатки приложений

launch_uninstaller_with_residue_context(programId, acknowledged) требует явного acknowledged=true, получает программу из актуального реестра, запоминает имя/издателя/ID и запускает прежний проверенный штатный деинсталлятор. Результат расширяет LaunchResult полем residue_context_id. Это запуск, а не доказательство завершения удаления.

inspect_app_residue(contextId) возвращает read-only ResidueReport: app_name, still_registered, complete, inspected_entries, candidates и warnings. Кандидат: path, reason, confidence, logical_bytes (строка или null), files, complete. Контекст ограничен текущим запуском и 30 минутами, максимум 16. Сканируется только LocalAppData/Roaming текущего профиля; бюджеты 10 000 записей/2 секунды/16 уровней/128 результатов. Никаких путей или операций удаления эта команда не принимает. Команды ограничены локальным main.


### Общий список и очистка (2026-10-16)

Все команды ограничены локальным main. Никакая из них не принимает пути удаления.

- create_general_cleanup() создаёт одну сессию; активная общая очистка блокирует замену.
- collect_general_cleanup(sessionId, taskId) переносит проверенную партию с её
  физическими идентичностями. Повторный вызов не удваивает список; resume отзывает
  обычный preview, но не переносит полномочия на новые файлы.
- finish_general_cleanup(sessionId, cancelled, failedCategories) закрывает снимок и
  возвращает GeneralIndex: session_id, items (object_id/category_id/size_bytes),
  total_bytes, partial, truncated, consumed, expires_at_ms. Ошибочные категории
  исключаются. Отменённый снимок нельзя исполнить. Повторное закрытие отвергается.
- get_general_cleanup_page(sessionId, offset, limit, query, selectedIds) выдаёт
  GeneralPage (items с display_path, total). limit 1–100, query до 256 байт;
  selectedIds=null означает весь список, массив — только указанные ID.
- prepare_general_cleanup(sessionId, selectedIds, acceptPartial) проверяет 1–50 000
  уникальных известных ID и согласие на неполноту. Возвращает confirmation_id,
  selected_count, selected_bytes, expires_at_ms. Подтверждение действует 120 секунд.
- execute_general_cleanup(confirmationId, acknowledged) требует true и однократно
  потребляет согласие. Возвращает GeneralJob: job_id, status, selected_count,
  processed, deleted, skipped, failed, deleted_bytes, operation_ids, message.
- get_general_cleanup_job(jobId) читает прогресс с учётом текущей партии.
- cancel_general_cleanup_job(jobId) останавливает текущую и последующие партии.

Состояния мастера: Running/Completed/Cancelled/Interrupted. Журнал каждой начатой
партии сохраняется прежним исполнителем; операция не повторяется после Unknown
или ошибки журнала. Снимок — до 50 000 объектов/30 минут, объектный ключ — полный
номер тома и 128-битный FILE_ID_INFO. Размеры остаются десятичными строками.
Подробные гарантии и границы — [решение 008](decisions/008-general-cleanup-selection.md).
