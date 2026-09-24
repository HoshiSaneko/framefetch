use crate::{engine::part_path, models::DownloadTask};
use std::{collections::HashSet, fs, io::ErrorKind, path::PathBuf};

pub fn removal_targets(tasks: &[DownloadTask], id: &str) -> Vec<DownloadTask> {
    let Some(selected) = tasks.iter().find(|t| t.id == id) else { return vec![]; };
    tasks.iter().filter(|task| {
        if task.id == id { return true; }
        if selected.platform != task.platform { return false; }
        let same_batch = selected.batch.as_ref().map(|b| &b.id) == task.batch.as_ref().map(|b| &b.id);
        if selected.discovery.is_some() && selected.batch.is_some() { return same_batch; }
        if !same_batch { return false; }
        match (&selected.storage, &task.storage) {
            (Some(a), Some(b)) => a.work_id == b.work_id && a.base == b.base,
            (None, None) => legacy_work(selected).is_some_and(|key| Some(key) == legacy_work(task)),
            _ => false,
        }
    }).cloned().collect()
}

fn legacy_work(task: &DownloadTask) -> Option<String> {
    if task.platform == "xiaohongshu" {
        let selection = task.xiaohongshu.as_ref()?;
        if !matches!(selection.kind.as_str(), "image" | "images" | "auto") { return None; }
        return url::Url::parse(&task.url).ok().map(|u| u.path().to_string());
    }
    if task.platform == "douyin" {
        if let Some((id, image)) = task.file_name.rsplit_once('-') {
            if id.bytes().all(|c| c.is_ascii_digit()) && image.split('.').next()?.parse::<usize>().is_ok() { return Some(id.into()); }
        }
        let url = url::Url::parse(&task.url).ok()?;
        let parts: Vec<_> = url.path().trim_matches('/').split('/').collect();
        if parts.len() == 2 && matches!(parts[0], "video" | "note") { return Some(parts[1].into()); }
    }
    None
}

// Delete only recorded files and then empty, task-owned directories.
// A file referenced by another task remains available to that task.
pub fn delete_task_files(targets: &[DownloadTask], remaining: &[DownloadTask]) -> Result<(), String> {
    let paths = |task: &DownloadTask| [PathBuf::from(&task.output_path), part_path(task)];
    let protected: HashSet<_> = remaining.iter().flat_map(paths).collect();
    let selected: HashSet<_> = targets.iter().flat_map(paths).filter(|p| !protected.contains(p)).collect();
    for task in targets {crate::bilibili_download::cleanup(task)?;crate::xiaohongshu_download::cleanup(task)?;}
    delete_files(selected)?;
    let mut folders = HashSet::new();
    for task in targets {
        if let Some(layout) = &task.storage {
            if let Some(dir) = &layout.directory { folders.insert(PathBuf::from(dir)); }
            if task.batch.is_some() { folders.insert(PathBuf::from(&layout.base)); }
        } else {
            let output = PathBuf::from(&task.output_path);
            if let Some(parent) = output.parent() {
                if parent.file_name().is_some_and(|n| n.to_string_lossy().starts_with("抖音-")) { folders.insert(parent.to_path_buf()); }
                if task.batch.is_some() {
                    for dir in [Some(parent), parent.parent()].into_iter().flatten() {
                        if dir.file_name().is_some_and(|n| n.to_string_lossy().starts_with("批量-")) { folders.insert(dir.to_path_buf()); }
                    }
                }
            }
        }
    }
    let mut folders: Vec<_> = folders.into_iter().collect();
    folders.sort_by_key(|p| std::cmp::Reverse(p.components().count()));
    for folder in folders {
        if !folder.is_absolute() || remaining.iter().any(|t| std::path::Path::new(&t.output_path).starts_with(&folder)) { continue; }
        match fs::symlink_metadata(&folder) {
            Ok(meta) if meta.is_dir() && !meta.file_type().is_symlink() => (),
            Ok(_) => continue,
            Err(e) if e.kind() == ErrorKind::NotFound => continue,
            Err(e) => return Err(format!("无法访问任务目录：{e}")),
        }
        match fs::remove_dir(&folder) {
            Ok(()) => (),
            Err(e) if matches!(e.kind(), ErrorKind::NotFound | ErrorKind::DirectoryNotEmpty) => (),
            Err(e) => return Err(format!("无法清理空任务目录 {}：{e}。可重试删除。", folder.display())),
        }
    }
    Ok(())
}

