//! Empirical Stress Tests for CleanCore Stage 6 Quality Gate (Challenger 2).
//!
//! Covers:
//! 1. Defender Status & Non-Elevation Guarantee (bitmask states, non-elevation verification).
//! 2. Diagnostics Thresholds Boundary Testing (exact boundaries for disk, RAM, CPU, startup count).
//! 3. Startup Approved Edge Cases (Task Manager StartupApproved byte slice decoding).
//! 4. Programs Registry Metadata Robustness (DisplayName, InstallDate, corrupt EstimatedSize, unquoted paths, malicious shells).

use cleaner_service::defender::{decode_provider_state, get_antivirus_status};
use cleaner_service::diagnostics::{
    evaluate_diagnostics_rules, CPU_WARNING_THRESHOLD_PERCENT, DISK_CRITICAL_THRESHOLD_BYTES,
    DISK_WARNING_THRESHOLD_BYTES, MEMORY_CRITICAL_THRESHOLD_PERCENT,
    MEMORY_WARNING_THRESHOLD_PERCENT, STARTUP_WARNING_THRESHOLD_COUNT,
};
use cleaner_service::programs::{
    is_forbidden_interpreter, list_installed_programs, parse_uninstaller_command,
};
use cleaner_service::startup::{extract_target_executable_path, parse_startup_approved_byte};
use cleaner_service::types::{HealthStatus, ProtectionState, ServiceError};

// ============================================================================
// 1. DEFENDER STATUS & NON-ELEVATION GUARANTEE
// ============================================================================

#[test]
fn test_defender_bitmask_decoding_comprehensive() {
    // Case 1: 0x061100 (Typical Windows Defender running + updated signatures)
    // 0x061100 = 0b0000_0110_0001_0001_0000_0000
    // Bit 0x1000 is set -> RTP = true
    // Bit 0x100 is set -> Signatures up to date = true
    let (rtp1, sigs1) = decode_provider_state(0x061100);
    assert!(rtp1, "0x061100 should decode RTP=true");
    assert!(sigs1, "0x061100 should decode Signatures=true");

    // Case 2: 0x060000 (RTP disabled, signatures not updated or unknown)
    let (rtp2, sigs2) = decode_provider_state(0x060000);
    assert!(!rtp2, "0x060000 should decode RTP=false");
    assert!(!sigs2, "0x060000 should decode Signatures=false");

    // Case 3: 0x000000 (Zero / uninitialized state)
    let (rtp3, sigs3) = decode_provider_state(0x000000);
    assert!(!rtp3, "0x000000 should decode RTP=false");
    assert!(!sigs3, "0x000000 should decode Signatures=false");

    // Case 4: 0x000010 (Legacy bit 0x10 set, bit 0x08 not set -> RTP true, sigs true)
    let (rtp4, sigs4) = decode_provider_state(0x000010);
    assert!(rtp4, "0x000010 should decode RTP=true");
    assert!(sigs4, "0x000010 should decode Signatures=true");

    // Case 5: 0x000018 (Legacy bit 0x10 set, but bit 0x08 is set indicating expired sigs)
    let (rtp5, sigs5) = decode_provider_state(0x000018);
    assert!(rtp5, "0x000018 should decode RTP=true");
    assert!(
        !sigs5,
        "0x000018 with bit 0x08 should decode Signatures=false"
    );

    // Case 6: 0x000100 (Only signatures updated, RTP off)
    let (rtp6, sigs6) = decode_provider_state(0x000100);
    assert!(!rtp6, "0x000100 should decode RTP=false");
    assert!(sigs6, "0x000100 should decode Signatures=true");

    // Case 7: 0x001000 (Only alternative RTP bit set, sigs off)
    let (rtp7, sigs7) = decode_provider_state(0x001000);
    assert!(rtp7, "0x001000 should decode RTP=true");
    assert!(!sigs7, "0x001000 should decode Signatures=false");

    // Case 8: Invalid / Extreme bit patterns
    let (rtp_max, sigs_max) = decode_provider_state(0xFFFFFFFF);
    assert!(rtp_max, "0xFFFFFFFF should have RTP=true");
    assert!(sigs_max, "0xFFFFFFFF should have Signatures=true");

    let (rtp_high, sigs_high) = decode_provider_state(0x80000000);
    assert!(!rtp_high, "0x80000000 should decode RTP=false");
    assert!(!sigs_high, "0x80000000 should decode Signatures=false");

    let (rtp_only8, sigs_only8) = decode_provider_state(0x000008);
    assert!(!rtp_only8, "0x000008 should decode RTP=false");
    assert!(!sigs_only8, "0x000008 should decode Signatures=false");
}

