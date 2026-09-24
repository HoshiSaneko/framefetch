use crate::{
    engine::{output_name, Engine, Shared},
    models::{now, DownloadTask, Status},
    xiaohongshu,
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
    #[serde(default)]
    pub image_index: Option<usize>,
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
    pub images: Vec<String>,
    #[serde(skip_serializing)]
    info: Value,
}

pub fn default_selection() -> Selection {
    Selection {
        kind: "auto".into(),
        format: None,
        image_index: None,
    }
}
pub fn canonical(input: &str) -> Result<String, String> {
    let input = input
        .split_whitespace()
        .find(|s| s.starts_with("https://") || s.starts_with("http://"))
        .unwrap_or(input)
        .trim_end_matches(['。', '，', ')', '）']);
    if crate::providers::for_url(input)?.info().id != "xiaohongshu" {
        return Err("请输入小红书作品链接".into());
    }
    let u = url::Url::parse(input).map_err(|_| "作品链接无效")?;
    if matches!(u.host_str(), Some("xhslink.com" | "www.xhslink.com")) {
        let path = u.path().trim_matches('/');
        if path.is_empty() || !path.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'/') {
            return Err("分享短链接无效".into());
        }
        return Ok(format!("https://xhslink.com/{path}"));
    }
    let segments: Vec<_> = u.path().trim_matches('/').split('/').collect();
    let id = match segments.as_slice() {
        ["explore", id] | ["discovery", "item", id] | ["user", "profile", _, id] => *id,
        _ => return Err("请复制单篇小红书笔记的分享链接".into()),
    };
    if id.len() != 24 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("小红书作品编号无效".into());
    }
    let mut result = url::Url::parse(&format!("https://www.xiaohongshu.com/explore/{id}")).unwrap();
    // The security token is part of the shared link, not disposable tracking data.
    for (k, v) in u
        .query_pairs()
        .filter(|(k, _)| matches!(k.as_ref(), "xsec_token" | "xsec_source"))
    {
        result.query_pairs_mut().append_pair(&k, &v);
    }
    Ok(result.into())
}
fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder().user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/132.0.0.0 Safari/537.36")
        .redirect(reqwest::redirect::Policy::none()).timeout(Duration::from_secs(20)).build().map_err(|e|e.to_string())
}
async fn resolve(input: &str) -> Result<String, String> {
    let mut value = canonical(input)?;
    let client = client()?;
    for _ in 0..5 {
        if url::Url::parse(&value).unwrap().host_str() != Some("xhslink.com") {
            return Ok(value);
        }
        let r = client
            .get(&value)
            .send()
            .await
            .map_err(|_| "小红书短链接解析失败")?;
        if !r.status().is_redirection() {
            return Err("分享链接已失效，请重新复制链接".into());
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
pub(crate) fn media_url(raw: &str) -> Option<String> {
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
            ["xhscdn.com", "xiaohongshu.com"]
                .iter()
                .any(|d| h == *d || h.ends_with(&format!(".{d}")))
        }))
    .then_some(value)
}
// Parse data only. Never execute JavaScript from a fetched page.
fn initial_state(html: &str) -> Result<Value, String> {
    let raw = html
        .split_once("window.__INITIAL_STATE__")
        .and_then(|(_, s)| s.split_once('=').map(|(_, v)| v))
        .ok_or("未找到笔记内容，请更新分享链接或登录小红书后重试")?;
    let raw = raw.split("</script>").next().unwrap_or(raw).trim();
    let bytes = raw.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut i = 0;
    let mut quoted = false;
    let mut escaped = false;
    while i < bytes.len() {
        let b = bytes[i];
        if quoted {
            output.push(b);
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                quoted = false;
            }
            i += 1;
            continue;
        }
        if b == b'"' {
            quoted = true;
        }
        if bytes[i..].starts_with(b"undefined") {
            output.extend_from_slice(b"null");
            i += 9;
        } else {
            output.push(b);
            i += 1;
        }
    }
    serde_json::Deserializer::from_slice(&output)
        .into_iter::<Value>()
        .next()
        .ok_or("笔记数据为空")?
        .map_err(|_| "笔记数据格式已变化，请更新应用".into())
}
fn note_topics(note: &Value) -> Vec<String> {
    static PATTERN: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(||
        regex::Regex::new(r"#([^#\s]+?)\[话题\]#|#([^#\s，。！？、；：,!?;:]+)").expect("valid topics"));
    let mut topics = Vec::new();
    let mut add = |value: &str| {
        let value = value.trim().trim_start_matches('#').trim_end_matches('#').trim_end_matches("[话题]").trim();
        if !value.is_empty() && !topics.iter().any(|t| t == value) { topics.push(value.to_string()); }
    };
    for field in ["title", "desc"] {
        for capture in PATTERN.captures_iter(note[field].as_str().unwrap_or("")) {
            if let Some(topic) = capture.get(1).or_else(|| capture.get(2)) { add(topic.as_str()); }
        }
    }
    if let Some(tags) = note["tagList"].as_array() {
        for tag in tags {
            if tag["type"].as_str().is_some_and(|kind| kind != "topic") { continue; }
            if let Some(name) = tag["name"].as_str() { add(name); }
        }
    }
    topics
}

