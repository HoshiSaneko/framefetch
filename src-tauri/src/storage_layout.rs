use crate::models::DownloadTask;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

// Missing metadata means a legacy task: never opt it into the new layout.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageLayout {
    pub base: String,
    pub date: String,
    pub work_id: String,
    pub ordinal: Option<usize>,
    pub directory: Option<String>,
}

fn short_id(id: &str) -> String {
    id.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(8)
        .collect()
}

pub fn safe_component(value: &str) -> String {
    safe_name(value, 48)
}

fn safe_name(value: &str, limit: usize) -> String {
    let clean = crate::links::safe_file_name(value);
    let clean: String = clean.chars().take(limit).collect();
    let clean = clean.trim_matches(|c| c == '.' || c == ' ');
    let stem = clean.split('.').next().unwrap_or("").to_ascii_uppercase();
    if matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ["COM", "LPT"].iter().any(|prefix| {
            stem.strip_prefix(prefix)
                .is_some_and(|s| s.len() == 1 && "123456789".contains(s))
        })
    {
        format!("_{clean}")
    } else {
        clean.to_string()
    }
}

pub fn initialize(job: &mut DownloadTask, root: &Path, existing: &[DownloadTask]) {
    let date = chrono::Local::now().format("%Y-%m-%d").to_string();
    let platform = match job.platform.as_str() {
        "telegram" => "Telegram",
        "bilibili" => "Bilibili",
        "douyin" => "抖音",
        "xiaohongshu" => "小红书",
        _ => "其他",
    };
    let mut base = root.join(platform);
    let mut ordinal = None;
    if let Some(batch) = &job.batch {
        let siblings: Vec<_> = existing
            .iter()
            .filter(|t| {
                t.platform == job.platform && t.batch.as_ref().is_some_and(|b| b.id == batch.id)
            })
            .collect();
        base = siblings
            .iter()
            .find_map(|t| t.storage.as_ref().map(|s| PathBuf::from(&s.base)))
            .unwrap_or_else(|| {
                base.join(format!(
                    "{date}_{}_{}",
                    safe_component(&batch.title),
                    short_id(&batch.id)
                ))
            });
        ordinal = Some(
            siblings
                .iter()
                .filter_map(|t| t.storage.as_ref().and_then(|s| s.ordinal))
                .max()
                .unwrap_or(0)
                + 1,
        );
    }
    job.output_path = base
        .join(format!("{}-pending", job.id))
        .to_string_lossy()
        .into_owned();
    job.storage = Some(StorageLayout {
        base: base.to_string_lossy().into_owned(),
        date,
        work_id: job.id.clone(),
        ordinal,
        directory: None,
    });
}

