//! OS integration. macOS first; every function here is the seam for
//! Windows/Linux later.

pub mod paths {
    use std::path::PathBuf;

    pub const APP_NAME: &str = "Corvane";

    /// `~/Library/Application Support/Corvane`
    pub fn app_support_dir() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(APP_NAME)
    }

    /// `~/Library/Logs/Corvane`
    pub fn logs_dir() -> PathBuf {
        #[cfg(target_os = "macos")]
        {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("Library/Logs")
                .join(APP_NAME)
        }
        #[cfg(not(target_os = "macos"))]
        {
            app_support_dir().join("logs")
        }
    }

    /// `~/Library/Caches/Corvane`
    pub fn cache_dir() -> PathBuf {
        dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(APP_NAME)
    }
}
