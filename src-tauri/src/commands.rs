use crate::{
    engine::{output_name, part_path, Shared},
    models::*,
    telegram,
};
use grammers_client::SignInError;
use std::{path::Path, sync::atomic::Ordering, time::Duration};
use tauri::{AppHandle, State, Manager};

#[tauri::command]
pub async fn playback_path(app: AppHandle, state: State<'_, Shared>, id: String) -> Result<String, String> {
    let data = state.0.data.lock().await;
    let task = data.tasks.iter().find(|t| t.id == id && t.status == Status::Completed)
        .ok_or("视频尚未下载完成")?;
    let path = Path::new(&task.output_path).canonicalize().map_err(|_| "文件已移动或删除")?;
    if !path.is_file() { return Err("文件不存在".into()); }
    app.asset_protocol_scope().allow_file(&path).map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().into_owned())
}
use tauri_plugin_opener::OpenerExt;

#[tauri::command]
pub async fn telegram_avatar(state: State<'_, Shared>) -> Result<Option<String>, String> {
    if !state.0.data.lock().await.account.connected { return Ok(None); }
    let client = state.0.auth.lock().await.client.clone().ok_or("Telegram 尚未连接")?;
    let result = tokio::time::timeout(Duration::from_secs(8), async {
        let user = client.get_me().await.ok()?;
        let peer = user.to_ref().await.ok()??;
        let photo = client.iter_profile_photos(peer).next().await.ok()??;
        telegram::thumbnail(&client, &grammers_client::media::Media::Photo(photo)).await
    }).await.unwrap_or(None);
    Ok(result)
}

#[tauri::command]
pub async fn probe_telegram(state: State<'_, Shared>) -> Result<(), String> {
    let mut auth = state.0.auth.lock().await;
    let settings = state.0.data.lock().await.settings.clone();
    validate_settings(&settings, true)?;
    let client = auth.connect(&state.0.root, &settings).await?;
    telegram::network(client.invoke(&grammers_tl_types::functions::help::GetConfig {})).await?;
    Ok(())
}

#[tauri::command]
pub async fn remove_download(
    app: AppHandle,
    state: State<'_, Shared>,
    id: String,
    delete_files: Option<bool>,
) -> Result<(), String> {
    let running = state.0.running.lock().await;
    let mut data = state.0.data.lock().await;
    let targets = crate::removal::removal_targets(&data.tasks, &id);
    if targets.iter().any(|t| running.contains_key(&t.id)) {
        return Err("请先暂停整个任务，待下载停止后再删除。".into());
    }
    if delete_files.unwrap_or(false) {
        let remaining: Vec<_> = data.tasks.iter().filter(|t| !targets.iter().any(|target| target.id == t.id)).cloned().collect();
        crate::removal::delete_task_files(&targets, &remaining)?;
    }
    data.tasks.retain(|t| !targets.iter().any(|target| target.id == t.id));
    drop(data);
    drop(running);
    state.0.persist(&app).await
}

