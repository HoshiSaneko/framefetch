//! WKWebView may have no native URL until its first navigation commits.
//! Wry can also queue startup evaluation without retaining its callback.
pub async fn current(window: &tauri::WebviewWindow) -> Result<String, String> {
    for _ in 0..3 {
        let (sender, receiver) = tokio::sync::oneshot::channel();
        let sender = std::sync::Mutex::new(Some(sender));
        window.eval_with_callback("window.location.href", move |value| {
            if let Some(sender) = sender.lock().unwrap_or_else(|e| e.into_inner()).take() {
                let _ = sender.send(value);
            }
        }).map_err(|e| e.to_string())?;
        match tokio::time::timeout(std::time::Duration::from_secs(2), receiver).await {
            Ok(Ok(raw)) if !raw.trim().is_empty() && raw.trim() != "null" => {
                return serde_json::from_str(&raw).map_err(|_| "无法读取登录页面地址".into());
            }
            _ => tokio::time::sleep(std::time::Duration::from_millis(200)).await,
        }
    }
    Err("登录页面尚未准备好，请稍后重试".into())
}
