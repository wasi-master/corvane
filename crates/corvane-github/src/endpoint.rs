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
    /// Always HTTPS, as GHD since 3.4.7; with `allow_http` an explicit
    /// `http://` is kept (flag `enterprise-plain-http`, desktop/desktop#20245).
    pub fn enterprise(input: &str, allow_http: bool) -> Option<Self> {
        let trimmed = input.trim().trim_end_matches('/');
        let http = trimmed
            .get(..7)
            .is_some_and(|p| p.eq_ignore_ascii_case("http://"));
        let without_scheme = if http {
            &trimmed[7..]
        } else {
            trimmed
                .get(..8)
                .filter(|p| p.eq_ignore_ascii_case("https://"))
                .map_or(trimmed, |_| &trimmed[8..])
        };
        let host = without_scheme.split('/').next()?.trim();
        if host.is_empty() || host.contains(char::is_whitespace) {
            return None;
        }
        if host.eq_ignore_ascii_case("github.com") || host.eq_ignore_ascii_case("api.github.com") {
            return Some(Self::github_com());
        }
        let scheme = if http && allow_http { "http" } else { "https" };
        Some(Self {
            web_base: format!("{scheme}://{host}"),
            api_base: format!("{scheme}://{host}/api/v3"),
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
        let e = Endpoint::enterprise("https://ghe.corp/", false).unwrap();
        assert_eq!(e.web_base, "https://ghe.corp");
        assert_eq!(e.api_base, "https://ghe.corp/api/v3");
        assert_eq!(e.host(), "ghe.corp");
        assert_eq!(
            Endpoint::enterprise("github.com", false).unwrap(),
            Endpoint::github_com()
        );
        assert!(Endpoint::enterprise("   ", false).is_none());
        let plain = |allow| Endpoint::enterprise("HTTP://ghe.corp/", allow).unwrap();
        assert_eq!(plain(false).api_base, "https://ghe.corp/api/v3");
        assert_eq!(plain(true).web_base, "http://ghe.corp");
        assert_eq!(plain(true).api_base, "http://ghe.corp/api/v3");
        assert_eq!(
            Endpoint::enterprise("ghe.corp", true).unwrap().web_base,
            "https://ghe.corp"
        );
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