fn delete_files(paths: HashSet<PathBuf>) -> Result<(), String> {
    // Validate the full set before deleting any file. Missing files are harmless.
    for path in &paths {
        if !path.is_absolute() { return Err("文件路径无效，已保留下载记录。".into()); }
        match fs::symlink_metadata(path) {
            Ok(meta) if !meta.is_file() || meta.file_type().is_symlink() =>
                return Err(format!("无法删除非普通文件 {}，已保留下载记录。", path.display())),
            Ok(_) => (),
            Err(e) if e.kind() == ErrorKind::NotFound => (),
            Err(e) => return Err(format!("无法访问 {}：{e}。已保留下载记录。", path.display())),
        }
    }
    for path in paths {
        match fs::remove_file(&path) {
            Ok(()) => (),
            Err(e) if e.kind() == ErrorKind::NotFound => (),
            Err(e) => return Err(format!("无法删除 {}：{e}。已保留记录，可重试；部分文件可能已删除。", path.display())),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn root() -> PathBuf {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/test-data").join(uuid::Uuid::new_v4().to_string());
        fs::create_dir_all(&root).unwrap();
        root
    }
    fn gallery(root: &std::path::Path, batch: bool) -> Vec<DownloadTask> {
        let mut first: DownloadTask = serde_json::from_value(serde_json::json!({
            "id":uuid::Uuid::new_v4().to_string(),"platform":"xiaohongshu","url":"https://www.xiaohongshu.com/explore/abc",
            "title":"相册","fileName":"pending","totalBytes":1,"downloadedBytes":1,"speed":0,"status":"completed","outputPath":"pending","createdAt":1,"error":null,"source":"小红书",
            "xiaohongshu":{"kind":"image","format":null,"imageIndex":0}
        })).unwrap();
        if batch { first.batch = Some(crate::models::DownloadBatch {id:uuid::Uuid::new_v4().to_string(),title:"批次".into()}); }
        crate::storage_layout::initialize(&mut first, root, &[]);
        crate::storage_layout::resolve(&mut first,"相册","图片_001.jpg").unwrap();
        let mut second = first.clone(); second.id = uuid::Uuid::new_v4().to_string(); second.xiaohongshu.as_mut().unwrap().image_index = Some(1);
        crate::storage_layout::resolve(&mut second,"相册","图片_002.jpg").unwrap();
        for task in [&first,&second] {
            fs::create_dir_all(std::path::Path::new(&task.output_path).parent().unwrap()).unwrap();
            fs::write(&task.output_path,b"image").unwrap();
            fs::write(part_path(task),b"partial").unwrap();
        }
        vec![first,second]
    }
    #[test]
    fn removing_one_gallery_row_deletes_all_files_records_and_empty_batch_folders() {
        let root = root(); let mut tasks = gallery(&root,true);
        let targets = removal_targets(&tasks,&tasks[0].id);
        assert_eq!(targets.len(),2);
        let base = PathBuf::from(&targets[0].storage.as_ref().unwrap().base);
        delete_task_files(&targets,&[]).unwrap();
        for task in &targets { assert!(!std::path::Path::new(&task.output_path).exists()); assert!(!part_path(task).exists()); }
        assert!(!base.exists());
        assert!(root.join("小红书").is_dir());
        tasks.retain(|task| !targets.iter().any(|t|t.id==task.id));
        assert!(tasks.is_empty()); // no old selection remains to block a fresh enqueue
        assert!(removal_targets(&tasks,&targets[1].id).is_empty());
    }
    #[test]
    fn single_file_directory_is_cleaned_but_untracked_neighbors_and_other_works_survive() {
        let root=root(); let tasks=gallery(&root,false);
        delete_task_files(&tasks[..1],&tasks[1..]).unwrap();
        assert!(std::path::Path::new(&tasks[1].output_path).is_file());
        let folder=std::path::Path::new(&tasks[1].output_path).parent().unwrap();
        let keep=folder.join("用户文件.txt"); fs::write(&keep,b"keep").unwrap();
        delete_task_files(&tasks[1..],&[]).unwrap();
        assert!(keep.is_file()); assert!(folder.is_dir());
        fs::remove_file(&keep).unwrap();
        delete_task_files(&tasks[1..],&[]).unwrap();
        assert!(!folder.exists());
    }
    #[test]
    fn legacy_gallery_selection_ignores_tokens_but_never_selects_other_works_or_platforms() {
        let root=root(); let mut tasks=gallery(&root,false);
        for task in &mut tasks { task.storage=None; }
        tasks[1].url.push_str("?xsec_token=changed");
        let mut other=tasks[0].clone(); other.id="other".into(); other.url="https://www.xiaohongshu.com/explore/other".into(); tasks.push(other);
        assert_eq!(removal_targets(&tasks,&tasks[0].id).len(),2);
        tasks[1].batch=Some(crate::models::DownloadBatch{id:"separate-batch".into(),title:"其他批次".into()});
        assert_eq!(removal_targets(&tasks,&tasks[0].id).len(),1);
    }
    #[test]
    fn deletes_selected_files_and_accepts_missing_files_without_removing_neighbors() {
        let root = root();
        let file = root.join("video.mp4");
        let partial = root.join(".framefetch-test.part");
        let neighbor = root.join("keep.txt");
        for path in [&file, &partial, &neighbor] { fs::write(path, b"test").unwrap(); }
        delete_files([file.clone(), partial.clone(), root.join("missing")].into()).unwrap();
        assert!(!file.exists());
        assert!(!partial.exists());
        assert!(neighbor.is_file());
        assert!(root.is_dir());
    }
    #[test]
    fn refuses_directories_before_deleting_any_file() {
        let root = root();
        let file = root.join("keep.txt");
        fs::write(&file, b"test").unwrap();
        assert!(delete_files([root, file.clone()].into()).is_err());
        assert!(file.is_file());
    }
}
