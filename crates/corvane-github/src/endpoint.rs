//! GitHub.com vs GitHub Enterprise Server URL layout (`lib/api.ts getEndpointForRepository`).

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Endpoint {
    /// `https://github.com` or `https://ghe.corp`
    pub web_base: String,
    /// `https://api.github.com` or `https://ghe.corp/api/v3`
    pub api_base: String,
}

impl Endpoint {
    pub fn github_com() -> Self {
        Self {
            web_base: "https://github.com".into(),
            api_base: "https://api.github.com".into(),
        }
    }

    /// From a user-entered enterprise address (`ghe.corp`, `https://ghe.corp/`).
    pub fn enterprise(input: &str) -> Option<Self> {
        let trimmed = input.trim().trim_end_matches('/');
        let without_scheme = trimmed
            .strip_prefix("https://")
            .or_else(|| trimmed.strip_prefix("http://"))
            .unwrap_or(trimmed);
        let host = without_scheme.split('/').next()?.trim();
        if host.is_empty() || host.contains(char::is_whitespace) {
            return None;
        }
        if host.eq_ignore_ascii_case("github.com") || host.eq_ignore_ascii_case("api.github.com") {
            return Some(Self::github_com());
        }
        Some(Self {
            web_base: format!("https://{host}"),
            api_base: format!("https://{host}/api/v3"),
        })
    }

    pub fn from_api_base(api_base: &str) -> Self {
        if api_base == "https://api.github.com" {
            Self::github_com()
        } else {
            let web_base = api_base.trim_end_matches("/api/v3").to_string();
            Self {
                web_base,
                api_base: api_base.to_string(),
            }
        }
    }

    pub fn is_dotcom(&self) -> bool {
        self.api_base == "https://api.github.com"
    }

    pub fn host(&self) -> &str {
        self.web_base
            .trim_start_matches("https://")
            .trim_start_matches("http://")
    }

    pub fn api(&self, path: &str) -> String {
        format!("{}/{}", self.api_base, path.trim_start_matches('/'))
    }

    pub fn web(&self, path: &str) -> String {
        format!("{}/{}", self.web_base, path.trim_start_matches('/'))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enterprise_parsing() {
        let e = Endpoint::enterprise("https://ghe.corp/").unwrap();
        assert_eq!(e.web_base, "https://ghe.corp");
        assert_eq!(e.api_base, "https://ghe.corp/api/v3");
        assert_eq!(e.host(), "ghe.corp");
        assert_eq!(
            Endpoint::enterprise("github.com").unwrap(),
            Endpoint::github_com()
        );
        assert!(Endpoint::enterprise("   ").is_none());
    }

    #[test]
    fn round_trips_api_base() {
        let e = Endpoint::from_api_base("https://ghe.corp/api/v3");
        assert_eq!(e.web_base, "https://ghe.corp");
        assert!(Endpoint::from_api_base("https://api.github.com").is_dotcom());
        assert_eq!(
            Endpoint::github_com().api("user"),
            "https://api.github.com/user"
        );
    }
}
