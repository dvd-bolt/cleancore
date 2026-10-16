//! Сбор сведений об источнике питания и состоянии аккумулятора (OPT-01).

use cleaner_core::models::PowerStatusSnapshot;

/// Считывает текущее состояние электропитания Windows.
#[cfg(windows)]
pub fn read_power_status() -> Option<PowerStatusSnapshot> {
    use windows_sys::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};

    let mut status = SYSTEM_POWER_STATUS {
        ACLineStatus: 255,
        BatteryFlag: 255,
        BatteryLifePercent: 255,
        SystemStatusFlag: 0,
        BatteryLifeTime: 0,
        BatteryFullLifeTime: 0,
    };

    let ok = unsafe { GetSystemPowerStatus(&mut status) };
    if ok == 0 {
        return None;
    }

    // ACLineStatus: 0 = Offline (Battery), 1 = Online (AC power), 255 = Unknown
    let is_ac_connected = status.ACLineStatus == 1;

    // BatteryFlag: 128 = No system battery, 255 = Unknown
    let is_battery_present = status.BatteryFlag != 128 && status.BatteryLifePercent != 255;
    let is_charging = (status.BatteryFlag & 8) != 0;

    let battery_percent = if status.BatteryLifePercent <= 100 {
        Some(status.BatteryLifePercent as f64)
    } else {
        None
    };

    // SystemStatusFlag: 1 = Battery saver is active
    let power_save_active = status.SystemStatusFlag == 1;

    Some(PowerStatusSnapshot {
        is_battery_present,
        is_ac_connected,
        battery_percent,
        is_charging,
        power_save_active,
    })
}

/// Проверяет, следует ли ограничить параллелизм и нагрузку тяжёлых задач
/// (например, при работе устройства от батареи или в режиме Battery Saver).
pub fn should_throttle_heavy_tasks() -> bool {
    if let Some(status) = read_power_status() {
        !status.is_ac_connected || status.power_save_active
    } else {
        false
    }
}

/// Проверяет, находится ли уровень заряда батареи на критически низком уровне (< 20%) без подключения сети.
pub fn is_critically_low_battery() -> bool {
    if let Some(status) = read_power_status() {
        if !status.is_ac_connected {
            if let Some(pct) = status.battery_percent {
                return pct < 20.0;
            }
        }
    }
    false
}

/// Заглушка для платформ, отличных от Windows.
#[cfg(not(windows))]
pub fn read_power_status() -> Option<PowerStatusSnapshot> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_power_status_structure() {
        let snapshot = PowerStatusSnapshot {
            is_battery_present: true,
            is_ac_connected: true,
            battery_percent: Some(85.0),
            is_charging: false,
            power_save_active: false,
        };
        assert!(snapshot.is_battery_present);
        assert!(snapshot.is_ac_connected);
        assert_eq!(snapshot.battery_percent, Some(85.0));
    }

    #[test]
    #[cfg(windows)]
    fn test_read_real_power_status_runs_without_panics() {
        // Вызов нативного Win32 API не должен падать с паникой
        let _ = read_power_status();
        let _ = should_throttle_heavy_tasks();
        let _ = is_critically_low_battery();
    }
}
