//! The backend for systems Tab Sweep cannot drive Edge on yet. The program
//! builds and opens there, and says this instead of listing tabs.

use super::{CloseReport, RawTab, Target};

const NOT_YET: &str = "Tab Sweep can only read Edge's tabs on Windows so far.";

pub fn scan() -> Result<Vec<RawTab>, String> {
    Err(NOT_YET.to_string())
}

pub fn close(
    _targets: &[Target],
    _on_step: impl FnMut(&CloseReport),
) -> Result<CloseReport, String> {
    Err(NOT_YET.to_string())
}

pub fn focus(_target: &Target) -> Result<(), String> {
    Err(NOT_YET.to_string())
}
