use futures_util::StreamExt;
use crate::{
    links::safe_file_name,
    models::*,
    telegram::{self, AuthState},
};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter};
use tokio::{
    io::{AsyncSeekExt, AsyncWriteExt},
    sync::Mutex,
};

pub struct Engine {
    pub root: PathBuf,
    pub data: Mutex<Snapshot>,
    pub auth: Mutex<AuthState>,
    pub running: Mutex<HashMap<String, Arc<AtomicU8>>>,
    disk: Mutex<()>,
}
pub struct Shared(pub Arc<Engine>);

impl Engine {
    pub fn load(root: PathBuf, download_dir: PathBuf) -> Result<Arc<Self>, String> {
        std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        let file = root.join("workspace.json");
        let mut disk = if file.exists() {
            serde_json::from_slice::<DiskData>(&std::fs::read(&file).map_err(|e| e.to_string())?)
                .map_err(|e| format!("无法读取工作区记录 {}：{e}", file.display()))?
        } else {
            DiskData {
                settings: Settings {
                    download_dir: download_dir.to_string_lossy().to_string(),
                    concurrency: 4,
                    ..Default::default()
                },
                tasks: vec![],
            }
        };
        // Preserve the saved root across restarts; only unset or invalid paths
        // fall back to the application's default. Existing task paths stay intact.
        if !Path::new(&disk.settings.download_dir).is_absolute() {
            disk.settings.download_dir = download_dir.to_string_lossy().into_owned();
        }
        if disk.settings.api_id.trim().is_empty() || disk.settings.api_hash.trim().is_empty() {
            let app_config: serde_json::Value =
                serde_json::from_str(include_str!("../telegram-app.json"))
                    .map_err(|e| e.to_string())?;
            disk.settings.api_id = app_config["apiId"].as_str().unwrap_or_default().into();
            disk.settings.api_hash = app_config["apiHash"].as_str().unwrap_or_default().into();
            atomic_write(
                &file,
                &serde_json::to_vec_pretty(&disk).map_err(|e| e.to_string())?,
            )?;
        }
        disk.settings.concurrency = disk.settings.concurrency.clamp(1, 4);
        for task in &mut disk.tasks {
            task.speed = 0;
            if matches!(task.status, Status::Downloading | Status::Resolving | Status::Queued) {
                task.status = Status::Paused;
                task.updated_at = now();
            }
        }
        // Existing output paths are authoritative; no automatic file migration.
        atomic_write(&file, &serde_json::to_vec_pretty(&disk).map_err(|e|e.to_string())?)?;
        Ok(Arc::new(Self {
            root,
            data: Mutex::new(Snapshot {
                settings: disk.settings,
                tasks: disk.tasks,
                account: Account::default(),
            }),
            auth: Mutex::new(AuthState::default()),
            running: Mutex::new(HashMap::new()),
            disk: Mutex::new(()),
        }))
    }

    pub fn emit(&self, app: &AppHandle) {
        let _ = app.emit("state-changed", ());
    }

    pub async fn persist(&self, app: &AppHandle) -> Result<(), String> {
        let _guard = self.disk.lock().await;
        let data = self.data.lock().await;
        let bytes = serde_json::to_vec_pretty(&DiskData {
            settings: data.settings.clone(),
            tasks: data.tasks.clone(),
        })
        .map_err(|e| e.to_string())?;
        drop(data);
        let path = self.root.join("workspace.json");
        atomic_write(&path, &bytes)?;
        self.emit(app);
        Ok(())
    }

