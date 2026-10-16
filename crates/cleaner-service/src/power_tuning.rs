//! Модуль управления профилями энергопотребления и производительности (OPT-05).
//!
//! Предоставляет чтение активной схемы питания Windows и безопасную настройку
//! процессорного троттлинга (PROCTHROTTLEMAX / PROCTHROTTLEMIN) без использования командных оболочек (cmd/powershell).

use serde::{Deserialize, Serialize};

/// Сведения об активной схеме электропитания Windows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PowerSchemeInfo {
    pub guid: String,
    pub name: String,
}

/// Процентные значения троттлинга процессора для питания от сети (AC) и батареи (DC).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PowerThrottleValues {
    pub max_ac: u32,
    pub min_ac: u32,
    pub max_dc: u32,
    pub min_dc: u32,
}

/// Готовые пресеты оптимизации энергопотребления.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PowerProfileKind {
    /// Тихий режим: максимум 99% (отключает агрессивный турбобуст, снижая шум кулеров и температуру на 5-15°C).
    Quiet,
    /// Сбалансированный: заводские 100% макс, 5% мин.
    Balanced,
    /// Производительный: 100% макс, 100% мин.
    Performance,
}

/// Полный снимок состояния электропитания для UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PowerProfileStatus {
    pub active_scheme: PowerSchemeInfo,
    pub throttles: PowerThrottleValues,
    pub current_profile: String,
    pub has_rollback_snapshot: bool,
    pub last_snapshot_time: Option<String>,
}

#[cfg(windows)]
fn get_powercfg_path() -> String {
    let system_root = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".to_string());
    format!("{system_root}\\System32\\powercfg.exe")
}

#[cfg(windows)]
fn run_powercfg_direct(args: &[&str]) -> Result<String, String> {
    let exe = get_powercfg_path();
    let output = std::process::Command::new(&exe)
        .args(args)
        .output()
        .map_err(|e| format!("Не удалось запустить powercfg: {e}"))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        let err = String::from_utf8_lossy(&output.stderr);
        Err(format!("powercfg завершился с ошибкой: {}", err.trim()))
    }
}

/// Считывает активную схему питания Windows.
pub fn read_active_scheme() -> Result<PowerSchemeInfo, String> {
    #[cfg(windows)]
    {
        let output = run_powercfg_direct(&["/getactivescheme"])?;
        let guid = parse_guid(&output).ok_or_else(|| {
            "Не удалось определить GUID активной схемы электропитания Windows".to_string()
        })?;
        let name = standard_scheme_name(&guid)
            .map(str::to_string)
            .or_else(|| parse_scheme_name(&output))
            .unwrap_or_else(|| "Схема электропитания Windows".to_string());

        Ok(PowerSchemeInfo { guid, name })
    }
    #[cfg(not(windows))]
    {
        Ok(PowerSchemeInfo {
            guid: "381b4222-f694-41f0-9685-ff5bb260df2e".to_string(),
            name: "Сбалансированная (эмуляция)".to_string(),
        })
    }
}

/// Считывает текущие значения троттлинга процессора (AC/DC).
pub fn read_throttle_values() -> Result<PowerThrottleValues, String> {
    #[cfg(windows)]
    {
        let max_ac_out = run_powercfg_direct(&[
            "/query",
            "SCHEME_CURRENT",
            "SUB_PROCESSOR",
            "PROCTHROTTLEMAX",
        ])
        .unwrap_or_default();
        let min_ac_out = run_powercfg_direct(&[
            "/query",
            "SCHEME_CURRENT",
            "SUB_PROCESSOR",
            "PROCTHROTTLEMIN",
        ])
        .unwrap_or_default();

        let (max_ac, max_dc) = parse_last_two_hex_values(&max_ac_out).unwrap_or((100, 100));
        let (min_ac, min_dc) = parse_last_two_hex_values(&min_ac_out).unwrap_or((5, 5));

        Ok(PowerThrottleValues {
            max_ac: max_ac.clamp(1, 100),
            min_ac: min_ac.clamp(1, 100),
            max_dc: max_dc.clamp(1, 100),
            min_dc: min_dc.clamp(1, 100),
        })
    }
    #[cfg(not(windows))]
    {
        Ok(PowerThrottleValues {
            max_ac: 100,
            min_ac: 5,
            max_dc: 100,
            min_dc: 5,
        })
    }
}

