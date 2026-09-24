#[cfg(test)]
use crate::{engine::part_path, models::{DownloadTask, Status}};
use std::{fs, io::Read, path::{Path, PathBuf}};

pub fn destination(current: &Path, id: &str, image: Option<usize>) -> Result<PathBuf,String> {
    if id.is_empty() || id.len()>30 || !id.bytes().all(|b|b.is_ascii_digit()) {return Err("作品编号无效".into());}
    let parent=current.parent().ok_or("保存路径无效")?;
    if !parent.is_absolute() {return Err("保存路径必须为绝对路径".into());}
    let folder=format!("抖音-{id}");
    let directory=if parent.file_name().is_some_and(|v|v==folder.as_str()) {parent.to_path_buf()} else {parent.join(folder)};
    Ok(directory.join(image.map(|index|format!("{:03}.jpg",index+1)).unwrap_or_else(||"视频.mp4".into())))
}

pub fn ensure_directory(output: &Path) -> Result<(),String> {
    let folder=output.parent().ok_or("作品目录无效")?;
    let root=folder.parent().ok_or("下载目录无效")?;
    fs::create_dir_all(root).map_err(|e|e.to_string())?;
    fs::create_dir_all(folder).map_err(|e|e.to_string())?;
    if !folder.canonicalize().map_err(|e|e.to_string())?.starts_with(root.canonicalize().map_err(|e|e.to_string())?) {return Err("作品目录指向了下载目录之外".into());}
    Ok(())
}

// Preserve the source until the new workspace paths have been committed.
// Retrying after interruption accepts only byte-identical destination files.
pub fn stage_file(source: &Path, target: &Path) -> Result<(),String> {
    if source==target || !source.exists() {return Ok(());}
    if target.exists() {
        if fs::symlink_metadata(target).map_err(|e|e.to_string())?.file_type().is_symlink() {return Err("目标文件是链接，已保留原文件".into());}
        let mut a=fs::File::open(source).map_err(|e|e.to_string())?;
        let mut b=fs::File::open(target).map_err(|e|e.to_string())?;
        if a.metadata().map_err(|e|e.to_string())?.len()!=b.metadata().map_err(|e|e.to_string())?.len() {return Err("作品目录存在同名文件，已保留原文件".into());}
        let mut x=[0u8;65536];let mut y=[0u8;65536];
        loop {let n=a.read(&mut x).map_err(|e|e.to_string())?; if n==0 {break;} b.read_exact(&mut y[..n]).map_err(|e|e.to_string())?;if x[..n]!=y[..n] {return Err("作品目录存在同名文件，已保留原文件".into());}}
        return Ok(());
    }
    match fs::hard_link(source,target) {
        Ok(()) => Ok(()),
        Err(_) => {
            // FAT/exFAT destinations do not support hard links.
            let mut input=fs::File::open(source).map_err(|e|e.to_string())?;
            let mut output=fs::OpenOptions::new().write(true).create_new(true).open(target).map_err(|e|format!("无法整理作品文件，原文件已保留：{e}"))?;
            let result=std::io::copy(&mut input,&mut output).and_then(|_|output.sync_all());
            drop(output);
            if let Err(error)=result {let _=fs::remove_file(target);return Err(format!("复制失败，原文件已保留：{error}"));}
            Ok(())
        }
    }
}

#[cfg(test)]
pub fn migrate(tasks: &mut [DownloadTask]) -> Vec<PathBuf> {
    let mut cleanup=vec![];
    for task in tasks {
        if task.platform!="douyin" || task.status==Status::Canceled {continue;}
        let Some(stem)=task.file_name.strip_suffix(".jpg").or_else(||task.file_name.strip_suffix(".mp4")) else {continue;};
        let (id,image)=if let Some((id,index))=stem.rsplit_once('-') {
            let Some(index)=index.parse::<usize>().ok().and_then(|n|n.checked_sub(1)) else {continue;};
            (id,Some(index))
        } else {(stem,None)};
        let old=PathBuf::from(&task.output_path);
        let Ok(new)=destination(&old,id,image) else {continue;};
        if old==new {continue;}
        let old_part=part_path(task);
        let mut updated=task.clone();updated.output_path=new.to_string_lossy().into_owned();
        let new_part=part_path(&updated);
        let result=(|| {
            ensure_directory(&new)?;
            if task.status==Status::Completed && !old.is_file() && !new.is_file() {return Err("作品原文件已移动或删除".into());}
            stage_file(&old,&new)?;
            stage_file(&old_part,&new_part)?;
            Ok::<(),String>(())
        })();
        match result {
            Ok(())=>{*task=updated; if old.is_file() {cleanup.push(old);} if old_part.is_file() {cleanup.push(old_part);}}
            Err(error)=>{task.error=Some(error);}
        }
    }
    cleanup
}

