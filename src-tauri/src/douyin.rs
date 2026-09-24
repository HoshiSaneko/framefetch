//! Native website login. Credentials remain in the dedicated WebView2 profile.
use serde::Serialize;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use tokio::sync::Mutex;

const LABEL: &str = "douyin-login";
const HOME: &str = "https://www.douyin.com/";
#[derive(Default)]
pub struct LoginGate(pub Mutex<Option<String>>);
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginStatus { session_present: bool }

#[derive(Serialize, serde::Deserialize)]
pub struct Profile { name: String, avatar: Option<String> }

#[tauri::command]
pub async fn douyin_profile(app: AppHandle, window: WebviewWindow, gate: tauri::State<'_, LoginGate>) -> Result<Profile, String> {
    main_only(&window)?;
    let _guard = gate.0.lock().await;
    let login = self::window(&app)?;
    if !has_session(&login)? { return Err("请先登录抖音".into()); }
    login.navigate("https://www.douyin.com/user/self".parse().unwrap()).map_err(|e| e.to_string())?;
    let result = async {
        for _ in 0..12 {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            let (tx, rx) = tokio::sync::oneshot::channel();
            let sender = std::sync::Mutex::new(Some(tx));
            login.eval_with_callback(include_str!("douyin_profile.js"), move |value| {
                if let Ok(mut slot) = sender.lock() { if let Some(tx) = slot.take() { let _ = tx.send(value); } }
            }).map_err(|e| e.to_string())?;
            let value = tokio::time::timeout(std::time::Duration::from_secs(2), rx).await.map_err(|_| "账号信息读取超时")?.map_err(|_| "登录窗口已关闭")?;
            if let Ok(mut profile) = serde_json::from_str::<Profile>(&value) {
                profile.avatar = profile.avatar.filter(|s| valid_avatar(s));
                return Ok(profile);
            }
            if value.contains("\"error\":true") { break; }
        }
        Err("暂时无法读取账号信息".into())
    }.await;
    let _ = login.navigate("about:blank".parse().unwrap());
    result
}
fn valid_avatar(value: &str) -> bool {
    url::Url::parse(value).is_ok_and(|u| u.scheme() == "https" && u.host_str().is_some_and(|host| {
        ["douyinpic.com", "byteimg.com", "ibyteimg.com", "douyinstatic.com"].iter().any(|domain| host == *domain || host.ends_with(&format!(".{domain}")))
    }))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QrStatus { logged_in: bool, image: Option<String> }

#[tauri::command]
pub async fn douyin_qr_start(app: AppHandle, window: WebviewWindow, gate: tauri::State<'_, LoginGate>) -> Result<String, String> {
    main_only(&window)?;
    let mut attempt = gate.0.lock().await;
    let login = self::window(&app)?;
    let id = uuid::Uuid::new_v4().to_string();
    login.hide().map_err(|e| e.to_string())?;
    login.navigate("https://www.douyin.com/user/self".parse().unwrap()).map_err(|e| e.to_string())?;
    *attempt = Some(id.clone());
    Ok(id)
}

#[tauri::command]
pub async fn douyin_qr_poll(app: AppHandle, window: WebviewWindow, gate: tauri::State<'_, LoginGate>, attempt_id: String) -> Result<QrStatus, String> {
    main_only(&window)?;
    let attempt = gate.0.lock().await;
    if attempt.as_deref() != Some(&attempt_id) { return Err("扫码会话已结束".into()); }
    let login = self::window(&app)?;
    if has_session(&login)? { return Ok(QrStatus { logged_in: true, image: None }); }
    let (tx, rx) = tokio::sync::oneshot::channel();
    let sender = std::sync::Mutex::new(Some(tx));
    login.eval_with_callback(include_str!("douyin_qr.js"), move |result| { if let Ok(mut slot) = sender.lock() { if let Some(tx) = slot.take() { let _ = tx.send(result); } } }).map_err(|e| e.to_string())?;
    let result = tokio::time::timeout(std::time::Duration::from_secs(5), rx).await.map_err(|_| "二维码读取超时，可打开官网登录窗口")?.map_err(|_| "登录窗口已关闭")?;
    let value: serde_json::Value = serde_json::from_str(&result).unwrap_or_default();
    let image = value.as_str().filter(|s| s.len() < 200_000 && s.starts_with("data:image/png;base64,")).map(str::to_owned);
    Ok(QrStatus { logged_in: false, image })
}

#[tauri::command]
pub async fn douyin_qr_cancel(app: AppHandle, window: WebviewWindow, gate: tauri::State<'_, LoginGate>, attempt_id: String) -> Result<(), String> {
    main_only(&window)?;
    let mut attempt = gate.0.lock().await;
    if attempt.as_deref() != Some(&attempt_id) { return Ok(()); }
    *attempt = None;
    if let Some(login) = app.get_webview_window(LABEL) {
        login.navigate("about:blank".parse().unwrap()).map_err(|e| e.to_string())?;
        login.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub(crate) fn main_only(caller: &WebviewWindow) -> Result<(), String> {
    if caller.label() == "main" { Ok(()) } else { Err("此窗口无权管理登录会话".into()) }
}
fn allowed_navigation(url: &url::Url) -> bool {
    url.as_str() == "about:blank" || (url.scheme() == "https" && url.host_str().is_some_and(|h| h == "douyin.com" || h.ends_with(".douyin.com")))
}
pub(crate) fn window(app: &AppHandle) -> Result<WebviewWindow, String> {
    if let Some(window) = app.get_webview_window(LABEL) { return Ok(window); }
    let directory = app.path().app_local_data_dir().map_err(|e| e.to_string())?.join("douyin-browser");
    std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    WebviewWindowBuilder::new(app, LABEL, WebviewUrl::External("about:blank".parse().unwrap()))
        .title("抖音登录 · FrameFetch").inner_size(1060.0, 760.0).min_inner_size(800.0, 600.0)
        .data_directory(directory).visible(false).on_navigation(allowed_navigation)
        .build().map_err(|e| format!("无法打开抖音登录窗口：{e}"))
}
pub(crate) fn has_session(window: &WebviewWindow) -> Result<bool, String> {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as i64;
    Ok(window.cookies_for_url(HOME.parse().unwrap()).map_err(|_| "无法读取抖音登录状态")?.iter().any(|c| {
        matches!(c.name(), "sessionid" | "sessionid_ss") && !c.value().is_empty()
            && c.expires_datetime().is_none_or(|expiry| expiry.unix_timestamp() > now)
    }))
}
#[tauri::command]
pub async fn douyin_login_open(app: AppHandle, window: WebviewWindow, gate: tauri::State<'_, LoginGate>) -> Result<(), String> {
    main_only(&window)?;
    let _guard = gate.0.lock().await;
    let login = self::window(&app)?;
    login.navigate(HOME.parse().unwrap()).map_err(|e| e.to_string())?;
    login.show().map_err(|e| e.to_string())?;
    login.set_focus().map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn douyin_login_status(app: AppHandle, window: WebviewWindow, gate: tauri::State<'_, LoginGate>) -> Result<LoginStatus, String> {
    main_only(&window)?;
    let _guard = gate.0.lock().await;
    Ok(LoginStatus { session_present: has_session(&self::window(&app)?)? })
}
#[tauri::command]
pub async fn douyin_login_finish(app: AppHandle, window: WebviewWindow, gate: tauri::State<'_, LoginGate>) -> Result<(), String> {
    main_only(&window)?;
    let _guard = gate.0.lock().await;
    let login = self::window(&app)?;
    if !has_session(&login)? { return Err("尚未检测到登录会话，请先在抖音窗口完成登录。".into()); }
    login.navigate("about:blank".parse().unwrap()).map_err(|e| e.to_string())?;
    login.hide().map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn douyin_logout(app: AppHandle, window: WebviewWindow, gate: tauri::State<'_, LoginGate>) -> Result<(), String> {
    main_only(&window)?;
    let _guard = gate.0.lock().await;
    let login = self::window(&app)?;
    // Stop the website before clearing its isolated profile to avoid refreshing the session.
    login.navigate("about:blank".parse().unwrap()).map_err(|e| e.to_string())?;
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
    fn login_navigation_stays_on_douyin() {
        for url in [HOME, "https://passport.douyin.com/login", "about:blank"] { assert!(allowed_navigation(&url.parse().unwrap())); }
        for url in ["http://www.douyin.com", "https://douyin.com.evil.test", "https://evildouyin.com", "file:///C:/secret", "https://localhost"] { assert!(!allowed_navigation(&url.parse().unwrap())); }
    }
}
