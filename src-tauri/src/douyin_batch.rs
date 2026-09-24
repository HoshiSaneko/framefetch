use crate::{engine::{Engine,Shared}, models::*, douyin_download};
use std::{path::Path, sync::atomic::{AtomicU8,Ordering},time::Duration};
use tauri::{AppHandle,Manager,State,WebviewWindow};

#[tauri::command]
pub async fn enqueue_douyin_batch(app:AppHandle,window:WebviewWindow,state:State<'_,Shared>,kind:String,source:Option<String>,title:Option<String>)->Result<(),String>{
    crate::douyin::main_only(&window)?;
    if !["author","folder","favorites","likes"].contains(&kind.as_str()){return Err("下载来源无效".into());}
    let source=source.unwrap_or_default();
    if source.len()>4096 || (matches!(kind.as_str(),"author"|"folder") && source.trim().is_empty()){return Err("请提供下载来源".into());}
    let mut data=state.0.data.lock().await;
    if !Path::new(&data.settings.download_dir).is_absolute(){return Err("请先设置下载文件夹".into());}
    let id=uuid::Uuid::new_v4().to_string();
    let title=title.filter(|s|!s.is_empty()).unwrap_or_else(||if kind=="author"{"博主全部作品".into()}else{"抖音批量下载".into()});
    let directory=Path::new(&data.settings.download_dir).join(format!("批量-{id}"));
    let mut task=DownloadTask { topics: vec![], storage: None, xiaohongshu:None, bilibili:None, id:id.clone(),platform:"douyin".into(),url:source.clone(),title:title.clone(),file_name:"批量下载".into(),thumbnail:None,total_bytes:0,downloaded_bytes:0,speed:0,status:Status::Queued,output_path:directory.join("pending").to_string_lossy().into_owned(),created_at:now(),updated_at:now(),error:None,source:"抖音".into(),media_id:0,batch:Some(DownloadBatch{id,title}),discovery:Some(Discovery{kind,source,cursor:"0".into(),seen:vec!["0".into()],pages:0,done:false})};
    crate::storage_layout::initialize(&mut task, Path::new(&data.settings.download_dir), &data.tasks);
    if let Some(layout) = &mut task.storage { layout.ordinal = None; }
    data.tasks.insert(0,task);
    drop(data);
    state.0.persist(&app).await
}

