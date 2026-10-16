# Открытые репозитории и заимствование

## Статус исследования

Первичный обзор выполнен 2026-10-13: README, лицензии/манифесты и отдельные исходники. Это предварительный отбор, не завершённый аудит безопасности или транзитивных лицензий. Это состояние первичного обзора; фактически подключённые компоненты перечислены в реестре ниже.

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

| Источник / пакет | Версия | Лицензия | Использующий модуль | Внесённые изменения / адаптация | Дата аудита и проверки |
| --- | --- | --- | --- | --- | --- |
| `sysinfo` (crates.io) | `0.33.1` | MIT | `cleaner-system` (модули 2.1–2.5) | Изолирован в адаптере `SystemCollector`. Первый замер CPU трактуется как `FieldValue::NotMeasured`. CPU процессов нормируется делением на число логических ядер (0–100%). Недоступные пути защищённых процессов ОС помечаются `FieldValue::AccessDenied`. Счётчики сетевых интерфейсов защищены от сброса через `saturating_sub`. | 2026-10-14, cargo test/clippy |
| `rusqlite` (crates.io) | `0.32.1` (`bundled`) | MIT | `cleaner-storage` (модуль 2.6) | Изолирован в `StorageManager`. SQLite используется для хранения локальных настроек в `%LOCALAPPDATA%\CleanCore\cleancore.db` (с fallback на in-memory). Валидация интервалов (500..60000 мс). | 2026-10-14, cargo test/clippy |

Библиотеки очистки и Корзины не принимаются автоматически: проверять гарантии против подмены пути согласно [THREAT_MODEL](THREAT_MODEL.md).

## Компоненты этапа 4 (2026-10-14)

Прямое использование библиотек без копирования исходников: uuid 1.28.0 — Apache-2.0 OR MIT; windows-sys 0.61.2 — MIT OR Apache-2.0. Версии/checksums закреплены Cargo.lock, лицензии прочитаны из Cargo metadata для x86_64-pc-windows-msvc. Обе зависимости ранее присутствовали транзитивно. Проверки Windows API и ограничения — [решение 002](decisions/002-parallel-stage4.md), [отчёт](reports/STAGE4.md). Это не является полным аудитом транзитивных лицензий или уязвимостей будущего релиза.

## Дополнения аудита этапов 1–7 (2026-10-14)

- @radix-ui/react-dialog 1.1.15: MIT, copyright WorkOS; исходный LICENSE прочитан
  в установленном пакете, адаптер AccessibleDialog собственный, исходники не копировались.
- Vitest 4.1.11: обновление инструмента тестирования после advisory; код пакета не менялся.
- postcss-selector-parser 7.1.6: фиксирован через pnpm-workspace.yaml overrides; проверены сборка и тесты.
- windows-sys 0.61.2: MIT/Apache-2.0, существующая зависимость дополнена bindings системного
  каталога и common dialog; реализации Windows не копировались.
- tauri-action 0.6.2: commit 84b9d35b5fc46c1e45415bdb6144030364f7ebc5,
  MIT по package.json этой ревизии; подключён только для сборки в workflow, без публикации.
- Остальные Actions закреплены на разрешённые commit SHA, а не объекты аннотированных тегов.

scripts/prepare_notices.py формирует inventory и сохраняет оригинальные license/notice
для locked Windows Rust/frontend зависимостей. 20 пакетов без корневого текста лицензии
требуют отдельной проверки до распространения; это не завершённый транзитивный аудит.
[Полный результат и advisories](reports/STAGES1_7.md).

## Runtime этапа 8 (2026-10-14)

llama.cpp b11429, commit d81235049384534c167caea52b85a694f6103d14.
Официальный архив llama-b11429-bin-win-cpu-x64.zip:
SHA-256 1283323272b04cd07905816a597a0da810918102de958f4ff6f7bbaa70ed2efe.
Использованы неизменённые llama-completion.exe/implementation, llama/common/ggml,
CPU backend DLL и libomp; полный перечень и hashes — runtime-manifest.json.
llama-cli/server и утилиты преобразования не поставляются. ggml-rpc.dll — связанная
зависимость официальной сборки; RPC endpoint и параметры CleanCore не использует.

MIT LICENSE прочитан на закреплённой ревизии, сохраняется как LICENSE-llama.cpp.
LICENSE-LLVM-OpenMP из архива сохранён рядом с libomp.dll; LLVM Apache-2.0 with
LLVM Exceptions/MIT notices не заменяются MIT собственного проекта. Исходники не
копировались, бинарники не изменялись. Provisioning проверяет archive/file hashes;
перед каждым запуском повторно проверяется каждый поставляемый файл. Проверены
--version, фиксированные аргументы и отказ повреждённой модели. Полноценный инференс
и независимый аудит происхождения официальных бинарников не проводились.

Веса в GGUF не входят в поставку и инвентаризацию приложения: их лицензия
проверяется отдельно при ручном выборе. [Решение](decisions/004-local-assistant.md).

## Исправления 2026-10-15

Использованы уже закреплённые windows-sys 0.61.2 и winreg 0.55.0, MIT/Apache-2.0
и MIT соответственно. Исходники не копировались; SetFileInformationByHandle вызывается
собственным адаптером. Проба transactions winreg выявила недоступный TxR в среде;
feature удалён, в поставке не применяется. StartupApproved реализован ограниченно
для известных форматов; это не публичный документированный контракт Windows.
Новая иконка — собственный SVG, конвертация существующим Tauri CLI.

## NTFS MFT 2026-10-15

Собственные parser/volume/index; сторонние исходники не копировались. Имеющийся windows-sys 0.59.0, MIT/Apache-2.0, дополнен features Win32_System_IO/Ioctl. Данные структур сверены по Microsoft Learn; ссылки и ограничения в решении 006. Новых бинарников или зависимостей нет.

## Treemap 2026-10-15

Собственная binary-partition geometry и Canvas renderer; код/графика WizTree и внешние библиотеки карт не копировались. windows-sys 0.61.2 (уже закреплён, MIT/Apache-2.0) получил Shell/Common/Com features; используются SHParseDisplayName и SHOpenFolderAndSelectItems по Microsoft Learn. [SHOpenFolderAndSelectItems](https://learn.microsoft.com/en-us/windows/win32/api/shlobj_core/nf-shlobj_core-shopenfolderandselectitems), сверено 2026-10-15.


## Расширение очистки 0.0.4

Код и определения BleachBit/Winapp2/FluentCleaner не переносились. Каталог, курсор, policy executor и UI — собственная реализация. Прямое использование уже locked sha2 0.10 в cleaner-execution: повторный SHA-256 под удерживаемыми handles, без переноса исходников.

Для Cargo-манифеста cleaner-cleanup напрямую использует уже присутствующий toml 1.1.8+spec-1.1.0, точное требование =1.1.8; registry checksum 20489e00e4d8741d6be680764cc12e270655e375a20d1011e844a9c3379e678d. Прочитаны Cargo.toml и LICENSE-MIT установленного пакета: MIT OR Apache-2.0, copyright Individual contributors. Собственные изменения в toml не вносились; парсер ограничен манифестом до 1 MiB. LICENSE должен сохраняться обычным inventory перед распространением. Это не полный транзитивный аудит.

windows-sys 0.61.2 расширен Toolhelp bindings; API проверено по Microsoft документации. Никаких новых драйверов, исполняемых helper или сетевых компонентов не добавлено.
