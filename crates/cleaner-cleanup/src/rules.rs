//! Каталог версионируемых правил анализа и очистки CleanCore.

use cleaner_core::safety::RiskLevel;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Описание правила очистки конкретной категории.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
}

impl CleanupRule {
    /// Возвращает минимальный возраст файлов в виде Duration.
    pub fn min_age_duration(&self) -> Duration {
        Duration::from_secs(self.min_age_days * 24 * 3600)
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
        Self {
            rules: vec![CleanupRule::user_temp_v1()],
        }
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
        assert!(catalog.get_rule("user-temp-v1").is_some());
        assert!(catalog.get_rule("non-existent-rule").is_none());
        assert_eq!(catalog.all_rules().len(), 1);
    }
}
