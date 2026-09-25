use crate::media_tools::tool;
use crate::{
    bilibili,
    engine::{output_name, Engine, Shared},
    models::{now, DownloadTask, Status},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    sync::atomic::{AtomicU8, Ordering},
    time::Duration,
};
use tauri::{AppHandle, Manager, WebviewWindow};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
};

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Selection {
    pub kind: String,
    pub format: Option<String>,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Format {
    pub id: String,
    pub label: String,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub author: Option<String>,
    pub topics: Vec<String>,
    pub url: String,
    pub title: String,
    pub thumbnail: Option<String>,
    pub formats: Vec<Format>,
    pub has_audio: bool,
}

pub fn canonical(input: &str) -> Result<String, String> {
    let provider = crate::providers::for_url(input)?;
    if provider.info().id != "bilibili" {
        return Err("请输入 Bilibili 视频链接".into());
    }
    let url = url::Url::parse(input).map_err(|_| "视频链接无效")?;
    if url.host_str() == Some("b23.tv") {
        if url.path().trim_matches('/').is_empty() {
            return Err("短链接无效".into());
        }
        return Ok(format!("https://b23.tv{}", url.path()));
    }
    let id = url
        .path()
        .trim_matches('/')
        .strip_prefix("video/")
        .ok_or("目前支持普通视频 BV / av 链接")?;
    let id = id.trim_end_matches('/');
    if !(id.starts_with("BV") && id.len() == 12 && id.bytes().all(|c| c.is_ascii_alphanumeric())
        || id
            .strip_prefix("av")
            .is_some_and(|s| !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit())))
    {
        return Err("视频编号无效".into());
    }
    let part = url
        .query_pairs()
        .find(|(k, _)| k == "p")
        .map(|(_, v)| {
            v.parse::<u32>()
                .ok()
                .filter(|n| *n > 0)
                .ok_or("分 P 编号无效")
        })
        .transpose()?
        .unwrap_or(1);
    Ok(format!("https://www.bilibili.com/video/{id}?p={part}"))
}
async fn resolve(input: &str) -> Result<String, String> {
    let mut value = canonical(input)?;
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0")
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;
    for _ in 0..5 {
        if url::Url::parse(&value).unwrap().host_str() != Some("b23.tv") {
            return Ok(value);
        }
        let r = client
            .get(&value)
            .send()
            .await
            .map_err(|_| "Bilibili 短链接解析失败")?;
        if !r.status().is_redirection() {
            return Err("Bilibili 短链接没有指向视频".into());
        }
        let next = r
            .headers()
            .get("location")
            .and_then(|s| s.to_str().ok())
            .ok_or("短链接跳转无效")?;
        value = canonical(
            url::Url::parse(&value)
                .unwrap()
                .join(next)
                .map_err(|_| "短链接跳转无效")?
                .as_str(),
        )?;
    }
    Err("短链接跳转次数过多".into())
}

