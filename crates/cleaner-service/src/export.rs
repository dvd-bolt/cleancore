//! Модуль 6.6: Безопасный экспорт отчётов в CSV с защитой от формульных инъекций (CWE-1236).

use crate::types::{
    CsvExportConfig, CsvExportSection, DiagnosticsSummary, InstalledProgram, ServiceError,
    StartupItem,
};

/// UTF-8 Byte Order Mark (BOM) для корректного распознавания кодировки русскоязычным Excel.
pub const UTF8_BOM: &str = "\u{FEFF}";

/// Символы, запускающие формулы или макросы в табличных процессорах (Excel, Calc).
const FORMULA_PREFIXES: &[char] = &['=', '+', '-', '@', '\t', '\r'];

/// Нейтрализует уязвимость CSV Formula Injection (CWE-1236).
///
/// Если строка (после возможного ведущего пробела) начинается со служебных символов
/// `=`, `+`, `-`, `@`, `\t` или `\r`, перед строкой добавляется апостроф `'`.
pub fn sanitize_csv_cell(value: &str) -> String {
    // Проверяем прямое начало со служебных символов (включая \t и \r)
    if FORMULA_PREFIXES.iter().any(|&p| value.starts_with(p)) {
        return format!("'{value}");
    }

    // Также обрабатываем случаи пробелов перед формулой (например, "   =1+1")
    let trimmed_spaces = value.trim_start_matches(' ');
    if FORMULA_PREFIXES
        .iter()
        .any(|&p| trimmed_spaces.starts_with(p))
    {
        return format!("'{value}");
    }

    value.to_string()
}

/// Форматирует значение ячейки по стандарту RFC 4180.
///
/// 1. Применяет нейтрализацию формульных инъекций.
/// 2. Если значение содержит запятую, точку с запятой, кавычки, переводы строк или начинается с апострофа,
///    оборачивает строку в двойные кавычки, удваивая внутренние кавычки (`""`).
pub fn format_csv_cell(value: &str) -> String {
    let sanitized = sanitize_csv_cell(value);

    let needs_quotes = sanitized.contains(',')
        || sanitized.contains(';')
        || sanitized.contains('"')
        || sanitized.contains('\n')
        || sanitized.contains('\r')
        || sanitized.starts_with('\'');

    if needs_quotes {
        let escaped = sanitized.replace('"', "\"\"");
        format!("\"{escaped}\"")
    } else {
        sanitized
    }
}

/// Собирает строку CSV-файла из списка ячеек с разделителем запятая и завершением CRLF.
pub fn format_csv_row(cells: &[&str]) -> String {
    let formatted: Vec<String> = cells.iter().map(|c| format_csv_cell(c)).collect();
    format!("{}\r\n", formatted.join(","))
}

