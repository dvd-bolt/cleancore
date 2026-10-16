//! Каталог версионируемых правил анализа и очистки CleanCore.

use cleaner_core::safety::RiskLevel;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Описание правила очистки конкретной категории.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CleanupRule {
    /// Уникальный идентификатор правила.
    pub id: String,
    /// Человекочитаемое название категории.
    pub name: String,
    /// Подробное описание и назначение файлов.
    pub description: String,
    /// Семантическая версия правила (для воспроизводимости аудита).
    pub version: String,
    /// Минимальный возраст файлов в днях (согласно ТЗ для первой категории: не моложе 7 дней).
    pub min_age_days: u64,
    /// Базовый уровень риска категории.
    pub risk_level: RiskLevel,
    #[serde(default)]
    pub extension: Option<String>,
    #[serde(default)]
    pub preview_only: bool,
}

impl CleanupRule {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.id.is_empty()
            || self.id.len() > 64
            || !self
                .id
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        {
            return Err("Некорректный ID правила");
        }
        let version: Vec<_> = self.version.split('.').collect();
        if version.len() != 3
            || version
                .iter()
                .any(|part| part.is_empty() || part.parse::<u32>().is_err())
        {
            return Err("Некорректная версия правила");
        }
        if self.name.is_empty()
            || self.name.len() > 256
            || self.description.is_empty()
            || self.description.len() > 4096
            || !(1..=36500).contains(&self.min_age_days)
            || self.risk_level == RiskLevel::Critical
        {
            return Err("Недопустимое описание или политика правила");
        }
        if self.extension.as_ref().is_some_and(|ext| {
            ext.is_empty() || ext.len() > 16 || !ext.bytes().all(|b| b.is_ascii_alphanumeric())
        }) {
            return Err("Недопустимое расширение");
        }
        Ok(())
    }
    /// Возвращает минимальный возраст файлов в виде Duration.
    pub fn min_age_duration(&self) -> Duration {
        Duration::from_secs(self.min_age_days.saturating_mul(24 * 3600))
    }

    /// Conservative inspection-only subset; no new filesystem root is authorized.
    pub fn user_temp_logs_v1() -> Self {
        let mut rule = Self::user_temp_v1();
        rule.id = "user-temp-logs-v1".into();
        rule.name = "Старые журналы в Temp".into();
        rule.description =
            "Только .log в проверенном пользовательском Temp, не моложе 30 суток. Только анализ."
                .into();
        rule.min_age_days = 30;
        rule.extension = Some("log".into());
        rule.preview_only = true;
        rule
    }
    /// Встроенное правило первой категории: временные файлы текущего пользователя.
    pub fn user_temp_v1() -> Self {
        Self {
            id: "user-temp-v1".to_string(),
            name: "Временные файлы пользователя".to_string(),
            description: "Файлы из временного каталога текущего пользователя (%LOCALAPPDATA%\\Temp), не изменявшиеся более 7 суток. Исключает системные папки, символические ссылки и точки соединения (junctions).".to_string(),
            version: "1.0.0".to_string(),
            min_age_days: 7,
            risk_level: RiskLevel::Low,
            extension: None,
            preview_only: false,
        }
    }
}

/// Каталог доступных правил анализа.
#[derive(Debug, Clone)]
pub struct RuleCatalog {
    rules: Vec<CleanupRule>,
}

impl Default for RuleCatalog {
    fn default() -> Self {
        Self::new()
    }
}

