//! Platform directory helpers.
//!
//! Provides XDG-compatible data and config directory resolution without
//! external dependencies. Matches the behaviour of the `dirs` crate:
//!
//! | Platform | Config dir                        | Data dir                          |
//! |----------|-----------------------------------|-----------------------------------|
//! | Linux    | `$XDG_CONFIG_HOME` or `~/.config` | `$XDG_DATA_HOME` or `~/.local/share` |
//! | macOS    | `~/Library/Application Support`   | `~/Library/Application Support`   |

use std::path::PathBuf;

/// Returns the user's home directory, or `None` if it cannot be determined.
fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

/// Returns the platform config directory.
///
/// - **Linux:** `$XDG_CONFIG_HOME` or `~/.config`
/// - **macOS:** `~/Library/Application Support`
pub fn config_dir() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        home_dir().map(|h| h.join("Library/Application Support"))
    }
    #[cfg(not(target_os = "macos"))]
    {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| home_dir().map(|h| h.join(".config")))
    }
}

/// Returns the platform data directory.
///
/// - **Linux:** `$XDG_DATA_HOME` or `~/.local/share`
/// - **macOS:** `~/Library/Application Support`
pub fn data_dir() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        home_dir().map(|h| h.join("Library/Application Support"))
    }
    #[cfg(not(target_os = "macos"))]
    {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| home_dir().map(|h| h.join(".local/share")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_dir_returns_some_when_home_is_set() {
        // HOME is always set in test environments
        let dir = config_dir();
        assert!(dir.is_some(), "config_dir should return Some");
        assert!(dir.unwrap().is_absolute(), "config_dir should be absolute");
    }

    #[test]
    fn data_dir_returns_some_when_home_is_set() {
        let dir = data_dir();
        assert!(dir.is_some(), "data_dir should return Some");
        assert!(dir.unwrap().is_absolute(), "data_dir should be absolute");
    }

    #[test]
    fn config_and_data_dirs_live_under_home() {
        let home = home_dir().expect("HOME should be set in tests");
        let config = config_dir().unwrap();
        let data = data_dir().unwrap();
        assert!(
            config.starts_with(&home),
            "config_dir {config:?} should be under HOME {home:?}"
        );
        assert!(
            data.starts_with(&home),
            "data_dir {data:?} should be under HOME {home:?}"
        );
    }

    #[test]
    fn config_and_data_dirs_are_distinct_from_home() {
        let home = home_dir().unwrap();
        let config = config_dir().unwrap();
        let data = data_dir().unwrap();
        assert_ne!(config, home, "config_dir should not be HOME itself");
        assert_ne!(data, home, "data_dir should not be HOME itself");
    }
}