    pub fn start_scheduler(self: &Arc<Self>, app: AppHandle) {
        let engine = self.clone();
        tauri::async_runtime::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_millis(250));
            loop {
                tick.tick().await;
                let mut running = engine.running.lock().await;
                let mut data = engine.data.lock().await;
                let connected = data.account.connected;
                let media_running = data.tasks.iter().filter(|t|t.discovery.is_none() && running.contains_key(&t.id)).count();
                let mut capacity = data.settings.concurrency.saturating_sub(media_running);
                let mut discovery_slot = !data.tasks.iter().any(|t|t.discovery.is_some() && running.contains_key(&t.id));
                let jobs: Vec<_> = data
                    .tasks
                    .iter_mut()
                    .rev()
                    .filter(|t| {
                        t.status == Status::Queued
                            && !running.contains_key(&t.id)
                            && (t.platform != "telegram" || connected)
                    })
                    .filter(|t| {
                        if t.discovery.is_some(){if discovery_slot {discovery_slot=false;true}else{false}}
                        else if capacity>0 {capacity-=1;true}else{false}
                    })
                    .map(|task| {
                        task.status = Status::Resolving;
                        task.updated_at = now();
                        task.error = None;
                        task.speed = 0;
                        task.clone()
                    })
                    .collect();
                drop(data);
                for job in jobs {
                    let control = Arc::new(AtomicU8::new(0));
                    running.insert(job.id.clone(), control.clone());
                    let worker = engine.clone();
                    let app = app.clone();
                    tauri::async_runtime::spawn(async move {
                        worker.emit(&app);
                        let result = worker.transfer(&job, &control, &app).await;
                        {
                            let mut data = worker.data.lock().await;
                            if let Some(task) = data.tasks.iter_mut().find(|t| t.id == job.id) {
                                task.speed = 0;
                                task.updated_at = now();
                                if control.load(Ordering::SeqCst) == 0 {
                                    match result {
                                        Ok(()) => {
                                            task.status = Status::Completed;
                                            task.error = None;
                                        }
                                        Err(error) => {
                                            task.status = Status::Failed;
                                            task.error = Some(error);
                                        }
                                    }
                                }
                            }
                        }
                        if control.load(Ordering::SeqCst) == 2 {
                            let _ = tokio::fs::remove_file(part_path(&job)).await;
                            if let Err(error) = crate::bilibili_download::cleanup(&job).and_then(|_|crate::xiaohongshu_download::cleanup(&job)) {
                                if let Some(t)=worker.data.lock().await.tasks.iter_mut().find(|t|t.id==job.id){t.error=Some(error);}
                            }
                        }
                        worker.running.lock().await.remove(&job.id);
                        if let Err(error) = worker.persist(&app).await {
                            eprintln!("Unable to persist task state: {error}");
                        }
                    });
                }
            }
        });
    }

    pub(crate) async fn update_progress(
        &self,
        id: &str,
        downloaded: u64,
        speed: u64,
        app: &AppHandle,
    ) {
        if let Some(task) = self.data.lock().await.tasks.iter_mut().find(|t| t.id == id) {
            task.downloaded_bytes = downloaded;
            task.updated_at = now();
            task.speed = speed;
        }
        self.emit(app);
    }

    async fn transfer(
        &self,
        job: &DownloadTask,
        control: &AtomicU8,
        app: &AppHandle,
    ) -> Result<(), String> {
        if job.discovery.is_some(){return crate::douyin_batch::discover(self,job,control,app).await;}
        let provider = crate::providers::get(&job.platform)?;
        crate::providers::require_available(provider)?;
        let detected = crate::providers::for_url(&job.url)?;
        if detected.info().id != provider.info().id {
            return Err("任务平台与链接不一致，请重新创建任务。".into());
        }
        provider.transfer(self, job, control, app).await
    }

    pub(crate) async fn transfer_telegram(
        &self,
        job: &DownloadTask,
        control: &AtomicU8,
        app: &AppHandle,
    ) -> Result<(), String> {
        let client = self
            .auth
            .lock()
            .await
            .client
            .clone()
            .ok_or("请先连接 Telegram")?;
        let resolved = tokio::select! {
            result = tokio::time::timeout(Duration::from_secs(90), telegram::resolve(&client, &job.url)) => result.map_err(|_| "解析超时，请检查网络后重试")??,
            _ = wait_stopped(control) => return Ok(()),
        };
        if job.media_id != 0 && job.media_id != resolved.media_id {
            return Err("消息中的媒体已经更换，请重新解析链接创建下载。".into());
        }
        let prepared = prepare_task(job, &resolved.preview, resolved.media_id)?;
        {
            let mut data = self.data.lock().await;
            if control.load(Ordering::SeqCst) != 0 { return Ok(()); }
            if let Some(task) = data.tasks.iter_mut().find(|t| t.id == job.id) {
                *task = prepared.clone();
            }
            drop(data);
            self.persist(app).await?;
        }
        let job = &prepared;
        if resolved.preview.thumbnail.is_some() {
            let mut data = self.data.lock().await;
            if let Some(task) = data.tasks.iter_mut().find(|t| t.id == job.id) { task.thumbnail = resolved.preview.thumbnail.clone(); }
        }
        let output = Path::new(&job.output_path);
        let parent = output.parent().ok_or("保存路径无效")?;
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("无法创建下载目录：{e}"))?;
        if output.exists() {
            return Err("目标文件已存在，已停止下载以避免覆盖。".into());
        }
        let part = part_path(job);
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(&part)
            .await
            .map_err(|e| format!("无法写入临时文件：{e}"))?;
        const CHUNK: u64 = crate::download::CHUNK;
        let length = file.metadata().await.map_err(|e| e.to_string())?.len();
        let mut offset = resume_offset(length, resolved.preview.size, CHUNK);
        file.set_len(offset).await.map_err(|e| e.to_string())?;
        file.seek(std::io::SeekFrom::Start(offset))
            .await
            .map_err(|e| e.to_string())?;
        let mut media = resolved.media.clone();
        let mut iter = crate::download::download_chunks(client.clone(), media.clone(), offset, resolved.preview.size);
        let mut retries = 0_u32;
        let mut stamp = Instant::now();
        let mut last_bytes = offset;
        self.update_progress(&job.id, offset, 0, app).await;
        loop {
            if control.load(Ordering::SeqCst) != 0 {
                file.flush().await.map_err(|e| e.to_string())?;
                return Ok(());
            }
            let chunk = tokio::select! {
                result = iter.next() => result,
                _ = wait_stopped(control) => { file.flush().await.map_err(|e| e.to_string())?; return Ok(()); }
            };
            match chunk {
                Some(Ok(bytes)) if !bytes.is_empty() => {
                    file.write_all(&bytes)
                        .await
                        .map_err(|e| format!("写入失败，请检查磁盘剩余空间：{e}"))?;
                    offset += bytes.len() as u64;
                    retries = 0;
                    if stamp.elapsed() >= Duration::from_millis(350) {
                        let speed =
                            ((offset - last_bytes) as f64 / stamp.elapsed().as_secs_f64()) as u64;
                        self.update_progress(&job.id, offset, speed, app).await;
                        stamp = Instant::now();
                        last_bytes = offset;
                    }
                }
                _ if control.load(Ordering::SeqCst) != 0 => return Ok(()),
                None | Some(Ok(_)) => break,
                Some(Err(error)) => {
                    // Release the old window before waiting or refreshing message metadata.
                    drop(iter);
                    if retries >= 3 || error.contains("分块重试耗尽") || error.contains("FLOOD_WAIT") || error.contains("失效") {
                        return Err(format!("下载中断（已保留进度，可继续下载）：{error}"));
                    }
                    retries += 1;
                    tokio::select! { _ = tokio::time::sleep(Duration::from_secs(1 << retries)) => {}, _ = wait_stopped(control) => return Ok(()) }
                    if needs_media_refresh(&error) {
                        let fresh = tokio::select! {
                            result = tokio::time::timeout(Duration::from_secs(90), telegram::resolve(&client, &job.url)) => {
                                result.map_err(|_| format!("刷新文件信息超时；原始下载错误：{error}"))?
                                    .map_err(|refresh| format!("刷新文件信息失败：{refresh}；原始下载错误：{error}"))?
                            },
                            _ = wait_stopped(control) => return Ok(()),
                        };
                        if fresh.media_id != job.media_id {
                            return Err("媒体已被替换，请重新创建下载。".into());
                        }
                        media = fresh.media;
                    }
                    // Resume from a confirmed chunk boundary rather than trusting iterator state after an error.
                    offset = resume_offset(offset, resolved.preview.size, CHUNK);
                    file.set_len(offset).await.map_err(|e| e.to_string())?;
                    file.seek(std::io::SeekFrom::Start(offset))
                        .await
                        .map_err(|e| e.to_string())?;
                    last_bytes = offset;
                    stamp = Instant::now();
                    iter = crate::download::download_chunks(client.clone(), media.clone(), offset, resolved.preview.size);
                }
            }
        }
        if resolved.preview.size > 0 && offset != resolved.preview.size {
            return Err(format!(
                "文件尚未下载完整：{offset}/{}，可继续重试。",
                resolved.preview.size
            ));
        }
        file.flush().await.map_err(|e| e.to_string())?;
        file.sync_all().await.map_err(|e| e.to_string())?;
        drop(file);
        if control.load(Ordering::SeqCst) != 0 {
            return Ok(());
        }
        // create_new prevents overwriting an existing file even if another program creates it mid-download.
        let mut destination = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(output)
            .await
            .map_err(|e| format!("无法创建目标文件：{e}"))?;
        let mut source = tokio::fs::File::open(&part)
            .await
            .map_err(|e| e.to_string())?;
        let copied = tokio::select! {
            result = tokio::io::copy(&mut source, &mut destination) => result.map_err(|e| format!("文件保存失败：{e}")),
            _ = wait_stopped(control) => Err("stopped".into()),
        };
        if let Err(error) = copied {
            drop(destination);
            let _ = tokio::fs::remove_file(output).await;
            return if control.load(Ordering::SeqCst) != 0 {
                Ok(())
            } else {
                Err(error)
            };
        }
        if let Err(error) = destination.sync_all().await {
            drop(destination);
            let _ = tokio::fs::remove_file(output).await;
            return Err(format!("无法完成文件写入：{error}"));
        }
        drop(destination);
        drop(source);
        // Commit completion under the same data lock used by pause/cancel.
        let mut data = self.data.lock().await;
        if control.load(Ordering::SeqCst) != 0 {
            drop(data);
            let _ = tokio::fs::remove_file(output).await;
            return Ok(());
        }
        if let Some(task) = data.tasks.iter_mut().find(|t| t.id == job.id) {
            task.status = Status::Completed;
            task.updated_at = now();
            task.downloaded_bytes = offset;
            task.speed = 0;
        }
        drop(data);
        let _ = tokio::fs::remove_file(&part).await;
        self.emit(app);
        Ok(())
    }
}

