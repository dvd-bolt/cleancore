//! Adversarial Security Test Suite for CleanCore Stage 6 Quality Gate.
//!
//! Tests:
//! 1. CWE-1236: CSV Formula Injection sanitization, prefix neutralization, RFC 4180 escaping.
//! 2. System Tools Whitelist & Path Traversal resilience.
//! 3. Uninstaller Launch Safety: Shell interpreter rejection and uninstaller command parsing.

use cleaner_service::export::{format_csv_cell, generate_csv_report, sanitize_csv_cell, UTF8_BOM};
use cleaner_service::programs::{is_forbidden_interpreter, parse_uninstaller_command};
use cleaner_service::tools::{get_system32_directory, get_tool_metadata, list_system_tools};
use cleaner_service::types::{
    CsvExportConfig, CsvExportSection, DiagnosticsSummary, HealthStatus, InstalledProgram,
    ServiceError, StartupItem, StartupSource, SystemToolId,
};

// ==============================================================================
// 1. CSV FORMULA INJECTION TESTS (CWE-1236)
// ==============================================================================

#[test]
fn test_adversarial_csv_formula_injection_direct_prefixes() {
    let test_cases = [
        // Prefix '=': DDE execution payload
        ("=cmd|' /C calc'!A0", "'=cmd|' /C calc'!A0"),
        ("=1+2", "'=1+2"),
        (
            "=HYPERLINK(\"http://evil.com\",\"Click\")",
            "'=HYPERLINK(\"http://evil.com\",\"Click\")",
        ),
        // Prefix '+': Calculation / execution
        ("+12345", "'+12345"),
        ("+cmd|' /C calc'!A0", "'+cmd|' /C calc'!A0"),
        // Prefix '-': Formula prefix
        ("-@SUM(A1:A10)", "'-@SUM(A1:A10)"),
        ("-5+10", "'-5+10"),
        ("-2*3", "'-2*3"),
        // Prefix '@': Function call prefix
        ("@echo off", "'@echo off"),
        ("@SUM(1,2)", "'@SUM(1,2)"),
        // Prefix '\t': Horizontal tab prefix
        ("\tTabPrefix", "'\tTabPrefix"),
        ("\t=cmd|' /C calc'!A0", "'\t=cmd|' /C calc'!A0"),
        // Prefix '\r': Carriage return prefix
        ("\rReturnPrefix", "'\rReturnPrefix"),
        ("\r+12345", "'\r+12345"),
    ];

    for (input, expected_sanitized) in test_cases {
        let sanitized = sanitize_csv_cell(input);
        assert_eq!(
            sanitized, expected_sanitized,
            "Failed sanitizing dangerous prefix for input: {:?}",
            input
        );
        assert!(
            sanitized.starts_with('\''),
            "Sanitized cell MUST start with single quote `'`: {:?}",
            sanitized
        );

        let formatted = format_csv_cell(input);
        assert!(
            formatted.starts_with('"') && formatted.ends_with('"'),
            "Formatted cell MUST be enclosed in double quotes per RFC 4180: {:?}",
            formatted
        );
        assert!(
            formatted.starts_with("\"'"),
            "Formatted cell content MUST start with quote prefix `\"'`: {:?}",
            formatted
        );
    }
}

#[test]
fn test_adversarial_csv_formula_injection_whitespace_padding() {
    let padded_cases = [
        "   =cmd|' /C calc'!A0",
        " =1+1",
        "       +9999",
        "  -SUM(A1:A10)",
        "   @malicious",
        " \t=calc",
        "   \r+calc",
    ];

    for input in padded_cases {
        let sanitized = sanitize_csv_cell(input);
        assert!(
            sanitized.starts_with('\''),
            "Whitespace-padded formula MUST be sanitized with leading quote: {:?}",
            input
        );

        let formatted = format_csv_cell(input);
        assert!(
            formatted.starts_with("\"'"),
            "Whitespace-padded formula MUST be quoted per RFC 4180: {:?}",
            formatted
        );
    }
}

