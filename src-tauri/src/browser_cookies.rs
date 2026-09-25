use tauri::{webview::Cookie, WebviewWindow};

/// Wry 0.55's WKWebView URL filter compares domains literally, excluding
/// parent-domain login cookies (douyin.com for www.douyin.com).
/// Keep native filtering on other platforms; scope the macOS store explicitly.
pub fn for_url(window: &WebviewWindow, url: url::Url) -> tauri::Result<Vec<Cookie<'static>>> {
    #[cfg(target_os = "macos")]
    {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as i64;
        Ok(window.cookies()?.into_iter().filter(|cookie| matches_url(cookie, &url, now)).collect())
    }
    #[cfg(not(target_os = "macos"))]
    window.cookies_for_url(url)
}

#[cfg(any(target_os = "macos", test))]
fn matches_url(cookie: &Cookie<'_>, url: &url::Url, now: i64) -> bool {
    let Some(host) = url.host_str() else { return false; };
    let Some(domain) = cookie.domain() else { return false; };
    let domain = domain.trim_start_matches('.').to_ascii_lowercase();
    let host = host.to_ascii_lowercase();
    if domain.is_empty() || !(host == domain || host.ends_with(&format!(".{domain}"))) {
        return false;
    }
    if cookie.secure().unwrap_or(false) && url.scheme() != "https" { return false; }
    if cookie.expires_datetime().is_some_and(|expiry| expiry.unix_timestamp() <= now) { return false; }
    let path = cookie.path().filter(|p| p.starts_with('/')).unwrap_or("/");
    let request_path = url.path();
    request_path == path || (request_path.starts_with(path)
        && (path.ends_with('/') || request_path.as_bytes().get(path.len()) == Some(&b'/')))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepts_parent_domain_login_cookies_but_not_other_hosts() {
        let cookie = Cookie::build(("sessionid", "test")).domain(".douyin.com").path("/").secure(true).build();
        for address in ["https://www.douyin.com/user/self", "https://douyin.com/"] {
            assert!(matches_url(&cookie, &address.parse().unwrap(), 100));
        }
        for address in ["https://evildouyin.com/", "https://douyin.com.evil.test/", "https://www.bilibili.com/", "http://www.douyin.com/"] {
            assert!(!matches_url(&cookie, &address.parse().unwrap(), 100));
        }
    }
    #[test]
    fn respects_path_expiry_and_missing_domain() {
        let url = "https://www.douyin.com/user/self".parse().unwrap();
        let mut cookie = Cookie::build(("sessionid", "test")).domain("www.douyin.com").path("/user").build();
        assert!(matches_url(&cookie, &url, 100));
        assert!(!matches_url(&cookie, &"https://www.douyin.com/username".parse().unwrap(), 100));
        cookie.set_expires(tauri::webview::cookie::time::OffsetDateTime::from_unix_timestamp(99).unwrap());
        assert!(!matches_url(&cookie, &url, 100));
        assert!(!matches_url(&Cookie::new("sessionid", "test"), &url, 100));
    }
}
