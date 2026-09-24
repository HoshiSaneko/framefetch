//! Platform boundary: implement DownloadProvider, then register it in PROVIDERS.
//! Transfer implementations share the engine's queue, persistence and cancellation.
use crate::{
    engine::Engine,
    models::{DownloadTask, MediaPreview},
    telegram,
};
use serde::Serialize;
use std::{future::Future, pin::Pin, sync::atomic::AtomicU8, time::Duration};
use tauri::AppHandle;

pub type ProviderFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, String>> + Send + 'a>>;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub available: bool,
    pub requires_account: bool,
    pub hosts: &'static [&'static str],
}

pub struct PreparedMedia {
    pub preview: MediaPreview,
}

pub trait DownloadProvider: Sync {
    fn info(&self) -> ProviderInfo;
    fn preview<'a>(&'a self, engine: &'a Engine, url: &'a str)
        -> ProviderFuture<'a, PreparedMedia>;
    fn transfer<'a>(
        &'a self,
        engine: &'a Engine,
        job: &'a DownloadTask,
        control: &'a AtomicU8,
        app: &'a AppHandle,
    ) -> ProviderFuture<'a, ()>;
}

struct Telegram;
impl DownloadProvider for Telegram {
    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            id: "telegram",
            name: "Telegram",
            available: true,
            requires_account: true,
            hosts: &["t.me", "www.t.me", "telegram.me"],
        }
    }
    fn preview<'a>(
        &'a self,
        engine: &'a Engine,
        url: &'a str,
    ) -> ProviderFuture<'a, PreparedMedia> {
        Box::pin(async move {
            if !engine.data.lock().await.account.connected {
                return Err("请先连接 Telegram".into());
            }
            let client = engine
                .auth
                .lock()
                .await
                .client
                .clone()
                .ok_or("Telegram 尚未连接")?;
            let resolved =
                tokio::time::timeout(Duration::from_secs(90), telegram::resolve(&client, url))
                    .await
                    .map_err(|_| "解析超时，请检查网络后重试")??;
            Ok(PreparedMedia {
                preview: resolved.preview,
            })
        })
    }
    fn transfer<'a>(
        &'a self,
        engine: &'a Engine,
        job: &'a DownloadTask,
        control: &'a AtomicU8,
        app: &'a AppHandle,
    ) -> ProviderFuture<'a, ()> {
        Box::pin(engine.transfer_telegram(job, control, app))
    }
}

