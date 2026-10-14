//! Модуль 6.3: Статус антивирусной защиты и Windows Defender.

use crate::types::{AntivirusProvider, AntivirusStatus, ProtectionState, ServiceError};

#[cfg(windows)]
use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ};
#[cfg(windows)]
use winreg::RegKey;

/// Декодирует битовую маску состояния провайдера безопасности Windows Security Center.
///
/// Возвращает кортеж `(real_time_protection, definitions_up_to_date)`.
/// - Бит 0x10 или 0x1000: активность защиты в реальном времени (RTP).
/// - Бит 0x100: актуальность баз сигнатур.
pub fn decode_provider_state(state: u32) -> (bool, bool) {
    if state == 0 {
        return (false, false);
    }
    let rtp = (state & 0x10) != 0 || (state & 0x1000) != 0;
    let signatures_up_to_date =
        (state & 0x100) != 0 || ((state & 0x10) != 0 && (state & 0x08) == 0);
    (rtp, signatures_up_to_date)
}

/// Проверяет состояние службы Windows Defender (WinDefend) через Windows Service Control Manager.
#[cfg(windows)]
fn query_windefend_service_status() -> Option<bool> {
    use windows_sys::Win32::System::Services::{
        CloseServiceHandle, OpenSCManagerW, OpenServiceW, QueryServiceStatus, SC_MANAGER_CONNECT,
        SERVICE_QUERY_STATUS, SERVICE_RUNNING, SERVICE_STATUS,
    };

    unsafe {
        let scm = OpenSCManagerW(std::ptr::null(), std::ptr::null(), SC_MANAGER_CONNECT);
        if scm.is_null() {
            return None;
        }

        let service_name: Vec<u16> = "WinDefend\0".encode_utf16().collect();
        let service = OpenServiceW(scm, service_name.as_ptr(), SERVICE_QUERY_STATUS);
        if service.is_null() {
            CloseServiceHandle(scm);
            return None;
        }

        let mut status: SERVICE_STATUS = std::mem::zeroed();
        let success = QueryServiceStatus(service, &mut status);
        CloseServiceHandle(service);
        CloseServiceHandle(scm);

        if success != 0 {
            Some(status.dwCurrentState == SERVICE_RUNNING)
        } else {
            None
        }
    }
}

/// Получает текущий статус антивирусной защиты системы.
///
/// При отсутствии прав или ограничении доступа политиками возвращает
/// `ProtectionState::LimitedAccess` без выброса ошибок и без запроса UAC elevation.
pub fn get_antivirus_status() -> Result<AntivirusStatus, ServiceError> {
    #[cfg(windows)]
    {
        get_antivirus_status_windows()
    }
    #[cfg(not(windows))]
    {
        Ok(AntivirusStatus {
            state: ProtectionState::LimitedAccess,
            providers: Vec::new(),
            defender_service_running: None,
            message: "Мониторинг антивирусной защиты доступен только на Windows".to_string(),
        })
    }
}

#[cfg(windows)]
fn get_antivirus_status_windows() -> Result<AntivirusStatus, ServiceError> {
    let mut providers = Vec::new();

    // 1. Опрос Security Center Provider\Av в реестре
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    if let Ok(av_key) =
        hklm.open_subkey_with_flags(r"SOFTWARE\Microsoft\Security Center\Provider\Av", KEY_READ)
    {
        for subkey_res in av_key.enum_keys().flatten() {
            if let Ok(provider_key) = av_key.open_subkey_with_flags(&subkey_res, KEY_READ) {
                let name: String = provider_key
                    .get_value("DISPLAYNAME")
                    .or_else(|_| provider_key.get_value("DisplayName"))
                    .unwrap_or_else(|_| subkey_res.clone());

                let product_exe: Option<String> = provider_key
                    .get_value("PRODUCTEXE")
                    .or_else(|_| provider_key.get_value("ProductExe"))
                    .ok();

                let reporting_exe: Option<String> = provider_key
                    .get_value("REPORTINGEXE")
                    .or_else(|_| provider_key.get_value("ReportingExe"))
                    .ok();

                let state_code: Option<u32> = provider_key
                    .get_value("STATE")
                    .or_else(|_| provider_key.get_value("State"))
                    .ok();

                let (rtp, sigs) = match state_code {
                    Some(code) => decode_provider_state(code),
                    None => (false, false),
                };

                providers.push(AntivirusProvider {
                    name,
                    product_exe,
                    reporting_exe,
                    state_code,
                    real_time_protection: rtp,
                    definitions_up_to_date: sigs,
                });
            }
        }
    }

    // 2. Опрос службы WinDefend через SCM
    let defender_service_running = query_windefend_service_status();

    // 3. Вычисление итогового состояния защиты
    let (state, message) = if !providers.is_empty() {
        let any_rtp = providers.iter().any(|p| p.real_time_protection);
        let all_updated = providers.iter().all(|p| p.definitions_up_to_date);

        if any_rtp && all_updated {
            (
                ProtectionState::Protected,
                "Антивирусная защита активна, базы сигнатур актуальны".to_string(),
            )
        } else if any_rtp {
            (
                ProtectionState::Warning,
                "Защита активна, но рекомендуется обновить базы сигнатур".to_string(),
            )
        } else {
            (
                ProtectionState::Disabled,
                "Защита в реальном времени отключена!".to_string(),
            )
        }
    } else if let Some(is_running) = defender_service_running {
        if is_running {
            (
                ProtectionState::Protected,
                "Служба Windows Defender активна и работает в штатном режиме".to_string(),
            )
        } else {
            (
                ProtectionState::Disabled,
                "Служба Windows Defender остановлена!".to_string(),
            )
        }
    } else {
        (
            ProtectionState::LimitedAccess,
            "Доступ к статусу защиты ограничен политиками безопасности Windows".to_string(),
        )
    };

    Ok(AntivirusStatus {
        state,
        providers,
        defender_service_running,
        message,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_provider_state_combinations() {
        // Нулевое состояние
        let (rtp0, sigs0) = decode_provider_state(0);
        assert!(!rtp0);
        assert!(!sigs0);

        // Бит 0x10 (RTP активен)
        let (rtp1, sigs1) = decode_provider_state(0x10);
        assert!(rtp1);
        assert!(sigs1);

        // Бит 0x1000 (RTP альтернативный)
        let (rtp2, sigs2) = decode_provider_state(0x1000);
        assert!(rtp2);
        assert!(!sigs2);

        // Типичный рабочий статус Defender (0x61100 = 397568)
        let (rtp_def, sigs_def) = decode_provider_state(0x61100);
        assert!(rtp_def);
        assert!(sigs_def);

        // Только базы обновлены (0x100), без RTP
        let (rtp_only_sigs, sigs_only) = decode_provider_state(0x100);
        assert!(!rtp_only_sigs);
        assert!(sigs_only);
    }

    #[test]
    fn test_get_antivirus_status_does_not_panic() {
        let status_res = get_antivirus_status();
        assert!(status_res.is_ok());
        let status = status_res.unwrap();
        assert!(!status.message.is_empty());
    }
}