/// Применяет значения троттлинга процессора через powercfg.exe.
pub fn apply_throttle_values(values: &PowerThrottleValues) -> Result<(), String> {
    #[cfg(windows)]
    {
        let max_ac = values.max_ac.clamp(1, 100).to_string();
        let max_dc = values.max_dc.clamp(1, 100).to_string();
        let min_ac = values.min_ac.clamp(1, 100).to_string();
        let min_dc = values.min_dc.clamp(1, 100).to_string();

        run_powercfg_direct(&[
            "/setacvalueindex",
            "SCHEME_CURRENT",
            "SUB_PROCESSOR",
            "PROCTHROTTLEMAX",
            &max_ac,
        ])?;
        run_powercfg_direct(&[
            "/setdcvalueindex",
            "SCHEME_CURRENT",
            "SUB_PROCESSOR",
            "PROCTHROTTLEMAX",
            &max_dc,
        ])?;
        run_powercfg_direct(&[
            "/setacvalueindex",
            "SCHEME_CURRENT",
            "SUB_PROCESSOR",
            "PROCTHROTTLEMIN",
            &min_ac,
        ])?;
        run_powercfg_direct(&[
            "/setdcvalueindex",
            "SCHEME_CURRENT",
            "SUB_PROCESSOR",
            "PROCTHROTTLEMIN",
            &min_dc,
        ])?;
        // Переактивация схемы для мгновенного применения настроек ядром Windows
        run_powercfg_direct(&["/setactive", "SCHEME_CURRENT"])?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = values;
        Ok(())
    }
}

/// Активирует указанную схему электропитания по её GUID.
pub fn set_active_scheme(guid: &str) -> Result<(), String> {
    #[cfg(windows)]
    {
        if !is_guid(guid) {
            return Err("Недопустимый формат GUID схемы питания".to_string());
        }
        run_powercfg_direct(&["/setactive", guid])?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = guid;
        Ok(())
    }
}

/// Определяет имя текущего профиля на основе значений троттлинга.
pub fn determine_profile(throttles: &PowerThrottleValues) -> String {
    if throttles.max_ac <= 99 && throttles.max_dc <= 99 {
        "Quiet".to_string()
    } else if throttles.max_ac == 100 && throttles.min_ac == 100 {
        "Performance".to_string()
    } else if throttles.max_ac == 100 && throttles.min_ac <= 10 {
        "Balanced".to_string()
    } else {
        "Custom".to_string()
    }
}

pub fn standard_scheme_name(guid: &str) -> Option<&'static str> {
    match guid.to_ascii_lowercase().as_str() {
        "381b4222-f694-41f0-9685-ff5bb260df2e" => Some("Сбалансированная"),
        "a1841308-3541-4fab-bc81-f71556f20b4a" => Some("Экономия энергии"),
        "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c" => Some("Высокая производительность"),
        "e9a42b02-d5df-448d-aa00-03f14749eb61" => Some("Максимальная производительность"),
        _ => None,
    }
}

fn parse_guid(text: &str) -> Option<String> {
    text.split(|c: char| !c.is_ascii_hexdigit() && c != '-')
        .find(|c| is_guid(c))
        .map(str::to_string)
}

fn is_guid(candidate: &str) -> bool {
    candidate.len() == 36
        && [8, 13, 18, 23]
            .into_iter()
            .all(|index| candidate.as_bytes()[index] == b'-')
        && candidate
            .bytes()
            .enumerate()
            .all(|(index, byte)| [8, 13, 18, 23].contains(&index) || byte.is_ascii_hexdigit())
}

fn parse_scheme_name(text: &str) -> Option<String> {
    let name = text
        .rsplit_once('(')?
        .1
        .split_once(')')?
        .0
        .trim()
        .to_string();
    (!name.is_empty()).then_some(name)
}

fn parse_last_two_hex_values(text: &str) -> Option<(u32, u32)> {
    let values = text
        .lines()
        .filter_map(|line| line.split_once("0x").map(|(_, value)| value))
        .filter_map(|value| {
            let digits = value
                .chars()
                .take_while(|c| c.is_ascii_hexdigit())
                .collect::<String>();
            u32::from_str_radix(&digits, 16).ok()
        })
        .collect::<Vec<_>>();
    Some((*values.get(values.len().checked_sub(2)?)?, *values.last()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_guid() {
        assert!(is_guid("381b4222-f694-41f0-9685-ff5bb260df2e"));
        assert!(!is_guid("not-a-guid"));
    }

    #[test]
    fn test_determine_profile() {
        let quiet = PowerThrottleValues {
            max_ac: 99,
            min_ac: 5,
            max_dc: 99,
            min_dc: 5,
        };
        assert_eq!(determine_profile(&quiet), "Quiet");

        let balanced = PowerThrottleValues {
            max_ac: 100,
            min_ac: 5,
            max_dc: 100,
            min_dc: 5,
        };
        assert_eq!(determine_profile(&balanced), "Balanced");

        let perf = PowerThrottleValues {
            max_ac: 100,
            min_ac: 100,
            max_dc: 100,
            min_dc: 5,
        };
        assert_eq!(determine_profile(&perf), "Performance");
    }

    #[test]
    fn test_parse_scheme_name() {
        let sample = "Power Scheme GUID: 381b4222-f694-41f0-9685-ff5bb260df2e  (Balanced)";
        assert_eq!(parse_scheme_name(sample), Some("Balanced".to_string()));
    }
}