#[test]
fn test_adversarial_csv_multiline_and_complex_delimiters() {
    // 1. Multiline value with commas, semicolons, and quotes
    let complex_cell = "Line 1\r\n\"Sub-Line 2, with comma and; semicolon\"\r\nLine 3";
    let formatted = format_csv_cell(complex_cell);

    assert!(formatted.starts_with('"') && formatted.ends_with('"'));
    // Internal quotes must be escaped as ""
    assert!(
        formatted.contains("\"\"Sub-Line 2, with comma and; semicolon\"\""),
        "Double quotes inside cell must be doubled: {:?}",
        formatted
    );

    // 2. Multiline cell with formula on first line
    let formula_multiline = "=cmd|' /C calc'!A0\r\nSecond line with \"quotes\"";
    let formatted_formula = format_csv_cell(formula_multiline);
    assert!(formatted_formula.starts_with("\"'="));
    assert!(formatted_formula.contains("\"\"quotes\"\""));

    // 3. Normal benign values should not be modified or quoted if not needed
    assert_eq!(
        sanitize_csv_cell("Normal Program v1.0"),
        "Normal Program v1.0"
    );
    assert_eq!(
        format_csv_cell("Normal Program v1.0"),
        "Normal Program v1.0"
    );

    // 4. Benign string with comma must be quoted without single quote prefix
    let comma_str = "Google, Inc.";
    assert_eq!(sanitize_csv_cell(comma_str), "Google, Inc.");
    assert_eq!(format_csv_cell(comma_str), "\"Google, Inc.\"");
}

#[test]
fn test_adversarial_csv_full_report_generation() {
    let config = CsvExportConfig {
        sections: vec![
            CsvExportSection::Programs,
            CsvExportSection::Startup,
            CsvExportSection::Diagnostics,
        ],
        include_system_components: true,
        target_path: None,
    };

    let programs = vec![InstalledProgram {
        id: "prog_vuln_1".to_string(),
        display_name: "=cmd|' /C calc'!A0".to_string(),
        publisher: Some("+Microsoft Corporation".to_string()),
        display_version: Some("-1.0.0".to_string()),
        install_date: Some("@20261014".to_string()),
        estimated_size_bytes: Some(1024),
        uninstall_string: Some("\tC:\\uninst.exe".to_string()),
        quiet_uninstall_string: Some("\rC:\\uninst.exe /quiet".to_string()),
        install_location: Some("C:\\Program Files\\TestApp".to_string()),
        registry_hive: "HKLM".to_string(),
        is_wow64: false,
        is_system_component: false,
    }];

    let startup = vec![StartupItem {
        id: "startup_vuln_1".to_string(),
        name: "@EvilTask".to_string(),
        command: "+evil.exe --run".to_string(),
        source: StartupSource::RegistryRun,
        location: "HKLM\\Run".to_string(),
        enabled: true,
        target_exists: false,
    }];

    let diagnostics = DiagnosticsSummary {
        health: HealthStatus::Warning,
        cpu_usage_percent: Some(99.0),
        memory_usage_percent: Some(85),
        disk_free_bytes: Some(1024),
        disk_total_bytes: Some(1024 * 1024),
        disk_usage_percent: Some(99),
        system_drive: "C:".to_string(),
        alerts: vec![cleaner_service::DiagnosticAlert {
            id: "alert_1".to_string(),
            level: HealthStatus::Critical,
            category: "=category".to_string(),
            title: "-High CPU Usage".to_string(),
            message: "+Overloaded system, \"immediate action required\"".to_string(),
            recommendation: Some("@Kill rogue processes".to_string()),
        }],
        recommendations: vec!["Regular maintenance".to_string()],
        timestamp_utc: "2026-10-14T12:00:00Z".to_string(),
    };

    let report = generate_csv_report(&config, &programs, &startup, &diagnostics)
        .expect("Generate full CSV report");

    // 1. Verify UTF-8 BOM
    assert!(
        report.starts_with(UTF8_BOM),
        "Report must start with UTF-8 BOM"
    );

    // 2. Verify all injected fields in programs section are neutralized
    assert!(report.contains("\"'=cmd|' /C calc'!A0\""));
    assert!(report.contains("\"'+Microsoft Corporation\""));
    assert!(report.contains("\"'-1.0.0\""));
    assert!(report.contains("\"'@20261014\""));
    assert!(report.contains("\"'\tC:\\uninst.exe\""));

    // 3. Verify all injected fields in startup section are neutralized
    assert!(report.contains("\"'@EvilTask\""));
    assert!(report.contains("\"'+evil.exe --run\""));

    // 4. Verify all injected fields in diagnostics section are neutralized
    assert!(report.contains("\"'=category\""));
    assert!(report.contains("\"'-High CPU Usage\""));
    assert!(report.contains("\"'+Overloaded system, \"\"immediate action required\"\"\""));
    assert!(report.contains("\"'@Kill rogue processes\""));
}