impl RuleCatalog {
    /// Создаёт каталог с базовым набором проверенных правил MVP.
    pub fn new() -> Self {
        let cache = |id: &str, name: &str, description: &str| {
            let mut rule = CleanupRule::user_temp_v1();
            rule.id = id.into();
            rule.name = name.into();
            rule.description = description.into();
            rule
        };
        let mut dumps = cache("crash-dumps-v1", "Старые дампы сбоев", "Только .dmp из пользовательского CrashDumps старше 30 суток. Будет потеряна диагностика сбоев.");
        dumps.min_age_days = 30;
        dumps.extension = Some("dmp".into());
        dumps.risk_level = RiskLevel::Medium;
        let mut system_temp = cache("windows-temp-v1", "Временные файлы Windows", "Только Windows\\Temp от 7 суток. Системная категория: проверьте список; занятые, недоступные и изменившиеся файлы пропускаются.");
        system_temp.risk_level = RiskLevel::Medium;
        let mut wer = cache("windows-wer-v1", "Старые отчёты об ошибках Windows", "Только ReportArchive и ReportQueue в Windows Error Reporting, от 30 суток. Диагностика сбоев будет потеряна.");
        wer.min_age_days = 30;
        wer.risk_level = RiskLevel::Medium;
        let mut rust_cache = cache("project-rust-target-v1", "Rust: кеш выбранного проекта", "Только target выбранного Cargo-проекта в пользовательских папках, файлы от 7 суток. Потребуется пересборка; старые бинарники и артефакты будут потеряны. Исходники не входят в правило.");
        rust_cache.risk_level = RiskLevel::Medium;
        let mut node_cache = cache("project-node-modules-v1", "Node.js: зависимости выбранного проекта", "Только node_modules выбранного package.json-проекта, от 7 суток. Потребуется переустановка зависимостей, возможно из сети; локальные изменения в node_modules будут потеряны. package.json и lock-файлы не удаляются.");
        node_cache.risk_level = RiskLevel::Medium;
        Self {
            rules: vec![
                CleanupRule::user_temp_v1(),
                CleanupRule::user_temp_logs_v1(),
                cache("directx-cache-v1", "Кеш DirectX", "Пользовательский D3DSCache: файлы от 7 суток. Возможна повторная компиляция шейдеров."),
                cache("chrome-cache-v1", "Кеш Google Chrome", "Только Cache, Code Cache и GPUCache в Default/Profile N, от 7 суток. Закройте Chrome. Cookies, пароли, история и данные сайтов исключены."),
                cache("edge-cache-v1", "Кеш Microsoft Edge", "Только Cache, Code Cache и GPUCache в Default/Profile N, от 7 суток. Закройте Edge, включая фоновые процессы. Данные сайтов исключены."),
                cache("firefox-cache-v1", "Кеш Firefox", "Только cache2 в локальных профилях Firefox, от 7 суток. Закройте Firefox. Roaming-профили и личные данные не обрабатываются."),
                dumps,
                system_temp,
                wer,
                rust_cache,
                node_cache,
            ],
        }
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        let mut seen = std::collections::HashSet::new();
        for rule in &self.rules {
            rule.validate()?;
            if !seen.insert(&rule.id) {
                return Err("Повторный ID правила");
            }
        }
        Ok(())
    }
    /// Поиск правила по идентификатору.
    pub fn get_rule(&self, id: &str) -> Option<&CleanupRule> {
        self.rules.iter().find(|r| r.id == id)
    }

    /// Все доступные правила каталога.
    pub fn all_rules(&self) -> &[CleanupRule] {
        &self.rules
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_temp_rule_properties() {
        let rule = CleanupRule::user_temp_v1();
        assert_eq!(rule.id, "user-temp-v1");
        assert_eq!(rule.min_age_days, 7);
        assert_eq!(rule.risk_level, RiskLevel::Low);
        assert_eq!(rule.min_age_duration().as_secs(), 7 * 24 * 3600);
    }

    #[test]
    fn test_rule_catalog_lookup() {
        let catalog = RuleCatalog::new();
        assert!(catalog.validate().is_ok());
        assert!(catalog.get_rule("user-temp-v1").is_some());
        assert!(catalog.get_rule("non-existent-rule").is_none());
        assert_eq!(catalog.all_rules().len(), 11);
        assert!(catalog.get_rule("user-temp-logs-v1").is_some());
    }
}
