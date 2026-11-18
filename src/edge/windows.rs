//! The Windows backend: three PowerShell scripts.
//!
//! Edge has no outside API for its tabs, so `scripts/scan.ps1` reads the tab
//! strip with UI Automation, `scripts/focus.ps1` switches to a tab, and
//! `scripts/close.ps1` selects a tab and sends Ctrl+W. The scripts are
//! embedded in the executable and written to the cache folder before each
//! run, where they can be read.

use std::fs;
use std::io::{BufRead, BufReader};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use super::{CloseReport, RawTab, Target};
use crate::paths;

const SCAN_SCRIPT: &str = include_str!("../../scripts/scan.ps1");
const CLOSE_SCRIPT: &str = include_str!("../../scripts/close.ps1");
const FOCUS_SCRIPT: &str = include_str!("../../scripts/focus.ps1");

/// PowerShell 7 when it is installed, then the Windows PowerShell every
/// machine has. Both are found on `PATH`.
const SHELLS: [&str; 2] = ["pwsh.exe", "powershell.exe"];

/// Process creation flag that stops a console window flashing up.
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Where the scripts and the list of tabs to close are written.
fn work_dir() -> Result<PathBuf, String> {
    let dir = paths::cache_dir().ok_or("could not find a folder for working files")?;
    fs::create_dir_all(&dir).map_err(|err| format!("could not create {}: {err}", dir.display()))?;
    Ok(dir)
}

fn write_work_file(name: &str, body: &str) -> Result<PathBuf, String> {
    let path = work_dir()?.join(name);
    fs::write(&path, body).map_err(|err| format!("could not write {}: {err}", path.display()))?;
    Ok(path)
}

fn spawn_script(script: &Path, args: &[&str]) -> Result<Child, String> {
    let mut last_error = String::new();
    for shell in SHELLS {
        let spawned = Command::new(shell)
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
            ])
            .arg(script)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .creation_flags(CREATE_NO_WINDOW)
            .spawn();
        match spawned {
            Ok(child) => return Ok(child),
            Err(err) => last_error = format!("could not start {shell}: {err}"),
        }
    }
    Err(last_error)
}

pub fn scan() -> Result<Vec<RawTab>, String> {
    let script = write_work_file("scan.ps1", SCAN_SCRIPT)?;
    let output = spawn_script(&script, &[])?
        .wait_with_output()
        .map_err(|err| format!("the scan did not finish: {err}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_scan(stdout.trim()).map_err(|err| {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let detail = stderr.lines().next().unwrap_or("").trim();
        if detail.is_empty() {
            err
        } else {
            format!("{err} ({detail})")
        }
    })
}

fn parse_scan(json: &str) -> Result<Vec<RawTab>, String> {
    if json.is_empty() {
        return Err("the scan returned nothing".to_string());
    }
    serde_json::from_str(json).map_err(|err| format!("could not read the scan: {err}"))
}

pub fn close(
    targets: &[Target],
    mut on_step: impl FnMut(&CloseReport),
) -> Result<CloseReport, String> {
    let script = write_work_file("close.ps1", CLOSE_SCRIPT)?;
    let json = serde_json::to_string(targets).map_err(|err| err.to_string())?;
    let list = write_work_file("targets.json", &json)?;

    let list_arg = list.to_string_lossy().into_owned();
    let mut child = spawn_script(&script, &["-TargetsPath", &list_arg])?;
    let stdout = child
        .stdout
        .take()
        .ok_or("no output from the close script")?;

    let mut report = CloseReport::default();
    for line in BufReader::new(stdout).lines() {
        let Ok(line) = line else { break };
        apply_line(&mut report, &line);
        on_step(&report);
    }
    let _ = child.wait();
    let _ = fs::remove_file(&list);
    Ok(report)
}

/// The close script prints one line per tab: `CLOSED`, `SKIP <reason>`, or a
/// final `STOP <reason>`.
fn apply_line(report: &mut CloseReport, line: &str) {
    let line = line.trim();
    if line == "CLOSED" {
        report.closed += 1;
    } else if line.starts_with("SKIP") {
        report.skipped += 1;
    } else if let Some(reason) = line.strip_prefix("STOP") {
        report.stopped = Some(reason.trim().to_string());
    }
}

pub fn focus(target: &Target) -> Result<(), String> {
    let script = write_work_file("focus.ps1", FOCUS_SCRIPT)?;
    // The title goes in a file because a command line mangles quotes and
    // non-ASCII characters.
    let title = write_work_file("focus-title.txt", &target.title)?;

    let window = target.window.to_string();
    let index = target.index.to_string();
    let title_arg = title.to_string_lossy().into_owned();
    let output = spawn_script(
        &script,
        &["-Hwnd", &window, "-Index", &index, "-TitlePath", &title_arg],
    )?
    .wait_with_output()
    .map_err(|err| format!("could not switch tabs: {err}"))?;
    let _ = fs::remove_file(&title);
    focus_result(&String::from_utf8_lossy(&output.stdout))
}

/// The focus script prints `OK`, or `FAIL <reason>`.
fn focus_result(stdout: &str) -> Result<(), String> {
    let line = stdout.lines().last().unwrap_or("").trim();
    if line == "OK" {
        return Ok(());
    }
    match line.strip_prefix("FAIL") {
        Some(reason) => Err(reason.trim().to_string()),
        None => Err("Edge did not switch to the tab.".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scan_json_is_read() {
        let tabs = parse_scan(
            r#"[{"hwnd":7,"window":"Work","index":0,"name":"Timesheet","active":true},
                {"hwnd":7,"window":"Work","index":1,"name":"Dashboard - Discarded"}]"#,
        )
        .unwrap();
        assert_eq!(tabs.len(), 2);
        assert_eq!(tabs[0].window, 7);
        assert!(tabs[0].active);
        assert!(!tabs[1].active);
        assert_eq!(tabs[1].index, 1);
    }

    #[test]
    fn no_edge_windows_is_an_empty_list_not_an_error() {
        assert!(parse_scan("[]").unwrap().is_empty());
        assert!(parse_scan("").is_err());
    }

    #[test]
    fn targets_are_written_with_the_field_names_the_scripts_read() {
        let json = serde_json::to_string(&[Target {
            window: 7,
            index: 3,
            title: "Timesheet".to_string(),
        }])
        .unwrap();
        assert_eq!(json, r#"[{"hwnd":7,"index":3,"title":"Timesheet"}]"#);
    }

    #[test]
    fn the_focus_script_answer_is_read() {
        assert_eq!(focus_result("OK\n"), Ok(()));
        assert_eq!(
            focus_result("FAIL the tab has moved since the scan. Press Rescan.\n"),
            Err("the tab has moved since the scan. Press Rescan.".to_string())
        );
        assert!(focus_result("").is_err());
    }

    #[test]
    fn close_lines_are_counted() {
        let mut report = CloseReport::default();
        for line in [
            "CLOSED",
            "SKIP tab moved",
            "CLOSED",
            "STOP a tab did not close",
        ] {
            apply_line(&mut report, line);
        }
        assert_eq!(report.closed, 2);
        assert_eq!(report.skipped, 1);
        assert_eq!(report.stopped.as_deref(), Some("a tab did not close"));
    }
}
