use crate::{
    douyin,
    engine::{part_path, Engine},
    models::{now, DownloadTask, Status},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicU8, Ordering},
    time::{Duration, Instant},
};
use tauri::{AppHandle, Manager, WebviewWindow};
use tokio::io::AsyncWriteExt;

pub fn canonical(input: &str) -> Result<String, String> {
    let raw = input
        .split_whitespace()
        .find(|s| s.starts_with("https://") || s.starts_with("http://"))
        .unwrap_or(input);
    let mut url = url::Url::parse(raw.trim_end_matches(['。', '，', ')', '）']))
        .map_err(|_| "请输入抖音作品链接")?;
    if !["http", "https"].contains(&url.scheme())
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
    {
        return Err("无效抖音链接".into());
    }
    let host = url.host_str().unwrap_or("").to_string();
    if ![
        "www.douyin.com",
        "douyin.com",
        "v.douyin.com",
        "www.iesdouyin.com",
    ]
    .contains(&host.as_str())
    {
        return Err("请输入抖音作品链接".into());
    }
    url.set_scheme("https").ok();
    if host == "v.douyin.com" {
        url.set_query(None);
        url.set_fragment(None);
        return Ok(url.to_string());
    }
    let id = url
        .path_segments()
        .and_then(|s| s.filter(|p| !p.is_empty()).next_back())
        .unwrap_or("");
    if id.is_empty() || !id.bytes().all(|b| b.is_ascii_digit()) {
        return Err("链接中没有抖音作品 ID".into());
    }
    let index = url
        .query_pairs()
        .find(|(k, _)| k == "image")
        .and_then(|(_, v)| v.parse::<usize>().ok());
    Ok(format!(
        "https://www.douyin.com/video/{id}{}",
        index.map(|i| format!("?image={i}")).unwrap_or_default()
    ))
}
fn media_host(url: &url::Url) -> bool {
    url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.port().is_none()
        && url.host_str().is_some_and(|h| {
            [
                "douyinvod.com",
                "douyin.com",
                "iesdouyin.com",
                "douyinpic.com",
                "byteimg.com",
                "ibyteimg.com",
                "ibytedtos.com",
                "amemv.com",
                "snssdk.com",
                "bytecdn.cn",
            ]
            .iter()
            .any(|d| h == *d || h.ends_with(&format!(".{d}")))
        })
}
fn http() -> Result<reqwest::Client, String> {
    reqwest::Client::builder().connect_timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::custom(|a| if a.previous().len() >= 8 || !media_host(a.url()) {a.stop()} else {a.follow()}))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/132.0.0.0 Safari/537.36")
        .build().map_err(|e| e.to_string())
}
pub async fn request(
    app: &AppHandle,
    kind: &str,
    cursor: &str,
    item: &str,
) -> Result<Value, String> {
    let gate = app.state::<douyin::LoginGate>();
    let attempt = gate.0.lock().await;
    if attempt.is_some() {
        return Err("请先完成抖音扫码登录".into());
    }
    let login = douyin::window(app)?;
    if !douyin::has_session(&login)? {
        return Err("请先登录抖音".into());
    }
    if crate::webview_url::current(&login).await?.as_str() == "about:blank" {
        login
            .navigate("https://www.douyin.com/user/self".parse().unwrap())
            .map_err(|e| e.to_string())?;
        tokio::time::sleep(Duration::from_secs(3)).await;
    }
    let script = include_str!("douyin_api.js").replace("__REQUEST__", &json!({"id": uuid::Uuid::new_v4().to_string(), "kind":kind, "cursor":cursor, "itemId":item}).to_string());
    for _ in 0..30 {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let sender = std::sync::Mutex::new(Some(tx));
        login
            .eval_with_callback(&script, move |value| {
                if let Ok(mut s) = sender.lock() {
                    if let Some(tx) = s.take() {
                        let _ = tx.send(value);
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        let raw = tokio::time::timeout(Duration::from_secs(3), rx)
            .await
            .map_err(|_| "抖音页面无响应")?
            .map_err(|_| "抖音窗口已关闭")?;
        let value: Value = serde_json::from_str(&raw).unwrap_or_default();
        if let Some(error) = value["error"].as_str() {
            return Err(error.into());
        }
        if let Some(data) = value.get("data") {
            return Ok(data.clone());
        }
        tokio::time::sleep(Duration::from_millis(700)).await;
    }
    Err("抖音请求超时，请打开官网登录验证后重试".into())
}
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    #[serde(default)]
    pub(crate) topics: Vec<String>,
    #[serde(default)]
    pub(crate) count: Option<u64>,
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) author: String,
    pub(crate) cover: Option<String>,
    pub(crate) url: String,
    pub(crate) images: usize,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Page {
    pub(crate) items: Vec<Item>,
    pub(crate) cursor: String,
    pub(crate) has_more: bool,
    pub(crate) source_id: String,
}
fn author_id(input: &str) -> Result<String, String> {
    if input.starts_with("MS4w") && input.len() <= 256 && input.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') {
        return Ok(input.into());
    }
    let url = url::Url::parse(input).map_err(|_| "请粘贴博主主页链接")?;
    if url.scheme() != "https" || !matches!(url.host_str(), Some("www.douyin.com" | "douyin.com")) || !url.username().is_empty() || url.password().is_some() || url.port().is_some() {
        return Err("请粘贴抖音博主主页链接".into());
    }
    let parts: Vec<_> = url.path().trim_matches('/').split('/').collect();
    if parts.len() != 2 || parts[0] != "user" || !parts[1].starts_with("MS4w") {return Err("链接中没有博主账号，请复制博主主页链接".into());}
    author_id(parts[1])
}
async fn resolve_author(input: &str) -> Result<String, String> {
    let raw = input.split_whitespace().find(|s| s.starts_with("https://")).unwrap_or(input).trim_end_matches(['。','，',')','）']);
    if let Ok(url) = url::Url::parse(raw) {
        if url.scheme() == "https" && url.host_str() == Some("v.douyin.com") && url.username().is_empty() && url.password().is_none() && url.port().is_none() {
            let response = http()?.get(url).timeout(Duration::from_secs(20)).send().await.map_err(|_| "无法解析博主分享链接，请重试")?;
            return author_id(response.url().as_str());
        }
    }
    author_id(raw)
}
fn first_url(value: &Value) -> Option<String> {
    value["url_list"]
        .as_array()?
        .iter()
        .filter_map(Value::as_str)
        .find(|s| url::Url::parse(s).is_ok_and(|u| media_host(&u)))
        .map(str::to_owned)
}
fn work_topics(value: &Value) -> Vec<String> {
    static PATTERN: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(||
        regex::Regex::new(r"#([^#\s，。！？、；：,!?;:]+)").expect("valid topic pattern"));
    let mut topics = Vec::new();
    let mut add = |name: &str| {
        let name = name.trim().trim_matches('#').trim();
        if !name.is_empty() && !topics.iter().any(|t| t == name) { topics.push(name.to_string()); }
    };
    for captures in PATTERN.captures_iter(value["desc"].as_str().unwrap_or("")) {
        if let Some(name) = captures.get(1) { add(name.as_str()); }
    }
    for (list, field) in [("text_extra", "hashtag_name"), ("cha_list", "cha_name")] {
        if let Some(entries) = value[list].as_array() {
            for entry in entries {
                if let Some(name) = entry[field].as_str() { add(name); }
            }
        }
    }
    topics
}

fn item(value: &Value) -> Option<Item> {
    let id = value["aweme_id"].as_str()?.to_string();
    if !id.bytes().all(|b| b.is_ascii_digit()) || id.is_empty() {
        return None;
    }
    Some(Item {
        topics: work_topics(value),
        count: None,
        title: value["desc"]
            .as_str()
            .unwrap_or("抖音作品")
            .chars()
            .take(150)
            .collect(),
        author: value["author"]["nickname"]
            .as_str()
            .unwrap_or("抖音")
            .into(),
        cover: first_url(&value["video"]["cover"]).or_else(|| first_url(&value["images"][0])),
        url: format!("https://www.douyin.com/video/{id}"),
        id,
        images: value["images"].as_array().map_or(0, Vec::len),
    })
}
#[tauri::command]
pub async fn douyin_library(
    app: AppHandle,
    window: WebviewWindow,
    kind: String,
    cursor: String,
    folder_id: Option<String>,
) -> Result<Page, String> {
    douyin::main_only(&window)?;
    if !["likes", "favorites", "folders", "folder", "author"].contains(&kind.as_str())
        || cursor.is_empty()
        || !cursor.bytes().all(|b| b.is_ascii_digit())
    {
        return Err("列表参数无效".into());
    }
    if kind == "folder"
        && !folder_id
            .as_deref()
            .is_some_and(|id| !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()))
    {
        return Err("收藏夹参数无效".into());
    }
    let source_id = if kind == "author" {resolve_author(folder_id.as_deref().unwrap_or("")).await?} else {folder_id.unwrap_or_default()};
    let data = request(&app, &kind, &cursor, &source_id).await?;
    let next = page_cursor(&data, &kind, &cursor);
    // Record only pagination metadata, never cookies, account IDs or media URLs.
    // This lets a real website response distinguish protocol changes from UI bugs.
    if let Ok(root) = app.path().app_data_dir() {
        let fields: serde_json::Map<String,Value> = ["max_cursor","min_cursor","cursor","next_cursor","next_max_cursor","has_more"]
            .iter().map(|key| ((*key).into(), data.get(*key).cloned().unwrap_or(Value::Null))).collect();
        let diagnostic = json!({"totalNumber":data.get("total_number"),"shape":response_shape(&data, 3),"kind":kind,"requested":cursor,"selected":next,"pagination":fields,"itemCount":data["aweme_list"].as_array().map_or(0,Vec::len),"keys":data.as_object().map(|v|v.keys().collect::<Vec<_>>())});
        let _ = std::fs::write(root.join("douyin-pagination.json"), diagnostic.to_string());
    }
    let items = if kind == "folders" {
        let list = folder_list(&data)?;
        list.iter().map(folder_item).collect::<Result<Vec<_>,_>>()?
    } else {
        data["aweme_list"]
            .as_array()
            .ok_or("作品列表响应格式已变化，请重新登录后重试")?
            .iter()
            .filter_map(item)
            .collect()
    };
    Ok(Page {
        items,
        cursor: next,
        has_more: data["has_more"]
            .as_bool()
            .unwrap_or_else(|| data["has_more"].as_f64() == Some(1.0)),
        source_id,
    })
}
fn folder_item(value: &Value) -> Result<Item, String> {
    // IDs can exceed JavaScript's exact integer range. Prefer the API's string
    // field, never a rounded numeric value returned through WebView2.
    let valid = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    let id = ["collects_id_str", "collects_id"].iter()
        .filter_map(|key| value[*key].as_str()).find(|s| valid(s)).map(str::to_owned)
        .or_else(|| value["collects_id"].as_u64().filter(|n| *n <= 9_007_199_254_740_991).map(|n|n.to_string()))
        .or_else(|| value["collects_id"].as_f64().filter(|n| n.is_finite() && *n >= 0.0 && *n <= 9_007_199_254_740_991.0 && n.fract() == 0.0).map(|n|format!("{n:.0}")))
        .ok_or("收藏夹编号解析失败，请刷新后重试")?;
    let count=value["total_number"].as_u64().or_else(||value["total_number"].as_f64().filter(|n|n.is_finite() && *n>=0.0 && *n<=9_007_199_254_740_991.0 && n.fract()==0.0).map(|n|n as u64));
    Ok(Item {topics:vec![],id, count, title:value["collects_name"].as_str().or_else(||value["name"].as_str()).unwrap_or("收藏夹").into(), author:String::new(),cover:None,url:String::new(),images:0})
}
fn folder_list(data: &Value) -> Result<&[Value], String> {
    let value = data.get("collects_list").or_else(|| data.get("collects")).ok_or("收藏夹响应格式已变化")?;
    if let Some(list) = value.as_array() { return Ok(list); }
    // The website returns null, not [], when the account has no custom folders.
    let no_more = data["has_more"] == json!(false) || data["has_more"].as_f64() == Some(0.0);
    if value.is_null() && no_more && data.get("total_number").is_none_or(|n|n.as_f64() == Some(0.0)) { return Ok(&[]); }
    Err("收藏夹响应格式已变化".into())
}
fn response_shape(value: &Value, depth: usize) -> Value {
    if depth == 0 {return json!(if value.is_array() {"array"} else if value.is_object() {"object"} else {"scalar"});}
    match value {
        Value::Object(map) => Value::Object(map.iter().map(|(k,v)|(k.clone(),response_shape(v,depth-1))).collect()),
        Value::Array(values) => json!({"length":values.len(),"first":values.first().map(|v|response_shape(v,depth-1))}),
        Value::Null => json!("null"),
        Value::String(_) => json!("string"),
        Value::Number(_) => json!("number"),
        Value::Bool(_) => json!("boolean"),
    }
}
// Likes normally use max_cursor; collection APIs use cursor. A placeholder
// field must not mask a usable continuation token from the same response.
fn page_cursor(data: &Value, kind: &str, current: &str) -> String {
    let fields = if matches!(kind, "likes" | "author") { ["max_cursor", "cursor"] } else { ["cursor", "max_cursor"] };
    fields.iter().filter_map(|key| {
        let value = &data[*key];
        let token = value.as_str().map(str::to_owned)
            .or_else(|| value.as_u64().map(|n| n.to_string()))
            .or_else(|| value.as_f64().filter(|n| n.is_finite() && *n >= 0.0 && *n <= 9_007_199_254_740_991.0 && n.fract() == 0.0).map(|n| format!("{n:.0}")))?;
        if token.is_empty() || !token.bytes().all(|b| b.is_ascii_digit()) { return None; }
        let normalized = token.trim_start_matches('0');
        if normalized.is_empty() || normalized == current.trim_start_matches('0') { None } else { Some(normalized.to_owned()) }
    }).next().unwrap_or_else(|| current.to_owned())
}

struct Resolved {
    info: Item,
    sources: Vec<String>,
    index: usize,
}
async fn resolve(app: &AppHandle, input: &str) -> Result<Resolved, String> {
    let mut link = canonical(input)?;
    if link.starts_with("https://v.douyin.com/") {
        let response = http()?
            .get(&link)
            .timeout(Duration::from_secs(20))
            .send()
            .await
            .map_err(|_| "短链接解析失败")?;
        link = canonical(response.url().as_str())?;
        if link.starts_with("https://v.douyin.com/") {
            return Err("短链接未能跳转到作品，请复制完整作品链接".into());
        }
    }
    let parsed = url::Url::parse(&link).unwrap();
    let id = parsed.path_segments().unwrap().next_back().unwrap();
    let index = parsed
        .query_pairs()
        .find(|(k, _)| k == "image")
        .and_then(|(_, v)| v.parse().ok())
        .unwrap_or(0);
    let data = request(app, "detail", "0", id).await?;
    let detail = &data["aweme_detail"];
    let info = item(detail).ok_or("作品不存在或当前账号无法访问")?;
    if info.id != id {
        return Err("返回作品与请求链接不一致".into());
    }
    let sources = if let Some(images) = detail["images"].as_array().filter(|a| !a.is_empty()) {
        images
            .iter()
            .map(|v| first_url(v).ok_or("图集图片地址不可用".to_string()))
            .collect::<Result<Vec<_>, _>>()?
    } else {
        vec![first_url(&detail["video"]["play_addr"]).ok_or("此作品没有可下载的视频地址")?]
    };
    if index >= sources.len() {
        return Err("图集序号无效".into());
    }
    Ok(Resolved {
        info,
        sources,
        index,
    })
}

pub async fn transfer(
    engine: &Engine,
    job: &DownloadTask,
    control: &AtomicU8,
    app: &AppHandle,
) -> Result<(), String> {
    let media = tokio::select! {r=resolve(app,&job.url)=>r?, _=stopped(control)=>return Ok(())};
    let mut prepared = job.clone();
    prepared.topics = media.info.topics.clone();
    let filename = if media.info.images > 0 {
        format!("{}-{}.jpg", media.info.id, media.index + 1)
    } else {
        format!("{}.mp4", media.info.id)
    };

    prepared.title = if media.info.images > 0 {
        format!("{} · {}", media.info.title, media.index + 1)
    } else {
        media.info.title.clone()
    };
    prepared.file_name = filename.clone();
    prepared.source = media.info.author.clone();
    prepared.thumbnail = media.info.cover.clone();
    let target = if prepared.storage.is_some() {
        if job.storage.as_ref().is_some_and(|s| s.directory.is_some()) && !job.output_path.ends_with("pending") {
            prepared.file_name = job.file_name.clone();
            PathBuf::from(&job.output_path)
        } else {
            let name = crate::storage_layout::media_name(if media.info.images > 0 { "image" } else { "video" }, (media.info.images > 0).then_some(media.index), if media.info.images > 0 { "jpg" } else { "mp4" });
            crate::storage_layout::resolve(&mut prepared, &media.info.title, &name)?;
            PathBuf::from(&prepared.output_path)
        }
    } else if !job.output_path.ends_with("pending") {
        PathBuf::from(&job.output_path)
    } else {
        crate::work_files::destination(Path::new(&job.output_path), &media.info.id, (media.info.images > 0).then_some(media.index))?
    };
    crate::work_files::ensure_directory(&target)?;
    prepared.output_path = target.to_string_lossy().into_owned();
    let old_part = part_path(job);
    let new_part = part_path(&prepared);
    crate::work_files::stage_file(&old_part, &new_part)?;
    prepared.status = Status::Downloading;
    {
        let mut data = engine.data.lock().await;
        if control.load(Ordering::SeqCst) != 0 {
            return Ok(());
        }
        if let Some(task) = data.tasks.iter_mut().find(|t| t.id == job.id) {
            *task = prepared.clone();
        }
        if media.index == 0 && media.sources.len() > 1 {
            for index in 1..media.sources.len() {
                let url = format!("{}?image={index}", media.info.url);
                if data
                    .tasks
                    .iter()
                    .any(|t| t.url == url && t.status != Status::Canceled)
                {
                    continue;
                }
                let mut child = prepared.clone();
                child.id = uuid::Uuid::new_v4().to_string();
                child.url = url;
                child.status = Status::Queued;
                child.title = format!("{} · {}", media.info.title, index + 1);
                child.file_name = format!("{}-{}.jpg", media.info.id, index + 1);
                if child.storage.is_some() {
                    crate::storage_layout::resolve(&mut child, &media.info.title, &crate::storage_layout::media_name("image", Some(index), "jpg"))?;
                } else {
                    child.output_path = crate::work_files::destination(&target, &media.info.id, Some(index))?.to_string_lossy().into_owned();
                }
                child.downloaded_bytes = 0;
                child.total_bytes = 0;
                child.speed = 0;
                data.tasks.push(child);
            }
        }
    }
    engine.persist(app).await?;
    if old_part != new_part {let _ = tokio::fs::remove_file(&old_part).await;}
    let output = Path::new(&prepared.output_path);
    if output.exists() {
        return Err("目标文件已存在，停止下载以避免覆盖".into());
    }

    let part = part_path(&prepared);
    let mut offset = tokio::fs::metadata(&part)
        .await
        .map(|m| m.len())
        .unwrap_or(0);
    let mut request = http()?
        .get(&media.sources[media.index])
        .header("Referer", "https://www.douyin.com/")
        .header("Accept-Encoding", "identity");
    if offset > 0 {
        request = request.header("Range", format!("bytes={offset}-"));
    }
    let mut response = tokio::select! {r=tokio::time::timeout(Duration::from_secs(30),request.send())=>r.map_err(|_|"连接媒体超时")?.map_err(|_|"无法连接媒体服务器")?, _=stopped(control)=>return Ok(())};
    if !response.status().is_success() {
        return Err(format!(
            "媒体服务器返回 {}，请重试以刷新链接",
            response.status()
        ));
    }
    let total = if response.status() == reqwest::StatusCode::PARTIAL_CONTENT {
        let range = response
            .headers()
            .get("content-range")
            .and_then(|v| v.to_str().ok())
            .ok_or("缺少续传范围")?;
        let (start, total) = parse_range(range)?;
        if start != offset {
            return Err("媒体服务器返回了错误的续传位置".into());
        }
        total
    } else {
        offset = 0;
        response.content_length().unwrap_or(0)
    };
    if offset > 0 && job.total_bytes > 0 && total != job.total_bytes {
        return Err("文件大小发生变化，请重新添加作品以避免错误续传".into());
    }
    let mime = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if mime.starts_with("text/") || mime.contains("json") {
        return Err("服务器返回的不是媒体文件，请重新登录后重试".into());
    }
    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(offset == 0)
        .append(offset > 0)
        .open(&part)
        .await
        .map_err(|e| e.to_string())?;
    {
        let mut data = engine.data.lock().await;
        if let Some(t) = data.tasks.iter_mut().find(|t| t.id == job.id) {
            t.total_bytes = total;
        }
    }
    let mut stamp = Instant::now();
    let mut last = offset;
    loop {
        let chunk = tokio::select! {r=tokio::time::timeout(Duration::from_secs(30),response.chunk())=>r.map_err(|_|"下载超时，请重试")?.map_err(|_|"下载连接中断，请重试")?, _=stopped(control)=>{file.flush().await.map_err(|e|e.to_string())?;return Ok(());}};
        let Some(bytes) = chunk else { break };
        file.write_all(&bytes).await.map_err(|e| e.to_string())?;
        offset += bytes.len() as u64;
        if stamp.elapsed().as_millis() > 400 {
            engine
                .update_progress(
                    &job.id,
                    offset,
                    ((offset - last) as f64 / stamp.elapsed().as_secs_f64()) as u64,
                    app,
                )
                .await;
            stamp = Instant::now();
            last = offset;
        }
    }
    if offset == 0 || (total > 0 && offset != total) {
        return Err("媒体文件不完整，请重试".into());
    }
    file.sync_all().await.map_err(|e| e.to_string())?;
    drop(file);
    let mut data = engine.data.lock().await;
    if control.load(Ordering::SeqCst) != 0 {
        return Ok(());
    }
    crate::work_files::stage_file(&part, output)?;
    let _ = tokio::fs::remove_file(&part).await;
    if let Some(t) = data.tasks.iter_mut().find(|t| t.id == job.id) {
        t.total_bytes = offset;
        t.downloaded_bytes = offset;
        t.speed = 0;
        t.status = Status::Completed;
        t.updated_at = now();
    }
    Ok(())
}
fn parse_range(value: &str) -> Result<(u64, u64), String> {
    let (range, total) = value
        .strip_prefix("bytes ")
        .and_then(|v| v.split_once('/'))
        .ok_or("无效续传范围")?;
    let (start, end) = range.split_once('-').ok_or("无效续传范围")?;
    let start = start.parse::<u64>().map_err(|_| "无效续传位置")?;
    let end = end.parse::<u64>().map_err(|_| "无效续传位置")?;
    let total = total.parse::<u64>().map_err(|_| "无效文件长度")?;
    if start > end || end >= total {
        return Err("无效续传范围".into());
    }
    Ok((start, total))
}
async fn stopped(control: &AtomicU8) {
    while control.load(Ordering::SeqCst) == 0 {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn folder_string_id_survives_webview_numeric_rounding() {
        let value:Value=serde_json::from_str(r#"{"collects_id":7541234567890123456.0,"collects_id_str":"7541234567890123457","collects_name":"旅行"}"#).unwrap();
        let folder=folder_item(&value).unwrap();
        assert_eq!(folder.id,"7541234567890123457");
        assert_eq!(folder.title,"旅行");
        assert_eq!(folder_item(&json!({"collects_id":123.0})).unwrap().id,"123");
        assert_eq!(folder_item(&json!({"collects_id":"7541234567890123457"})).unwrap().id,"7541234567890123457");
        assert!(folder_item(&json!({"collects_id":7541234567890123456.0})).is_err());
        assert!(folder_item(&json!({"collects_id_str":"invalid"})).is_err());
        let list=vec![value,json!({"collects_name":"缺少编号"})];
        assert!(list.iter().map(folder_item).collect::<Result<Vec<_>,_>>().is_err());
    }
    #[test]
    fn author_links_require_a_douyin_homepage() {
        assert_eq!(author_id("https://www.douyin.com/user/MS4w-test_123?from=share").unwrap(), "MS4w-test_123");
        assert_eq!(author_id("MS4w-test_123").unwrap(), "MS4w-test_123");
        for input in ["https://evil.com/user/MS4w-test", "https://www.douyin.com/video/123", "https://www.douyin.com/user/self", "https://name@www.douyin.com/user/MS4w-test", "https://www.douyin.com/user/MS4w-test/extra"] {assert!(author_id(input).is_err());}
        assert_eq!(page_cursor(&json!({"max_cursor":1234.0,"cursor":2}),"author","0"),"1234");
    }
    #[test]
    fn empty_folders_are_not_a_schema_error() {
        assert!(folder_list(&json!({"collects_list":null,"has_more":false,"total_number":0})).unwrap().is_empty());
        assert!(folder_list(&json!({"collects_list":null,"has_more":0.0,"total_number":0.0})).unwrap().is_empty());
        assert_eq!(folder_list(&json!({"collects_list":[{"collects_id":"123","collects_name":"收藏夹"}],"has_more":false})).unwrap().len(),1);
        assert!(folder_list(&json!({"has_more":false})).is_err());
        assert!(folder_list(&json!({"collects_list":null,"has_more":true})).is_err());
        assert!(folder_list(&json!({"collects_list":null,"has_more":false,"total_number":2})).is_err());
    }

    #[test]
    fn pagination_ignores_placeholder_fields_and_preserves_endpoint_priority() {
        // WebView2 callback serialization can represent JS timestamps as floats.
        let actual: Value = serde_json::from_str(r#"{"max_cursor":1787201527000.0,"has_more":1}"#).unwrap();
        assert_eq!(page_cursor(&actual,"likes","0"),"1787201527000");
        assert_eq!(page_cursor(&serde_json::from_str::<Value>(r#"{"max_cursor":1.787201527e12}"#).unwrap(),"likes","0"),"1787201527000");
        assert_eq!(page_cursor(&json!({"max_cursor":12.5}),"likes","0"),"0");
        assert_eq!(page_cursor(&json!({"max_cursor":9007199254740992.0}),"likes","0"),"0");
        assert_eq!(page_cursor(&json!({"max_cursor":0,"cursor":20}),"likes","0"),"20");
        assert_eq!(page_cursor(&json!({"max_cursor":null,"cursor":"40"}),"likes","20"),"40");
        assert_eq!(page_cursor(&json!({"max_cursor":20,"cursor":40}),"likes","20"),"40");
        assert_eq!(page_cursor(&json!({"max_cursor":100,"cursor":40}),"favorites","20"),"40");
        assert_eq!(page_cursor(&json!({"max_cursor":"1720000000123","cursor":40}),"likes","0"),"1720000000123");
        assert_eq!(page_cursor(&json!({"max_cursor":0,"cursor":null}),"likes","20"),"20");
        assert_eq!(page_cursor(&json!({"max_cursor":"bad","cursor":-1}),"likes","20"),"20");
    }

    #[test]
    fn maps_video_and_gallery_without_untrusted_media_urls() {
        let tagged = item(&json!({
            "aweme_id":"789", "desc":format!("海边日落 {} #旅行 #风景", "正文".repeat(160)),
            "text_extra":[{"hashtag_name":"旅行"},{"hashtag_name":"摄影"},{"user_id":"person","nickname":"不是话题"}],
            "cha_list":[{"cha_name":"日常"},{"cha_name":"摄影"}]
        })).unwrap();
        assert_eq!(tagged.topics, vec!["旅行","风景","摄影","日常"]);
        assert!(!tagged.title.contains("#旅行")); // topics survive title truncation
        let restored: Item = serde_json::from_slice(&serde_json::to_vec(&tagged).unwrap()).unwrap();
        assert_eq!(restored.topics, tagged.topics);
        assert!(work_topics(&json!({"desc":"普通作品"})).is_empty());
        let video = json!({"aweme_id":"123", "desc":"作品", "author":{"nickname":"作者"}, "video":{"cover":{"url_list":["https://evil.test/cover", "https://p3.douyinpic.com/cover"]}}});
        let mapped = item(&video).unwrap();
        assert_eq!(mapped.url, "https://www.douyin.com/video/123");
        assert_eq!(
            mapped.cover.as_deref(),
            Some("https://p3.douyinpic.com/cover")
        );
        assert_eq!(mapped.images, 0);
        let gallery = json!({"aweme_id":"456","images":[{"url_list":["https://p3.byteimg.com/a"]},{"url_list":["https://p3.byteimg.com/b"]}]});
        assert_eq!(item(&gallery).unwrap().images, 2);
        assert_eq!(
            canonical("https://www.douyin.com/note/456?image=1").unwrap(),
            "https://www.douyin.com/video/456?image=1"
        );
        assert!(item(&json!({"aweme_id":"../../path"})).is_none());
        assert!(first_url(
            &json!({"url_list":["http://p3.byteimg.com/a","https://byteimg.com.evil.test/a"]})
        )
        .is_none());
    }
    #[test]
    fn links_and_range() {
        assert_eq!(
            canonical("分享 https://www.douyin.com/video/123?foo=1 文案").unwrap(),
            "https://www.douyin.com/video/123"
        );
        assert!(canonical("https://douyin.com.evil.test/video/123").is_err());
        assert_eq!(parse_range("bytes 10-19/20").unwrap(), (10, 20));
        assert!(parse_range("bytes 10-20/20").is_err());
        assert!(!media_host(&"https://127.0.0.1/video".parse().unwrap()));
    }
}
