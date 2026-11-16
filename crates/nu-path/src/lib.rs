#![doc = include_str!("../README.md")]
mod assert_path_eq;
mod components;
pub mod dots;
pub mod expansions;
pub mod form;
mod helpers;
mod path;
mod tilde;
mod trailing_slash;

pub use components::components;
pub use expansions::{
    canonicalize_with, expand_path, expand_path_with, expand_to_real_path, locate_in_dirs,
};
pub use helpers::{cache_dir, data_dir, home_dir, nu_config_dir};
pub use path::*;
pub use tilde::expand_tilde;
pub use trailing_slash::{has_trailing_slash, strip_trailing_slash};

/// Recognize Windows device namespace paths without querying the filesystem.
pub fn is_windows_device_path(path: impl AsRef<std::path::Path>) -> bool {
    #[cfg(windows)]
    {
        let text = path.as_ref().as_os_str().to_string_lossy();
        text.starts_with(r"\\.\") || text.starts_with(r"\\?\")
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        false
    }
}

#[cfg(test)]
mod device_tests {
    #[test]
    fn device_paths_are_platform_specific() {
        assert_eq!(super::is_windows_device_path(r"\\.\NUL"), cfg!(windows));
        assert_eq!(super::is_windows_device_path(r"\\?\C:\file"), cfg!(windows));
        assert!(!super::is_windows_device_path("ordinary.txt"));
    }
}
