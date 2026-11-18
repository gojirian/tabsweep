//! The few things that differ by operating system and are not about the
//! browser: where Tab Sweep keeps its files, which program opens a text file,
//! and what the reopen-tab shortcut is called.
//!
//! Nothing here names a real folder. Every path is built from the
//! environment variables each platform sets for the purpose.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The folder name used under each base folder.
const APP_DIR: &str = "tabsweep";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    Windows,
    MacOs,
    /// Linux and the other Unix systems, which follow the XDG folders.
    Other,
}

impl Platform {
    pub const CURRENT: Self = if cfg!(windows) {
        Self::Windows
    } else if cfg!(target_os = "macos") {
        Self::MacOs
    } else {
        Self::Other
    };
}

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// Where files the user edits live, such as `rules.txt`.
#[must_use]
pub fn config_dir() -> Option<PathBuf> {
    config_dir_for(Platform::CURRENT, &env_path)
}

/// Where files Tab Sweep writes for its own use live. They can be deleted at
/// any time.
// Only a browser backend writes working files, and only Windows has one so far.
#[cfg_attr(not(windows), allow(dead_code))]
#[must_use]
pub fn cache_dir() -> Option<PathBuf> {
    cache_dir_for(Platform::CURRENT, &env_path)
}

fn config_dir_for(platform: Platform, env: &dyn Fn(&str) -> Option<PathBuf>) -> Option<PathBuf> {
    let base = match platform {
        Platform::Windows => env("APPDATA")?,
        Platform::MacOs => env("HOME")?.join("Library").join("Application Support"),
        Platform::Other => match env("XDG_CONFIG_HOME") {
            Some(dir) => dir,
            None => env("HOME")?.join(".config"),
        },
    };
    Some(base.join(APP_DIR))
}

#[cfg_attr(not(windows), allow(dead_code))]
fn cache_dir_for(platform: Platform, env: &dyn Fn(&str) -> Option<PathBuf>) -> Option<PathBuf> {
    let base = match platform {
        Platform::Windows => env("LOCALAPPDATA")?,
        Platform::MacOs => env("HOME")?.join("Library").join("Caches"),
        Platform::Other => match env("XDG_CACHE_HOME") {
            Some(dir) => dir,
            None => env("HOME")?.join(".cache"),
        },
    };
    Some(base.join(APP_DIR))
}

/// Open a text file in the platform's plain text editor. Nothing happens if
/// the editor cannot be started.
pub fn open_in_text_editor(path: &Path) {
    let mut command = match Platform::CURRENT {
        Platform::Windows => Command::new("notepad.exe"),
        Platform::MacOs => {
            let mut open = Command::new("open");
            open.arg("-t");
            open
        }
        Platform::Other => Command::new("xdg-open"),
    };
    let _ = command.arg(path).spawn();
}

/// The browser shortcut that brings back the last closed tab.
#[must_use]
pub fn reopen_tab_shortcut() -> &'static str {
    match Platform::CURRENT {
        Platform::MacOs => "Cmd+Shift+T",
        Platform::Windows | Platform::Other => "Ctrl+Shift+T",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A made-up environment, so the tests do not depend on the machine.
    fn env(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<PathBuf> {
        move |name| {
            pairs
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| PathBuf::from(value))
        }
    }

    #[test]
    fn windows_uses_the_roaming_and_local_app_data_folders() {
        let env = env(&[("APPDATA", "roaming"), ("LOCALAPPDATA", "local")]);
        assert_eq!(
            config_dir_for(Platform::Windows, &env),
            Some(Path::new("roaming").join("tabsweep"))
        );
        assert_eq!(
            cache_dir_for(Platform::Windows, &env),
            Some(Path::new("local").join("tabsweep"))
        );
    }

    #[test]
    fn macos_uses_the_library_folders_under_home() {
        let env = env(&[("HOME", "home")]);
        assert_eq!(
            config_dir_for(Platform::MacOs, &env),
            Some(Path::new("home/Library/Application Support/tabsweep").to_path_buf())
        );
        assert_eq!(
            cache_dir_for(Platform::MacOs, &env),
            Some(Path::new("home/Library/Caches/tabsweep").to_path_buf())
        );
    }

    #[test]
    fn other_systems_use_xdg_folders_and_fall_back_to_home() {
        let xdg = env(&[("XDG_CONFIG_HOME", "config"), ("XDG_CACHE_HOME", "cache")]);
        assert_eq!(
            config_dir_for(Platform::Other, &xdg),
            Some(Path::new("config").join("tabsweep"))
        );
        assert_eq!(
            cache_dir_for(Platform::Other, &xdg),
            Some(Path::new("cache").join("tabsweep"))
        );

        let home = env(&[("HOME", "home")]);
        assert_eq!(
            config_dir_for(Platform::Other, &home),
            Some(Path::new("home/.config/tabsweep").to_path_buf())
        );
        assert_eq!(
            cache_dir_for(Platform::Other, &home),
            Some(Path::new("home/.cache/tabsweep").to_path_buf())
        );
    }

    #[test]
    fn a_missing_variable_gives_no_folder_rather_than_a_guess() {
        let empty = env(&[]);
        for platform in [Platform::Windows, Platform::MacOs, Platform::Other] {
            assert_eq!(config_dir_for(platform, &empty), None);
            assert_eq!(cache_dir_for(platform, &empty), None);
        }
    }
}
