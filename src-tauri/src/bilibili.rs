//! Native website login with an isolated WebView2 profile.
//! Downloads export narrowly scoped, temporary cookies; see bilibili_download.
use serde::Serialize;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use tokio::sync::Mutex;

const LABEL: &str = "bilibili-login";
const HOME: &str = "https://www.bilibili.com/";
#[derive(Default)]
pub struct LoginGate(pub Mutex<Option<String>>);
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginStatus {
    session_present: bool,
}

#[derive(Serialize, serde::Deserialize)]
pub struct Profile {
    name: String,
    avatar: Option<String>,
}

#[tauri::command]
pub async fn bilibili_profile(app: AppHandle, window: WebviewWindow) -> Result<Profile, String> {
    main_only(&window)?;
    let cookie = cookie_header(&self::window(&app)?)?;
    let text = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| e.to_string())?
        .get("https://api.bilibili.com/x/web-interface/nav")
        .header("Cookie", cookie)
        .header("Referer", HOME)
        .send()
        .await
        .map_err(|_| "账号信息读取失败")?
        .text()
        .await
        .map_err(|_| "账号信息读取失败")?;
    let v: serde_json::Value = serde_json::from_str(&text).map_err(|_| "账号信息无效")?;
    if v["data"]["isLogin"] != true {
        return Err("Bilibili 登录已失效，请重新连接".into());
    }
    Ok(Profile {
        name: v["data"]["uname"]
            .as_str()
            .unwrap_or("Bilibili 账号")
            .into(),
        avatar: v["data"]["face"]
            .as_str()
            .filter(|s| valid_avatar(s))
            .map(str::to_owned),
    })
}
fn valid_avatar(value: &str) -> bool {
    url::Url::parse(value).is_ok_and(|u| {
        u.scheme() == "https"
            && u.host_str()
                .is_some_and(|h| h == "hdslb.com" || h.ends_with(".hdslb.com"))
    })
}
pub(crate) fn cookie_header(window: &WebviewWindow) -> Result<String, String> {
    Ok(window
        .cookies_for_url(HOME.parse().unwrap())
        .map_err(|_| "无法读取 Bilibili 会话")?
        .iter()
        .filter(|c| !c.name().contains(['\r', '\n', ';']) && !c.value().contains(['\r', '\n', ';']))
        .map(|c| format!("{}={}", c.name(), c.value()))
        .collect::<Vec<_>>()
        .join("; "))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QrStatus {
    logged_in: bool,
    image: Option<String>,
}

#[tauri::command]
pub async fn bilibili_qr_start(
    app: AppHandle,
    window: WebviewWindow,
    gate: tauri::State<'_, LoginGate>,
) -> Result<String, String> {
    main_only(&window)?;
    let mut attempt = gate.0.lock().await;
    let login = self::window(&app)?;
    if has_session(&login)? {
        if let Err(error) = bilibili_profile(app.clone(), window.clone()).await {
            if error.contains("已失效") {
                login.clear_all_browsing_data().map_err(|e| e.to_string())?;
            }
        }
    }
    let id = uuid::Uuid::new_v4().to_string();
    login.hide().map_err(|e| e.to_string())?;
    login
        .navigate("https://passport.bilibili.com/login".parse().unwrap())
        .map_err(|e| e.to_string())?;
    *attempt = Some(id.clone());
    Ok(id)
}

#[tauri::command]
pub async fn bilibili_qr_poll(
    app: AppHandle,
    window: WebviewWindow,
    gate: tauri::State<'_, LoginGate>,
    attempt_id: String,
) -> Result<QrStatus, String> {
    main_only(&window)?;
    let attempt = gate.0.lock().await;
    if attempt.as_deref() != Some(&attempt_id) {
        return Err("扫码会话已结束".into());
    }
    let login = self::window(&app)?;
    if has_session(&login)? && bilibili_profile(app.clone(), window.clone()).await.is_ok() {
        return Ok(QrStatus {
            logged_in: true,
            image: None,
        });
    }
    let (tx, rx) = tokio::sync::oneshot::channel();
    let sender = std::sync::Mutex::new(Some(tx));
    login
        .eval_with_callback(include_str!("bilibili_qr.js"), move |result| {
            if let Ok(mut slot) = sender.lock() {
                if let Some(tx) = slot.take() {
                    let _ = tx.send(result);
                }
            }
        })
        .map_err(|e| e.to_string())?;
    let result = tokio::time::timeout(std::time::Duration::from_secs(5), rx)
        .await
        .map_err(|_| "二维码读取超时，可打开官网登录窗口")?
        .map_err(|_| "登录窗口已关闭")?;
    let value: serde_json::Value = serde_json::from_str(&result).unwrap_or_default();
    let image = value
        .as_str()
        .filter(|s| s.len() < 200_000 && s.starts_with("data:image/png;base64,"))
        .map(str::to_owned);
    Ok(QrStatus {
        logged_in: false,
        image,
    })
}

#[tauri::command]
pub async fn bilibili_qr_cancel(
    app: AppHandle,
    window: WebviewWindow,
    gate: tauri::State<'_, LoginGate>,
    attempt_id: String,
) -> Result<(), String> {
    main_only(&window)?;
    let mut attempt = gate.0.lock().await;
    if attempt.as_deref() != Some(&attempt_id) {
        return Ok(());
    }
    *attempt = None;
    if let Some(login) = app.get_webview_window(LABEL) {
        login
            .navigate("about:blank".parse().unwrap())
            .map_err(|e| e.to_string())?;
        login.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub(crate) fn main_only(caller: &WebviewWindow) -> Result<(), String> {
    if caller.label() == "main" {
        Ok(())
    } else {
        Err("此窗口无权管理登录会话".into())
    }
}
fn allowed_navigation(url: &url::Url) -> bool {
    url.as_str() == "about:blank"
        || (url.scheme() == "https"
            && url
                .host_str()
                .is_some_and(|h| h == "bilibili.com" || h.ends_with(".bilibili.com")))
}
pub(crate) fn window(app: &AppHandle) -> Result<WebviewWindow, String> {
    if let Some(window) = app.get_webview_window(LABEL) {
        return Ok(window);
    }
    let directory = app
        .path()
        .app_local_data_dir()
        .map_err(|e| e.to_string())?
        .join("bilibili-browser");
    std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    WebviewWindowBuilder::new(
        app,
        LABEL,
        WebviewUrl::External("about:blank".parse().unwrap()),
    )
    .title("哔哩哔哩登录 · FrameFetch")
    .inner_size(1060.0, 760.0)
    .min_inner_size(800.0, 600.0)
    .data_directory(directory)
    .visible(false)
    .on_navigation(allowed_navigation)
    .build()
    .map_err(|e| format!("无法打开哔哩哔哩登录窗口：{e}"))
}
pub(crate) fn has_session(window: &WebviewWindow) -> Result<bool, String> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    Ok(window
        .cookies_for_url(HOME.parse().unwrap())
        .map_err(|_| "无法读取哔哩哔哩登录状态")?
        .iter()
        .any(|c| {
            c.name() == "SESSDATA"
                && !c.value().is_empty()
                && c.expires_datetime()
                    .is_none_or(|expiry| expiry.unix_timestamp() > now)
        }))
}
#[tauri::command]
pub async fn bilibili_login_open(
    app: AppHandle,
    window: WebviewWindow,
    gate: tauri::State<'_, LoginGate>,
) -> Result<(), String> {
    main_only(&window)?;
    let _guard = gate.0.lock().await;
    let login = self::window(&app)?;
    login
        .navigate("https://passport.bilibili.com/login".parse().unwrap())
        .map_err(|e| e.to_string())?;
    login.show().map_err(|e| e.to_string())?;
    login.set_focus().map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn bilibili_login_status(
    app: AppHandle,
    window: WebviewWindow,
    gate: tauri::State<'_, LoginGate>,
) -> Result<LoginStatus, String> {
    main_only(&window)?;
    let _guard = gate.0.lock().await;
    Ok(LoginStatus {
        session_present: has_session(&self::window(&app)?)?,
    })
}
#[tauri::command]
pub async fn bilibili_login_finish(
    app: AppHandle,
    window: WebviewWindow,
    gate: tauri::State<'_, LoginGate>,
) -> Result<(), String> {
    main_only(&window)?;
    let _guard = gate.0.lock().await;
    let login = self::window(&app)?;
    if !has_session(&login)? {
        return Err("尚未检测到登录会话，请先在哔哩哔哩窗口完成登录。".into());
    }
    login
        .navigate("about:blank".parse().unwrap())
        .map_err(|e| e.to_string())?;
    login.hide().map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn bilibili_logout(
    app: AppHandle,
    window: WebviewWindow,
    gate: tauri::State<'_, LoginGate>,
) -> Result<(), String> {
    main_only(&window)?;
    let _guard = gate.0.lock().await;
    let login = self::window(&app)?;
    let engine = app.state::<crate::engine::Shared>();
    let running = engine.0.running.lock().await;
    if engine
        .0
        .data
        .lock()
        .await
        .tasks
        .iter()
        .any(|t| t.platform == "bilibili" && running.contains_key(&t.id))
    {
        return Err("请先暂停 Bilibili 下载，再断开连接".into());
    }
    // Stop the website before clearing its isolated profile to avoid refreshing the session.
    login
        .navigate("about:blank".parse().unwrap())
        .map_err(|e| e.to_string())?;
    login.clear_all_browsing_data().map_err(|e| e.to_string())?;
    for cookie in login.cookies().map_err(|e| e.to_string())? {
        login.delete_cookie(cookie).map_err(|e| e.to_string())?;
    }
    login.hide().map_err(|e| e.to_string())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn login_navigation_stays_on_bilibili() {
        for url in [HOME, "https://passport.bilibili.com/login", "about:blank"] {
            assert!(allowed_navigation(&url.parse().unwrap()));
        }
        for url in [
            "http://www.bilibili.com",
            "https://bilibili.com.evil.test",
            "https://evilbilibili.com",
            "file:///C:/secret",
            "https://localhost",
        ] {
            assert!(!allowed_navigation(&url.parse().unwrap()));
        }
    }
}

// Remove only stale, UUID-named credential exports left by a crashed prior run.
pub fn clean_stale_exports(app: &AppHandle) -> Result<(), String> {
    let dir = app
        .path()
        .app_local_data_dir()
        .map_err(|e| e.to_string())?
        .join("bilibili-private");
    if !dir.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if entry.file_type().map_err(|e| e.to_string())?.is_file()
            && name
                .strip_prefix("cookies-")
                .and_then(|s| s.strip_suffix(".txt"))
                .is_some_and(|s| uuid::Uuid::parse_str(s).is_ok())
        {
            std::fs::remove_file(entry.path()).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
