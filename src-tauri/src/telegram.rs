use crate::{
    links::{parse_link, safe_file_name, Destination},
    models::{Account, MediaPreview, Settings},
};
use grammers_client::{
    client::{LoginToken, PasswordToken},
    media::Media,
    peer::Peer,
    Client,
};
use grammers_mtsender::{ConnectionParams, SenderPool};
use grammers_session::storages::SqliteSession;
use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{task::JoinHandle, time::timeout};

#[derive(Default)]
pub struct AuthState {
    pub session: Option<Arc<SqliteSession>>,
    pub qr: Option<crate::qr_login::QrAttempt>,
    pub qr_update: Arc<AtomicBool>,
    pub client: Option<Client>,
    pub login_token: Option<LoginToken>,
    pub password_token: Option<PasswordToken>,
    pub runner: Option<JoinHandle<()>>,
    pub updates: Option<JoinHandle<()>>,
}

impl AuthState {
    pub fn disconnect(&mut self) {
        if let Some(client) = self.client.take() {
            client.disconnect();
        }
        if let Some(task) = self.runner.take() {
            task.abort();
        }
        if let Some(task) = self.updates.take() {
            task.abort();
        }
        self.login_token = None;
        self.password_token = None;
        self.qr = None;
        self.session = None;
        self.qr_update.store(false, Ordering::SeqCst);
    }
    pub async fn connect(&mut self, root: &Path, settings: &Settings) -> Result<Client, String> {
        if let Some(client) = &self.client {
            return Ok(client.clone());
        }
        let api_id = settings
            .api_id
            .parse::<i32>()
            .ok()
            .filter(|id| *id > 0)
            .ok_or("请先填写有效的 API ID")?;
        let session = Arc::new(
            SqliteSession::open(root.join("telegram.session.sqlite"))
                .await
                .map_err(|e| format!("无法打开本地会话：{e}"))?,
        );
        let pool = SenderPool::with_configuration(
            session.clone(),
            api_id,
            ConnectionParams {
                device_model: "FrameFetch Desktop".into(),
                app_version: "0.1.0".into(),
                proxy_url: if settings.proxy_url.trim().is_empty() {
                    None
                } else {
                    Some(settings.proxy_url.clone())
                },
                ..Default::default()
            },
        );
        let client = Client::new(pool.handle);
        self.runner = Some(tokio::spawn(async move {
            pool.runner.run().await;
        }));
        let mut updates = pool.updates;
        let qr_update = self.qr_update.clone();
        self.updates = Some(tokio::spawn(async move {
            while let Some(update) = updates.recv().await {
                if crate::qr_login::is_login_update(&update) {
                    qr_update.store(true, Ordering::SeqCst);
                }
            }
        }));
        self.session = Some(session);
        self.client = Some(client.clone());
        Ok(client)
    }
}

pub async fn network<T>(
    future: impl std::future::Future<Output = Result<T, grammers_mtsender::InvocationError>>,
) -> Result<T, String> {
    timeout(Duration::from_secs(35), future)
        .await
        .map_err(|_| "连接 Telegram 超时，请检查网络或 SOCKS5 代理。".to_string())?
        .map_err(friendly_error)
}

pub fn friendly_error(error: impl std::fmt::Display) -> String {
    let text = error.to_string();
    if text.contains("PHONE_CODE_INVALID") {
        "验证码不正确，请重新输入。".into()
    } else if text.contains("PHONE_CODE_EXPIRED") {
        "验证码已过期，请重新获取。".into()
    } else if text.contains("PHONE_NUMBER_INVALID") {
        "手机号格式不正确，请包含国家区号。".into()
    } else if text.contains("FLOOD_WAIT") {
        format!("Telegram 暂时限制请求频率，请稍后重试。{text}")
    } else if text.contains("AUTH_KEY_UNREGISTERED") || text.contains("SESSION_REVOKED") {
        "会话已失效，请重新连接 Telegram。".into()
    } else if text.contains("CHANNEL_PRIVATE") {
        "当前账号无法访问此频道，请先在 Telegram 中加入。".into()
    } else {
        text
    }
}