pub fn resume_offset(length: u64, expected: u64, chunk: u64) -> u64 {
    let safe = if expected > 0 {
        length.min(expected)
    } else {
        length
    };
    safe / chunk * chunk
}
async fn wait_stopped(control: &AtomicU8) {
    while control.load(Ordering::SeqCst) == 0 {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
pub fn part_path(task: &DownloadTask) -> PathBuf {
    Path::new(&task.output_path)
        .parent()
        .unwrap_or(Path::new("."))
        .join(format!(".framefetch-{}.part", task.id))
}
pub fn output_name(id: &str, file_name: &str) -> String {
    format!("{}-{}", &id[..8], safe_file_name(file_name))
}
pub fn atomic_write(path: &Path, data: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let temp = path.with_extension("json.tmp");
    let mut file = std::fs::File::create(&temp).map_err(|e| e.to_string())?;
    file.write_all(data).map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())?;
    drop(file);
    std::fs::rename(temp, path).map_err(|e| format!("无法保存工作区：{e}"))
}

fn needs_media_refresh(error: &str) -> bool {
    error.contains("FILE_REFERENCE_EXPIRED") || error.contains("FILE_REFERENCE_INVALID")
}

#[cfg(test)]
mod tests {
    #[test]
    fn only_expired_file_references_need_message_refresh() {
        assert!(super::needs_media_refresh("400 FILE_REFERENCE_EXPIRED"));
        assert!(super::needs_media_refresh("400 FILE_REFERENCE_INVALID"));
        assert!(!super::needs_media_refresh("下载分块超时，请重试"));
        assert!(!super::needs_media_refresh("连接 Telegram 超时，请检查网络或 SOCKS5 代理。"));
    }
    use super::*;
    #[test]
    fn queued_metadata_is_resolved_once_without_changing_resume_location() {
        let job = DownloadTask { topics: vec![], storage: None, xiaohongshu: None, bilibili: None, discovery: None, batch: None,
            id: "12345678-job".into(), platform: "telegram".into(),
            url: "https://t.me/example/1".into(), title: "消息 1".into(),
            file_name: "待解析".into(), thumbnail: None, total_bytes: 0,
            downloaded_bytes: 0, speed: 0, status: Status::Resolving,
            output_path: std::env::temp_dir().join("12345678-pending").to_string_lossy().into_owned(),
            created_at: 1, updated_at: 1, error: None, source: "Telegram".into(), media_id: 0,
        };
        let mut preview = MediaPreview {
            topics: vec!["旅行".into(), "摄影".into()],
            url: job.url.clone(), title: "Video".into(), file_name: "video.mp4".into(),
            thumbnail: Some("data:image/jpeg;base64,test".into()), size: 2_000_000,
            source: "Channel".into(), kind: "video".into(),
        };
        let mut resolved = prepare_task(&job, &preview, 42).unwrap();
        assert_eq!(resolved.status, Status::Downloading);
        assert_eq!(resolved.media_id, 42);
        assert_eq!(resolved.total_bytes, preview.size);
        assert_eq!(resolved.thumbnail, preview.thumbnail);
        assert_eq!(resolved.topics, preview.topics);
        assert!(resolved.output_path.ends_with("12345678-video.mp4"));
        assert_eq!(part_path(&resolved), part_path(&job));
        resolved.downloaded_bytes = 524_288;
        preview.file_name = "renamed.mp4".into();
        let resumed = prepare_task(&resolved, &preview, 42).unwrap();
        assert_eq!(resumed.output_path, resolved.output_path);
        assert_eq!(resumed.downloaded_bytes, 524_288);
        assert_eq!(resumed.file_name, "video.mp4");
        let mut new_job = job.clone();
        crate::storage_layout::initialize(&mut new_job, &std::env::temp_dir(), &[]);
        let new_resolved = prepare_task(&new_job, &preview, 42).unwrap();
        assert!(new_resolved.output_path.ends_with("视频.mp4"));
        assert!(Path::new(&new_resolved.output_path).starts_with(std::env::temp_dir().join("Telegram")));
        let restored: DownloadTask = serde_json::from_slice(&serde_json::to_vec(&new_resolved).unwrap()).unwrap();
        assert_eq!(restored.topics, preview.topics);
        preview.title = "标题改变".into();
        assert_eq!(prepare_task(&restored, &preview, 42).unwrap().output_path, new_resolved.output_path);
    }

    #[test]
    fn resumes_only_from_complete_chunks() {
        assert_eq!(resume_offset(600_000, 2_000_000, 524_288), 524_288);
        assert_eq!(resume_offset(9_000_000, 1_100_000, 524_288), 1_048_576);
        assert_eq!(resume_offset(0, 100, 524_288), 0);
    }

    #[tokio::test]
    async fn interrupted_downloads_are_restored_paused() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target/test-data")
            .join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir_all(&root).unwrap();
        let mut disk = DiskData {
            settings: Settings {
                download_dir: root.join("downloads").to_string_lossy().to_string(),
                concurrency: 2,
                ..Default::default()
            },
            tasks: vec![DownloadTask { topics: vec![], storage: None, xiaohongshu: None, bilibili: None, discovery: None, batch: None,
                thumbnail: None,
                id: "test-job".into(),
                platform: "telegram".into(),
                url: "https://t.me/example/1".into(),
                title: "file".into(),
                file_name: "file.mp4".into(),
                total_bytes: 2_000_000,
                downloaded_bytes: 524_288,
                speed: 1000,
                status: Status::Downloading,
                output_path: root.join("file.mp4").to_string_lossy().to_string(),
                created_at: 0,
                updated_at: 0,
                error: None,
                source: "test".into(),
                media_id: 1,
            }],
        };
        let mut resolving = disk.tasks[0].clone();
        resolving.id = "resolving-job".into();
        resolving.status = Status::Resolving;
        resolving.media_id = 0;
        disk.tasks.push(resolving);
        let path = root.join("workspace.json");
        atomic_write(&path, &serde_json::to_vec(&disk).unwrap()).unwrap();
        // Also exercises atomic replacement on Windows.
        atomic_write(&path, &serde_json::to_vec(&disk).unwrap()).unwrap();
        let downloads = root.join("portable-tool").join("downloads");
        let engine = Engine::load(root.clone(), downloads.clone()).unwrap();
        let data = engine.data.lock().await;
        assert_eq!(data.settings.download_dir, disk.settings.download_dir);
        assert_eq!(data.tasks[0].output_path, disk.tasks[0].output_path);
        let saved: DiskData = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(saved.settings.download_dir, data.settings.download_dir);
        assert_eq!(data.tasks[0].status, Status::Paused);
        assert_eq!(data.tasks[0].speed, 0);
        assert_eq!(data.tasks[0].downloaded_bytes, 524_288);
        assert!(!data.account.connected);
        assert_eq!(data.tasks[1].status, Status::Paused);
        assert_eq!(data.tasks[1].media_id, 0);
        drop(data);
        let restarted = Engine::load(root.clone(), downloads.clone()).unwrap();
        assert_eq!(restarted.data.lock().await.settings.download_dir, disk.settings.download_dir);
        disk.settings.download_dir = String::new();
        atomic_write(&path, &serde_json::to_vec(&disk).unwrap()).unwrap();
        let fallback = Engine::load(root, downloads.clone()).unwrap();
        assert_eq!(Path::new(&fallback.data.lock().await.settings.download_dir), downloads);
    }
}





// Resolve queued links only when a worker starts, before creating any output file.
fn prepare_task(job: &DownloadTask, preview: &MediaPreview, media_id: i64) -> Result<DownloadTask, String> {
    let mut task = job.clone();
    task.status = Status::Downloading;
    task.topics = preview.topics.clone();
    if job.media_id == 0 {
        let parent = Path::new(&job.output_path).parent().ok_or("保存路径无效")?;
        task.output_path = parent.join(output_name(&job.id, &preview.file_name)).to_string_lossy().into_owned();
        task.file_name = preview.file_name.clone();
        if task.storage.is_some() {
            let ext = Path::new(&preview.file_name).extension().and_then(|v| v.to_str()).unwrap_or("bin");
            let name = if preview.kind == "文件" { preview.file_name.clone() } else {
                crate::storage_layout::media_name(&preview.kind, (preview.kind == "图片").then_some(0), ext)
            };
            crate::storage_layout::resolve(&mut task, &preview.title, &name)?;
        }
        task.title = preview.title.clone();
        task.source = preview.source.clone();
        task.total_bytes = preview.size;
        task.thumbnail = preview.thumbnail.clone();
        task.media_id = media_id;
    }
    Ok(task)
}