#[test]
fn test_defender_status_non_elevation_guarantee() {
    // Empirical test: get_antivirus_status must NEVER prompt for elevation
    // and must complete swiftly without blocking or hanging on a dialog.
    let start = std::time::Instant::now();
    let res = get_antivirus_status();
    let elapsed = start.elapsed();

    assert!(
        elapsed.as_secs() < 3,
        "get_antivirus_status took too long ({:?}), possible modal or hang",
        elapsed
    );
    assert!(
        res.is_ok(),
        "get_antivirus_status should return Ok(...) even in restricted mode"
    );

    let status = res.unwrap();
    // Verify valid status enum
    match status.state {
        ProtectionState::Protected => {
            assert!(!status.message.is_empty());
        }
        ProtectionState::Warning => {
            assert!(!status.message.is_empty());
        }
        ProtectionState::Disabled => {
            assert!(!status.message.is_empty());
        }
        ProtectionState::LimitedAccess => {
            assert!(
                status.message.contains("ограничен") || status.message.contains("Windows"),
                "LimitedAccess message must explain policy restrictions"
            );
        }
        ProtectionState::Unknown => {
            // Unknown is also valid if completely undecodable
        }
    }
}

// ============================================================================
// 2. DIAGNOSTICS THRESHOLDS BOUNDARY TESTING
// ============================================================================

#[test]
fn test_diagnostics_disk_exact_boundaries() {
    let drive = "C:";
    let total = Some(500 * 1024 * 1024 * 1024);

    // Boundary 1: Exactly 15 GB (16,106,127,360 bytes)
    // Rule: free < DISK_WARNING_THRESHOLD_BYTES -> NOT satisfied (15 GB is NOT < 15 GB)
    // Result: Healthy
    let (health_15gb, alerts_15gb, _) = evaluate_diagnostics_rules(
        Some(10.0),
        Some(40),
        Some(DISK_WARNING_THRESHOLD_BYTES),
        total,
        drive,
        5,
    );
    assert_eq!(
        health_15gb,
        HealthStatus::Healthy,
        "At exactly 15 GB, disk should be Healthy"
    );
    assert!(
        alerts_15gb.is_empty(),
        "No alerts expected at exactly 15 GB free disk"
    );

    // Boundary 2: Exactly 14.99 GB (14.99 * 1024^3 bytes = 16,095,390,269)
    let free_14_99 = (14.99 * 1024.0 * 1024.0 * 1024.0) as u64;
    let (health_14_99, alerts_14_99, recs_14_99) =
        evaluate_diagnostics_rules(Some(10.0), Some(40), Some(free_14_99), total, drive, 5);
    assert_eq!(
        health_14_99,
        HealthStatus::Warning,
        "At 14.99 GB, disk should trigger Warning"
    );
    assert_eq!(alerts_14_99.len(), 1);
    assert_eq!(alerts_14_99[0].id, "disk_warning");
    assert!(recs_14_99
        .iter()
        .any(|r| r.contains("профилактическую очистку")));

    // Boundary 3: Exactly 10 GB (10 * 1024^3 bytes = 10,737,418,240)
    // Rule: free < DISK_CRITICAL_THRESHOLD_BYTES is strict (< 10 GB),
    // so exactly 10 GB falls into warning (< 15 GB).
    let (health_10gb, alerts_10gb, _) = evaluate_diagnostics_rules(
        Some(10.0),
        Some(40),
        Some(DISK_CRITICAL_THRESHOLD_BYTES),
        total,
        drive,
        5,
    );
    assert_eq!(
        health_10gb,
        HealthStatus::Warning,
        "At exactly 10 GB, disk should be Warning (< 15 GB, but not < 10 GB)"
    );
    assert_eq!(alerts_10gb.len(), 1);
    assert_eq!(alerts_10gb[0].id, "disk_warning");

    // Boundary 4: Exactly 9.99 GB (9.99 * 1024^3 bytes = 10,726,681,149)
    let free_9_99 = (9.99 * 1024.0 * 1024.0 * 1024.0) as u64;
    let (health_9_99, alerts_9_99, recs_9_99) =
        evaluate_diagnostics_rules(Some(10.0), Some(40), Some(free_9_99), total, drive, 5);
    assert_eq!(
        health_9_99,
        HealthStatus::Critical,
        "At 9.99 GB, disk should trigger Critical"
    );
    assert_eq!(alerts_9_99.len(), 1);
    assert_eq!(alerts_9_99[0].id, "disk_critical");
    assert!(recs_9_99
        .iter()
        .any(|r| r.contains("Освободите место на системном диске")));

    // Boundary 5: Exactly 0 bytes free
    let (health_0b, alerts_0b, recs_0b) =
        evaluate_diagnostics_rules(Some(10.0), Some(40), Some(0), total, drive, 5);
    assert_eq!(
        health_0b,
        HealthStatus::Critical,
        "At 0 bytes, disk should trigger Critical"
    );
    assert_eq!(alerts_0b.len(), 1);
    assert_eq!(alerts_0b[0].id, "disk_critical");
    assert!(
        alerts_0b[0].message.contains("0.0 ГБ"),
        "Message must correctly format 0.0 ГБ without div by zero"
    );
    assert!(recs_0b.iter().any(|r| r.contains("Освободите место")));
}

