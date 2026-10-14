# Этап 4 — результат в отдельной ветке

**Актуально после объединения:** компоненты перенесены в main (`42da025`, `9990b95`), scan-time TrustedCandidate и общий набор Object IDs подключены. Реальная Корзина выключена. [Текущий отчёт](reports/STAGES1_4.md). Дальнейший текст сохранён как историческая инструкция до объединения.

Дата: 2026-10-14. Статус: независимые компоненты подготовлены; реальное исполнение заблокировано.

Ветка: `codex/stage4-execution`.
Worktree: `C:/Users/dvd/.codex/worktrees/stage4-execution/clean`.

Коммиты для объединения поверх готового этапа 3:
- `9241f01` — execution lifecycle, Windows read-only checks, SQLite journal, narrow IPC.
- `cded579` — подтверждение, результаты, журнал UI и документация.

Коммит `3b64cca` — только снимок ранее существовавшей рабочей папки; его не нужно переносить как реализацию этапа 4.

Перед объединением сохранить/зафиксировать текущую работу этапа 3. Разрешать конфликты state/commands/capabilities/Cargo по обеим веткам; не перезаписывать общие файлы целиком.

Инструкция: `C:/Users/dvd/.codex/worktrees/stage4-execution/clean/docs/STAGE4_INTEGRATION.md`.
Отчёт: `C:/Users/dvd/.codex/worktrees/stage4-execution/clean/docs/reports/STAGE4.md`.

Главное условие: сканер должен захватывать TrustedCandidate во время scan. Старые строковые пути не преобразуются в разрешение удалить файл. Production DisabledRecycle выключен; включение только после доказанной привязки операции Корзины и общей Windows приёмки. Окончательное удаление отсутствует.

Проверено: 45 Rust-тестов, 14 frontend-тестов, fmt/clippy, сборки, документация. Symlink-test ignored из-за недоступной привилегии; junction/ACL/live recycle/native GUI не приняты.