struct Bilibili;
impl DownloadProvider for Bilibili {
    fn info(&self)->ProviderInfo {ProviderInfo{id:"bilibili",name:"哔哩哔哩",available:true,requires_account:false,hosts:&["bilibili.com","www.bilibili.com","m.bilibili.com","b23.tv"]}}
    fn preview<'a>(&'a self,_:&'a Engine,_:&'a str)->ProviderFuture<'a,PreparedMedia>{Box::pin(async{Err("请使用 Bilibili 视频解析选择下载内容".into())})}
    fn transfer<'a>(&'a self,engine:&'a Engine,job:&'a DownloadTask,control:&'a AtomicU8,app:&'a AppHandle)->ProviderFuture<'a,()>{Box::pin(crate::bilibili_download::transfer(engine,job,control,app))}
}
static TELEGRAM: Telegram = Telegram;
static BILIBILI: Bilibili = Bilibili;
struct Douyin;
impl DownloadProvider for Douyin {
    fn info(&self) -> ProviderInfo { DOUYIN_INFO.clone() }
    fn preview<'a>(&'a self, _: &'a Engine, _: &'a str) -> ProviderFuture<'a, PreparedMedia> {
        Box::pin(async {Err("请直接加入下载，抖音作品将在后台解析".into())})
    }
    fn transfer<'a>(&'a self, engine: &'a Engine, job: &'a DownloadTask, control: &'a AtomicU8, app: &'a AppHandle) -> ProviderFuture<'a, ()> {
        Box::pin(crate::douyin_download::transfer(engine,job,control,app))
    }
}
static DOUYIN: Douyin = Douyin;
static DOUYIN_INFO: ProviderInfo = ProviderInfo {
    id: "douyin",
    name: "抖音",
    available: true,
    requires_account: true,
    hosts: &[
        "douyin.com",
        "www.douyin.com",
        "v.douyin.com",
        "www.iesdouyin.com",
    ],
};
struct Xiaohongshu;
impl DownloadProvider for Xiaohongshu {
    fn info(&self)->ProviderInfo {ProviderInfo{id:"xiaohongshu",name:"小红书",available:true,requires_account:false,hosts:&["xiaohongshu.com","www.xiaohongshu.com","www.xhslink.com","xhslink.com"]}}
    fn preview<'a>(&'a self,_:&'a Engine,_:&'a str)->ProviderFuture<'a,PreparedMedia>{Box::pin(async{Err("请使用小红书选项解析作品，或直接加入下载".into())})}
    fn transfer<'a>(&'a self,engine:&'a Engine,job:&'a DownloadTask,control:&'a AtomicU8,app:&'a AppHandle)->ProviderFuture<'a,()>{Box::pin(crate::xiaohongshu_download::transfer(engine,job,control,app))}
}
static XIAOHONGSHU: Xiaohongshu = Xiaohongshu;
static PROVIDERS: [&'static dyn DownloadProvider; 4] = [&TELEGRAM, &BILIBILI, &DOUYIN, &XIAOHONGSHU];

pub fn catalog() -> Vec<ProviderInfo> {
    PROVIDERS.iter().map(|p| p.info()).collect()
}
fn unavailable(info: &ProviderInfo) -> String {
    format!("{}下载尚未接入，暂时无法创建任务。", info.name)
}
pub fn get(id: &str) -> Result<&'static dyn DownloadProvider, String> {
    PROVIDERS
        .iter()
        .copied()
        .find(|p| p.info().id == id)
        .ok_or_else(|| "暂不支持此平台".into())
}
pub fn for_url(input: &str) -> Result<&'static dyn DownloadProvider, String> {
    let url = url::Url::parse(input.trim()).map_err(|_| "请输入完整的媒体链接")?;
    if !["http", "https"].contains(&url.scheme())
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
    {
        return Err("请输入有效的 HTTP 或 HTTPS 平台链接".into());
    }
    let host = url.host_str().ok_or("链接缺少域名")?;
    PROVIDERS
        .iter()
        .copied()
        .find(|p| p.info().hosts.contains(&host))
        .ok_or_else(|| "暂不支持此链接的平台".into())
}
pub fn require_available(provider: &dyn DownloadProvider) -> Result<(), String> {
    if provider.info().available {
        Ok(())
    } else {
        Err(unavailable(&provider.info()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn detect_canonical_and_short_links() {
        for (url, id) in [
            ("https://t.me/demo/123", "telegram"),
            ("https://www.bilibili.com/video/BV123", "bilibili"),
            ("https://b23.tv/demo", "bilibili"),
            ("https://v.douyin.com/demo/", "douyin"),
        ] {
            assert_eq!(for_url(url).unwrap().info().id, id);
        }
    }
    #[test]
    fn reject_spoofed_domains_and_non_web_urls() {
        for url in [
            "https://t.me.evil.test/a",
            "https://bilibili.com.evil.test/video",
            "https://t.me@evil.test/a",
            "https://a@t.me/demo/1",
            "file:///tmp/video",
            "https://t.me:9000/demo/1",
        ] {
            assert!(for_url(url).is_err(), "{url}");
        }
    }
    #[test]
    fn registered_platforms_are_available() {
        for id in ["telegram","douyin","bilibili"] {assert!(require_available(get(id).unwrap()).is_ok());}
        assert!(!get("bilibili").unwrap().info().requires_account);
        assert!(get("unknown").is_err());
    }
}