pub async fn discover(engine:&Engine,job:&DownloadTask,control:&AtomicU8,app:&AppHandle)->Result<(),String>{
    let mut scan=job.discovery.clone().ok_or("批量任务无效")?;
    while !scan.done && control.load(Ordering::SeqCst)==0 {
        // Discovery has its own scheduler slot; even one download slot can start
        // transferring the previous page while this request reads the next one.
        let window=app.get_webview_window("main").ok_or("主窗口不可用")?;
        let page=douyin_download::douyin_library(app.clone(),window,scan.kind.clone(),scan.cursor.clone(),Some(scan.source.clone())).await?;
        let mut data=engine.data.lock().await;
        if control.load(Ordering::SeqCst)!=0 || !data.tasks.iter().any(|t|t.id==job.id && t.status==Status::Resolving){return Ok(());}
        let repeated=apply_page(&mut data,job,&mut scan,page)?;
        drop(data);
        engine.persist(app).await?;
        if repeated{return Err("分页没有继续，可重试；已读取的作品继续下载。".into());}
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    Ok(())
}

fn apply_page(data:&mut Snapshot,job:&DownloadTask,scan:&mut Discovery,page:douyin_download::Page)->Result<bool,String>{
        let mut batch=job.batch.clone().ok_or("批量任务无效")?;
        if scan.kind=="author" {if let Some(item)=page.items.first(){batch.title=format!("{} · 全部作品",item.author);}}
        for item in &page.items {
            if data.tasks.iter().any(|t|t.discovery.is_none() && t.status!=Status::Canceled && t.url.split('?').next()==Some(item.url.as_str())){continue;}
            let mut child=job.clone();
            child.topics = item.topics.clone();
            child.id=uuid::Uuid::new_v4().to_string();child.discovery=None;child.batch=Some(batch.clone());child.url=item.url.clone();child.title=item.title.clone();child.source=item.author.clone();child.thumbnail=item.cover.clone();child.status=Status::Queued;
            child.output_path=Path::new(&job.output_path).with_file_name(format!("{}-pending",child.id)).to_string_lossy().into_owned();
            if let Some(layout) = &mut child.storage {
                layout.work_id = child.id.clone();
                layout.directory = None;
                layout.ordinal = Some(data.tasks.iter().filter(|t| t.batch.as_ref().is_some_and(|b| b.id == batch.id)).filter_map(|t| t.storage.as_ref().and_then(|s| s.ordinal)).max().unwrap_or(0) + 1);
            }
            data.tasks.push(child);
        }
        let repeated=page.has_more && (page.cursor=="0" || scan.seen.contains(&page.cursor));
        scan.pages+=1;
        if !page.source_id.is_empty(){scan.source=page.source_id;}
        if !repeated {scan.cursor=page.cursor;scan.seen.push(scan.cursor.clone());scan.done=!page.has_more;}
        if let Some(parent)=data.tasks.iter_mut().find(|t|t.id==job.id){parent.discovery=Some(scan.clone());parent.title=batch.title.clone();parent.batch=Some(batch);}
    Ok(repeated)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn root()->DownloadTask {serde_json::from_value(serde_json::json!({"id":"root","platform":"douyin","url":"homepage","title":"批量","fileName":"批量下载","totalBytes":0,"downloadedBytes":0,"speed":0,"status":"resolving","outputPath":"C:/Downloads/batch/pending","createdAt":1,"error":null,"source":"抖音","batch":{"id":"root","title":"批量"},"discovery":{"kind":"author","source":"homepage","cursor":"0","seen":["0"],"pages":0,"done":false}})).unwrap()}
    fn page(cursor:&str,more:bool)->douyin_download::Page {douyin_download::Page{items:vec![douyin_download::Item{topics:vec!["旅行".into()],count:None,id:"123".into(),title:"作品".into(),author:"作者".into(),cover:None,url:"https://www.douyin.com/video/123".into(),images:0}],cursor:cursor.into(),has_more:more,source_id:"MS4w-author".into()}}
    #[test]
    fn first_page_is_downloadable_before_discovery_finishes_and_checkpoint_survives_restart(){
        let job=root();let mut scan=job.discovery.clone().unwrap();
        let mut data=Snapshot{settings:Settings::default(),account:Account::default(),tasks:vec![job.clone()]};
        assert!(!apply_page(&mut data,&job,&mut scan,page("100",true)).unwrap());
        assert_eq!(data.tasks[1].status,Status::Queued);
        assert!(data.tasks[1].discovery.is_none());
        assert_eq!(data.tasks[1].topics, vec!["旅行"]);
        assert!(!data.tasks[0].discovery.as_ref().unwrap().done);
        let restored:DownloadTask=serde_json::from_slice(&serde_json::to_vec(&data.tasks[0]).unwrap()).unwrap();
        assert_eq!(restored.discovery.as_ref().unwrap().cursor,"100");
        assert!(!apply_page(&mut data,&job,&mut scan,page("200",false)).unwrap());
        assert_eq!(data.tasks.len(),2); // overlapping pages must not enqueue twice
        assert!(scan.done);
    }
    #[test]
    fn repeated_cursor_keeps_checkpoint_and_does_not_claim_completion(){
        let job=root();let mut scan=job.discovery.clone().unwrap();
        let mut data=Snapshot{settings:Settings::default(),account:Account::default(),tasks:vec![job.clone()]};
        assert!(apply_page(&mut data,&job,&mut scan,page("0",true)).unwrap());
        assert!(!scan.done);assert_eq!(scan.cursor,"0");assert_eq!(data.tasks[1].status,Status::Queued);
    }
}