// ==============================================================================
// 2. SYSTEM TOOLS WHITELIST & PATH TRAVERSAL TESTS
// ==============================================================================

#[test]
fn test_adversarial_system_tools_id_whitelist_serde_rejection() {
    let malicious_tool_payloads = [
        "\"../../cmd.exe\"",
        "\"..\\\\..\\\\cmd.exe\"",
        "\"calc.exe\"",
        "\"calc\"",
        "\"powershell.exe\"",
        "\"powershell\"",
        "\"cmd.exe\"",
        "\"cmd\"",
        "\"notepad.exe\"",
        "\"regedit.exe\"",
        "\"/bin/sh\"",
        "\"explorer.exe\"",
        "\"rundll32.exe\"",
        "\"\"",
        "\"   \"",
        "\"CLEANMGR\"",     // Must be snake_case cleanmgr
        "\"cleanmgr.exe\"", // Enum variant is cleanmgr without .exe
    ];

    for payload in malicious_tool_payloads {
        let res = serde_json::from_str::<SystemToolId>(payload);
        assert!(
            res.is_err(),
            "Unauthorized tool ID payload {:?} MUST be rejected by deserializer",
            payload
        );
    }
}

#[test]
fn test_system_tools_strictly_resolve_to_system32() {
    let sys32 = get_system32_directory();
    assert!(
        sys32.is_absolute(),
        "System32 directory must be an absolute path: {:?}",
        sys32
    );
    assert!(
        sys32.to_string_lossy().to_lowercase().ends_with("system32"),
        "System32 path must end with System32: {:?}",
        sys32
    );

    let tools = list_system_tools();
    assert_eq!(
        tools.len(),
        5,
        "Strictly exactly 5 tools must be whitelisted"
    );

    let allowed_executables = [
        "cleanmgr.exe",
        "dfrgui.exe",
        "resmon.exe",
        "taskmgr.exe",
        "control.exe",
    ];

    for tool in tools {
        let (metadata, exe_path, args) = get_tool_metadata(tool.id);
        assert_eq!(metadata.id, tool.id);
        assert!(
            exe_path.starts_with(&sys32),
            "Tool {:?} path '{}' must reside inside System32 directory '{}'",
            tool.id,
            exe_path.display(),
            sys32.display()
        );

        let file_name = exe_path
            .file_name()
            .and_then(|n| n.to_str())
            .expect("Valid file name")
            .to_lowercase();

        assert!(
            allowed_executables.contains(&file_name.as_str()),
            "Executable '{}' is not in allowed whitelist: {:?}",
            file_name,
            allowed_executables
        );

        if tool.id == SystemToolId::Appwiz {
            assert_eq!(file_name, "control.exe");
            assert_eq!(args, vec!["appwiz.cpl"]);
        } else {
            assert!(args.is_empty());
        }
    }
}

// ==============================================================================
// 3. UNINSTALLER LAUNCH SAFETY & SHELL REJECTION TESTS
// ==============================================================================