#[test]
fn test_diagnostics_memory_exact_boundaries() {
    let drive = "C:";
    let disk = Some(100 * 1024 * 1024 * 1024);

    // Boundary 1: Exactly 80% RAM (Warning threshold is > 80%)
    let (health_80, alerts_80, _) = evaluate_diagnostics_rules(
        Some(10.0),
        Some(MEMORY_WARNING_THRESHOLD_PERCENT),
        disk,
        disk,
        drive,
        5,
    );
    assert_eq!(
        health_80,
        HealthStatus::Healthy,
        "At exactly 80%, RAM should be Healthy (> 80% required for warning)"
    );
    assert!(alerts_80.is_empty());

    // Boundary 2: Exactly 81% RAM (corresponds to > 80.0%)
    let (health_81, alerts_81, recs_81) =
        evaluate_diagnostics_rules(Some(10.0), Some(81), disk, disk, drive, 5);
    assert_eq!(
        health_81,
        HealthStatus::Warning,
        "At 81%, RAM should trigger Warning"
    );
    assert_eq!(alerts_81.len(), 1);
    assert_eq!(alerts_81[0].id, "memory_warning");
    assert!(recs_81.iter().any(|r| r.contains("активных процессов")));

    // Boundary 3: Exactly 92% RAM (Critical threshold is > 92%)
    let (health_92, alerts_92, _) = evaluate_diagnostics_rules(
        Some(10.0),
        Some(MEMORY_CRITICAL_THRESHOLD_PERCENT),
        disk,
        disk,
        drive,
        5,
    );
    assert_eq!(
        health_92,
        HealthStatus::Warning,
        "At exactly 92%, RAM should still be Warning (> 92% required for critical)"
    );
    assert_eq!(alerts_92.len(), 1);
    assert_eq!(alerts_92[0].id, "memory_warning");

    // Boundary 4: Exactly 93% RAM (corresponds to > 92.0%)
    let (health_93, alerts_93, recs_93) =
        evaluate_diagnostics_rules(Some(10.0), Some(93), disk, disk, drive, 5);
    assert_eq!(
        health_93,
        HealthStatus::Critical,
        "At 93%, RAM should trigger Critical"
    );
    assert_eq!(alerts_93.len(), 1);
    assert_eq!(alerts_93[0].id, "memory_critical");
    assert!(recs_93
        .iter()
        .any(|r| r.contains("ресурсоёмкие приложения")));

    // Boundary 5: Exactly 100% RAM
    let (health_100, alerts_100, _) =
        evaluate_diagnostics_rules(Some(10.0), Some(100), disk, disk, drive, 5);
    assert_eq!(
        health_100,
        HealthStatus::Critical,
        "At 100%, RAM should trigger Critical"
    );
    assert_eq!(alerts_100.len(), 1);
    assert_eq!(alerts_100[0].id, "memory_critical");
}