#[cfg(test)]
mod tests {
    use super::*;
    fn job(root: &Path, index: usize, status: Status) -> DownloadTask {
        DownloadTask { topics: vec![], storage: None, xiaohongshu: None, bilibili: None, discovery: None, batch: None,id:format!("job-{index}"),platform:"douyin".into(),url:format!("https://www.douyin.com/video/123?image={}",index-1),title:format!("作品 · {index}"),file_name:format!("123-{index}.jpg"),thumbnail:None,total_bytes:3,downloaded_bytes:3,speed:0,status,output_path:root.join(format!("old-{index}.jpg")).to_string_lossy().into_owned(),created_at:1,updated_at:1,error:None,source:"作者".into(),media_id:0}
    }
    #[test]
    fn gallery_paths_are_numbered_and_resume_never_nests_folders() {
        let root=std::env::temp_dir();
        let first=destination(&root.join("pending"),"123",Some(0)).unwrap();
        assert_eq!(first,root.join("抖音-123/001.jpg"));
        assert_eq!(destination(&first,"123",Some(1)).unwrap(),root.join("抖音-123/002.jpg"));
        assert!(destination(&first,"../bad",Some(0)).is_err());
    }
    #[test]
    fn migrates_completed_images_and_partial_bytes_without_deleting_before_commit() {
        let root=std::env::temp_dir().join(format!("framefetch-work-test-{}",uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let mut tasks=vec![job(&root,1,Status::Completed),job(&root,2,Status::Paused)];
        let old=PathBuf::from(&tasks[0].output_path);let partial=part_path(&tasks[1]);
        fs::write(&old,b"one").unwrap();fs::write(&partial,b"two").unwrap();
        let cleanup=migrate(&mut tasks);
        assert_eq!(cleanup.len(),2);
        assert_eq!(fs::read(&tasks[0].output_path).unwrap(),b"one");
        assert_eq!(fs::read(part_path(&tasks[1])).unwrap(),b"two");
        assert!(old.exists() && partial.exists());
        assert_eq!(Path::new(&tasks[0].output_path).parent(),Path::new(&tasks[1].output_path).parent());
        assert!(migrate(&mut tasks).is_empty());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn conflicts_preserve_original_files_and_records() {
        let root=std::env::temp_dir().join(format!("framefetch-work-test-{}",uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let mut tasks=vec![job(&root,1,Status::Completed)];
        let old=tasks[0].output_path.clone();
        let target=destination(Path::new(&old),"123",Some(0)).unwrap();ensure_directory(&target).unwrap();
        fs::write(&old,b"one").unwrap();fs::write(&target,b"bad").unwrap();
        assert!(migrate(&mut tasks).is_empty());
        assert_eq!(tasks[0].output_path,old);assert!(tasks[0].error.is_some());
        assert_eq!(fs::read(&target).unwrap(),b"bad");assert_eq!(fs::read(&old).unwrap(),b"one");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn startup_preserves_legacy_paths_and_files() {
        let root=std::env::temp_dir().join(format!("framefetch-work-test-{}",uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let task=job(&root,1,Status::Completed);let old=task.output_path.clone();
        fs::write(&old,b"one").unwrap();
        let disk=crate::models::DiskData {settings:crate::models::Settings::default(),tasks:vec![task]};
        fs::write(root.join("workspace.json"),serde_json::to_vec(&disk).unwrap()).unwrap();
        let engine=crate::engine::Engine::load(root.clone(),root.clone()).unwrap();
        drop(engine);
        let saved:crate::models::DiskData=serde_json::from_slice(&fs::read(root.join("workspace.json")).unwrap()).unwrap();
        assert_eq!(fs::read(&saved.tasks[0].output_path).unwrap(),b"one");
        assert_eq!(saved.tasks[0].output_path, old);
        assert!(Path::new(&old).exists());
        let engine=crate::engine::Engine::load(root.clone(),root.clone()).unwrap();drop(engine);
        fs::remove_dir_all(root).unwrap();
    }
}