#[test]
fn test_adversarial_uninstaller_rejects_command_shells_standard() {
    let shell_attacks = [
        // cmd.exe variants
        "cmd.exe /c del /f /q C:\\Windows",
        "CMD.EXE /C calc.exe",
        "cmd /c calc",
        "cmd /k echo pwned",
        "C:\\Windows\\System32\\cmd.exe /c calc",
        "\"C:\\Windows\\System32\\cmd.exe\" /c calc",
        "C:/Windows/System32/cmd.exe /c calc",
        // powershell variants
        "powershell -Command Remove-Item -Force -Recurse C:\\",
        "powershell.exe -ExecutionPolicy Bypass -Command calc",
        "POWERSHELL.EXE -Command calc",
        "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe -NoProfile",
        "\"C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe\" -c dir",
        // pwsh variants
        "pwsh -c Remove-Item -Path C:\\",
        "pwsh.exe -c calc",
        // Scripting hosts with .exe
        "wscript.exe //B //Nologo C:\\malware.vbs",
        "cscript.exe evil.vbs",
        // Linux/Unix shells with .exe
        "bash.exe -c \"rm -rf /\"",
        "sh.exe -c calc",
    ];

    for attack in shell_attacks {
        let res = parse_uninstaller_command(attack);
        assert!(
            res.is_err(),
            "Shell invocation attack {:?} MUST be rejected with error, got: {:?}",
            attack,
            res
        );
        match res.unwrap_err() {
            ServiceError::InvalidCommand(msg) => {
                assert!(
                    msg.contains("Запрещён запуск") || msg.contains("оболочек"),
                    "Error message must clearly identify forbidden shell: {}",
                    msg
                );
            }
            other => panic!(
                "Expected InvalidCommand error for {:?}, got: {:?}",
                attack, other
            ),
        }
    }
}

#[test]
fn test_adversarial_uninstaller_scripting_hosts_without_extension_vulnerability() {
    // Challenge is_forbidden_interpreter on wscript/cscript/bash/sh without extension
    // We observe the empirical behavior:
    let wscript_res = parse_uninstaller_command("wscript malware.vbs");
    let cscript_res = parse_uninstaller_command("cscript script.js");
    let bash_res = parse_uninstaller_command("bash -c calc");
    let sh_res = parse_uninstaller_command("sh -c calc");

    // Currently in programs.rs, matches! includes:
    // "cmd.exe" | "cmd" | "powershell.exe" | "powershell" | "pwsh.exe" | "pwsh"
    // | "wscript.exe" | "cscript.exe" | "bash.exe" | "sh.exe"
    // Notice wscript/cscript/bash/sh without .exe are missing!
    // We empirically document whether they are allowed or blocked:
    let wscript_allowed = wscript_res.is_ok();
    let cscript_allowed = cscript_res.is_ok();
    let bash_allowed = bash_res.is_ok();
    let sh_allowed = sh_res.is_ok();

    println!(
        "Empirical vulnerability check: wscript without .exe allowed = {}",
        wscript_allowed
    );
    println!(
        "Empirical vulnerability check: cscript without .exe allowed = {}",
        cscript_allowed
    );
    println!(
        "Empirical vulnerability check: bash without .exe allowed = {}",
        bash_allowed
    );
    println!(
        "Empirical vulnerability check: sh without .exe allowed = {}",
        sh_allowed
    );

    // This documents the exact gap for our Challenger report!
}

#[test]
fn test_adversarial_uninstaller_environment_variable_expansion_block() {
    // Attackers using %COMSPEC% to hide cmd.exe
    let comspec_attack = "%COMSPEC% /c calc.exe";
    let res = parse_uninstaller_command(comspec_attack);
    assert!(
        res.is_err(),
        "%COMSPEC% expansion MUST still trigger shell forbidden check: {:?}",
        res
    );

    // %SystemRoot%\System32\cmd.exe
    let sysroot_attack = "%SystemRoot%\\System32\\cmd.exe /c dir";
    let res2 = parse_uninstaller_command(sysroot_attack);
    assert!(
        res2.is_err(),
        "%SystemRoot% cmd.exe MUST still trigger shell forbidden check: {:?}",
        res2
    );
}