struct CookieFile(PathBuf);
impl Drop for CookieFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
fn cookies(app: &AppHandle) -> Result<CookieFile, String> {
    let window = bilibili::window(app)?;
    let dir = app
        .path()
        .app_local_data_dir()
        .map_err(|e| e.to_string())?
        .join("bilibili-private");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("cookies-{}.txt", uuid::Uuid::new_v4()));
    let mut text = String::from("# Netscape HTTP Cookie File\n");
    for c in crate::browser_cookies::for_url(&window, "https://www.bilibili.com/".parse().unwrap())
        .map_err(|_| "无法读取 Bilibili 会话")?
    {
        let domain = c.domain().unwrap_or(".bilibili.com");
        if !(domain.trim_start_matches('.') == "bilibili.com" || domain.ends_with(".bilibili.com"))
        {
            continue;
        }
        if [domain, c.name(), c.value(), c.path().unwrap_or("/")]
            .iter()
            .any(|s| s.contains(['\r', '\n', '\t']))
        {
            continue;
        }
        text.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            domain,
            if domain.starts_with('.') {
                "TRUE"
            } else {
                "FALSE"
            },
            c.path().unwrap_or("/"),
            if c.secure().unwrap_or(false) {
                "TRUE"
            } else {
                "FALSE"
            },
            c.expires_datetime()
                .map(|d| d.unix_timestamp())
                .unwrap_or(0),
            c.name(),
            c.value()
        ));
    }
    use std::io::Write;
    let guard = CookieFile(path);
    std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&guard.0)
        .and_then(|mut f| f.write_all(text.as_bytes()))
        .map_err(|_| "无法准备 Bilibili 本机会话")?;
    Ok(guard)
}
fn command(app: &AppHandle, cookie: &CookieFile) -> Result<Command, String> {
    let mut c = Command::new(tool(app, "yt-dlp")?);
    c.args([
        "--ignore-config",
        "--encoding",
        "utf-8",
        "--no-playlist",
        "--no-warnings",
        "--no-colors",
        "--socket-timeout",
        "20",
        "--retries",
        "2",
        "--extractor-retries",
        "1",
        "--no-check-formats",
    ])
    .arg("--cookies")
    .arg(&cookie.0)
    .stdin(Stdio::null())
    .kill_on_drop(true);
    #[cfg(unix)]
    c.process_group(0);
    #[cfg(windows)]
    c.creation_flags(0x08000000);
    Ok(c)
}
fn friendly_error(raw: &str) -> String {
    let s = raw.to_lowercase();
    if s.contains("requested format") {
        "所选清晰度已不可用，请重新解析并选择清晰度。".into()
    } else if s.contains("login") || s.contains("premium") {
        "此内容需要登录或账号权限，请在平台连接中重新登录 Bilibili。".into()
    } else if s.contains("412") || s.contains("403") || s.contains("352") {
        "Bilibili 暂时拒绝了请求，请完成官网登录验证后稍后重试。".into()
    } else if s.contains("timed out") || s.contains("timeout") {
        "Bilibili 请求超时，请检查网络后重试。".into()
    } else {
        "Bilibili 解析或下载失败，请检查链接、登录状态及网络后重试。".into()
    }
}
fn image_url(raw: &str) -> Option<String> {
    let value = if raw.starts_with("//") {
        format!("https:{raw}")
    } else {
        raw.replacen("http://", "https://", 1)
    };
    let u = url::Url::parse(&value).ok()?;
    (u.scheme() == "https"
        && u.username().is_empty()
        && u.password().is_none()
        && u.port().is_none()
        && u.host_str().is_some_and(|h| {
            ["hdslb.com", "biliimg.com"]
                .iter()
                .any(|d| h == *d || h.ends_with(&format!(".{d}")))
        }))
    .then_some(value)
}
fn metadata(v: &Value, url: String) -> Result<Preview, String> {
    if v["_type"] == "playlist" {
        return Err("请使用包含分 P 编号的单视频链接".into());
    }
    let formats = v["formats"].as_array().ok_or("未解析到可下载格式")?;
    let audio = formats
        .iter()
        .filter(|f| f["vcodec"] == "none" && f["acodec"].as_str().is_some_and(|s| s != "none"))
        .max_by_key(|f| {
            (
                if f["ext"] == "m4a" { 1 } else { 0 },
                f["abr"].as_f64().unwrap_or(0.0) as u64,
            )
        })
        .and_then(|f| f["format_id"].as_str());
    // Prefer an AVC stream per quality for broad playback compatibility.
    let mut candidates: Vec<_> = formats
        .iter()
        .filter(|f| f["vcodec"].as_str().is_some_and(|s| s != "none"))
        .collect();
    candidates.sort_by_key(|f| {
        (
            std::cmp::Reverse(
                f["quality"]
                    .as_i64()
                    .unwrap_or(f["height"].as_i64().unwrap_or(0)),
            ),
            !f["vcodec"].as_str().unwrap_or("").starts_with("avc"),
        )
    });
    let mut qualities = std::collections::HashSet::new();
    let mut result = Vec::new();
    for f in candidates {
        if f["vcodec"].as_str().is_none_or(|s| s == "none") {
            continue;
        }
        let Some(id) = f["format_id"].as_str() else {
            continue;
        };
        if !safe_format(id) {
            continue;
        }
        let id = if f["acodec"] == "none" {
            let Some(a) = audio else { continue };
            if !safe_format(a) {
                continue;
            }
            format!("{id}+{a}")
        } else {
            id.to_owned()
        };
        let height = f["height"].as_u64().unwrap_or(0);
        let quality = f["quality"].as_i64().unwrap_or(height as i64);
        if !qualities.insert(quality) {
            continue;
        }
        let codec = f["vcodec"]
            .as_str()
            .unwrap_or("")
            .split('.')
            .next()
            .unwrap_or("");
        let name = f["format"]
            .as_str()
            .filter(|s| s.contains('P'))
            .map(str::to_owned)
            .unwrap_or_else(|| format!("{height}P"));
        let label = if codec == "avc1" || codec == "avc" {
            name
        } else {
            format!("{name} · 高效编码")
        };
        result.push(Format { id, label });
    }
    Ok(Preview {
        author: ["uploader", "channel", "creator"].iter()
            .filter_map(|key| v[*key].as_str()).map(str::trim)
            .find(|name| !name.is_empty()).map(str::to_owned),
        topics: video_topics(v),
        url,
        title: v["title"]
            .as_str()
            .unwrap_or("Bilibili 视频")
            .chars()
            .take(200)
            .collect(),
        thumbnail: v["thumbnail"].as_str().and_then(image_url),
        formats: result,
        has_audio: audio.is_some(),
    })
}
fn video_topics(v: &Value) -> Vec<String> {
    let mut topics = Vec::new();
    if let Some(tags) = v["tags"].as_array() {
        for tag in tags.iter().filter_map(Value::as_str) {
            let tag = tag.trim();
            if !tag.is_empty() && !topics.iter().any(|t| t == tag) { topics.push(tag.to_string()); }
        }
    }
    topics
}