/// Генерирует комплексный CSV-отчёт на основе переданной конфигурации.
pub fn generate_csv_report(
    config: &CsvExportConfig,
    programs: &[InstalledProgram],
    startup: &[StartupItem],
    diagnostics: &DiagnosticsSummary,
) -> Result<String, ServiceError> {
    let mut out = String::new();
    out.push_str(UTF8_BOM);

    let include_all =
        config.sections.contains(&CsvExportSection::All) || config.sections.is_empty();

    // 1. Метаданные отчёта
    out.push_str(&format_csv_row(&["ОТЧЁТ ЦЕНТРА ОБСЛУЖИВАНИЯ CLEANCORE"]));
    out.push_str(&format_csv_row(&[
        "Дата формирования (UTC)",
        &diagnostics.timestamp_utc,
    ]));
    out.push_str("\r\n");

    // 2. Секция диагностики и здоровья системы
    if include_all || config.sections.contains(&CsvExportSection::Diagnostics) {
        out.push_str(&format_csv_row(&[
            "[РАЗДЕЛ: ДИАГНОСТИКА И ЗДОРОВЬЕ СИСТЕМЫ]",
        ]));
        out.push_str(&format_csv_row(&[
            "Общее состояние здоровья",
            match diagnostics.health {
                crate::types::HealthStatus::Healthy => "Штатное (Healthy)",
                crate::types::HealthStatus::Warning => "Внимание (Warning)",
                crate::types::HealthStatus::Critical => "Критическое (Critical)",
                crate::types::HealthStatus::Unknown => "Неизвестно (Unknown)",
            },
        ]));

        if let Some(cpu) = diagnostics.cpu_usage_percent {
            out.push_str(&format_csv_row(&[
                "Загрузка процессора (CPU)",
                &format!("{:.1}%", cpu),
            ]));
        }
        if let Some(mem) = diagnostics.memory_usage_percent {
            out.push_str(&format_csv_row(&[
                "Загрузка памяти (RAM)",
                &format!("{}%", mem),
            ]));
        }
        if let Some(free) = diagnostics.disk_free_bytes {
            let free_gb = free as f64 / (1024.0 * 1024.0 * 1024.0);
            out.push_str(&format_csv_row(&[
                &format!(
                    "Свободное место на системном диске ({})",
                    diagnostics.system_drive
                ),
                &format!("{:.2} ГБ", free_gb),
            ]));
        }

        out.push_str(&format_csv_row(&["Предупреждения и алерты"]));
        out.push_str(&format_csv_row(&[
            "Уровень",
            "Категория",
            "Заголовок",
            "Сообщение",
            "Рекомендация",
        ]));

        if diagnostics.alerts.is_empty() {
            out.push_str(&format_csv_row(&[
                "Норма",
                "Общее",
                "Замечаний нет",
                "Система функционирует нормально",
                "Регулярно проводите профилактическую очистку",
            ]));
        } else {
            for alert in &diagnostics.alerts {
                let lvl = match alert.level {
                    crate::types::HealthStatus::Healthy => "Норма",
                    crate::types::HealthStatus::Warning => "Предупреждение",
                    crate::types::HealthStatus::Critical => "Критический",
                    crate::types::HealthStatus::Unknown => "Неизвестно",
                };
                let rec = alert.recommendation.as_deref().unwrap_or("—");
                out.push_str(&format_csv_row(&[
                    lvl,
                    &alert.category,
                    &alert.title,
                    &alert.message,
                    rec,
                ]));
            }
        }
        out.push_str("\r\n");
    }

    // 3. Секция установленных программ
    if include_all || config.sections.contains(&CsvExportSection::Programs) {
        out.push_str(&format_csv_row(&["[РАЗДЕЛ: УСТАНОВЛЕННЫЕ ПРОГРАММЫ]"]));
        out.push_str(&format_csv_row(&[
            "Название программы",
            "Издатель",
            "Версия",
            "Дата установки",
            "Размер (байт)",
            "Ветка реестра",
            "Архитектура",
            "Строка деинсталляции",
        ]));

        for prog in programs {
            if !config.include_system_components && prog.is_system_component {
                continue;
            }
            let publ = prog.publisher.as_deref().unwrap_or("—");
            let ver = prog.display_version.as_deref().unwrap_or("—");
            let date = prog.install_date.as_deref().unwrap_or("—");
            let size = prog
                .estimated_size_bytes
                .map(|s| s.to_string())
                .unwrap_or_else(|| "—".to_string());
            let arch = if prog.is_wow64 {
                "32-bit (WOW64)"
            } else {
                "64-bit"
            };
            let unins = prog
                .uninstall_string
                .as_deref()
                .or(prog.quiet_uninstall_string.as_deref())
                .unwrap_or("—");

            out.push_str(&format_csv_row(&[
                &prog.display_name,
                publ,
                ver,
                date,
                &size,
                &prog.registry_hive,
                arch,
                unins,
            ]));
        }
        out.push_str("\r\n");
    }

    // 4. Секция автозагрузки
    if include_all || config.sections.contains(&CsvExportSection::Startup) {
        out.push_str(&format_csv_row(&["[РАЗДЕЛ: ЭЛЕМЕНТЫ АВТОЗАГРУЗКИ]"]));
        out.push_str(&format_csv_row(&[
            "Имя элемента",
            "Команда запуска",
            "Источник",
            "Расположение",
            "Состояние",
            "Файл на диске",
        ]));

        for item in startup {
            let src = match item.source {
                crate::types::StartupSource::RegistryRun => "Реестр (Run)",
                crate::types::StartupSource::RegistryRunOnce => "Реестр (RunOnce)",
                crate::types::StartupSource::StartupFolderUser => "Папка автозапуска (User)",
                crate::types::StartupSource::StartupFolderCommon => "Папка автозапуска (Common)",
            };
            let status = if item.enabled {
                "Включён"
            } else {
                "Отключён"
            };
            let exists = if item.target_exists {
                "Существует"
            } else {
                "Отсутствует"
            };

            out.push_str(&format_csv_row(&[
                &item.name,
                &item.command,
                src,
                &item.location,
                status,
                exists,
            ]));
        }
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{HealthStatus, InstalledProgram, StartupItem, StartupSource};

    #[test]
    fn test_csv_sanitize_formula_prefixes() {
        assert_eq!(
            sanitize_csv_cell("=cmd|' /C calc'!A0"),
            "'=cmd|' /C calc'!A0"
        );
        assert_eq!(sanitize_csv_cell("+12345"), "'+12345");
        assert_eq!(sanitize_csv_cell("-5+10"), "'-5+10");
        assert_eq!(sanitize_csv_cell("@SUM(A1:A10)"), "'@SUM(A1:A10)");
        assert_eq!(sanitize_csv_cell("\tmalicious_tab"), "'\tmalicious_tab");
        assert_eq!(sanitize_csv_cell("\rmalicious_cr"), "'\rmalicious_cr");
        assert_eq!(
            sanitize_csv_cell("   =TRIMMED_FORMULA"),
            "'   =TRIMMED_FORMULA"
        );
        assert_eq!(
            sanitize_csv_cell("Normal Program Name"),
            "Normal Program Name"
        );
        assert_eq!(sanitize_csv_cell("CleanCore v0.3.0"), "CleanCore v0.3.0");
    }

    #[test]
    fn test_rfc4180_escaping_and_quoting() {
        // Запятая
        assert_eq!(format_csv_cell("Foo, Bar"), "\"Foo, Bar\"");
        // Точка с запятой
        assert_eq!(format_csv_cell("Foo; Bar"), "\"Foo; Bar\"");
        // Двойные кавычки
        assert_eq!(
            format_csv_cell("Foo \"Bar\" Baz"),
            "\"Foo \"\"Bar\"\" Baz\""
        );
        // Перевод строки
        assert_eq!(format_csv_cell("Line 1\nLine 2"), "\"Line 1\nLine 2\"");
        // Формула с запятой: экранируется апострофом, затем обрамляется кавычками
        assert_eq!(format_csv_cell("=SUM(A1, B1)"), "\"\'=SUM(A1, B1)\"");
        // Простая строка без спецсимволов не обрамляется кавычками
        assert_eq!(format_csv_cell("CleanCore"), "CleanCore");
    }

    #[test]
    fn test_utf8_bom_presence_in_report() {
        let config = CsvExportConfig {
            sections: vec![CsvExportSection::All],
            include_system_components: false,
            target_path: None,
        };
        let diagnostics = DiagnosticsSummary {
            health: HealthStatus::Healthy,
            cpu_usage_percent: Some(25.0),
            memory_usage_percent: Some(45),
            disk_free_bytes: Some(100 * 1024 * 1024 * 1024),
            disk_total_bytes: Some(512 * 1024 * 1024 * 1024),
            disk_usage_percent: Some(80),
            system_drive: "C:".to_string(),
            alerts: Vec::new(),
            recommendations: vec!["Всё в порядке".to_string()],
            timestamp_utc: "2026-10-14T12:00:00Z".to_string(),
        };

        let report =
            generate_csv_report(&config, &[], &[], &diagnostics).expect("Generate CSV report");

        assert!(report.starts_with(UTF8_BOM));
        assert!(report.contains("[РАЗДЕЛ: ДИАГНОСТИКА И ЗДОРОВЬЕ СИСТЕМЫ]"));
        assert!(report.contains("[РАЗДЕЛ: УСТАНОВЛЕННЫЕ ПРОГРАММЫ]"));
        assert!(report.contains("[РАЗДЕЛ: ЭЛЕМЕНТЫ АВТОЗАГРУЗКИ]"));
    }

    #[test]
    fn test_report_includes_programs_and_startup() {
        let config = CsvExportConfig {
            sections: vec![CsvExportSection::Programs, CsvExportSection::Startup],
            include_system_components: false,
            target_path: None,
        };
        let diagnostics = DiagnosticsSummary {
            health: HealthStatus::Healthy,
            cpu_usage_percent: None,
            memory_usage_percent: None,
            disk_free_bytes: None,
            disk_total_bytes: None,
            disk_usage_percent: None,
            system_drive: "C:".to_string(),
            alerts: Vec::new(),
            recommendations: Vec::new(),
            timestamp_utc: "2026-10-14T12:00:00Z".to_string(),
        };

        let programs = vec![InstalledProgram {
            id: "HKLM_64_TestApp".to_string(),
            display_name: "=InjectedApp".to_string(),
            publisher: Some("Test Publisher, Inc.".to_string()),
            display_version: Some("1.2.3".to_string()),
            install_date: Some("20261014".to_string()),
            estimated_size_bytes: Some(10485760),
            uninstall_string: Some("C:\\App\\uninst.exe".to_string()),
            quiet_uninstall_string: None,
            install_location: Some("C:\\App".to_string()),
            registry_hive: "HKLM".to_string(),
            is_wow64: false,
            is_system_component: false,
        }];

        let startup = vec![StartupItem {
            id: "HKCU_Run_TestStart".to_string(),
            name: "@EvilStartup".to_string(),
            command: "C:\\App\\app.exe --tray".to_string(),
            source: StartupSource::RegistryRun,
            location: "HKCU\\Run".to_string(),
            enabled: true,
            target_exists: false,
        }];

        let report = generate_csv_report(&config, &programs, &startup, &diagnostics)
            .expect("Generate report");

        // Проверяем экранирование инъекций формул в ячейках
        assert!(report.contains("\"'=InjectedApp\""));
        assert!(report.contains("\"Test Publisher, Inc.\""));
        assert!(report.contains("\"'@EvilStartup\""));
        // Секция диагностики не должна присутствовать, так как не выбрана
        assert!(!report.contains("[РАЗДЕЛ: ДИАГНОСТИКА И ЗДОРОВЬЕ СИСТЕМЫ]"));
    }
}