pub fn account(user: &grammers_client::peer::User) -> Account {
    Account {
        connected: true,
        name: user.first_name().unwrap_or("Telegram 用户").to_string(),
        username: user.username().unwrap_or("").to_string(),
    }
}

pub struct Resolved {
    pub preview: MediaPreview,
    pub media: Media,
    pub media_id: i64,
}

pub async fn resolve(client: &Client, input: &str) -> Result<Resolved, String> {
    let link = parse_link(input)?;
    let peer = match link.destination {
        Destination::Public(username) => network(client.resolve_username(&username))
            .await.map_err(|error| format!("查找频道失败：{error}"))?
            .ok_or("没有找到此频道或用户")?,
        Destination::Private(id) => {
            // Dialog iteration obtains a legitimate access hash for joined channels.
            let mut dialogs = client.iter_dialogs();
            let mut found = None;
            while let Some(dialog) = network(dialogs.next()).await.map_err(|error| format!("读取频道列表失败：{error}"))? {
                if dialog.peer().id().bare_id_unchecked() == id
                    && !matches!(dialog.peer(), Peer::User(_))
                {
                    found = Some(dialog.peer().clone());
                    break;
                }
            }
            found.ok_or("未找到此私密频道，请确认当前账号已经加入。")?
        }
    };
    let reference = peer
        .to_ref()
        .await
        .map_err(friendly_error)?
        .ok_or("无法访问此频道")?;
    let messages = network(client.get_messages_by_id(reference, &[link.message_id])).await
        .map_err(|error| format!("读取消息失败：{error}"))?;
    let message = messages
        .into_iter()
        .flatten()
        .next()
        .ok_or("此消息不存在或已被删除")?;
    // Resolve accessible media and let the download RPC determine availability.
    // noforwards is not used as a local download capability check.
    let media = message.media().ok_or("这条消息没有可下载的媒体或文件")?;
    let (name, size, kind, media_id) = match &media {
        Media::Photo(photo) => {
            if photo.raw.ttl_seconds.is_some() {
                return Err("不支持保存阅后即焚媒体".into());
            }
            (
                format!("photo-{}.jpg", message.id()),
                photo.size().unwrap_or(0) as u64,
                "图片",
                photo.id(),
            )
        }
        Media::Document(doc) => document_info(doc, message.id())?,
        Media::Sticker(sticker) => document_info(&sticker.document, message.id())?,
        _ => return Err("当前仅支持视频、图片、音频和文件，不支持此消息类型。".into()),
    };
    let file_name = safe_file_name(&name);
    let title = if message.text().trim().is_empty() {
        file_name.clone()
    } else {
        message
            .text()
            .lines()
            .next()
            .unwrap_or(&file_name)
            .chars()
            .take(120)
            .collect()
    };
    let thumbnail = thumbnail(client, &media).await;
    Ok(Resolved {
        preview: MediaPreview {
            topics: message_topics(message.text(), message.fmt_entities().map(Vec::as_slice).unwrap_or(&[])),
            url: link.canonical,
            title,
            file_name,
            thumbnail,
            size,
            source: peer.name().unwrap_or("Telegram").to_string(),
            kind: kind.into(),
        },
        media,
        media_id,
    })
}

fn message_topics(text: &str, entities: &[grammers_tl_types::enums::MessageEntity]) -> Vec<String> {
    // Telegram entity offsets and lengths count UTF-16 code units, not UTF-8 bytes.
    let utf16: Vec<u16> = text.encode_utf16().collect();
    let mut topics = Vec::new();
    for entity in entities {
        let grammers_tl_types::enums::MessageEntity::Hashtag(tag) = entity else { continue };
        let (Ok(start), Ok(length)) = (usize::try_from(tag.offset), usize::try_from(tag.length)) else { continue };
        let Some(end) = start.checked_add(length) else { continue };
        let Some(units) = utf16.get(start..end) else { continue };
        let Ok(value) = String::from_utf16(units) else { continue };
        let Some(topic) = value.strip_prefix('#').filter(|topic| !topic.is_empty()) else { continue };
        if !topics.iter().any(|existing| existing == topic) {
            topics.push(topic.to_string());
        }
    }
    topics
}