fn safe_format(s: &str) -> bool {
    !s.is_empty()
        && s.len() < 100
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}
async fn preview(app: &AppHandle, input: &str) -> Result<Preview, String> {
    let url = resolve(input).await?;
    let cookie = cookies(app)?;
    let output = tokio::time::timeout(
        Duration::from_secs(75),
        command(app, &cookie)?
            .args(["--dump-single-json", "--skip-download", "--", &url])
            .output(),
    )
    .await
    .map_err(|_| "视频解析超时，请重试")?
    .map_err(|_| "下载组件启动失败")?;
    if !output.status.success() {
        return Err(friendly_error(&String::from_utf8_lossy(&output.stderr)));
    }
    let v = serde_json::from_slice(&output.stdout).map_err(|_| "视频信息读取失败")?;
    metadata(&v, url)
}
#[tauri::command]
pub async fn bilibili_preview(
    app: AppHandle,
    window: WebviewWindow,
    state: tauri::State<'_, Shared>,
    url: String,
) -> Result<Preview, String> {
    bilibili::main_only(&window)?;
    let result = preview(&app, &url).await?;
    let mut data = state.0.data.lock().await;
    let mut changed = false;
    for task in &mut data.tasks {
        if task.platform == "bilibili" && (task.url == url || task.url.split('?').next() == result.url.split('?').next()) {
            if !result.topics.is_empty() && task.topics != result.topics {
                task.topics = result.topics.clone();
                changed = true;
            }
            if let Some(author) = &result.author {
                if &task.source != author { task.source = author.clone(); changed = true; }
            }
        }
    }
    drop(data);
    if changed { state.0.persist(&app).await?; }
    Ok(result)
}
#[tauri::command]
pub async fn enqueue_bilibili(
    app: AppHandle,
    window: WebviewWindow,
    state: tauri::State<'_, Shared>,
    url: String,
    selection: Selection,
    include_cover: Option<bool>,
) -> Result<(), String> {
    bilibili::main_only(&window)?;
    if !["video", "cover", "audio"].contains(&selection.kind.as_str()) {
        return Err("下载类型无效".into());
    }
    let include_cover = include_cover.unwrap_or(false) && selection.kind == "video";
    let p = preview(&app, &url).await?;
    let selection = if selection.kind == "video" {
        if !p
            .formats
            .iter()
            .any(|f| Some(&f.id) == selection.format.as_ref())
        {
            return Err("清晰度已变化，请重新解析".into());
        }
        selection
    } else {
        Selection {
            kind: selection.kind,
            format: None,
        }
    };
    if (selection.kind == "cover" || include_cover) && p.thumbnail.is_none() {
        return Err("此视频没有可用封面".into());
    }
    if selection.kind == "audio" && !p.has_audio {
        return Err("此视频没有可用音频".into());
    }
    if selection.kind != "cover" {
        tool(&app, "ffmpeg")?;
    }
    let mut data = state.0.data.lock().await;
    if data.tasks.iter().any(|t| {
        t.url == p.url && (t.bilibili.as_ref() == Some(&selection) || (include_cover && t.bilibili.as_ref().is_some_and(|s| s.kind == "cover"))) && t.status != Status::Canceled
    }) {
        return Err("相同下载已在列表中，请继续或重试原任务".into());
    }
    let id = uuid::Uuid::new_v4().to_string();
    let folder = Path::new(&data.settings.download_dir);
    if !folder.is_absolute() {
        return Err("下载目录必须为绝对路径".into());
    }
    let suffix = match selection.kind.as_str() {
        "cover" => "封面.jpg",
        "audio" => "音频.m4a",
        _ => "视频.mp4",
    };
    let file_name = crate::links::safe_file_name(&format!("{}-{suffix}", p.title));
    let output = folder.join(output_name(&id, &file_name));
    let mut job = DownloadTask { topics: p.topics, storage: None, xiaohongshu: None,
        bilibili: Some(selection),
        discovery: None,
        batch: None,
        id,
        platform: "bilibili".into(),
        url: p.url,
        title: p.title,
        file_name,
        thumbnail: p.thumbnail,
        total_bytes: 0,
        downloaded_bytes: 0,
        speed: 0,
        status: Status::Queued,
        output_path: output.to_string_lossy().into_owned(),
        created_at: now(),
        updated_at: now(),
        error: None,
        source: p.author.unwrap_or_else(|| "哔哩哔哩".into()),
        media_id: 0,
    };
    crate::storage_layout::initialize(&mut job, folder, &data.tasks);
    let title = job.title.clone();
    crate::storage_layout::resolve(&mut job, &title, suffix)?;
    let cover = if include_cover { Some(companion_cover(&job)?) } else { None };
    data.tasks.insert(0, job);
    if let Some(cover) = cover { data.tasks.insert(1, cover); }
    drop(data);
    state.0.persist(&app).await
}
fn companion_cover(video: &DownloadTask) -> Result<DownloadTask, String> {
    let mut cover = video.clone();
    cover.id = uuid::Uuid::new_v4().to_string();
    cover.bilibili = Some(Selection { kind: "cover".into(), format: None });
    crate::storage_layout::resolve(&mut cover, &video.title, "封面.jpg")?;
    Ok(cover)
}

