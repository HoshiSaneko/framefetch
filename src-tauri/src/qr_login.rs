use crate::{
    commands::validate_settings,
    engine::Shared,
    models::{Account, Settings},
    telegram::{self, AuthState},
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use grammers_client::client::PasswordToken;
use grammers_session::{types::PeerInfo, updates::UpdatesLike, Session};
use grammers_tl_types as tl;
use serde::Serialize;
use std::{
    sync::atomic::Ordering,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, State};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QrResult {
    pub step: String,
    pub url: Option<String>,
    pub expires_at: u64,
    pub hint: String,
    pub account: Option<Account>,
}

pub struct QrAttempt {
    pub id: String,
    pub current: QrResult,
    // A failed/slow import can be retried on the destination DC without losing its token.
    migration: Option<Vec<u8>>,
}

fn now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn login_url(token: &[u8]) -> String {
    format!("tg://login?token={}", URL_SAFE_NO_PAD.encode(token))
}
fn needs_export(result: &QrResult, updated: bool, now: u64) -> bool {
    result.step == "qr" && (updated || result.url.is_none() || result.expires_at <= now)
}
fn result(step: &str) -> QrResult {
    QrResult {
        step: step.into(),
        url: None,
        expires_at: 0,
        hint: String::new(),
        account: None,
    }
}

pub fn is_login_update(update: &UpdatesLike) -> bool {
    let is_login = |u: &tl::enums::Update| matches!(u, tl::enums::Update::LoginToken);
    match update {
        UpdatesLike::Updates(tl::enums::Updates::UpdateShort(u)) => is_login(&u.update),
        UpdatesLike::Updates(tl::enums::Updates::Updates(u)) => u.updates.iter().any(is_login),
        UpdatesLike::Updates(tl::enums::Updates::Combined(u)) => u.updates.iter().any(is_login),
        UpdatesLike::ConnectionClosed => true,
        _ => false,
    }
}

pub fn validate_attempt(auth: &AuthState, id: &str) -> Result<(), String> {
    if auth.qr.as_ref().is_some_and(|q| q.id == id) {
        Ok(())
    } else {
        Err("二维码登录已结束，请重新生成二维码。".into())
    }
}

async fn advance(auth: &mut AuthState, settings: &Settings) -> Result<QrResult, String> {
    let client = auth.client.clone().ok_or("请重新生成二维码")?;
    let api_id = settings
        .api_id
        .parse::<i32>()
        .map_err(|_| "API ID 无效，请检查连接设置")?;
    for _ in 0..4 {
        let import_token = auth.qr.as_ref().and_then(|q| q.migration.clone());
        let response = tokio::time::timeout(Duration::from_secs(35), async {
            if let Some(token) = import_token {
                client
                    .invoke(&tl::functions::auth::ImportLoginToken { token })
                    .await
            } else {
                client
                    .invoke(&tl::functions::auth::ExportLoginToken {
                        api_id,
                        api_hash: settings.api_hash.clone(),
                        except_ids: vec![],
                    })
                    .await
            }
        })
        .await
        .map_err(|_| "连接 Telegram 超时，请检查网络或代理后重试。")?;
        match response {
            Ok(tl::enums::auth::LoginToken::Token(token)) => {
                if token.token.is_empty() || token.expires <= 0 {
                    return Err("Telegram 返回了无效的二维码，请重试。".into());
                }
                let current = QrResult {
                    url: Some(login_url(&token.token)),
                    expires_at: token.expires as u64,
                    ..result("qr")
                };
                if let Some(qr) = auth.qr.as_mut() {
                    qr.current = current.clone();
                    qr.migration = None;
                }
                return Ok(current);
            }
            Ok(tl::enums::auth::LoginToken::MigrateTo(migration)) => {
                let session = auth.session.as_ref().ok_or("本地会话不可用，请重新登录")?;
                if session
                    .dc_option(migration.dc_id)
                    .map_err(telegram::friendly_error)?
                    .is_none()
                {
                    return Err("Telegram 返回了未知的数据中心，请稍后重试。".into());
                }
                session
                    .set_home_dc_id(migration.dc_id)
                    .await
                    .map_err(telegram::friendly_error)?;
                if let Some(qr) = auth.qr.as_mut() {
                    qr.migration = Some(migration.token);
                }
            }
            Ok(tl::enums::auth::LoginToken::Success(_)) => {
                let user = telegram::network(client.get_me()).await?;
                let peer = user
                    .to_ref()
                    .await
                    .map_err(telegram::friendly_error)?
                    .ok_or("无法读取已登录账号")?;
                auth.session
                    .as_ref()
                    .ok_or("本地会话不可用")?
                    .cache_peer(&PeerInfo::User {
                        id: user.id().bare_id_unchecked(),
                        auth: Some(peer.auth),
                        bot: Some(user.is_bot()),
                        is_self: Some(true),
                    })
                    .await
                    .map_err(telegram::friendly_error)?;
                auth.login_token = None;
                auth.password_token = None;
                auth.qr = None;
                return Ok(QrResult {
                    account: Some(telegram::account(&user)),
                    ..result("connected")
                });
            }
            Err(error) if error.to_string().contains("SESSION_PASSWORD_NEEDED") => {
                let password =
                    telegram::network(client.invoke(&tl::functions::account::GetPassword {}))
                        .await?;
                let token = PasswordToken::new(password.into());
                let current = QrResult {
                    hint: token.hint().unwrap_or("").to_string(),
                    ..result("password")
                };
                auth.password_token = Some(token);
                if let Some(qr) = auth.qr.as_mut() {
                    qr.current = current.clone();
                    qr.migration = None;
                }
                return Ok(current);
            }
            Err(error) => return Err(telegram::friendly_error(error)),
        }
    }
    Err("Telegram 数据中心切换未完成，请重新生成二维码。".into())
}

#[tauri::command]
pub async fn start_qr_login(
    app: AppHandle,
    state: State<'_, Shared>,
    settings: Settings,
    attempt_id: String,
) -> Result<QrResult, String> {
    validate_settings(&settings, true)?;
    uuid::Uuid::parse_str(&attempt_id).map_err(|_| "登录请求无效")?;
    let mut auth = state.0.auth.lock().await;
    let mut data = state.0.data.lock().await;
    if data.account.connected {
        return Ok(QrResult {
            account: Some(data.account.clone()),
            ..result("connected")
        });
    }
    if data.settings.api_id != settings.api_id
        || data.settings.api_hash != settings.api_hash
        || data.settings.proxy_url != settings.proxy_url
    {
        auth.disconnect();
    }
    data.settings = settings.clone();
    drop(data);
    state.0.persist(&app).await?;
    auth.login_token = None;
    auth.password_token = None;
    auth.qr_update.store(false, Ordering::SeqCst);
    auth.qr = Some(QrAttempt {
        id: attempt_id,
        current: result("qr"),
        migration: None,
    });
    let client = auth.connect(&state.0.root, &settings).await?;
    if telegram::network(client.is_authorized()).await? {
        let account = telegram::account(&telegram::network(client.get_me()).await?);
        state.0.data.lock().await.account = account.clone();
        auth.qr = None;
        state.0.emit(&app);
        return Ok(QrResult {
            account: Some(account),
            ..result("connected")
        });
    }
    let current = advance(&mut auth, &settings).await?;
    if let Some(account) = &current.account {
        state.0.data.lock().await.account = account.clone();
        state.0.emit(&app);
    }
    Ok(current)
}

#[tauri::command]
pub async fn poll_qr_login(
    app: AppHandle,
    state: State<'_, Shared>,
    attempt_id: String,
) -> Result<QrResult, String> {
    let mut auth = state.0.auth.lock().await;
    validate_attempt(&auth, &attempt_id)?;
    let current = auth.qr.as_ref().unwrap().current.clone();
    let updated = auth.qr_update.swap(false, Ordering::SeqCst);
    if !needs_export(&current, updated, now_seconds()) {
        return Ok(current);
    }
    let settings = state.0.data.lock().await.settings.clone();
    let next = match advance(&mut auth, &settings).await {
        Ok(next) => next,
        Err(error) => {
            auth.qr_update.store(true, Ordering::SeqCst);
            return Err(error);
        }
    };
    if let Some(account) = &next.account {
        state.0.data.lock().await.account = account.clone();
        state.0.emit(&app);
    }
    Ok(next)
}

#[tauri::command]
pub async fn cancel_qr_login(state: State<'_, Shared>, attempt_id: String) -> Result<(), String> {
    let mut auth = state.0.auth.lock().await;
    // An old dialog's cleanup must never cancel a newer login attempt.
    if auth.qr.as_ref().is_some_and(|q| q.id == attempt_id) {
        auth.qr = None;
        auth.password_token = None;
        if !state.0.data.lock().await.account.connected {
            auth.disconnect();
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn token_encoding_is_url_safe_without_padding() {
        assert_eq!(login_url(&[251, 255]), "tg://login?token=-_8");
    }
    #[test]
    fn does_not_rotate_live_code_until_update_or_expiry() {
        let current = QrResult {
            url: Some("local-token".into()),
            expires_at: 130,
            ..result("qr")
        };
        assert!(!needs_export(&current, false, 100));
        assert!(needs_export(&current, true, 100));
        assert!(needs_export(&current, false, 130));
        assert!(!needs_export(&result("password"), true, 999));
    }
    #[test]
    fn stale_attempt_cannot_access_new_login() {
        let mut auth = AuthState::default();
        auth.qr = Some(QrAttempt {
            id: "new".into(),
            current: result("qr"),
            migration: None,
        });
        assert!(validate_attempt(&auth, "old").is_err());
        assert!(validate_attempt(&auth, "new").is_ok());
    }
    #[test]
    fn detects_confirmation_update() {
        let update =
            UpdatesLike::Updates(tl::enums::Updates::UpdateShort(tl::types::UpdateShort {
                update: tl::enums::Update::LoginToken,
                date: 0,
            }));
        assert!(is_login_update(&update));
    }
}
