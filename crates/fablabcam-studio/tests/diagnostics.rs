use fablabcam_studio::diagnostics::{self, cli::Options, Diagnostics, LogLevel, Settings};
use serde_json::json;
use std::path::Path;
use std::process::Command;

#[test]
fn settings_and_export_preserve_only_intended_diagnostics() {
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join("diagnostics");
    let service = Diagnostics::open(directory.clone()).unwrap();
    service.record(true, "hidden_detail", json!({}));
    let settings = Settings {
        level: LogLevel::Debug,
        full_backtrace: false,
    };
    service.configure(&settings, true).unwrap();
    service.record(true, "visible_detail", json!({"shape_id":12}));
    std::fs::write(directory.join("core.123"), "MEMORY_SECRET").unwrap();
    std::fs::write(directory.join("project.cb"), "PROJECT_SECRET").unwrap();
    let destination = root.path().join("report.txt");
    service.export(&destination).unwrap();
    let report = std::fs::read_to_string(destination).unwrap();
    assert!(report.contains("visible_detail"));
    assert!(!report.contains("hidden_detail"));
    assert!(!report.contains("MEMORY_SECRET"));
    assert!(!report.contains("PROJECT_SECRET"));
    assert!(!report.contains("settings.json"));
    service.shutdown();
    assert_eq!(Diagnostics::open(directory).unwrap().settings(), settings);
}

#[test]
fn journal_rotates_and_reports_write_failures() {
    let root = tempfile::tempdir().unwrap();
    let service = Diagnostics::open(root.path().to_owned()).unwrap();
    for n in 0..20 {
        service.record(false, "rotation", json!({"n":n,"data":"x".repeat(300_000)}));
    }
    let logs: Vec<_> = std::fs::read_dir(root.path())
        .unwrap()
        .map(Result::unwrap)
        .filter(|e| e.path().extension().is_some_and(|s| s == "jsonl"))
        .collect();
    assert_eq!(logs.len(), 4);
    assert!(logs
        .iter()
        .all(|e| e.metadata().unwrap().len() <= 1_048_576));
    service.shutdown();
    service.record(false, "late_event", json!({}));
    assert!(service.warning().unwrap().contains("Journal fermé"));
}

#[test]
fn invalid_preferences_and_invalid_directory_are_visible() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("settings.json"), "not json").unwrap();
    let service = Diagnostics::open(root.path().to_owned()).unwrap();
    assert!(service.warning().unwrap().contains("illisibles"));
    assert!(Diagnostics::open(root.path().join("settings.json")).is_err());
}

#[test]
fn cli_options_do_not_become_project_paths_or_persist_core_mode() {
    let mut args = [
        "--diagnostics-dir",
        "/tmp/diagnostics-test",
        "--backtrace",
        "off",
        "--log-level",
        "debug",
        "--core-on-panic",
        "drawing.dxf",
    ]
    .map(str::to_owned)
    .to_vec();
    let options = Options::parse(&mut args).unwrap();
    assert_eq!(args, ["drawing.dxf"]);
    assert!(options.core_on_panic);
    assert!(!options.settings(Settings::default()).full_backtrace);
    assert_eq!(options.level, Some(LogLevel::Debug));
    for values in [
        vec!["--backtrace"],
        vec!["--log-level", "trace"],
        vec!["--backtrace", "--core-on-panic"],
    ] {
        assert!(Options::parse(&mut values.into_iter().map(str::to_owned).collect()).is_err());
    }
}

fn run_child(root: &Path, mode: &str) -> std::process::Output {
    Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "panic_child", "--nocapture"])
        .env("FABLABCAM_DIAGNOSTIC_TEST_MODE", mode)
        .env("FABLABCAM_DIAGNOSTIC_TEST_DIRECTORY", root)
        .output()
        .unwrap()
}