#[tauri::command]
pub async fn open_download(
    app: AppHandle,
    state: State<'_, Shared>,
    id: String,
) -> Result<(), String> {
    let data = state.0.data.lock().await;
    let task = data
        .tasks
        .iter()
        .find(|t| t.id == id && t.status == Status::Completed)
        .ok_or("文件尚未下载完成")?;
    if !Path::new(&task.output_path).is_file() {
        return Err("文件已移动或删除。".into());
    }
    app.opener()
        .open_path(&task.output_path, None::<String>)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn download_path_exists(state: State<'_, Shared>, path: String) -> Result<bool, String> {
    let data = state.0.data.lock().await;
    let target = Path::new(&path);
    if target != Path::new(&data.settings.download_dir)
        && !data.tasks.iter().any(|t| {
            let file = Path::new(&t.output_path);
            file == target || file.parent() == Some(target)
        })
    {
        return Ok(false);
    }
    Ok(Path::new(&path).exists())
}

#[tauri::command]
pub async fn open_external_link(app: AppHandle, url: String) -> Result<(), String> {
    let parsed = url::Url::parse(&url).map_err(|_| "无效链接")?;
    if !matches!(parsed.scheme(), "https" | "http") {
        return Err("仅允许打开网页链接。".into());
    }
    app.opener()
        .open_url(url, None::<String>)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn snapshot(state: State<'_, Shared>) -> Result<Snapshot, String> {
    Ok(state.0.data.lock().await.clone())
}

pub(crate) fn validate_settings(settings: &Settings, require_api: bool) -> Result<(), String> {
    if !(1..=4).contains(&settings.concurrency) {
        return Err("并发任务数应为 1 到 4".into());
    }
    if !Path::new(&settings.download_dir).is_absolute() {
        return Err("请选择一个有效的绝对路径作为下载文件夹".into());
    }
    if require_api || !settings.api_id.is_empty() || !settings.api_hash.is_empty() {
        if settings
            .api_id
            .parse::<i32>()
            .ok()
            .filter(|x| *x > 0)
            .is_none()
            || settings.api_hash.len() != 32
            || !settings.api_hash.bytes().all(|c| c.is_ascii_hexdigit())
        {
            return Err("请输入有效的 API ID 和 32 位 API Hash".into());
        }
    }
    if !settings.proxy_url.trim().is_empty() {
        let proxy = url::Url::parse(&settings.proxy_url).map_err(|_| "代理地址格式无效")?;
        if proxy.scheme() != "socks5" || proxy.host_str().is_none() || proxy.port().is_none() {
            return Err("代理格式应为 socks5://主机:端口".into());
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn save_settings(
    app: AppHandle,
    state: State<'_, Shared>,
    settings: Settings,
) -> Result<Settings, String> {
    validate_settings(&settings, false)?;
    let mut auth = state.0.auth.lock().await;
    let mut data = state.0.data.lock().await;
    let connection_changed = data.settings.api_id != settings.api_id
        || data.settings.api_hash != settings.api_hash
        || data.settings.proxy_url != settings.proxy_url;
    if connection_changed && data.account.connected {
        return Err("修改连接设置前，请先退出 Telegram 账号。".into());
    }
    if connection_changed {
        auth.disconnect();
    }
    data.settings = settings.clone();
    drop(data);
    drop(auth);
    state.0.persist(&app).await?;
    Ok(settings)
}

#[tauri::command]
pub async fn restore_session(app: AppHandle, state: State<'_, Shared>) -> Result<Account, String> {
    let mut auth = state.0.auth.lock().await;
    let settings = state.0.data.lock().await.settings.clone();
    if settings.api_id.is_empty() || !state.0.root.join("telegram.session.sqlite").exists() {
        return Ok(Account::default());
    }
    let client = auth.connect(&state.0.root, &settings).await?;
    if !telegram::network(client.is_authorized()).await? {
        return Ok(Account::default());
    }
    let account = telegram::account(&telegram::network(client.get_me()).await?);
    state.0.data.lock().await.account = account.clone();
    state.0.emit(&app);
    Ok(account)
}

#[tauri::command]
pub async fn send_code(
    app: AppHandle,
    state: State<'_, Shared>,
    settings: Settings,
    phone: String,
) -> Result<AuthResult, String> {
    validate_settings(&settings, true)?;
    if !phone.starts_with('+')
        || !(8..=16).contains(&phone.len())
        || !phone[1..].bytes().all(|x| x.is_ascii_digit())
    {
        return Err("请输入包含国家区号的有效手机号".into());
    }
    let mut auth = state.0.auth.lock().await;
    let mut data = state.0.data.lock().await;
    if data.account.connected {
        return Ok(AuthResult {
            step: "connected".into(),
            hint: String::new(),
            account: Some(data.account.clone()),
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
    let client = auth.connect(&state.0.root, &settings).await?;
    if telegram::network(client.is_authorized()).await? {
        let account = telegram::account(&telegram::network(client.get_me()).await?);
        state.0.data.lock().await.account = account.clone();
        state.0.emit(&app);
        return Ok(AuthResult {
            step: "connected".into(),
            hint: String::new(),
            account: Some(account),
        });
    }
    auth.login_token =
        Some(telegram::network(client.request_login_code(&phone, &settings.api_hash)).await?);
    auth.password_token = None;
    Ok(AuthResult {
        step: "code".into(),
        hint: String::new(),
        account: None,
    })
}

#[tauri::command]
pub async fn sign_in(
    app: AppHandle,
    state: State<'_, Shared>,
    code: String,
) -> Result<AuthResult, String> {
    let mut auth = state.0.auth.lock().await;
    let client = auth.client.clone().ok_or("请先发送验证码")?;
    let token = auth
        .login_token
        .as_ref()
        .ok_or("验证码已失效，请重新发送")?;
    let result = tokio::time::timeout(Duration::from_secs(35), client.sign_in(token, code.trim()))
        .await
        .map_err(|_| "登录超时，请重试")?;
    match result {
        Ok(user) => {
            auth.login_token = None;
            auth.password_token = None;
            let account = telegram::account(&user);
            state.0.data.lock().await.account = account.clone();
            state.0.emit(&app);
            Ok(AuthResult {
                step: "connected".into(),
                hint: String::new(),
                account: Some(account),
            })
        }
        Err(SignInError::PasswordRequired(token)) => {
            let hint = token.hint().unwrap_or("").to_string();
            auth.password_token = Some(token);
            Ok(AuthResult {
                step: "password".into(),
                hint,
                account: None,
            })
        }
        Err(SignInError::InvalidCode) => Err("验证码不正确，请重新输入。".into()),
        Err(SignInError::SignUpRequired) => {
            Err("此号码尚未注册，请先使用 Telegram 官方客户端注册。".into())
        }
        Err(error) => Err(telegram::friendly_error(error)),
    }
}

#[tauri::command]
pub async fn check_password(
    app: AppHandle,
    state: State<'_, Shared>,
    password: String,
    attempt_id: Option<String>,
) -> Result<AuthResult, String> {
    let mut auth = state.0.auth.lock().await;
    if let Some(id) = attempt_id.as_deref() {
        crate::qr_login::validate_attempt(&auth, id)?;
        if !auth
            .qr
            .as_ref()
            .is_some_and(|qr| qr.current.step == "password")
        {
            return Err("请先使用手机扫码确认登录。".into());
        }
    }
    let client = auth.client.clone().ok_or("请重新登录")?;
    // Refresh SRP parameters on each try so timeout and invalid-password retries remain usable.
    if attempt_id.is_some() {
        let password_info = telegram::network(
            client.invoke(&grammers_tl_types::functions::account::GetPassword {}),
        )
        .await?;
        auth.password_token = Some(grammers_client::client::PasswordToken::new(
            password_info.into(),
        ));
    }
    let token = auth.password_token.take().ok_or("验证已失效，请重新登录")?;
    let result = tokio::time::timeout(
        Duration::from_secs(35),
        client.check_password(token, password.as_bytes()),
    )
    .await
    .map_err(|_| "验证超时，请重新开始登录")?;
    match result {
        Ok(user) => {
            auth.login_token = None;
            auth.qr = None;
            let account = telegram::account(&user);
            state.0.data.lock().await.account = account.clone();
            state.0.emit(&app);
            Ok(AuthResult {
                step: "connected".into(),
                hint: String::new(),
                account: Some(account),
            })
        }
        Err(SignInError::InvalidPassword(token)) => {
            auth.password_token = Some(token);
            Err("两步验证密码不正确，请重试。".into())
        }
        Err(error) => Err(telegram::friendly_error(error)),
    }
}

#[tauri::command]
pub async fn logout(app: AppHandle, state: State<'_, Shared>) -> Result<(), String> {
    let mut auth = state.0.auth.lock().await;
    let running = state.0.running.lock().await;
    if !running.is_empty() {
        return Err("请先暂停下载，等待任务停止后再退出账号。".into());
    }
    let client = auth.client.clone().ok_or("当前没有已连接的账号")?;
    let previous_account = {
        let mut data = state.0.data.lock().await;
        let previous = data.account.clone();
        data.account.connected = false;
        previous
    };
    drop(running);
    if let Err(error) = telegram::network(client.sign_out()).await {
        if !error.contains("会话已失效") {
            state.0.data.lock().await.account = previous_account;
            state.0.emit(&app);
            return Err(error);
        }
    }
    auth.disconnect();
    let mut data = state.0.data.lock().await;
    data.account = Account::default();
    for task in &mut data.tasks {
        if task.status == Status::Queued {
            task.status = Status::Paused;
            task.updated_at = now();
        }
    }
    drop(data);
    state.0.persist(&app).await
}

#[tauri::command]
pub async fn preview_link(state: State<'_, Shared>, url: String) -> Result<MediaPreview, String> {
    let provider = crate::providers::for_url(&url)?;
    crate::providers::require_available(provider)?;
    provider.preview(&state.0, &url).await.map(|r| r.preview)
}

#[tauri::command]
pub async fn enqueue_download(
    app: AppHandle,
    state: State<'_, Shared>,
    url: String,
    download_dir: Option<String>,
    batch: Option<crate::models::DownloadBatch>,
) -> Result<(), String> {
    let input = url.split_whitespace().find(|s| s.starts_with("https://") || s.starts_with("http://")).unwrap_or(&url).trim_end_matches(['。', '，', ')', '）']);
    let provider = crate::providers::for_url(input)?;
    crate::providers::require_available(provider)?;
    let (canonical, label) = if provider.info().id == "douyin" {
        (crate::douyin_download::canonical(input)?, "抖音作品".to_string())
    } else if provider.info().id == "xiaohongshu" {
        (crate::xiaohongshu_download::canonical(input)?, "小红书作品".to_string())
    } else if provider.info().id == "bilibili" {
        (crate::bilibili_download::canonical(input)?, "Bilibili 视频".to_string())
    } else {let link=crate::links::parse_link(input)?; (link.canonical, format!("消息 {}",link.message_id))};
    let mut data = state.0.data.lock().await;
    if data
        .tasks
        .iter()
        .any(|t| t.url == canonical && t.status != Status::Canceled && (provider.info().id != "bilibili" || t.bilibili.as_ref().is_some_and(|s| s.kind == "video")))
    {
        return Err("这条消息已在下载列表中，请在原任务上继续或重试。".into());
    }
    let id = uuid::Uuid::new_v4().to_string();
    let folder = download_dir
        .as_deref()
        .unwrap_or(&data.settings.download_dir);
    if !Path::new(folder).is_absolute() {
        return Err("请选择有效的绝对下载路径".into());
    }
    let mut directory = Path::new(folder).to_path_buf();
    if let Some(batch) = &batch {
        if uuid::Uuid::parse_str(&batch.id).is_err() || batch.title.chars().count() > 200 {return Err("批量任务参数无效".into());}
        directory = directory.join(format!("批量-{}",batch.id));
    }
    let output = directory.join(output_name(&id, "pending"));
    let mut job = DownloadTask { topics: vec![], storage: None, xiaohongshu: (provider.info().id == "xiaohongshu").then(crate::xiaohongshu_download::default_selection),
        bilibili: (provider.info().id == "bilibili").then(|| crate::bilibili_download::Selection {kind:"video".into(),format:None}),
        discovery: None,
        batch,
        id,
        platform: provider.info().id.into(),
        url: canonical,
        title: label.clone(),
        file_name: format!("待解析 · {label}"),
        thumbnail: None,
        total_bytes: 0,
        downloaded_bytes: 0,
        speed: 0,
        status: Status::Queued,
        output_path: output.to_string_lossy().to_string(),
        created_at: now(),
        updated_at: now(),
        error: None,
        source: provider.info().name.into(),
        media_id: 0,
    };
    crate::storage_layout::initialize(&mut job, Path::new(folder), &data.tasks);
    data.tasks.insert(0, job);
    drop(data);
    state.0.persist(&app).await
}

#[tauri::command]
pub async fn control_download(
    app: AppHandle,
    state: State<'_, Shared>,
    id: String,
    action: String,
) -> Result<(), String> {
    let running = state.0.running.lock().await;
    let mut data = state.0.data.lock().await;
    if let Some(batch_id)=data.tasks.iter().find(|t|t.id==id && t.discovery.is_some()).and_then(|t|t.batch.as_ref()).map(|b|b.id.clone()) {
        if !["pause","resume","cancel"].contains(&action.as_str()){return Err("未知任务操作".into());}
        for task in data.tasks.iter_mut().filter(|t|t.batch.as_ref().is_some_and(|b|b.id==batch_id)) {
            if matches!(task.status,Status::Completed|Status::Canceled){continue;}
            match action.as_str(){
                "pause"=>{if let Some(c)=running.get(&task.id){c.store(1,Ordering::SeqCst);}task.status=Status::Paused;task.speed=0;},
                "resume"=>{if matches!(task.status,Status::Paused|Status::Failed){task.status=Status::Queued;task.error=None;}},
                "cancel"=>{if let Some(c)=running.get(&task.id){c.store(2,Ordering::SeqCst);}task.status=Status::Canceled;task.speed=0;},
                _=>{}
            }
            task.updated_at=now();
        }
        drop(data);drop(running);return state.0.persist(&app).await;
    }
    let connected = data.account.connected;
    let task = data
        .tasks
        .iter_mut()
        .find(|t| t.id == id)
        .ok_or("没有找到该任务")?;
    if task.status == Status::Completed {
        return Err("该任务已经完成。".into());
    }
    let mut remove_part = None;
    let cleanup_bilibili = if action == "cancel" && !running.contains_key(&id) {Some(task.clone())} else {None};
    match action.as_str() {
        "pause" => {
            if let Some(control) = running.get(&id) {
                control.store(1, Ordering::SeqCst);
            }
            task.status = Status::Paused;
            task.speed = 0;
        }
        "resume" => {
            let provider = crate::providers::get(&task.platform)?;
            crate::providers::require_available(provider)?;
            if provider.info().id == "telegram" && provider.info().requires_account && !connected {
                return Err("请先连接 Telegram".into());
            }
            if task.status == Status::Canceled {
                return Err("任务已取消，请重新添加消息链接。".into());
            }
            if !matches!(task.status, Status::Paused | Status::Failed) {
                return Err("该任务已经在队列中。".into());
            }
            task.status = Status::Queued;
            task.error = None;
        }
        "cancel" => {
            if let Some(control) = running.get(&id) {
                control.store(2, Ordering::SeqCst);
            } else {
                remove_part = Some(part_path(task));
            }
            task.status = Status::Canceled;
            task.speed = 0;
        }
        _ => return Err("未知任务操作".into()),
    }
    task.updated_at = now();
    drop(data);
    drop(running);
    if let Some(job) = cleanup_bilibili {crate::bilibili_download::cleanup(&job)?;crate::xiaohongshu_download::cleanup(&job)?;}
    if let Some(path) = remove_part {
        let _ = tokio::fs::remove_file(path).await;
    }
    state.0.persist(&app).await
}

#[tauri::command]
pub async fn reveal_download(
    app: AppHandle,
    state: State<'_, Shared>,
    id: String,
) -> Result<(), String> {
    let data = state.0.data.lock().await;
    let task = data
        .tasks
        .iter()
        .find(|t| t.id == id && t.status == Status::Completed)
        .ok_or("该文件尚未下载完成")?;
    if task.discovery.is_some(){
        let directory=Path::new(&task.output_path).parent().ok_or("任务目录无效")?;
        return app.opener().open_path(directory.to_string_lossy().to_string(),None::<String>).map_err(|e|e.to_string());
    }
    if !Path::new(&task.output_path).is_file() {
        return Err("文件已移动或删除。".into());
    }
    if task.batch.is_some() {
        if let Some(layout) = &task.storage {
            return app.opener().open_path(&layout.base, None::<String>).map_err(|e|e.to_string());
        }
        let folder = Path::new(&task.output_path).parent().and_then(Path::parent).ok_or("任务目录无效")?;
        return app.opener().open_path(folder.to_string_lossy().to_string(), None::<String>).map_err(|e|e.to_string());
    }
    app.opener()
        .reveal_item_in_dir(&task.output_path)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn open_download_folder(app: AppHandle, state: State<'_, Shared>) -> Result<(), String> {
    let folder = state.0.data.lock().await.settings.download_dir.clone();
    tokio::fs::create_dir_all(&folder)
        .await
        .map_err(|e| e.to_string())?;
    app.opener()
        .open_path(folder, None::<String>)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn file_thumbnail(
    state: State<'_, Shared>,
    id: String,
) -> Result<Option<String>, String> {
    use base64::Engine;
    use tokio::io::AsyncReadExt;
    let task = state
        .0
        .data
        .lock()
        .await
        .tasks
        .iter()
        .find(|t| t.id == id)
        .cloned()
        .ok_or("任务不存在")?;
    if task.status != Status::Completed {
        return Ok(None);
    }
    let ext = Path::new(&task.output_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !["jpg", "jpeg", "png", "webp"].contains(&ext.as_str()) {
        return Ok(None);
    }
    let file = tokio::fs::File::open(&task.output_path)
        .await
        .map_err(|e| e.to_string())?;
    const LIMIT: u64 = 8 * 1024 * 1024;
    if file.metadata().await.map_err(|e| e.to_string())?.len() > LIMIT {
        return Ok(None);
    }
    let mut bytes = Vec::new();
    file.take(LIMIT + 1)
        .read_to_end(&mut bytes)
        .await
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > LIMIT {
        return Ok(None);
    }
    let mime = if bytes.starts_with(&[0x89, b'P', b'N', b'G', 13, 10, 26, 10]) {
        "image/png"
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        "image/jpeg"
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP".as_slice()) {
        "image/webp"
    } else {
        return Ok(None);
    };
    Ok(Some(format!(
        "data:{};base64,{}",
        mime,
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )))
}

#[tauri::command]
pub fn list_platforms() -> Vec<crate::providers::ProviderInfo> {
    crate::providers::catalog()
}