fn note_metadata(state: &Value, url: String) -> Result<Preview, String> {
    let parsed = url::Url::parse(&url).map_err(|_| "作品链接无效")?;
    let id = parsed
        .path_segments()
        .and_then(|mut s| s.next_back())
        .ok_or("作品编号无效")?;
    let note = &state["note"]["noteDetailMap"][id]["note"];
    if !note.is_object() {
        return Err("笔记不可访问，请使用最新分享链接并在平台连接中登录小红书".into());
    }
    let title = note["title"]
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| note["desc"].as_str())
        .unwrap_or("小红书作品")
        .chars()
        .take(160)
        .collect::<String>();
    let images = note["imageList"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    item["infoList"]
                        .as_array()
                        .and_then(|infos| infos.iter().find(|i| i["imageScene"] == "WB_DFT"))
                        .and_then(|i| i["url"].as_str())
                        .and_then(media_url)
                        .or_else(|| item["urlDefault"].as_str().and_then(media_url))
                        .or_else(|| item["urlPre"].as_str().and_then(media_url))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let mut raw_formats = Vec::new();
    if let Some(streams) = note["video"]["media"]["stream"].as_object() {
        for (codec, streams) in streams {
            if let Some(streams) = streams.as_array() {
                for stream in streams {
                    let Some(url) =
                        stream["masterUrl"]
                            .as_str()
                            .and_then(media_url)
                            .or_else(|| {
                                stream["backupUrls"].as_array().and_then(|a| {
                                    a.iter()
                                        .filter_map(|s| s.as_str().and_then(media_url))
                                        .next()
                                })
                            })
                    else {
                        continue;
                    };
                    let width = stream["width"].as_u64().unwrap_or(0);
                    let height = stream["height"].as_u64().unwrap_or(0);
                    let quality = width.min(height);
                    let bitrate = stream["avgBitrate"].as_u64().unwrap_or(0);
                    // Stable identifiers survive re-parsing, including reordered response arrays.
                    let codec = codec
                        .chars()
                        .filter(|c| c.is_ascii_alphanumeric())
                        .collect::<String>();
                    let fid = format!("{codec}-{width}x{height}-{bitrate}");
                    raw_formats.push(serde_json::json!({"format_id":fid,"url":url,"ext":"mp4","width":width,"height":height,
                    "quality":quality,"tbr":bitrate as f64/1000.,"vcodec":stream["videoCodec"].as_str().unwrap_or(&codec),
                    "acodec":stream["audioCodec"].as_str().unwrap_or("aac"),"filesize":stream["size"].as_u64()}));
                }
            }
        }
    }
    raw_formats.sort_by_key(|f| {
        (
            std::cmp::Reverse(f["quality"].as_u64().unwrap_or(0)),
            !matches!(f["vcodec"].as_str(), Some("h264" | "avc1")),
            std::cmp::Reverse(f["tbr"].as_f64().unwrap_or(0.) as u64),
        )
    });
    raw_formats.dedup_by(|a, b| a["format_id"] == b["format_id"]);
    let mut qualities = std::collections::HashSet::new();
    let formats = raw_formats
        .iter()
        .filter(|f| qualities.insert(f["quality"].as_u64().unwrap_or(0)))
        .map(|f| Format {
            id: f["format_id"].as_str().unwrap().into(),
            label: match f["quality"].as_u64().unwrap_or(0) {
                0 => "默认画质".into(),
                q => format!("{q}P"),
            },
        })
        .collect::<Vec<_>>();
    let is_video = note["type"] == "video" || !formats.is_empty();
    if is_video && formats.is_empty() {
        return Err("未获取到视频地址，请登录小红书后重新解析".into());
    }
    if !is_video && images.is_empty() {
        return Err("这篇笔记没有可下载的图片或视频".into());
    }
    let info = serde_json::json!({"id":id,"title":title,"extractor":"FrameFetchXHS","webpage_url":url,"formats":raw_formats,
        "http_headers":{"Referer":"https://www.xiaohongshu.com/","User-Agent":"Mozilla/5.0"}});
    Ok(Preview {
        author: ["nickname", "nickName"].iter()
            .filter_map(|key| note["user"][*key].as_str()).map(str::trim)
            .find(|name| !name.is_empty()).map(str::to_owned),
        topics: note_topics(note),
        url,
        title,
        thumbnail: images.first().cloned(),
        formats,
        has_audio: is_video,
        images: if is_video { vec![] } else { images },
        info,
    })
}
async fn preview(app: &AppHandle, input: &str) -> Result<Preview, String> {
    let url = resolve(input).await?;
    if let Ok(p) = http_preview(app, &url).await {
        return Ok(p);
    }
    let state = xiaohongshu::note(app, &url).await?;
    note_metadata(&state, url)
}
async fn http_preview(app: &AppHandle, url: &str) -> Result<Preview, String> {
    use futures_util::StreamExt;
    let response = client()?
        .get(url)
        .header("Referer", "https://www.xiaohongshu.com/")
        .header("Cookie", xiaohongshu::cookie_header(app)?)
        .send()
        .await
        .map_err(|_| "小红书请求失败，请检查网络")?;
    if !response.status().is_success() {
        return Err("小红书暂时拒绝访问，请打开官网登录验证，并重新复制分享链接".into());
    }
    let mut body = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| "笔记读取中断")?;
        if body.len() + chunk.len() > 8_000_000 {
            return Err("笔记数据过大".into());
        }
        body.extend_from_slice(&chunk);
    }
    note_metadata(
        &initial_state(&String::from_utf8_lossy(&body))?,
        url.to_owned(),
    )
}
#[tauri::command]
pub async fn xiaohongshu_preview(
    app: AppHandle,
    window: WebviewWindow,
    state: tauri::State<'_, Shared>,
    url: String,
) -> Result<Preview, String> {
    xiaohongshu::main_only(&window)?;
    let result = preview(&app, &url).await?;
    let mut data = state.0.data.lock().await;
    let mut changed = false;
    for task in &mut data.tasks {
        if task.platform == "xiaohongshu" && (task.url == url || task.url.split('?').next() == result.url.split('?').next()) {
            if task.topics != result.topics {
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
fn expand_selection(p: &Preview, s: &Selection) -> Result<Vec<Selection>, String> {
    let selection = match s.kind.as_str() {
        "auto" if !p.formats.is_empty() => Selection {
            kind: "video".into(),
            format: Some(p.formats[0].id.clone()),
            image_index: None,
        },
        "auto" => {
            return Ok((0..p.images.len())
                .map(|i| Selection {
                    kind: "image".into(),
                    format: None,
                    image_index: Some(i),
                })
                .collect())
        }
        "video" if p.formats.iter().any(|f| Some(&f.id) == s.format.as_ref()) => s.clone(),
        "cover" if p.thumbnail.is_some() => Selection {
            kind: "cover".into(),
            format: None,
            image_index: None,
        },
        "audio" if p.has_audio => Selection {
            kind: "audio".into(),
            format: None,
            image_index: None,
        },
        "image" if s.image_index.is_some_and(|i| i < p.images.len()) => Selection {
            kind: "image".into(),
            format: None,
            image_index: s.image_index,
        },
        _ => return Err("所选内容已不可用，请重新解析".into()),
    };
    Ok(vec![selection])
}
fn same_selection(t: &DownloadTask, url: &str, s: &Selection) -> bool {
    t.platform == "xiaohongshu"
        && t.url == url
        && t.xiaohongshu.as_ref() == Some(s)
        && t.status != Status::Canceled
}
fn prepare_job(job: &mut DownloadTask, p: &Preview, s: Selection, folder: &Path) -> Result<(), String> {
    let keep_path = job.xiaohongshu.as_ref() == Some(&s) && !job.output_path.is_empty() && !job.output_path.ends_with("pending");
    let suffix = match s.kind.as_str() {
        "cover" => "封面.jpg".into(),
        "audio" => "音频.m4a".into(),
        "image" => format!("图片-{:02}.jpg", s.image_index.unwrap_or(0) + 1),
        _ => "视频.mp4".into(),
    };
    job.title = p.title.clone();
    job.topics = p.topics.clone();
    if let Some(author) = &p.author { job.source = author.clone(); }
    job.url = p.url.clone();
    if !keep_path {
        if job.storage.is_some() {
            let name = if s.kind == "image" { crate::storage_layout::media_name("image", s.image_index, "jpg") } else { suffix };
            crate::storage_layout::resolve(job, &p.title, &name)?;
        } else {
            job.file_name = crate::links::safe_file_name(&format!("{}-{suffix}", p.title));
            job.output_path = folder.join(output_name(&job.id, &job.file_name)).to_string_lossy().into_owned();
        }
    }
    job.thumbnail = if s.kind == "image" {
        s.image_index.and_then(|i| p.images.get(i)).cloned()
    } else {
        p.thumbnail.clone()
    };
    job.xiaohongshu = Some(s);
    Ok(())
}
#[tauri::command]
pub async fn enqueue_xiaohongshu(
    app: AppHandle,
    window: WebviewWindow,
    state: tauri::State<'_, Shared>,
    url: String,
    selection: Selection,
    images: Option<Vec<usize>>,
) -> Result<(), String> {
    xiaohongshu::main_only(&window)?;
    let p = preview(&app, &url).await?;
    let selections = if selection.kind == "images" {
        let mut selected = images.ok_or("请选择图片")?;
        selected.sort_unstable();
        selected.dedup();
        if selected.is_empty() || selected.len() > 100 {
            return Err("请选择需要下载的图片".into());
        }
        selected
            .into_iter()
            .map(|i| {
                expand_selection(
                    &p,
                    &Selection {
                        kind: "image".into(),
                        format: None,
                        image_index: Some(i),
                    },
                )
                .map(|v| v[0].clone())
            })
            .collect::<Result<Vec<_>, _>>()?
    } else {
        expand_selection(&p, &selection)?
    };
    let mut data = state.0.data.lock().await;
    let folder = PathBuf::from(&data.settings.download_dir);
    if !folder.is_absolute() {
        return Err("下载目录必须为绝对路径".into());
    }
    let mut added = 0;
    let mut group_layout: Option<crate::storage_layout::StorageLayout> = None;
    for s in selections {
        if data.tasks.iter().any(|t| same_selection(t, &p.url, &s)) {
            continue;
        }
        let mut job = DownloadTask { topics: vec![], storage: None,
            xiaohongshu: None,
            bilibili: None,
            discovery: None,
            batch: None,
            id: uuid::Uuid::new_v4().to_string(),
            platform: "xiaohongshu".into(),
            url: p.url.clone(),
            title: p.title.clone(),
            file_name: String::new(),
            thumbnail: None,
            total_bytes: 0,
            downloaded_bytes: 0,
            speed: 0,
            status: Status::Queued,
            output_path: String::new(),
            created_at: now(),
            updated_at: now(),
            error: None,
            source: "小红书".into(),
            media_id: 0,
        };
        crate::storage_layout::initialize(&mut job, &folder, &data.tasks);
        if let Some(layout) = &group_layout { job.storage = Some(layout.clone()); }
        prepare_job(&mut job, &p, s, &folder)?;
        group_layout = job.storage.clone();
        data.tasks.insert(0, job);
        added += 1;
    }
    if added == 0 {
        return Err("所选内容已在下载列表中，请继续或重试原任务".into());
    }
    drop(data);
    state.0.persist(&app).await
}
fn command(app: &AppHandle) -> Result<Command, String> {
    let mut c = Command::new(tool(app, "yt-dlp.exe")?);
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
    ])
    .stdin(Stdio::null())
    .kill_on_drop(true);
    #[cfg(windows)]
    c.creation_flags(0x08000000);
    Ok(c)
}
fn tool(app: &AppHandle, name: &str) -> Result<PathBuf, String> {
    let mut roots = vec![app
        .path()
        .resource_dir()
        .map_err(|e| e.to_string())?
        .join("bin")];
    if cfg!(debug_assertions) {
        roots.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("bin"));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(p) = exe.parent() {
            roots.push(p.join("bin"));
        }
    }
    roots
        .into_iter()
        .map(|p| p.join(name))
        .find(|p| p.is_file())
        .ok_or_else(|| format!("缺少下载组件 {name}，请重新安装完整版本"))
}
fn friendly_error(raw: &str) -> String {
    let s = raw.to_lowercase();
    if s.contains("requested format") {
        "所选清晰度已不可用，请重新解析并选择清晰度。".into()
    } else if s.contains("login") || s.contains("premium") {
        "此内容需要登录或账号权限，请在平台连接中重新登录 小红书。".into()
    } else if s.contains("412") || s.contains("403") || s.contains("352") {
        "小红书 暂时拒绝了请求，请完成官网登录验证后稍后重试。".into()
    } else if s.contains("timed out") || s.contains("timeout") {
        "小红书 请求超时，请检查网络后重试。".into()
    } else {
        "小红书 解析或下载失败，请检查链接、登录状态及网络后重试。".into()
    }
}
pub fn work_dir(job: &DownloadTask) -> Result<PathBuf, String> {
    if uuid::Uuid::parse_str(&job.id).is_err() {
        return Err("任务编号无效".into());
    }
    let parent = Path::new(&job.output_path)
        .parent()
        .filter(|p| p.is_absolute())
        .ok_or("下载目录无效")?;
    let dir = parent.join(format!(".framefetch-{}.xiaohongshu", job.id));
    if let Ok(m) = std::fs::symlink_metadata(&dir) {
        if m.file_type().is_symlink() || !m.is_dir() {
            return Err("任务临时路径无效".into());
        }
    }
    Ok(dir)
}
pub fn cleanup(job: &DownloadTask) -> Result<(), String> {
    if job.platform != "xiaohongshu" {
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
    let _ = child.kill().await;
    let _ = child.wait().await;
}
pub async fn transfer(
    engine: &Engine,
    job: &DownloadTask,
    control: &AtomicU8,
    app: &AppHandle,
) -> Result<(), String> {
    let p = tokio::select! { value=preview(app,&job.url)=>value?, _=wait_stopped(control)=>return Ok(()) };
    let mut prepared = job.clone();
    let selection = prepared
        .xiaohongshu
        .clone()
        .unwrap_or_else(default_selection);
    let selections = expand_selection(&p, &selection)?;
    let first = selections.first().ok_or("没有可下载的内容")?.clone();
    let folder = Path::new(&job.output_path).parent().ok_or("下载目录无效")?;
    prepare_job(&mut prepared, &p, first, folder)?;
    {
        let mut data = engine.data.lock().await;
        if control.load(Ordering::SeqCst) != 0 {
            return Ok(());
        }
        let task = data
            .tasks
            .iter_mut()
            .find(|t| t.id == job.id)
            .ok_or("任务已移除")?;
        *task = prepared.clone();
        for selected in selections.into_iter().skip(1) {
            if data
                .tasks
                .iter()
                .any(|t| same_selection(t, &p.url, &selected))
            {
                continue;
            }
            let mut child = prepared.clone();
            child.id = uuid::Uuid::new_v4().to_string();
            child.status = Status::Queued;
            child.output_path.clear();
            child.downloaded_bytes = 0;
            child.total_bytes = 0;
            child.speed = 0;
            prepare_job(&mut child, &p, selected, folder)?;
            data.tasks.push(child);
        }
    }
    engine.persist(app).await?;
    let job = &prepared;
    let selection = job.xiaohongshu.as_ref().ok_or("任务缺少下载选项")?;
    let dir = work_dir(job)?;
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| e.to_string())?;
    if control.load(Ordering::SeqCst) != 0 {
        return Ok(());
    }
    if selection.kind == "cover" || selection.kind == "image" {
        return cover(engine, job, control, app, &dir).await;
    }
    let info_path = dir.join("media-info.json");
    std::fs::write(
        &info_path,
        serde_json::to_vec(&p.info).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let ffmpeg = tool(app, "ffmpeg.exe")?;
    let mut cmd = command(app)?;
    cmd.arg("--load-info-json").arg(&info_path);
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
        cmd.args(["-f", "bestaudio/best", "-x", "--audio-format", "m4a"]);
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
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
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
        .and_then(media_url)
        .ok_or("封面地址无效，请重新解析")?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(45))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| e.to_string())?;
    let response = tokio::select! {r=client.get(url).header("Referer","https://www.xiaohongshu.com/").send()=>r.map_err(|_|"封面下载失败")?.error_for_status().map_err(|_|"封面已失效，请重新解析")?,_=wait_stopped(control)=>return Ok(())};
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
    use super::*;
    const ID: &str = "6aa7b8680000000025037390";
    fn sample(note: Value) -> Value {
        serde_json::json!({"note":{"noteDetailMap":{ID:{"note":note}}}})
    }
    fn note_url() -> String {
        format!("https://www.xiaohongshu.com/explore/{ID}")
    }
    #[test]
    fn share_links_preserve_security_tokens_and_reject_other_routes() {
        let result=canonical(&format!("分享 https://www.xiaohongshu.com/discovery/item/{ID}?source=webshare&xsec_token=a%2Bb%3D&xsec_source=pc_share 复制打开")).unwrap();
        let parsed = url::Url::parse(&result).unwrap();
        assert_eq!(parsed.path(), format!("/explore/{ID}"));
        assert_eq!(
            parsed.query_pairs().collect::<Vec<_>>(),
            vec![
                ("xsec_token".into(), "a+b=".into()),
                ("xsec_source".into(), "pc_share".into())
            ]
        );
        assert_eq!(
            canonical("http://xhslink.com/a/Ab12").unwrap(),
            "https://xhslink.com/a/Ab12"
        );
        for u in [
            "https://xhslink.com.evil.test/a",
            "https://www.xiaohongshu.com/user/profile/abcd",
            "https://xiaohongshu.com/explore/123",
            "https://user@xhslink.com/Ab12",
            "https://xhslink.com:9000/Ab12",
        ] {
            assert!(canonical(u).is_err(), "{u}");
        }
    }
    #[test]
    fn parses_undefined_without_modifying_strings_or_executing_page_code() {
        let state=initial_state(r#"<script>window.__INITIAL_STATE__ = {"missing":undefined,"title":"undefined 可爱头像 \\"}; alert('never');</script>"#).unwrap();
        assert!(state["missing"].is_null());
        assert_eq!(state["title"], "undefined 可爱头像 \\");
        assert!(initial_state("<html>请先登录</html>").is_err());
    }
    #[test]
    fn galleries_select_numbered_images_and_default_to_all() {
        let p=note_metadata(&sample(serde_json::json!({"type":"normal","title":"头像","imageList":[
            {"urlDefault":"http://sns-webpic-qc.xhscdn.com/one.jpg"},
            {"urlPre":"https://sns-webpic-qc.xhscdn.com/low.jpg","infoList":[{"imageScene":"WB_DFT","url":"https://sns-webpic-qc.xhscdn.com/two.webp"}]}
        ]})),note_url()).unwrap();
        assert_eq!(p.images.len(), 2);
        assert!(p.images[0].starts_with("https://"));
        assert!(p.images[1].ends_with("two.webp"));
        assert!(p.formats.is_empty());
        assert!(!p.has_audio);
        let selections = expand_selection(&p, &default_selection()).unwrap();
        assert_eq!(selections.len(), 2);
        assert_eq!(selections[1].image_index, Some(1));
        let mut job: DownloadTask = serde_json::from_value(serde_json::json!({
            "id":uuid::Uuid::new_v4().to_string(),"platform":"xiaohongshu","url":note_url(),"title":"pending","fileName":"pending",
            "totalBytes":0,"downloadedBytes":0,"speed":0,"status":"queued","outputPath":"pending","createdAt":1,"error":null,"source":"小红书"
        })).unwrap();
        let root = std::env::temp_dir();
        crate::storage_layout::initialize(&mut job, &root, &[]);
        prepare_job(&mut job, &p, selections[0].clone(), &root).unwrap();
        let mut child = job.clone(); child.id = uuid::Uuid::new_v4().to_string(); child.output_path.clear();
        prepare_job(&mut child, &p, selections[1].clone(), &root).unwrap();
        assert_eq!(job.file_name, "图片_001.jpg");
        assert_eq!(child.file_name, "图片_002.jpg");
        assert_eq!(Path::new(&job.output_path).parent(), Path::new(&child.output_path).parent());
        // Older resolved jobs keep even their custom filename when re-parsed.
        job.storage = None;
        job.output_path = root.join("legacy-original.jpg").to_string_lossy().into_owned();
        let original = job.output_path.clone();
        prepare_job(&mut job, &p, selections[0].clone(), &root).unwrap();
        assert_eq!(job.output_path, original);
        assert!(expand_selection(
            &p,
            &Selection {
                kind: "image".into(),
                format: None,
                image_index: Some(2)
            }
        )
        .is_err());
        assert!(expand_selection(
            &p,
            &Selection {
                kind: "audio".into(),
                format: None,
                image_index: None
            }
        )
        .is_err());
    }

    #[test]
    fn topics_are_collected_outside_the_title_and_persisted_with_the_task() {
        let p = note_metadata(&sample(serde_json::json!({
            "type":"normal", "title":"手表睡眠监测", "user":{"nickname":" 笔记作者 "}, "desc":format!("{} #苹果手表[话题]# #睡眠监测", "正文".repeat(200)),
            "tagList":[{"type":"topic","name":"苹果手表"},{"type":"topic","name":"数码体验"},{"type":"user","name":"作者"}],
            "imageList":[{"urlDefault":"https://sns-webpic-qc.xhscdn.com/one.jpg"}]
        })), note_url()).unwrap();
        assert_eq!(p.title, "手表睡眠监测");
        assert_eq!(p.author.as_deref(), Some("笔记作者"));
        assert_eq!(p.topics, vec!["苹果手表", "睡眠监测", "数码体验"]);
        let mut job: DownloadTask = serde_json::from_value(serde_json::json!({
            "id":uuid::Uuid::new_v4().to_string(),"platform":"xiaohongshu","url":note_url(),"title":"pending","fileName":"pending",
            "totalBytes":0,"downloadedBytes":0,"speed":0,"status":"queued","outputPath":"pending","createdAt":1,"error":null,"source":"小红书"
        })).unwrap();
        assert!(job.topics.is_empty());
        let root = std::env::temp_dir();
        crate::storage_layout::initialize(&mut job, &root, &[]);
        prepare_job(&mut job, &p, expand_selection(&p, &default_selection()).unwrap()[0].clone(), &root).unwrap();
        let saved: DownloadTask = serde_json::from_slice(&serde_json::to_vec(&job).unwrap()).unwrap();
        assert_eq!(saved.topics, p.topics);
        assert_eq!(saved.source, "笔记作者");
        assert!(!saved.output_path.contains("苹果手表"));
        assert!(note_topics(&serde_json::json!({"title":"没有话题","desc":"普通正文"})).is_empty());
    }
    #[test]
    fn video_quality_is_actual_short_edge_and_has_stable_format_ids() {
        let p=note_metadata(&sample(serde_json::json!({"type":"video","title":"竖屏视频","imageList":[{"urlDefault":"https://sns-webpic-qc.xhscdn.com/cover.jpg"}],"video":{"media":{"stream":{"h264":[
            {"masterUrl":"https://sns-video-bd.xhscdn.com/v720.mp4","width":720,"height":1280,"avgBitrate":1000000,"videoCodec":"h264"},
            {"masterUrl":"https://sns-video-bd.xhscdn.com/v1080.mp4","width":1080,"height":1920,"avgBitrate":2000000,"videoCodec":"h264"}
        ]}}}})),note_url()).unwrap();
        assert!(p.images.is_empty());
        assert!(p.has_audio);
        assert_eq!(p.formats[0].label, "1080P");
        assert_eq!(p.formats[1].label, "720P");
        let selected = expand_selection(&p, &default_selection()).unwrap();
        assert_eq!(selected[0].format.as_ref(), Some(&p.formats[0].id));
        assert!(expand_selection(
            &p,
            &Selection {
                kind: "video".into(),
                format: Some("best/evil".into()),
                image_index: None
            }
        )
        .is_err());
    }
    #[test]
    fn external_media_and_login_pages_never_become_downloads() {
        for u in [
            "file:///C:/secret",
            "http://localhost/video.mp4",
            "https://xhscdn.com.evil.test/a",
            "https://user@sns-video-bd.xhscdn.com/a",
            "https://sns-video-bd.xhscdn.com:9000/a",
        ] {
            assert!(media_url(u).is_none());
        }
        assert!(
            note_metadata(&serde_json::json!({"user":{"loggedIn":false}}), note_url()).is_err()
        );
        assert!(note_metadata(&sample(serde_json::json!({"type":"normal","imageList":[{"urlDefault":"https://evil.test/a"}]})),note_url()).is_err());
    }
    #[test]
    fn cleanup_keeps_other_tasks_and_unexpected_directories() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target/test-data")
            .join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir_all(&root).unwrap();
        let job:DownloadTask=serde_json::from_value(serde_json::json!({"id":uuid::Uuid::new_v4().to_string(),"platform":"xiaohongshu","url":note_url(),"title":"x","fileName":"x.mp4","totalBytes":0,"downloadedBytes":0,"speed":0,"status":"paused","outputPath":root.join("x.mp4").to_string_lossy(),"createdAt":1,"error":null,"source":"小红书"})).unwrap();
        let dir = work_dir(&job).unwrap();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(root.join("keep.jpg"), b"keep").unwrap();
        std::fs::write(dir.join("media.part"), b"part").unwrap();
        cleanup(&job).unwrap();
        assert!(root.join("keep.jpg").exists());
        assert!(!dir.exists());
        std::fs::create_dir_all(dir.join("nested")).unwrap();
        assert!(cleanup(&job).is_err());
    }
}
