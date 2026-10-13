# Открытые репозитории и заимствование

## Статус исследования

Первичный обзор выполнен 2026-10-13: README, лицензии/манифесты и отдельные исходники. Это предварительный отбор, не завершённый аудит безопасности или транзитивных лицензий. Ничего из этих проектов пока не подключено и не скопировано.

| Источник | Полезная часть | Предварительное решение | Лицензия просмотренного компонента |
| --- | --- | --- | --- |
| [Tauri](https://github.com/tauri-apps/tauri) | Оболочка, IPC, разрешения, упаковка | Зависимость этапа 1 | Проверить выбранные crates и ревизию |
| [sysinfo](https://github.com/GuillaumeGomez/sysinfo) | CPU/RAM/процессы/диски/сеть | Зависимость через адаптер этапа 2 | MIT |
| [windows-rs](https://github.com/microsoft/windows-rs) | Windows API и COM | Необходимые bindings системного слоя | MIT / Apache-2.0 |
| [rusqlite](https://github.com/rusqlite/rusqlite) | SQLite и транзакции | Зависимость этапов 2–4 | MIT |
| [Radix](https://github.com/radix-ui/primitives) | Диалоги и доступные примитивы | Отдельные UI-компоненты | MIT |
| [TanStack Table](https://github.com/TanStack/table) | Сортировка, фильтры, выбор | Таблицы процессов и preview | MIT |
| [Czkawka](https://github.com/qarmin/czkawka) | Поиск дубликатов и обход | Узкая адаптация этапа 5 | Ядро MIT; другие части/ресурсы отдельно |
| [dua-cli](https://github.com/Byron/dua-cli) | Анализ каталогов | Архитектурный образец, небольшие функции | MIT |
| [BCUninstaller](https://github.com/BCUninstaller/Bulk-Crap-Uninstaller) | Чтение установленных приложений | Образец для собственной Rust-реализации | Основной проект Apache-2.0; компоненты отдельно |
| [BleachBit](https://github.com/bleachbit/bleachbit) | Категории и последствия очистки | Изучение поведения без переноса GPL-кода/определений | GPL-3.0-or-later для кода и cleaners |
| [tauri-action](https://github.com/tauri-apps/tauri-action) | Desktop CI и релизы | GitHub Action с закреплённой ревизией | Проверить перед подключением |
| [llama.cpp](https://github.com/ggml-org/llama.cpp) | Локальный inference | Изолированный адаптер этапа 8 | MIT; лицензия весов проверяется отдельно |

## Просмотренные исходники

| Проект / ревизия | Часть | Что наблюдалось |
| --- | --- | --- |
| Czkawka `eb8b91dbb4d25dff416202674ee0de84ee4b5e5c` | [duplicate/core.rs](https://github.com/qarmin/czkawka/blob/eb8b91dbb4d25dff416202674ee0de84ee4b5e5c/czkawka_core/src/tools/duplicate/core.rs) | Группировка по размеру, prehash/full hash, фильтрация hard links, отмена и прогресс |
| Та же ревизия | [dir_traversal.rs](https://github.com/qarmin/czkawka/blob/eb8b91dbb4d25dff416202674ee0de84ee4b5e5c/czkawka_core/src/common/dir_traversal.rs) | Обход с фильтрами, предупреждениями, отменой и группировкой |
| dua-cli `2eeb1e88e7fbc2423b1b1d363be926731c94f82e` | [traverse.rs](https://github.com/Byron/dua-cli/blob/2eeb1e88e7fbc2423b1b1d363be926731c94f82e/src/traverse.rs) | Организация файлового обхода для анализа объёма |
| BCUninstaller `30da609384c98ba6e35c6530129541ea4ff3970a` | [RegistryFactory.cs](https://github.com/BCUninstaller/Bulk-Crap-Uninstaller/blob/30da609384c98ba6e35c6530129541ea4ff3970a/source/UninstallTools/Factory/RegistryFactory.cs) | Сбор установленных приложений из записей реестра |

В [Cargo.toml Czkawka Core](https://github.com/qarmin/czkawka/blob/master/czkawka_core/Cargo.toml) есть зависимости для изображений, аудио и архивов. Не подключать всё ядро ради дубликатов. Просмотренная default branch может отличаться от стабильного релиза: выбранный release API проверять отдельно.

## Проверенные источники лицензий

- [sysinfo LICENSE](https://github.com/GuillaumeGomez/sysinfo/blob/main/LICENSE).
- [windows-rs license-mit](https://github.com/microsoft/windows-rs/blob/master/license-mit).
- [rusqlite Cargo.toml](https://github.com/rusqlite/rusqlite/blob/master/Cargo.toml).
- [Czkawka Core Cargo.toml](https://github.com/qarmin/czkawka/blob/master/czkawka_core/Cargo.toml).
- [dua-cli Cargo.toml](https://github.com/Byron/dua-cli/blob/main/Cargo.toml).
- [BCUninstaller Licence.txt](https://github.com/BCUninstaller/Bulk-Crap-Uninstaller/blob/master/Licence.txt).
- [TanStack LICENSE](https://github.com/TanStack/table/blob/main/LICENSE).
- [Radix LICENSE](https://github.com/radix-ui/primitives/blob/main/LICENSE).
- [llama.cpp LICENSE](https://github.com/ggml-org/llama.cpp/blob/master/LICENSE).

## Процесс допуска

1. Выбрать конкретную версию/commit SHA и необходимые файлы либо package.
2. Проверить собственную лицензию файла, ресурсы и транзитивные зависимости.
3. Проверить поддержку целевой Windows, build requirements и уязвимости.
4. Сначала использовать зависимость; vendoring оправдывать размером/границами, не удобством копирования.
5. Сохранить copyright, LICENSE и требуемый NOTICE. Отметить изменённые файлы.
6. Изолировать адаптацию, покрыть собственными тестами и описать ограничения.
7. Заполнить запись реестра ниже; обновление требует повторной проверки.

MIT-лицензия собственного кода не переписывает лицензии заимствованных частей. В выбранной политике GPL-код и cleaner definitions BleachBit в MIT-реализацию не переносить. Не копировать чужие иконки/fixtures по лицензии ядра.

## Реестр фактического включения

**Пока пуст.** Наличие проекта в обзоре не означает включение в CleanCore.

Для каждой включённой части записать: источник, ревизию/package version, список файлов или package, лицензию, расположение уведомлений, внесённые изменения, использующий модуль, проверки и дату аудита.

Библиотеки очистки и Корзины не принимаются автоматически: проверять гарантии против подмены пути согласно [THREAT_MODEL](THREAT_MODEL.md).