#[cfg(test)]
mod topic_tests {
    use super::message_topics;
    use grammers_tl_types::{enums::MessageEntity, types::MessageEntityHashtag};

    fn hashtag(offset: i32, length: i32) -> MessageEntity {
        MessageEntity::Hashtag(MessageEntityHashtag { offset, length })
    }

    #[test]
    fn extracts_full_caption_with_utf16_offsets_and_deduplicates() {
        let text = format!("🎬{}\n#旅行 #video_2026 #旅行", "正文".repeat(100));
        let offset = text[..text.find('#').unwrap()].encode_utf16().count() as i32;
        assert_eq!(message_topics(&text, &[
            hashtag(offset, 3), hashtag(offset + 4, 11), hashtag(offset + 16, 3),
        ]), vec!["旅行", "video_2026"]);
    }

    #[test]
    fn ignores_unmarked_text_and_invalid_entities() {
        assert!(message_topics("https://example.com/#anchor #plain", &[]).is_empty());
        assert!(message_topics("🎬 #", &[
            hashtag(-1, 2), hashtag(0, 1), hashtag(3, 1),
            hashtag(0, -1), hashtag(100, 3),
        ]).is_empty());
    }
}

fn document_info(
    doc: &grammers_client::media::Document,
    message_id: i32,
) -> Result<(String, u64, &'static str, i64), String> {
    if doc.raw.ttl_seconds.is_some() {
        return Err("不支持保存阅后即焚媒体".into());
    }
    let mime = doc.mime_type().unwrap_or("application/octet-stream");
    let (kind, ext) = if mime.starts_with("video/") {
        ("视频", "mp4")
    } else if mime.starts_with("audio/") {
        ("音频", if mime.contains("ogg") { "ogg" } else { "mp3" })
    } else if mime.starts_with("image/") {
        ("图片", "jpg")
    } else {
        ("文件", "bin")
    };
    Ok((
        doc.name()
            .map(str::to_string)
            .unwrap_or_else(|| format!("file-{message_id}.{ext}")),
        doc.size().unwrap_or(0) as u64,
        kind,
        doc.id(),
    ))
}



// Thumbnail failure must never prevent media downloads. Bound both time and payload.
pub(crate) async fn thumbnail(client: &Client, media: &Media) -> Option<String> {
    use base64::Engine as _;
    use grammers_client::media::{Downloadable, PhotoSize};
    let thumbs = match media {
        Media::Document(doc) => doc.thumbs(),
        Media::Photo(photo) => photo.thumbs(),
        _ => return None,
    };
    let embedded = thumbs.iter().filter(|t| !matches!(t, PhotoSize::Path(_)))
        .filter_map(|t| t.to_data()).find(|b| !b.is_empty() && b.len() <= 65536);
    let remote = thumbs.iter().filter(|t| t.size() > 0 && t.size() <= 65536 && t.to_raw_input_location().is_some())
        .max_by_key(|t| t.size());
    let bytes = if let Some(thumb) = remote {
        let mut iter = client.iter_download(thumb);
        match tokio::time::timeout(Duration::from_secs(2), iter.next()).await {
            Ok(Ok(Some(bytes))) if !bytes.is_empty() && bytes.len() <= 65536 => Some(bytes),
            _ => embedded,
        }
    } else { embedded }?;
    let mime = if bytes.starts_with(&[0xff, 0xd8]) { "image/jpeg" }
        else if bytes.starts_with(b"\x89PNG") { "image/png" }
        else if bytes.starts_with(b"RIFF") { "image/webp" } else { return None; };
    Some(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
}
