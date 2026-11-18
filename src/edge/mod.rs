//! Reading, showing and closing Microsoft Edge's tabs.
//!
//! This file is the part the rest of the program talks to: the tab types and
//! three calls. How those calls reach the browser is different on each
//! operating system and lives in a backend file. `windows.rs` is the only
//! real one so far; `unsupported.rs` answers everywhere else with a sentence
//! saying so. A new platform is a new backend file with the same three
//! functions.
//!
//! Every call blocks for seconds, so the window runs them on a worker thread.

use serde::{Deserialize, Serialize};

#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as backend;

#[cfg(not(windows))]
mod unsupported;
#[cfg(not(windows))]
use unsupported as backend;

/// One tab as a scan reports it.
#[derive(Clone, Debug, Deserialize)]
pub struct RawTab {
    /// A number naming the browser window the tab is in. It only means
    /// something to the backend that produced it.
    #[serde(rename = "hwnd")]
    pub window: i64,
    /// The tab's place in its window's strip, from zero.
    pub index: usize,
    /// The tab's name with any notes the browser adds on the end.
    pub name: String,
    /// Whether this is the tab on screen in its window.
    #[serde(default)]
    pub active: bool,
}

/// A tab to show or close: the window, its place in the strip at scan time,
/// and the title it had, which the backend checks before acting on it.
#[derive(Clone, Debug, Serialize)]
pub struct Target {
    #[serde(rename = "hwnd")]
    pub window: i64,
    pub index: usize,
    pub title: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CloseReport {
    pub closed: usize,
    pub skipped: usize,
    /// Set when closing gave up part way, with the reason.
    pub stopped: Option<String>,
}

/// Every tab in every Edge browser window. An empty list means Edge is not
/// open.
pub fn scan() -> Result<Vec<RawTab>, String> {
    backend::scan()
}

/// Close `targets`, calling `on_step` with the report so far after each tab.
pub fn close(targets: &[Target], on_step: impl FnMut(&CloseReport)) -> Result<CloseReport, String> {
    backend::close(targets, on_step)
}

/// Switch Edge to `target` and bring its window to the front. The error is a
/// sentence for the user, such as the tab having moved since the scan.
pub fn focus(target: &Target) -> Result<(), String> {
    backend::focus(target)
}