#[test]
fn test_diagnostics_cpu_exact_boundaries() {
    let drive = "C:";
    let disk = Some(100 * 1024 * 1024 * 1024);

    // Boundary 1: Exactly 85.0% CPU (Threshold is > 85.0%)
    let (health_85, alerts_85, _) = evaluate_diagnostics_rules(
        Some(CPU_WARNING_THRESHOLD_PERCENT),
        Some(50),
        disk,
        disk,
        drive,
        5,
    );
    assert_eq!(
        health_85,
        HealthStatus::Healthy,
        "At exactly 85.0%, CPU should be Healthy (> 85.0% required)"
    );
    assert!(alerts_85.is_empty());

    // Boundary 2: Exactly 85.1% CPU
    let (health_85_1, alerts_85_1, recs_85_1) =
        evaluate_diagnostics_rules(Some(85.1), Some(50), disk, disk, drive, 5);
    assert_eq!(
        health_85_1,
        HealthStatus::Warning,
        "At 85.1%, CPU should trigger Warning"
    );
    assert_eq!(alerts_85_1.len(), 1);
    assert_eq!(alerts_85_1[0].id, "cpu_warning");
    assert!(recs_85_1.iter().any(|r| r.contains("фоновые процессы")));

    // Boundary 3: 0.0% CPU
    let (health_0, alerts_0, _) =
        evaluate_diagnostics_rules(Some(0.0), Some(50), disk, disk, drive, 5);
    assert_eq!(health_0, HealthStatus::Healthy);
    assert!(alerts_0.is_empty());

    // Boundary 4: 100.0% CPU
    let (health_100, alerts_100, _) =
        evaluate_diagnostics_rules(Some(100.0), Some(50), disk, disk, drive, 5);
    assert_eq!(health_100, HealthStatus::Warning);
    assert_eq!(alerts_100.len(), 1);
}

#[test]
fn test_diagnostics_startup_items_exact_boundaries() {
    let drive = "C:";
    let disk = Some(100 * 1024 * 1024 * 1024);

    // Boundary 1: Exactly 15 startup items (Threshold is > 15)
    let (health_15, alerts_15, _) = evaluate_diagnostics_rules(
        Some(10.0),
        Some(40),
        disk,
        disk,
        drive,
        STARTUP_WARNING_THRESHOLD_COUNT,
    );
    assert_eq!(
        health_15,
        HealthStatus::Healthy,
        "At exactly 15 items, startup should be Healthy (> 15 required)"
    );
    assert!(alerts_15.is_empty());

    // Boundary 2: Exactly 16 startup items
    let (health_16, alerts_16, recs_16) =
        evaluate_diagnostics_rules(Some(10.0), Some(40), disk, disk, drive, 16);
    assert_eq!(
        health_16,
        HealthStatus::Warning,
        "At 16 items, startup should trigger Warning"
    );
    assert_eq!(alerts_16.len(), 1);
    assert_eq!(alerts_16[0].id, "startup_warning");
    assert!(recs_16.iter().any(|r| r.contains("проверить автозапуск")));
}

#[test]
fn test_diagnostics_mixed_severity_precedence() {
    let drive = "C:";
    // Critical disk (< 10GB) + Warning CPU (> 85%) + Warning RAM (> 80%)
    let (health, alerts, recs) = evaluate_diagnostics_rules(
        Some(90.0),
        Some(85),
        Some(5 * 1024 * 1024 * 1024), // 5 GB
        Some(100 * 1024 * 1024 * 1024),
        drive,
        20, // 20 startup items
    );

    // Critical must take precedence over Warning
    assert_eq!(
        health,
        HealthStatus::Critical,
        "HealthStatus::Critical must take precedence when any alert is Critical"
    );
    assert_eq!(
        alerts.len(),
        4,
        "Should have 4 alerts: disk, memory, cpu, startup"
    );
    assert_eq!(recs.len(), 4, "Should have 4 recommendations");
}

