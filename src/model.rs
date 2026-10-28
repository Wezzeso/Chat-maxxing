use std::time::Duration;
use url::Url;

pub const IDLE_DELAY: Duration = Duration::from_secs(10 * 60);

#[derive(Clone, Copy, Debug)]
pub struct Service {
    pub id: &'static str,
    pub name: &'static str,
    pub url: &'static str,
    pub origins: &'static [&'static str],
    pub auth_origins: &'static [&'static str],
}

pub static SERVICES: [Service; 5] = [
    Service {
        id: "whatsapp",
        name: "WhatsApp",
        url: "https://web.whatsapp.com/",
        origins: &["https://web.whatsapp.com"],
        auth_origins: &[],
    },
    Service {
        id: "telegram",
        name: "Telegram",
        url: "https://web.telegram.org/k/",
        origins: &["https://web.telegram.org"],
        auth_origins: &[],
    },
    Service {
        id: "instagram",
        name: "Instagram",
        url: "https://www.instagram.com/direct/inbox/",
        origins: &["https://www.instagram.com", "https://instagram.com"],
        auth_origins: &["https://www.facebook.com", "https://m.facebook.com"],
    },
    Service {
        id: "linkedin",
        name: "LinkedIn",
        url: "https://www.linkedin.com/messaging/",
        origins: &["https://www.linkedin.com", "https://linkedin.com"],
        auth_origins: &[],
    },
    Service {
        id: "gmail",
        name: "Gmail",
        url: "https://mail.google.com/mail/u/0/#inbox",
        origins: &["https://mail.google.com"],
        auth_origins: &["https://accounts.google.com"],
    },
];

// Restrict top-level navigation to ordinary HTTPS URLs. In particular, a URL
// containing a service name in its username is never one of that service's pages.
fn secure_uri(uri: &str) -> Option<Url> {
    if !uri.get(..8)?.eq_ignore_ascii_case("https://") {
        return None;
    }
    let authority = uri[8..].split(['/', '\\', '?', '#']).next()?;
    if authority.contains('@') {
        return None;
    }
    let parsed = Url::parse(uri).ok()?;
    if parsed.scheme() != "https"
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.port().is_some()
    {
        return None;
    }
    Some(parsed)
}

impl Service {
    pub fn owns_uri(&self, uri: &str) -> bool {
        secure_uri(uri)
            .map(|parsed| {
                self.origins
                    .contains(&parsed.origin().ascii_serialization().as_str())
            })
            .unwrap_or(false)
    }

    pub fn is_auth_uri(&self, uri: &str) -> bool {
        secure_uri(uri)
            .map(|parsed| {
                self.auth_origins
                    .contains(&parsed.origin().ascii_serialization().as_str())
            })
            .unwrap_or(false)
    }

    pub fn is_allowed_navigation(&self, uri: &str) -> bool {
        secure_uri(uri).is_some()
    }
}

/// The caller marks authentication and external navigation as protected and
/// requests WebKit's ordinary before-unload check before destroying a view.
#[allow(clippy::too_many_arguments)]
pub fn should_sleep(
    active: bool,
    keep_awake: bool,
    loading: bool,
    audible: bool,
    capturing: bool,
    auth: bool,
    elapsed: Duration,
) -> bool {
    !(active || keep_awake || loading || audible || capturing || auth) && elapsed >= IDLE_DELAY
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_inactive_unprotected_views_sleep_after_ten_minutes() {
        for elapsed in [29, 30, 35, 599] {
            assert!(!should_sleep(
                false,
                false,
                false,
                false,
                false,
                false,
                Duration::from_secs(elapsed)
            ));
        }
        assert!(should_sleep(
            false,
            false,
            false,
            false,
            false,
            false,
            Duration::from_secs(600)
        ));
        for protected in 0..6 {
            let mut flags = [false; 6];
            flags[protected] = true;
            assert!(!should_sleep(
                flags[0],
                flags[1],
                flags[2],
                flags[3],
                flags[4],
                flags[5],
                Duration::from_secs(600)
            ));
        }
    }

    #[test]
    fn each_service_accepts_its_own_origin_and_auth_stays_separate() {
        for service in SERVICES {
            assert!(service.owns_uri(service.url));
            assert!(!service.is_auth_uri(service.url));
        }
        let gmail = &SERVICES[4];
        assert!(gmail.is_auth_uri("https://accounts.google.com/v3/signin/identifier"));
        assert!(!gmail.owns_uri("https://accounts.google.com/v3/signin/identifier"));
        assert!(gmail.owns_uri("HTTPS://MAIL.GOOGLE.COM:443/mail/u/0/#sent"));
    }

    #[test]
    fn service_origins_reject_lookalikes_userinfo_and_nonstandard_ports() {
        for uri in [
            "http://web.whatsapp.com/",
            "https://web.whatsapp.com.evil.test/",
            "https://web.whatsapp.com@evil.test/",
            "https://@web.whatsapp.com/",
            "https://evil.test@web.whatsapp.com/",
            "https://web.whatsapp.com:444/",
            "https://web.whatsapp.com:80/",
            "https://web.whatsаpp.com/", // Cyrillic a.
            "javascript:alert(1)",
            "file:///tmp/messages.html",
            "about:blank",
            "not a uri",
        ] {
            assert!(!SERVICES[0].owns_uri(uri), "{uri}");
        }
    }

    #[test]
    fn external_https_links_are_allowed_without_claiming_their_origin() {
        let service = &SERVICES[0];
        let uri = "https://example.org/article?next=https://web.whatsapp.com/";
        assert!(service.is_allowed_navigation(uri));
        assert!(!service.owns_uri(uri));
        assert!(!service.is_auth_uri(uri));
        for uri in [
            "data:text/html,hello",
            "javascript:alert(1)",
            "http://example.org/",
            "https://user:password@example.org/",
        ] {
            assert!(!service.is_allowed_navigation(uri));
        }
    }
}
