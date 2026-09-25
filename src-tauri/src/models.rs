use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub download_dir: String,
    pub concurrency: usize,
    pub api_id: String,
    pub api_hash: String,
    pub proxy_url: String,
}

#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub connected: bool,
    pub name: String,
    pub username: String,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Queued,
    Resolving,
    Downloading,
    Paused,
    Completed,
    Failed,
    Canceled,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadBatch {
    pub id: String,
    pub title: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadTask {
    #[serde(default)]
    pub topics: Vec<String>,
    #[serde(default)]
    pub storage: Option<crate::storage_layout::StorageLayout>,
    #[serde(default)]
    pub xiaohongshu: Option<crate::xiaohongshu_download::Selection>,
    #[serde(default)]
    pub bilibili: Option<crate::bilibili_download::Selection>,
    #[serde(default)]
    pub discovery: Option<Discovery>,
    #[serde(default)]
    pub batch: Option<DownloadBatch>,
    pub id: String,
    pub platform: String,
    pub url: String,
    pub title: String,
    pub file_name: String,
    #[serde(default)]
    pub thumbnail: Option<String>,
    pub total_bytes: u64,
    pub downloaded_bytes: u64,
    pub speed: u64,
    pub status: Status,
    pub output_path: String,
    pub created_at: u64,
    #[serde(default)]
    pub updated_at: u64,
    pub error: Option<String>,
    pub source: String,
    #[serde(default)]
    pub media_id: i64,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Discovery {
    pub kind: String,
    pub source: String,
    pub cursor: String,
    pub seen: Vec<String>,
    pub pages: usize,
    pub done: bool,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct DiskData {
    pub settings: Settings,
    pub tasks: Vec<DownloadTask>,
}

#[derive(Clone, Serialize)]
pub struct Snapshot {
    pub settings: Settings,
    pub account: Account,
    pub tasks: Vec<DownloadTask>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaPreview {
    pub topics: Vec<String>,
    pub url: String,
    pub title: String,
    pub file_name: String,
    #[serde(default)]
    pub thumbnail: Option<String>,
    pub size: u64,
    pub source: String,
    pub kind: String,
}

#[derive(Serialize)]
pub struct AuthResult {
    pub step: String,
    pub hint: String,
    pub account: Option<Account>,
}

pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