pub fn resolve(job: &mut DownloadTask, title: &str, name: &str) -> Result<(), String> {
    let clean_title = match job.platform.as_str() {
        "xiaohongshu" => without_topics(title, "小红书作品"),
        "douyin" => without_topics(title, "抖音作品"),
        _ => title.to_string(),
    };
    let title = clean_title.as_str();
    let layout = job.storage.as_mut().ok_or("任务没有目录信息")?;
    let directory = layout.directory.get_or_insert_with(|| {
        let prefix = layout
            .ordinal
            .map(|i| format!("{i:03}"))
            .unwrap_or_else(|| layout.date.clone());
        Path::new(&layout.base)
            .join(format!(
                "{prefix}_{}_{}",
                safe_component(title),
                short_id(&layout.work_id)
            ))
            .to_string_lossy()
            .into_owned()
    });
    if !Path::new(directory).is_absolute() {
        return Err("下载目录必须为绝对路径".into());
    }
    // Preserve extensions, including names of ordinary Telegram attachments.
    let ext = Path::new(name)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    let stem = Path::new(name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("文件");
    let stem = safe_name(stem, 160);
    job.file_name = if ext.is_empty() {
        stem
    } else {
        format!("{stem}.{}", crate::links::safe_file_name(ext))
    };
    job.output_path = Path::new(directory)
        .join(&job.file_name)
        .to_string_lossy()
        .into_owned();
    Ok(())
}

fn without_topics(title: &str, fallback: &str) -> String {
    static TOPICS: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(||
        regex::Regex::new(r"#([^#\s]+?)\[话题\]#|#([^#\s，。！？、；：,!?;:]+)").expect("valid topic pattern"));
    let cleaned = TOPICS.replace_all(title, " ").split_whitespace().collect::<Vec<_>>().join(" ");
    if cleaned.is_empty() { fallback.into() } else { cleaned }
}

pub fn media_name(kind: &str, index: Option<usize>, extension: &str) -> String {
    let label = match kind {
        "video" | "视频" => "视频",
        "audio" | "音频" => "音频",
        "cover" => "封面",
        "image" | "图片" => "图片",
        _ => "文件",
    };
    let number = index.map(|n| format!("_{:03}", n + 1)).unwrap_or_default();
    format!("{label}{number}.{extension}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::DownloadBatch;

    fn task(platform: &str) -> DownloadTask {
        serde_json::from_value(serde_json::json!({
            "id": uuid::Uuid::new_v4().to_string(), "platform": platform,
            "url":"https://example.test/work", "title":"待解析", "fileName":"pending",
            "totalBytes":0,"downloadedBytes":0,"speed":0,"status":"queued",
            "outputPath":std::env::temp_dir().join("pending"), "createdAt":1,"error":null,"source":platform
        })).unwrap()
    }

    #[test]
    fn all_platforms_have_separate_readable_work_folders() {
        let root = std::env::temp_dir().join("downloads");
        for (platform, folder) in [
            ("telegram", "Telegram"),
            ("bilibili", "Bilibili"),
            ("douyin", "抖音"),
            ("xiaohongshu", "小红书"),
        ] {
            let mut job = task(platform);
            initialize(&mut job, &root, &[]);
            resolve(&mut job, "旅行随拍", "视频.mp4").unwrap();
            let path = Path::new(&job.output_path);
            assert_eq!(path.parent().unwrap().parent().unwrap(), root.join(folder));
            assert!(path
                .parent()
                .unwrap()
                .file_name()
                .unwrap()
                .to_string_lossy()
                .contains("_旅行随拍_"));
            assert_eq!(job.file_name, "视频.mp4");
        }
    }

    #[test]
    fn batch_siblings_keep_one_batch_directory_and_unique_work_directories() {
        let root = std::env::temp_dir().join("downloads");
        let batch = DownloadBatch {
            id: uuid::Uuid::new_v4().to_string(),
            title: "我的收藏".into(),
        };
        let mut a = task("douyin");
        a.batch = Some(batch.clone());
        initialize(&mut a, &root, &[]);
        resolve(&mut a, "同名作品", "视频.mp4").unwrap();
        let mut b = task("douyin");
        b.batch = Some(batch);
        initialize(&mut b, &root, &[a.clone()]);
        resolve(&mut b, "同名作品", "视频.mp4").unwrap();
        assert_eq!(
            a.storage.as_ref().unwrap().base,
            b.storage.as_ref().unwrap().base
        );
        assert_ne!(a.output_path, b.output_path);
        assert!(Path::new(&a.output_path)
            .parent()
            .unwrap()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("001_"));
        assert!(Path::new(&b.output_path)
            .parent()
            .unwrap()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("002_"));
    }

    #[test]
    fn images_share_work_directory_and_restart_keeps_location() {
        let mut a = task("xiaohongshu");
        initialize(&mut a, &std::env::temp_dir(), &[]);
        resolve(&mut a, "相册", &media_name("image", Some(0), "jpg")).unwrap();
        let mut b = a.clone();
        b.id = uuid::Uuid::new_v4().to_string();
        resolve(&mut b, "相册", &media_name("image", Some(1), "jpg")).unwrap();
        assert_eq!(
            Path::new(&a.output_path).parent(),
            Path::new(&b.output_path).parent()
        );
        assert_eq!(b.file_name, "图片_002.jpg");
        let original = b.output_path.clone();
        let mut restored: DownloadTask =
            serde_json::from_slice(&serde_json::to_vec(&b).unwrap()).unwrap();
        resolve(&mut restored, "标题已经变化", "图片_002.jpg").unwrap();
        assert_eq!(original, restored.output_path);
    }

    #[test]
    fn same_title_and_separate_downloads_cannot_overwrite_each_other() {
        let mut a = task("bilibili");
        let mut b = task("bilibili");
        for t in [&mut a, &mut b] {
            initialize(t, &std::env::temp_dir(), &[]);
            resolve(t, "同名视频", "视频.mp4").unwrap();
        }
        assert_ne!(a.output_path, b.output_path);
    }

    #[test]
    fn filenames_are_safe_and_extensions_and_attachments_are_preserved() {
        assert_eq!(safe_component("CON"), "_CON");
        assert_eq!(safe_component("LPT1.txt"), "_LPT1.txt");
        assert!(!safe_component("../../名字:*?").contains(['/', '\\', ':', '*', '?']));
        assert!(safe_component(&"长".repeat(200)).chars().count() <= 48);
        let mut t = task("telegram");
        initialize(&mut t, &std::env::temp_dir(), &[]);
        resolve(&mut t, "资料", "课程资料.zip").unwrap();
        assert_eq!(t.file_name, "课程资料.zip");
        resolve(&mut t, "资料", "音频.ogg").unwrap();
        assert!(t.output_path.ends_with("音频.ogg"));
        assert!(task("telegram").storage.is_none());
    }

    #[test]
    fn xiaohongshu_topics_do_not_become_folder_names_or_rename_existing_folders() {
        let mut job = task("xiaohongshu");
        initialize(&mut job, &std::env::temp_dir(), &[]);
        let title = "出门在外身份是自己给的 #搞笑[话题]# #旅行";
        resolve(&mut job, title, "视频.mp4").unwrap();
        assert!(job.output_path.contains("出门在外身份是自己给的"));
        assert!(!job.output_path.contains("搞笑"));
        assert!(!job.output_path.contains('#'));
        assert_eq!(without_topics("#摄影[话题]# #旅行", "小红书作品"), "小红书作品");
        assert_eq!(without_topics("前文 #旅行[话题]# 后文", "小红书作品"), "前文 后文");
        assert_eq!(without_topics("#摄影 #旅行", "抖音作品"), "抖音作品");
        let old = std::env::temp_dir().join("原目录_#话题").to_string_lossy().into_owned();
        job.storage.as_mut().unwrap().directory = Some(old.clone());
        resolve(&mut job, title, "视频.mp4").unwrap();
        assert_eq!(Path::new(&job.output_path).parent(), Some(Path::new(&old)));
    }
}