#[test]
fn test_diagnostics_unknown_state_when_metrics_missing() {
    let drive = "C:";
    let (health, alerts, recs) = evaluate_diagnostics_rules(None, None, None, None, drive, 0);
    assert_eq!(
        health,
        HealthStatus::Unknown,
        "HealthStatus should be Unknown when all metrics are None"
    );
    assert!(alerts.is_empty());
    assert!(recs.is_empty());
}

// ============================================================================
// 3. STARTUP APPROVED EDGE CASES
// ============================================================================

#[test]
fn test_startup_approved_byte_edge_cases() {
    // 1. Standard 0x02 -> Enabled
    assert!(
        parse_startup_approved_byte(&[0x02]),
        "0x02 should be enabled"
    );
    assert!(
        parse_startup_approved_byte(&[0x02, 0x00, 0x00, 0x00, 0x12, 0x34]),
        "0x02 prefix should be enabled"
    );

    // 2. Standard 0x03 -> Disabled
    assert!(
        !parse_startup_approved_byte(&[0x03]),
        "0x03 should be disabled"
    );
    assert!(
        !parse_startup_approved_byte(&[0x03, 0x00, 0x00, 0x00, 0xAA, 0xBB]),
        "0x03 prefix should be disabled"
    );

    // 3. Empty byte slice
    assert!(
        parse_startup_approved_byte(&[]),
        "Empty byte slice should default to enabled (true)"
    );

    // 4. Truncated single-byte slice
    assert!(
        parse_startup_approved_byte(&[0x02]),
        "Single byte 0x02 handled"
    );
    assert!(
        !parse_startup_approved_byte(&[0x03]),
        "Single byte 0x03 handled"
    );

    // 5. Unexpected / Even byte values (even means enabled)
    assert!(
        parse_startup_approved_byte(&[0x00]),
        "0x00 (even) should be enabled"
    );
    assert!(
        parse_startup_approved_byte(&[0x06]),
        "0x06 (even) should be enabled"
    );
    assert!(
        parse_startup_approved_byte(&[0x80]),
        "0x80 (even) should be enabled"
    );

    // 6. Unexpected / Odd byte values (odd means disabled)
    assert!(
        !parse_startup_approved_byte(&[0x01]),
        "0x01 (odd) should be disabled"
    );
    assert!(
        !parse_startup_approved_byte(&[0x07]),
        "0x07 (odd) should be disabled"
    );
    assert!(
        !parse_startup_approved_byte(&[0x81]),
        "0x81 (odd) should be disabled"
    );
    assert!(
        !parse_startup_approved_byte(&[0xFF]),
        "0xFF (odd) should be disabled"
    );
}

#[test]
fn test_startup_target_executable_extraction_edge_cases() {
    // Quoted path with space in path and space in args
    let cmd1 = r#""C:\Program Files\Common Files\service.exe" --run-daemon"#;
    assert_eq!(
        extract_target_executable_path(cmd1),
        r#"C:\Program Files\Common Files\service.exe"#
    );

    // Unquoted path with spaces before .exe and spaces in args
    let cmd2 = r#"C:\Program Files (x86)\App\tool.exe /silent /autostart"#;
    assert_eq!(
        extract_target_executable_path(cmd2),
        r#"C:\Program Files (x86)\App\tool.exe"#
    );

    // Unquoted batch script
    let cmd3 = r#"C:\Scripts\autorun.bat /start"#;
    assert_eq!(
        extract_target_executable_path(cmd3),
        r#"C:\Scripts\autorun.bat"#
    );

    // Incomplete quotes (leading quote without closing quote is preserved by extract_target_executable_path)
    let cmd4 = r#""C:\Program Files\Incomplete\app.exe"#;
    assert_eq!(
        extract_target_executable_path(cmd4),
        r#""C:\Program Files\Incomplete\app.exe"#
    );

    // Empty or whitespace
    assert_eq!(extract_target_executable_path(""), "");
    assert_eq!(extract_target_executable_path("   "), "");
}

