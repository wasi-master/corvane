//! OS integration. macOS first; every function here is the seam for
//! Windows/Linux later.

pub mod apps;
pub mod editors;
pub mod keychain;
pub mod shells;
pub mod trash;

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

    /// GitHub Desktop's default clone location: `~/Documents/GitHub`.
    pub fn default_clone_dir() -> PathBuf {
        dirs::document_dir()
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| PathBuf::from("."))
            .join("GitHub")
    }

    /// Candidate clone locations offered during onboarding, existing ones only
    /// except the GHD default which is always offered.
    pub fn clone_dir_candidates() -> Vec<PathBuf> {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let mut out = vec![default_clone_dir()];
        for candidate in [home.join("Developer"), home.join("Work")] {
            if candidate.is_dir() {
                out.push(candidate);
            }
        }
        out
    }
}