#[test]
fn panic_reports_survive_two_failures_without_terminal_backtrace_environment() {
    let root = tempfile::tempdir().unwrap();
    let result = run_child(root.path(), "reports");
    assert!(!result.status.success());
    let mut reports: Vec<_> = std::fs::read_dir(root.path())
        .unwrap()
        .map(Result::unwrap)
        .filter(|e| e.file_name().to_string_lossy().starts_with("panic-"))
        .map(|e| std::fs::read_to_string(e.path()).unwrap())
        .collect();
    reports.sort();
    assert_eq!(
        reports.len(),
        2,
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(reports
        .iter()
        .any(|r| r.contains("first diagnostic failure")));
    assert!(reports
        .iter()
        .any(|r| r.contains("second diagnostic failure")));
    assert!(
        reports.iter().all(|r| r.contains("panic_child")),
        "{reports:?}"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for entry in std::fs::read_dir(root.path()).unwrap().map(Result::unwrap) {
            assert_eq!(entry.metadata().unwrap().permissions().mode() & 0o077, 0);
        }
    }
}

#[test]
fn panic_child() {
    let Ok(mode) = std::env::var("FABLABCAM_DIAGNOSTIC_TEST_MODE") else {
        return;
    };
    let directory = std::env::var_os("FABLABCAM_DIAGNOSTIC_TEST_DIRECTORY").unwrap();
    let service = diagnostics::initialize(directory.into()).unwrap();
    if mode == "reports" {
        let _ = std::panic::catch_unwind(|| panic!("first diagnostic failure"));
        panic!("second diagnostic failure");
    }
    #[cfg(target_os = "macos")]
    if mode == "limits" {
        let mut original = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        assert_eq!(
            unsafe { libc::getrlimit(libc::RLIMIT_CORE, &mut original) },
            0
        );
        if service.set_core_armed(true).is_ok() {
            service.set_core_armed(false).unwrap();
            let mut restored = original;
            assert_eq!(
                unsafe { libc::getrlimit(libc::RLIMIT_CORE, &mut restored) },
                0
            );
            assert_eq!(restored.rlim_cur, original.rlim_cur);
            assert_eq!(restored.rlim_max, original.rlim_max);
        }
        let denied = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        assert_eq!(unsafe { libc::setrlimit(libc::RLIMIT_CORE, &denied) }, 0);
        assert!(!diagnostics::core_dump::status().available);
        assert!(service.set_core_armed(true).is_err());
        assert!(!service.core_armed());
    }
    #[cfg(target_os = "macos")]
    if mode == "core" || mode == "real_core" {
        if let Err(error) = service.set_core_armed(true) {
            eprintln!("CORE_UNAVAILABLE: {error}");
            return;
        }
        if mode == "core" {
            // Test SIGABRT without writing a memory dump on developer/CI hosts.
            let mut limit = libc::rlimit {
                rlim_cur: 0,
                rlim_max: 0,
            };
            assert_eq!(unsafe { libc::getrlimit(libc::RLIMIT_CORE, &mut limit) }, 0);
            limit.rlim_cur = 0;
            assert_eq!(unsafe { libc::setrlimit(libc::RLIMIT_CORE, &limit) }, 0);
        }
        panic!("controlled core diagnostic test");
    }
    if mode == "unwritable" {
        let moved = service.directory().with_extension("moved");
        std::fs::rename(service.directory(), &moved).unwrap();
        std::fs::write(service.directory(), "not a directory").unwrap();
        panic!("fallback diagnostic failure");
    }
}

#[test]
fn panic_report_failure_falls_back_to_stderr() {
    let root = tempfile::tempdir().unwrap();
    let result = run_child(&root.path().join("diagnostics"), "unwritable");
    assert!(!result.status.success());
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("Rapport de panne non enregistré"),
        "{stderr}"
    );
    assert!(stderr.contains("fallback diagnostic failure"));
}

#[cfg(target_os = "macos")]
#[test]
fn opt_in_core_uses_abort_after_writing_the_report() {
    use std::os::unix::process::ExitStatusExt;
    let root = tempfile::tempdir().unwrap();
    let result = run_child(root.path(), "core");
    if String::from_utf8_lossy(&result.stderr).contains("CORE_UNAVAILABLE:") {
        return;
    }
    assert_eq!(result.status.signal(), Some(libc::SIGABRT));
    assert!(std::fs::read_dir(root.path())
        .unwrap()
        .map(Result::unwrap)
        .any(|e| e.file_name().to_string_lossy().starts_with("panic-")));
}

#[test]
fn simultaneous_instances_rotate_independently_and_reject_oversized_events() {
    let root = tempfile::tempdir().unwrap();
    let first = Diagnostics::open(root.path().to_owned()).unwrap();
    let second = Diagnostics::open(root.path().to_owned()).unwrap();
    for n in 0..20 {
        first.record(false, "rotation", json!({"n":n,"data":"x".repeat(300_000)}));
    }
    second.record(false, "other_instance", json!({}));
    first.record(false, "oversized", json!({"data":"x".repeat(1_048_576)}));
    assert!(first.warning().unwrap().contains("volumineux"));
    let report = root
        .path()
        .parent()
        .unwrap()
        .join(format!("diagnostics-test-{}.txt", std::process::id()));
    second.export(&report).unwrap();
    let exported = std::fs::read_to_string(&report).unwrap();
    std::fs::remove_file(report).unwrap();
    assert!(exported.contains("other_instance"));
    assert!(!exported.contains("oversized"));
}

#[cfg(target_os = "macos")]
#[test]
fn core_limit_is_restored_and_a_prohibited_limit_is_reported() {
    let root = tempfile::tempdir().unwrap();
    let result = run_child(root.path(), "limits");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}
