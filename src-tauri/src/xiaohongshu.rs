//! Official website login in a platform-isolated WebView profile.
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use tokio::sync::Mutex;

const HOME: &str = "https://www.xiaohongshu.com/explore";
const LABEL: &str = "xiaohongshu-login";
#[derive(Default)]
pub struct LoginGate(pub Mutex<Option<String>>);
#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginStatus {
    pub session_present: bool,
    pub name: Option<String>,
    pub avatar: Option<String>,
}
pub(crate) fn main_only(window: &WebviewWindow) -> Result<(), String> {
    crate::bilibili::main_only(window)
}
fn allowed_navigation(u: &url::Url) -> bool {
    u.as_str() == "about:blank"
        || (u.scheme() == "https"
            && u.username().is_empty()
            && u.password().is_none()
            && u.port().is_none()
            && u.host_str()
                .is_some_and(|h| h == "xiaohongshu.com" || h.ends_with(".xiaohongshu.com")))
}
pub(crate) fn window(app: &AppHandle) -> Result<WebviewWindow, String> {
    if let Some(w) = app.get_webview_window(LABEL) {
        return Ok(w);
    }
    let directory = app
        .path()
        .app_local_data_dir()
        .map_err(|e| e.to_string())?
        .join("xiaohongshu-browser");
    std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    WebviewWindowBuilder::new(
        app,
        LABEL,
        WebviewUrl::External("about:blank".parse().unwrap()),
    )
    .title("小红书登录 · FrameFetch")
    .inner_size(1060., 760.)
    .min_inner_size(800., 600.)
    .data_directory(directory)
    .visible(false)
    .on_navigation(allowed_navigation)
    .build()
    .map_err(|_| "无法打开小红书登录窗口".into())
}
pub(crate) fn cookie_header(app: &AppHandle) -> Result<String, String> {
    Ok(window(app)?
        .cookies_for_url(HOME.parse().unwrap())
        .map_err(|_| "无法读取小红书会话")?
        .iter()
        .filter(|c| !c.name().contains(['\r', '\n', ';']) && !c.value().contains(['\r', '\n', ';']))
        .map(|c| format!("{}={}", c.name(), c.value()))
        .collect::<Vec<_>>()
        .join("; "))
}
async fn read_status(login: &WebviewWindow) -> Result<Option<LoginStatus>, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let sender = std::sync::Mutex::new(Some(tx));
    login
        .eval_with_callback(include_str!("xiaohongshu_session.js"), move |value| {
            if let Ok(mut slot) = sender.lock() {
                if let Some(tx) = slot.take() {
                    let _ = tx.send(value);
                }
            }
        })
        .map_err(|_| "无法读取小红书登录状态")?;
    let raw = tokio::time::timeout(std::time::Duration::from_secs(3), rx)
        .await
        .map_err(|_| "小红书登录状态读取超时")?
        .map_err(|_| "登录窗口已关闭")?;
    let mut value: Option<LoginStatus> =
        serde_json::from_str(&raw).map_err(|_| "登录状态读取失败")?;
    if let Some(s) = &mut value {
        s.avatar = s
            .avatar
            .take()
            .and_then(|u| crate::xiaohongshu_download::media_url(&u));
        s.name = s.name.take().map(|s| s.chars().take(80).collect());
    }
    Ok(value)
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QrStatus {
    logged_in: bool,
    image: Option<String>,
    profile: Option<LoginStatus>,
}
#[tauri::command]
pub async fn xiaohongshu_qr_start(
    app: AppHandle,
    window: WebviewWindow,
    gate: tauri::State<'_, LoginGate>,
) -> Result<String, String> {
    main_only(&window)?;
    let mut attempt = gate.0.lock().await;
    let login = self::window(&app)?;
    login.hide().map_err(|e| e.to_string())?;
    login
        .navigate("https://www.xiaohongshu.com/login".parse().unwrap())
        .map_err(|e| e.to_string())?;
    let id = uuid::Uuid::new_v4().to_string();
    *attempt = Some(id.clone());
    Ok(id)
}
#[tauri::command]
pub async fn xiaohongshu_qr_poll(
    app: AppHandle,
    window: WebviewWindow,
    gate: tauri::State<'_, LoginGate>,
    attempt_id: String,
) -> Result<QrStatus, String> {
    main_only(&window)?;
    let mut attempt = gate.0.lock().await;
    if attempt.as_deref() != Some(&attempt_id) {
        return Err("扫码会话已结束".into());
    }
    let login = self::window(&app)?;
    if let Some(profile) = read_status(&login).await?.filter(|s| s.session_present) {
        login.hide().map_err(|e| e.to_string())?;
        *attempt = None;
        return Ok(QrStatus {
            logged_in: true,
            image: None,
            profile: Some(profile),
        });
    }
    let (tx, rx) = tokio::sync::oneshot::channel();
    let sender = std::sync::Mutex::new(Some(tx));
    login
        .eval_with_callback(include_str!("xiaohongshu_qr.js"), move |raw| {
            if let Ok(mut slot) = sender.lock() {
                if let Some(tx) = slot.take() {
                    let _ = tx.send(raw);
                }
            }
        })
        .map_err(|_| "二维码读取失败")?;
    let raw = tokio::time::timeout(std::time::Duration::from_secs(3), rx)
        .await
        .map_err(|_| "二维码读取超时，请刷新重试")?
        .map_err(|_| "扫码会话已关闭")?;
    let value: serde_json::Value = serde_json::from_str(&raw).unwrap_or_default();
    let image = value
        .as_str()
        .filter(|s| s.len() < 200_000 && s.starts_with("data:image/png;base64,"))
        .map(str::to_owned);
    Ok(QrStatus {
        logged_in: false,
        image,
        profile: None,
    })
}
#[tauri::command]
pub async fn xiaohongshu_qr_cancel(
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
// Fall back to the signed-in official browser when the HTTP page requires browser verification.
pub(crate) async fn note(app: &AppHandle, url: &str) -> Result<serde_json::Value, String> {
    let gate = app.state::<LoginGate>();
    let attempt = gate.0.lock().await;
    if attempt.is_some() {
        return Err("请先完成或关闭小红书扫码登录，再解析作品".into());
    }
    let login = self::window(app)?;
    if login.is_visible().unwrap_or(true) {
        return Err("请先在小红书官网完成登录，关闭官网窗口后重新解析".into());
    }
    let parsed = url::Url::parse(url).map_err(|_| "作品链接无效")?;
    let id = parsed
        .path_segments()
        .and_then(|mut s| s.next_back())
        .ok_or("作品编号无效")?;
    let encoded = serde_json::to_string(id).map_err(|e| e.to_string())?;
    let script = include_str!("xiaohongshu_note.js").replace("__NOTE_ID__", &encoded);
    login.navigate(parsed).map_err(|e| e.to_string())?;
    let result = async {
        for _ in 0..18 {
            tokio::time::sleep(std::time::Duration::from_millis(700)).await;
            let (tx, rx) = tokio::sync::oneshot::channel();
            let sender = std::sync::Mutex::new(Some(tx));
            login
                .eval_with_callback(&script, move |value| {
                    if let Ok(mut slot) = sender.lock() {
                        if let Some(tx) = slot.take() {
                            let _ = tx.send(value);
                        }
                    }
                })
                .map_err(|_| "官网笔记读取失败")?;
            let raw = tokio::time::timeout(std::time::Duration::from_secs(2), rx)
                .await
                .map_err(|_| "官网读取超时")?
                .map_err(|_| "官网窗口已关闭")?;
            if raw.len() > 8_000_000 {
                return Err("笔记数据过大".into());
            }
            let value: serde_json::Value = serde_json::from_str(&raw).unwrap_or_default();
            if value["error"].as_bool() == Some(true) {
                break;
            }
            if value["note"].is_object() {
                return Ok(value);
            }
        }
        Err(
            "小红书暂时无法读取此笔记，请在平台连接中登录并完成官网验证，再使用最新分享链接重试"
                .into(),
        )
    }
    .await;
    let _ = login.navigate(HOME.parse().unwrap());
    result
}
#[tauri::command]
pub async fn xiaohongshu_login_open(
    app: AppHandle,
    window: WebviewWindow,
    gate: tauri::State<'_, LoginGate>,
) -> Result<(), String> {
    main_only(&window)?;
    let _guard = gate.0.lock().await;
    let login = self::window(&app)?;
    login
        .navigate(HOME.parse().unwrap())
        .map_err(|e| e.to_string())?;
    login.show().map_err(|e| e.to_string())?;
    login.set_focus().map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn xiaohongshu_login_status(
    app: AppHandle,
    window: WebviewWindow,
    gate: tauri::State<'_, LoginGate>,
) -> Result<LoginStatus, String> {
    main_only(&window)?;
    let attempt = gate.0.lock().await;
    let login = self::window(&app)?;
    if attempt.is_some() {
        return Ok(read_status(&login).await?.unwrap_or_default());
    }
    if login.url().map_err(|e| e.to_string())?.as_str() == "about:blank" {
        login
            .navigate(HOME.parse().unwrap())
            .map_err(|e| e.to_string())?;
    }
    for _ in 0..10 {
        if let Some(status) = read_status(&login).await? {
            return Ok(status);
        }
        tokio::time::sleep(std::time::Duration::from_millis(700)).await;
    }
    Err("暂时无法确认小红书登录状态，请打开官网完成验证".into())
}
#[tauri::command]
pub async fn xiaohongshu_login_finish(
    app: AppHandle,
    window: WebviewWindow,
    gate: tauri::State<'_, LoginGate>,
) -> Result<(), String> {
    main_only(&window)?;
    let _guard = gate.0.lock().await;
    let login = self::window(&app)?;
    if !read_status(&login)
        .await?
        .is_some_and(|s| s.session_present)
    {
        return Err("请先在小红书官网完成登录".into());
    }
    login.hide().map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn xiaohongshu_logout(
    app: AppHandle,
    window: WebviewWindow,
    gate: tauri::State<'_, LoginGate>,
) -> Result<(), String> {
    main_only(&window)?;
    let _guard = gate.0.lock().await;
    let engine = app.state::<crate::engine::Shared>();
    let running = engine.0.running.lock().await;
    if engine
        .0
        .data
        .lock()
        .await
        .tasks
        .iter()
        .any(|t| t.platform == "xiaohongshu" && running.contains_key(&t.id))
    {
        return Err("请先暂停小红书下载，再断开连接".into());
    }
    let login = self::window(&app)?;
    login
        .navigate("about:blank".parse().unwrap())
        .map_err(|e| e.to_string())?;
    login.clear_all_browsing_data().map_err(|e| e.to_string())?;
    for c in login.cookies().map_err(|e| e.to_string())? {
        login.delete_cookie(c).map_err(|e| e.to_string())?;
    }
    login.hide().map_err(|e| e.to_string())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn restrict_login_navigation() {
        assert!(allowed_navigation(&HOME.parse().unwrap()));
        for s in [
            "https://xiaohongshu.com.evil.test",
            "https://user@www.xiaohongshu.com",
            "http://www.xiaohongshu.com",
            "file:///C:/test",
        ] {
            assert!(!allowed_navigation(&s.parse().unwrap()));
        }
    }
}