#[test]
fn test_is_forbidden_interpreter_exact_matching() {
    // Should return true for forbidden shells
    assert!(is_forbidden_interpreter("cmd.exe"));
    assert!(is_forbidden_interpreter("cmd"));
    assert!(is_forbidden_interpreter("powershell.exe"));
    assert!(is_forbidden_interpreter("powershell"));
    assert!(is_forbidden_interpreter("pwsh.exe"));
    assert!(is_forbidden_interpreter("pwsh"));
    assert!(is_forbidden_interpreter("wscript.exe"));
    assert!(is_forbidden_interpreter("cscript.exe"));
    assert!(is_forbidden_interpreter("bash.exe"));
    assert!(is_forbidden_interpreter("sh.exe"));
    assert!(is_forbidden_interpreter("C:\\Windows\\System32\\cmd.exe"));
    assert!(is_forbidden_interpreter("C:/Windows/System32/cmd.exe"));

    // Legitimate uninstallers containing "cmd" in substring must NOT be blocked
    assert!(!is_forbidden_interpreter(
        "C:\\App\\command_uninstaller.exe"
    ));
    assert!(!is_forbidden_interpreter("C:\\App\\cmdtool.exe"));
    assert!(!is_forbidden_interpreter(
        "C:\\Program Files\\App\\uninstall.exe"
    ));
}

#[test]
fn test_legitimate_uninstaller_parsing_variations() {
    // 1. Quoted path with spaces and arguments
    let cmd1 = r#""C:\Program Files\Vendor\Software\uninstall.exe" /S /NORESTART"#;
    let (exe1, args1) = parse_uninstaller_command(cmd1).expect("Parse quoted path with args");
    assert_eq!(exe1, r#"C:\Program Files\Vendor\Software\uninstall.exe"#);
    assert_eq!(args1, vec!["/S", "/NORESTART"]);

    // 2. Quoted path without arguments
    let cmd2 = r#""C:\Program Files (x86)\App\unins000.exe""#;
    let (exe2, args2) = parse_uninstaller_command(cmd2).expect("Parse quoted path without args");
    assert_eq!(exe2, r#"C:\Program Files (x86)\App\unins000.exe"#);
    assert!(args2.is_empty());

    // 3. Unquoted path with spaces
    let cmd3 = r#"C:\Program Files\VideoLAN\VLC\uninstall.exe /S"#;
    let (exe3, args3) = parse_uninstaller_command(cmd3).expect("Parse unquoted path with spaces");
    assert_eq!(exe3, r#"C:\Program Files\VideoLAN\VLC\uninstall.exe"#);
    assert_eq!(args3, vec!["/S"]);

    // 4. MsiExec uppercase
    let cmd4 = "MsiExec.exe /X{12345678-ABCD-1234-ABCD-1234567890AB} /quiet";
    let (exe4, args4) = parse_uninstaller_command(cmd4).expect("Parse msiexec uppercase");
    assert!(exe4.to_lowercase().ends_with("system32\\msiexec.exe"));
    assert_eq!(
        args4,
        vec!["/X{12345678-ABCD-1234-ABCD-1234567890AB}", "/quiet"]
    );

    // 5. MsiExec lowercase without .exe
    let cmd5 = "msiexec /I{GUID} /qn";
    let (exe5, args5) = parse_uninstaller_command(cmd5).expect("Parse msiexec no extension");
    assert!(exe5.to_lowercase().ends_with("system32\\msiexec.exe"));
    assert_eq!(args5, vec!["/I{GUID}", "/qn"]);

    // 6. Empty or blank inputs
    assert!(parse_uninstaller_command("").is_err());
    assert!(parse_uninstaller_command("   ").is_err());
    assert!(parse_uninstaller_command("\t\r\n").is_err());
}