pub fn work_dir(job: &DownloadTask) -> Result<PathBuf, String> {
    if uuid::Uuid::parse_str(&job.id).is_err() {
        return Err("任务编号无效".into());
    }
    let parent = Path::new(&job.output_path)
        .parent()
        .filter(|p| p.is_absolute())
        .ok_or("下载目录无效")?;
    let dir = parent.join(format!(".framefetch-{}.bilibili", job.id));
    if let Ok(m) = std::fs::symlink_metadata(&dir) {
        if m.file_type().is_symlink() || !m.is_dir() {
            return Err("任务临时路径无效".into());
        }
    }
    Ok(dir)
}
pub fn cleanup(job: &DownloadTask) -> Result<(), String> {
    if job.platform != "bilibili" {
        return Ok(());
    }
    let dir = work_dir(job)?;
    if !dir.exists() {
        return Ok(());
    }
    // Only delete regular files in this UUID-owned flat work directory.
    let files = std::fs::read_dir(&dir)
        .map_err(|e| e.to_string())?
        .map(|entry| {
            entry.map_err(|e| e.to_string()).and_then(|e| {
                let t = e.file_type().map_err(|e| e.to_string())?;
                if !t.is_file() || t.is_symlink() {
                    return Err("临时目录存在非普通文件，已保留".into());
                }
                Ok(e.path())
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    for f in files {
        std::fs::remove_file(f).map_err(|e| e.to_string())?;
    }
    std::fs::remove_dir(dir).map_err(|e| e.to_string())
}
async fn stop(child: &mut tokio::process::Child) {
    #[cfg(windows)]
    if let Some(id) = child.id() {
        let mut c = Command::new("taskkill.exe");
        c.args(["/PID", &id.to_string(), "/T", "/F"])
            .creation_flags(0x08000000)
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let _ = c.status().await;
    }
    #[cfg(unix)]
    if let Some(id) = child.id() {
        // Stop ffmpeg descendants together with their yt-dlp parent.
        let _ = Command::new("/bin/kill")
            .args(["-KILL", "--", &format!("-{id}")])
            .stdout(Stdio::null()).stderr(Stdio::null()).status().await;
    }
    let _ = child.kill().await;
    let _ = child.wait().await;
}
fn prepare_default(job: &mut DownloadTask, p: Preview) -> Result<(), String> {
    let format = p
        .formats
        .first()
        .ok_or("没有可下载的视频格式，请连接 Bilibili 后重试")?
        .id
        .clone();
    let name = crate::links::safe_file_name(&format!("{}-视频.mp4", p.title));
    let output = Path::new(&job.output_path)
        .parent()
        .ok_or("下载目录无效")?
        .join(output_name(&job.id, &name));
    job.url = p.url;
    job.topics = p.topics;
    if let Some(author) = p.author { job.source = author; }
    job.title = p.title;
    job.thumbnail = p.thumbnail;
    job.file_name = name;
    job.output_path = output.to_string_lossy().into_owned();
    if job.storage.is_some() {
        let title = job.title.clone();
        crate::storage_layout::resolve(job, &title, "视频.mp4")?;
    }
    job.bilibili = Some(Selection {
        kind: "video".into(),
        format: Some(format),
    });
    Ok(())
}

pub async fn transfer(
    engine: &Engine,
    job: &DownloadTask,
    control: &AtomicU8,
    app: &AppHandle,
) -> Result<(), String> {
    // Default link downloads resolve in the queue, without a preview dialog.
    let mut prepared = job.clone();
    if prepared
        .bilibili
        .as_ref()
        .is_some_and(|s| s.kind == "video" && s.format.is_none())
    {
        let p = tokio::select! {
            value = preview(app, &job.url) => value?,
            _ = wait_stopped(control) => return Ok(()),
        };
        prepare_default(&mut prepared, p)?;
        let mut data = engine.data.lock().await;
        if control.load(Ordering::SeqCst) != 0 {
            return Ok(());
        }
        let task = data
            .tasks
            .iter_mut()
            .find(|t| t.id == job.id)
            .ok_or("下载任务已移除")?;
        *task = prepared.clone();
        drop(data);
        engine.persist(app).await?;
    }
    let job = &prepared;
    let selection = job.bilibili.as_ref().ok_or("请重新创建 Bilibili 任务")?;
    let dir = work_dir(job)?;
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| e.to_string())?;
    if control.load(Ordering::SeqCst) != 0 {
        return Ok(());
    }
    if selection.kind == "cover" {
        return cover(engine, job, control, app, &dir).await;
    }
    let cookie = cookies(app)?;
    let ffmpeg = tool(app, "ffmpeg")?;
    let mut cmd = command(app, &cookie)?;
    cmd.arg("--ffmpeg-location")
        .arg(ffmpeg.parent().unwrap())
        .args([
            "--newline",
            "--progress",
            "--continue",
            "--no-overwrites",
            "--progress-template",
            "download:FFPROGRESS:%(progress)j",
            "--print",
            "after_move:FFFILE:%(filepath)s",
        ])
        .arg("-o")
        .arg(dir.join("media.%(ext)s"));
    if selection.kind == "audio" {
        cmd.args([
            "-f",
            "bestaudio[ext=m4a]/bestaudio",
            "-x",
            "--audio-format",
            "m4a",
        ]);
    } else {
        cmd.args([
            "-f",
            selection.format.as_deref().ok_or("请选择清晰度")?,
            "--merge-output-format",
            "mp4",
            "--remux-video",
            "mp4",
        ]);
    }
    cmd.args(["--", &job.url])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|_| "下载组件启动失败")?;
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
    let mut errors = BufReader::new(child.stderr.take().unwrap()).lines();
    let mut error = String::new();
    let mut out_done = false;
    let mut err_done = false;
    let mut finished = None;
    let mut streams = std::collections::HashMap::<String, (u64, u64)>::new();
    {
        let mut data = engine.data.lock().await;
        if let Some(t) = data.tasks.iter_mut().find(|t| t.id == job.id) {
            if control.load(Ordering::SeqCst) == 0 {
                t.status = Status::Downloading;
            }
        }
    }
    engine.emit(app);
    let started = std::time::Instant::now();
    loop {
        if control.load(Ordering::SeqCst) != 0 {
            stop(&mut child).await;
            return Ok(());
        }
        tokio::select! {
            line=lines.next_line(), if !out_done => {match line {
                Ok(Some(s))=>{
                    if let Some(raw)=s.strip_prefix("FFPROGRESS:"){if let Ok(v)=serde_json::from_str::<Value>(raw){
                        let downloaded=v["downloaded_bytes"].as_u64().unwrap_or(0);let total=v["total_bytes"].as_u64().or_else(||v["total_bytes_estimate"].as_u64()).unwrap_or(0);
                        let name=v["filename"].as_str().unwrap_or("media").to_owned();
                        streams.insert(name,(downloaded,total));
                        let downloaded=streams.values().map(|(d,_)|*d).sum();let total=streams.values().map(|(_,t)|*t).sum();
                        if let Some(t)=engine.data.lock().await.tasks.iter_mut().find(|t|t.id==job.id){t.total_bytes=total;}
                        engine.update_progress(&job.id,downloaded,if v["status"]=="finished" {0}else{v["speed"].as_f64().unwrap_or(0.0) as u64},app).await;
                    }}
                    if let Some(file)=s.strip_prefix("FFFILE:"){finished=Some(PathBuf::from(file));}
                },Ok(None)=>out_done=true,Err(_)=>{stop(&mut child).await;return Err("下载进度读取失败".into());}
            }},
            line=errors.next_line(), if !err_done => {match line{Ok(Some(s))=>{if error.len()<16000{error.push_str(&s);}},_=>err_done=true}},
            _=tokio::time::sleep(Duration::from_millis(100))=>{}
        }
        if out_done && err_done {
            break;
        }
        if started.elapsed() > Duration::from_secs(86400) {
            stop(&mut child).await;
            return Err("下载超时，请继续任务重试".into());
        }
    }
    let status = child.wait().await.map_err(|_| "下载进程异常退出")?;
    if control.load(Ordering::SeqCst) != 0 {
        return Ok(());
    }
    if !status.success() {
        return Err(friendly_error(&error));
    }
    let path = finished.ok_or("下载组件未返回完成文件")?;
    if path.parent() != Some(dir.as_path()) || !path.is_file() {
        return Err("下载文件路径无效".into());
    }
    commit(engine, job, &path, control, app).await?;
    if control.load(Ordering::SeqCst) != 0 {
        return Ok(());
    }
    cleanup(job)?;
    Ok(())
}
async fn commit(
    engine: &Engine,
    job: &DownloadTask,
    path: &Path,
    control: &AtomicU8,
    app: &AppHandle,
) -> Result<(), String> {
    if control.load(Ordering::SeqCst) != 0 {
        return Ok(());
    }
    let size = std::fs::metadata(path).map_err(|e| e.to_string())?.len();
    if size == 0 {
        return Err("下载文件为空".into());
    }
    crate::work_files::stage_file(path, Path::new(&job.output_path))?;
    if let Some(t) = engine
        .data
        .lock()
        .await
        .tasks
        .iter_mut()
        .find(|t| t.id == job.id)
    {
        t.total_bytes = size;
        t.downloaded_bytes = size;
        t.speed = 0;
    }
    engine.emit(app);
    Ok(())
}
async fn cover(
    engine: &Engine,
    job: &DownloadTask,
    control: &AtomicU8,
    app: &AppHandle,
    dir: &Path,
) -> Result<(), String> {
    use futures_util::StreamExt;
    use tokio::io::AsyncWriteExt;
    let url = job
        .thumbnail
        .as_deref()
        .and_then(image_url)
        .ok_or("封面地址无效，请重新解析")?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(45))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| e.to_string())?;
    let response = tokio::select! {r=client.get(url).header("Referer","https://www.bilibili.com/").send()=>r.map_err(|_|"封面下载失败")?.error_for_status().map_err(|_|"封面已失效，请重新解析")?,_=wait_stopped(control)=>return Ok(())};
    let content = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let ext = match content.split(';').next().unwrap_or("") {
        "image/jpeg" => "jpg",
        "image/png" => "png",
        "image/webp" => "webp",
        _ => return Err("封面返回了不支持的图片格式".into()),
    };
    let mut prepared = job.clone();
    let output = Path::new(&job.output_path).with_extension(ext);
    prepared.output_path = output.to_string_lossy().into_owned();
    prepared.file_name = Path::new(&job.file_name)
        .with_extension(ext)
        .to_string_lossy()
        .into_owned();
    {
        let mut data = engine.data.lock().await;
        if let Some(t) = data.tasks.iter_mut().find(|t| t.id == job.id) {
            if control.load(Ordering::SeqCst) != 0 {
                return Ok(());
            }
            t.output_path = prepared.output_path.clone();
            t.file_name = prepared.file_name.clone();
            t.status = Status::Downloading;
            t.total_bytes = response.content_length().unwrap_or(0);
        }
    }
    engine.persist(app).await?;
    let path = dir.join("cover.part");
    let mut f = tokio::fs::File::create(&path)
        .await
        .map_err(|e| e.to_string())?;
    let mut stream = response.bytes_stream();
    let mut size = 0;
    loop {
        let next = tokio::select! {v=stream.next()=>v,_=wait_stopped(control)=>return Ok(())};
        let Some(chunk) = next else { break };
        let bytes = chunk.map_err(|_| "封面下载中断")?;
        size += bytes.len() as u64;
        if size > 30_000_000 {
            return Err("封面文件过大".into());
        }
        f.write_all(&bytes).await.map_err(|e| e.to_string())?;
        engine.update_progress(&job.id, size, 0, app).await;
    }
    f.sync_all().await.map_err(|e| e.to_string())?;
    drop(f);
    commit(engine, &prepared, &path, control, app).await?;
    cleanup(job)
}
async fn wait_stopped(control: &AtomicU8) {
    while control.load(Ordering::SeqCst) == 0 {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
#[cfg(test)]
mod tests {

    #[test]
    fn video_and_cover_share_storage_but_have_distinct_files_and_workers() {
        let root = std::env::temp_dir().join("framefetch-combined-test");
        let mut video: DownloadTask = serde_json::from_value(serde_json::json!({
            "id":uuid::Uuid::new_v4().to_string(),"platform":"bilibili",
            "url":"https://www.bilibili.com/video/BV1xx411c7mD?p=1",
            "title":"Example","fileName":"pending","totalBytes":0,
            "downloadedBytes":0,"speed":0,"status":"queued",
            "outputPath":root.join("pending").to_string_lossy(),
            "createdAt":1,"error":null,"source":"Author",
            "bilibili":{"kind":"video","format":"80+30280"}
        })).unwrap();
        crate::storage_layout::initialize(&mut video, &root, &[]);
        crate::storage_layout::resolve(&mut video, "Example", "视频.mp4").unwrap();
        let cover = companion_cover(&video).unwrap();
        assert_ne!(video.id, cover.id);
        assert_eq!(Path::new(&video.output_path).parent(), Path::new(&cover.output_path).parent());
        assert!(video.output_path.ends_with("视频.mp4"));
        assert!(cover.output_path.ends_with("封面.jpg"));
        assert_eq!(video.storage.as_ref().unwrap().work_id, cover.storage.as_ref().unwrap().work_id);
        assert_eq!(cover.bilibili.as_ref().unwrap().kind, "cover");
        assert!(cover.bilibili.as_ref().unwrap().format.is_none());
        assert_ne!(work_dir(&video).unwrap(), work_dir(&cover).unwrap());
        let jobs = vec![video.clone(), cover];
        assert_eq!(crate::removal::removal_targets(&jobs, &video.id).len(), 2);
    }
    use super::*;
    #[test]
    fn cancellation_cleanup_is_confined_to_the_task_directory() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target/test-data")
            .join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir_all(&root).unwrap();
        let job:DownloadTask=serde_json::from_value(serde_json::json!({"id":uuid::Uuid::new_v4().to_string(),"platform":"bilibili","url":"https://www.bilibili.com/video/BV1xx411c7mD?p=1","title":"test","fileName":"test.mp4","totalBytes":0,"downloadedBytes":0,"speed":0,"status":"paused","outputPath":root.join("test.mp4").to_string_lossy(),"createdAt":1,"error":null,"source":"Bilibili"})).unwrap();
        let dir = work_dir(&job).unwrap();
        std::fs::create_dir_all(&dir).unwrap();
        let neighbor = root.join("keep.mp4");
        std::fs::write(&neighbor, b"keep").unwrap();
        std::fs::write(dir.join("media.part"), b"partial").unwrap();
        cleanup(&job).unwrap();
        assert!(!dir.exists());
        assert_eq!(std::fs::read(&neighbor).unwrap(), b"keep");
        std::fs::create_dir_all(dir.join("unexpected-directory")).unwrap();
        std::fs::write(dir.join("media.part"), b"partial").unwrap();
        assert!(cleanup(&job).is_err());
        assert!(dir.join("media.part").exists());
    }
    #[test]
    fn default_link_selects_first_available_video_and_persists_resolved_metadata() {
        let mut job: DownloadTask=serde_json::from_value(serde_json::json!({"id":"12345678-1234-1234-1234-123456789abc","platform":"bilibili","url":"https://b23.tv/test","title":"待解析","fileName":"pending","totalBytes":0,"downloadedBytes":0,"speed":0,"status":"resolving","outputPath":std::env::temp_dir().join("pending").to_string_lossy(),"createdAt":1,"error":null,"source":"Bilibili"})).unwrap();
        let p=Preview{author:Some("示例UP主".into()),topics: vec!["游戏".into(), "赛事".into()], url:"https://www.bilibili.com/video/BV1xx411c7mD?p=1".into(),title:"Example".into(),thumbnail:Some("https://i0.hdslb.com/cover.jpg".into()),formats:vec![Format{id:"80+30280".into(),label:"1080P".into()},Format{id:"64+30280".into(),label:"720P".into()}],has_audio:true};
        prepare_default(&mut job,p).unwrap();
        assert_eq!(job.topics, vec!["游戏", "赛事"]);
        assert_eq!(job.source, "示例UP主");
        let saved: DownloadTask = serde_json::from_slice(&serde_json::to_vec(&job).unwrap()).unwrap();
        assert_eq!(saved.topics, job.topics);
        assert_eq!(saved.source, "示例UP主");
        let selection=job.bilibili.as_ref().unwrap();assert_eq!(selection.kind,"video");assert_eq!(selection.format.as_deref(),Some("80+30280"));assert_eq!(job.title,"Example");assert!(job.output_path.ends_with("12345678-Example-视频.mp4"));assert!(job.thumbnail.is_some());
        let empty=Preview{author:Some("示例UP主".into()),topics: vec!["游戏".into(), "赛事".into()], url:job.url.clone(),title:"Example".into(),thumbnail:None,formats:vec![],has_audio:true};
        assert!(prepare_default(&mut job,empty).is_err());
        let mut new_job = job.clone();
        crate::storage_layout::initialize(&mut new_job, &std::env::temp_dir(), &[]);
        let p = Preview {author:Some("示例UP主".into()),topics: vec!["游戏".into()], url: job.url.clone(), title: "新作品".into(), thumbnail: None, formats: vec![Format {id:"80+30280".into(),label:"1080P".into()}], has_audio:true};
        prepare_default(&mut new_job, p).unwrap();
        assert_eq!(new_job.file_name, "视频.mp4");
        assert!(Path::new(&new_job.output_path).starts_with(std::env::temp_dir().join("Bilibili")));
    }
    #[test]
    fn canonical_limits_urls_and_preserves_part() {
        assert_eq!(
            canonical("https://m.bilibili.com/video/BV1xx411c7mD?p=3&share=bad").unwrap(),
            "https://www.bilibili.com/video/BV1xx411c7mD?p=3"
        );
        for s in [
            "https://bilibili.com.evil.test/video/BV1xx411c7mD",
            "https://user@bilibili.com/video/BV1xx411c7mD",
            "https://www.bilibili.com/video/../../x",
            "https://www.bilibili.com/video/BV1xx411c7mD?p=0",
        ] {
            assert!(canonical(s).is_err());
        }
    }
    #[test]
    fn actual_formats_only_and_images_restricted() {
        let author = metadata(&serde_json::json!({"formats":[],"uploader":" 视频作者 ","channel":"频道名称"}), "https://www.bilibili.com/video/BV1xx411c7mD?p=1".into()).unwrap();
        assert_eq!(author.author.as_deref(), Some("视频作者"));
        assert_eq!(metadata(&serde_json::json!({"formats":[],"uploader":" ","channel":"备用作者"}), "video".into()).unwrap().author.as_deref(), Some("备用作者"));
        assert!(metadata(&serde_json::json!({"formats":[]}), "video".into()).unwrap().author.is_none());
        let tags = serde_json::json!({"formats":[],"tags":["游戏"," 赛事 ","游戏","",null,12]});
        assert_eq!(metadata(&tags, "https://www.bilibili.com/video/BV1xx411c7mD?p=1".into()).unwrap().topics, vec!["游戏", "赛事"]);
        assert!(video_topics(&serde_json::json!({})).is_empty());
        let v = serde_json::json!({"title":"test","thumbnail":"https://i0.hdslb.com/bfs/test.jpg","formats":[{"format_id":"30280","vcodec":"none","acodec":"mp4a","ext":"m4a"},{"format_id":"80","vcodec":"avc1","acodec":"none","height":1080,"fps":30}]});
        let p = metadata(&v, "url".into()).unwrap();
        assert!(p.has_audio);
        assert_eq!(p.formats.len(), 1);
        assert_eq!(p.formats[0].id, "80+30280");
        assert!(image_url("https://hdslb.com.evil.test/cover").is_none());
        assert!(!safe_format("best/evil"));
    }
}