// ============================================================================
// 4. PROGRAMS REGISTRY METADATA ROBUSTNESS
// ============================================================================

#[test]
fn test_programs_uninstaller_command_robustness() {
    // Unquoted path with spaces and arguments
    let cmd1 = r#"C:\Program Files\My App\unins000.exe -uninstall -silent"#;
    let (exe1, args1) = parse_uninstaller_command(cmd1).expect("Parse unquoted uninstaller");
    assert_eq!(exe1, r#"C:\Program Files\My App\unins000.exe"#);
    assert_eq!(args1, vec!["-uninstall", "-silent"]);

    // Quoted path with spaces
    let cmd2 = r#""C:\Program Files\My App\unins000.exe" /VERYSILENT"#;
    let (exe2, args2) = parse_uninstaller_command(cmd2).expect("Parse quoted uninstaller");
    assert_eq!(exe2, r#"C:\Program Files\My App\unins000.exe"#);
    assert_eq!(args2, vec!["/VERYSILENT"]);

    // MsiExec variants
    let cmd3 = "msiexec.exe /X{11111111-2222-3333-4444-555555555555} /quiet";
    let (exe3, args3) = parse_uninstaller_command(cmd3).expect("Parse msiexec");
    assert!(exe3.to_lowercase().ends_with("msiexec.exe"));
    assert_eq!(
        args3,
        vec!["/X{11111111-2222-3333-4444-555555555555}", "/quiet"]
    );

    let cmd4 = "msiexec /I{GUID} /passive";
    let (exe4, args4) = parse_uninstaller_command(cmd4).expect("Parse msiexec shorthand");
    assert!(exe4.to_lowercase().ends_with("msiexec.exe"));
    assert_eq!(args4, vec!["/I{GUID}", "/passive"]);

    // Environment variables expansion
    let cmd5 = r#""%ProgramFiles%\App\uninstaller.exe""#;
    let (exe5, args5) = parse_uninstaller_command(cmd5).expect("Expand %ProgramFiles%");
    assert!(!exe5.contains("%ProgramFiles%"));
    assert!(args5.is_empty());

    // Hostile commands: cmd.exe, powershell.exe, wscript.exe, bash.exe
    let hostile_commands = [
        "cmd.exe /c del /f /q C:\\*",
        "cmd /k echo pwn",
        "powershell.exe -ExecutionPolicy Bypass -Command Remove-Item C:\\*",
        "powershell -c ls",
        "pwsh.exe -File script.ps1",
        "pwsh script.ps1",
        "wscript.exe malware.vbs",
        "cscript.exe malware.vbs",
        "bash.exe -c rm -rf /",
        "sh.exe -c rm",
        r#""C:\Windows\System32\cmd.exe" /c echo bad"#,
    ];

    for hostile in hostile_commands {
        let res = parse_uninstaller_command(hostile);
        assert!(
            matches!(res, Err(ServiceError::InvalidCommand(_))),
            "Expected InvalidCommand for hostile string '{}', got {:?}",
            hostile,
            res
        );
    }

    // Empty strings
    assert!(matches!(
        parse_uninstaller_command(""),
        Err(ServiceError::InvalidCommand(_))
    ));
    assert!(matches!(
        parse_uninstaller_command("   "),
        Err(ServiceError::InvalidCommand(_))
    ));
}

#[test]
fn test_forbidden_interpreter_detector() {
    assert!(is_forbidden_interpreter("cmd.exe"));
    assert!(is_forbidden_interpreter("cmd"));
    assert!(is_forbidden_interpreter("C:\\Windows\\System32\\cmd.exe"));
    assert!(is_forbidden_interpreter("powershell.exe"));
    assert!(is_forbidden_interpreter("powershell"));
    assert!(is_forbidden_interpreter(
        "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe"
    ));
    assert!(is_forbidden_interpreter("pwsh.exe"));
    assert!(is_forbidden_interpreter("wscript.exe"));
    assert!(is_forbidden_interpreter("cscript.exe"));
    assert!(is_forbidden_interpreter("bash.exe"));
    assert!(is_forbidden_interpreter("sh.exe"));

    // Allowed programs
    assert!(!is_forbidden_interpreter("unins000.exe"));
    assert!(!is_forbidden_interpreter(
        "C:\\Program Files\\App\\uninstall.exe"
    ));
    assert!(!is_forbidden_interpreter("setup.exe"));
}

#[test]
fn test_real_registry_installed_programs_resilience() {
    // Run real scan across Windows registry hives without administrative rights
    let res = list_installed_programs(false);
    assert!(
        res.is_ok(),
        "list_installed_programs(false) must not fail on Windows"
    );
    let programs = res.unwrap();

    // Verify program entry consistency
    for p in &programs {
        assert!(!p.id.is_empty(), "Program ID cannot be empty");
        assert!(
            !p.display_name.trim().is_empty(),
            "DisplayName must be non-empty and trimmed: {:?}",
            p
        );
        assert!(
            p.registry_hive == "HKLM" || p.registry_hive == "HKCU",
            "Registry hive must be HKLM or HKCU"
        );

        if let Some(ref date) = p.install_date {
            assert!(
                !date.trim().is_empty(),
                "InstallDate, if present, must be non-empty"
            );
        }
        if let Some(ref uninst) = p.uninstall_string {
            assert!(
                !uninst.trim().is_empty(),
                "UninstallString, if present, must be non-empty"
            );
        }
        if let Some(size) = p.estimated_size_bytes {
            // Sane bound: size is u64, should not exceed 100 TB
            assert!(
                size < 100 * 1024 * 1024 * 1024 * 1024,
                "EstimatedSize in bytes exceeds 100 TB: {}",
                size
            );
        }
    }

    // Also run with system components
    let res_sys = list_installed_programs(true);
    assert!(
        res_sys.is_ok(),
        "list_installed_programs(true) must succeed"
    );
    let programs_sys = res_sys.unwrap();
    assert!(
        programs_sys.len() >= programs.len(),
        "Including system components should yield >= standard programs"
    );
}

#[cfg(windows)]
#[test]
fn test_registry_simulated_corrupt_metadata_resilience() {
    // Stress test winreg metadata parsing against simulated corrupted keys in HKCU
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let test_hive_path = r"Software\CleanCore_Challenger2_TestSubkey";

    // Create test key
    let (test_key, _) = hkcu
        .create_subkey(test_hive_path)
        .expect("Create test registry subkey in HKCU");

    // 1. Missing DisplayName test:
    // Only set Publisher, no DisplayName
    test_key.set_value("Publisher", &"Vendor Corp").ok();
    let display_name_res: Result<String, _> = test_key.get_value("DisplayName");
    assert!(
        display_name_res.is_err(),
        "Missing DisplayName should return Err"
    );

    // 2. Corrupt EstimatedSize: write REG_SZ (string) instead of REG_DWORD
    test_key
        .set_value("EstimatedSize", &"10240")
        .expect("Write string EstimatedSize");
    let size_u32_res: Result<u32, _> = test_key.get_value("EstimatedSize");
    let size_u64_res: Result<u64, _> = test_key.get_value("EstimatedSize");
    // Verify that attempting to read REG_SZ as u32/u64 fails gracefully without panic
    assert!(
        size_u32_res.is_err(),
        "Reading string EstimatedSize as u32 must fail gracefully"
    );
    assert!(
        size_u64_res.is_err(),
        "Reading string EstimatedSize as u64 must fail gracefully"
    );

    // 3. Valid EstimatedSize as REG_DWORD
    let dword_val: u32 = 20480;
    test_key
        .set_value("EstimatedSizeDword", &dword_val)
        .expect("Write DWORD EstimatedSize");
    let read_dword: u32 = test_key
        .get_value("EstimatedSizeDword")
        .expect("Read DWORD");
    assert_eq!(read_dword, 20480);

    // Clean up test key
    let _ = hkcu.delete_subkey(test_hive_path);
}
