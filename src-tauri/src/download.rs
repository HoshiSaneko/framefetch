use crate::telegram;
use futures_util::{stream::{self, BoxStream}, StreamExt};
use grammers_client::{media::Media, Client};
use std::{future::Future, sync::{Arc, Mutex}, time::{Duration, Instant}};

pub const CHUNK: u64 = 512 * 1024;
// Restore the original shared-client path and eight-request window.
const WINDOW: usize = 8;
const PART_RETRIES: usize = 2;

fn transient(error: &str) -> bool {
    let lower = error.to_lowercase();
    if ["flood", "file_reference", "auth", "失效"].iter().any(|s| lower.contains(s)) { return false; }
    ["超时", "timeout", "timed out", "connection reset", "connection closed", "broken pipe", "unexpected eof"]
        .iter().any(|s| lower.contains(s))
}

async fn retry_part<F, Fut>(position: u64, expected: usize, fetch: &F, delay: Duration) -> Result<Vec<u8>, String>
where F: Fn(u64) -> Fut, Fut: Future<Output = Result<Vec<u8>, String>> {
    for attempt in 0..=PART_RETRIES {
        match fetch(position).await {
            Ok(bytes) if bytes.len() == expected => return Ok(bytes),
            Ok(bytes) => return Err(format!("文件分块不完整：位置 {position}，收到 {} 字节，预期 {expected} 字节", bytes.len())),
            Err(error) if transient(&error) => {
                if attempt == PART_RETRIES {
                    return Err(format!("分块重试耗尽（位置 {position}，已重试 {PART_RETRIES} 次）：{error}"));
                }
                #[cfg(debug_assertions)]
                eprintln!("[telegram-download] offset={position} retry={} transient=true", attempt + 1);
                tokio::time::sleep(delay * (1 << attempt)).await;
            }
            Err(error) => return Err(error),
        }
    }
    unreachable!()
}

struct TransferStats { start: Instant, parts: u64, bytes: u64, elapsed_ms: u128, max_ms: u128 }
impl TransferStats {
    fn new() -> Self { Self { start: Instant::now(), parts: 0, bytes: 0, elapsed_ms: 0, max_ms: 0 } }
    fn received(&mut self, bytes: usize, elapsed: Duration) {
        self.parts += 1; self.bytes += bytes as u64;
        self.elapsed_ms += elapsed.as_millis(); self.max_ms = self.max_ms.max(elapsed.as_millis());
        #[cfg(debug_assertions)]
        if self.parts == 1 || self.parts % 16 == 0 {
            eprintln!("[telegram-download] parts={} received_bytes={} average_KiB_s={:.1} average_part_ms={} max_part_ms={} window={WINDOW}",
                self.parts, self.bytes, self.bytes as f64 / 1024.0 / self.start.elapsed().as_secs_f64().max(0.001),
                self.elapsed_ms / self.parts as u128, self.max_ms);
        }
    }
}
// Buffered preserves offset order while polling up to WINDOW requests concurrently.
// Dropping the stream cancels pending futures; no detached tasks may keep downloading.
fn ordered_chunks<F, Fut>(offset: u64, size: u64, chunk: u64, fetch: F) -> impl futures_util::Stream<Item = Result<Vec<u8>, String>>
where F: Fn(u64) -> Fut, Fut: Future<Output = Result<Vec<u8>, String>> {
    let fetch = Arc::new(fetch);
    let stats = Arc::new(Mutex::new(TransferStats::new()));
    stream::iter((offset..size).step_by(chunk as usize)).map(move |position| {
        let fetch = fetch.clone();
        let stats = stats.clone();
        async move {
            let started = Instant::now();
            let expected = (size - position).min(chunk) as usize;
            let bytes = retry_part(position, expected, fetch.as_ref(), Duration::from_secs(1)).await?;
            stats.lock().unwrap_or_else(|e| e.into_inner()).received(bytes.len(), started.elapsed());
            Ok(bytes)
        }
    }).buffered(WINDOW)
}

pub fn download_chunks(client: Client, media: Media, offset: u64, size: u64) -> BoxStream<'static, Result<Vec<u8>, String>> {
    if size > 0 {
        ordered_chunks(offset, size, CHUNK, move |position| {
            let client = client.clone();
            let media = media.clone();
            async move {
                let mut iter = client.iter_download(&media)
                    .chunk_size(CHUNK as i32).skip_chunks((position / CHUNK) as i32);
                telegram::network(iter.next()).await?.ok_or_else(|| "文件分块意外结束".into())
            }
        }).boxed()
    } else {
        let iter = client.iter_download(&media).chunk_size(CHUNK as i32).skip_chunks((offset / CHUNK) as i32);
        stream::try_unfold(iter, |mut iter| async move {
            match telegram::network(iter.next()).await? {
                Some(bytes) if !bytes.is_empty() => Ok(Some((bytes, iter))),
                _ => Ok(None),
            }
        }).boxed()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
    #[tokio::test]
    async fn transient_failure_retries_only_the_failed_offset() {
        let attempts = Arc::new([AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(0)]);
        let output = ordered_chunks(0, 3, 1, {
            let attempts = attempts.clone();
            move |position| {
                let count = attempts[position as usize].fetch_add(1, Ordering::SeqCst);
                async move {
                    if position == 1 && count == 0 { Err("connection reset".into()) }
                    else { Ok(vec![position as u8]) }
                }
            }
        }).collect::<Vec<_>>().await;
        assert_eq!(output, vec![Ok(vec![0]), Ok(vec![1]), Ok(vec![2])]);
        assert_eq!(attempts.iter().map(|a| a.load(Ordering::SeqCst)).collect::<Vec<_>>(), vec![1,2,1]);
    }

    #[tokio::test]
    async fn exhausted_retries_and_permanent_errors_are_bounded() {
        let calls = AtomicUsize::new(0);
        let fetch = |_| { calls.fetch_add(1, Ordering::SeqCst); async { Err("timeout".into()) } };
        let error = retry_part(42, 1, &fetch, Duration::ZERO).await.unwrap_err();
        assert!(error.contains("分块重试耗尽"));
        assert_eq!(calls.load(Ordering::SeqCst), 3);
        for error in ["FLOOD_WAIT_10", "FILE_REFERENCE_EXPIRED", "AUTH_KEY_UNREGISTERED", "RPC failure"] {
            let calls = AtomicUsize::new(0);
            let fetch = |_| { calls.fetch_add(1, Ordering::SeqCst); async { Err(error.into()) } };
            assert_eq!(retry_part(0, 1, &fetch, Duration::ZERO).await.unwrap_err(), error);
            assert_eq!(calls.load(Ordering::SeqCst), 1);
        }
    }

    #[tokio::test]
    async fn cancelling_backoff_does_not_send_another_request() {
        let calls = AtomicUsize::new(0);
        let fetch = |_| { calls.fetch_add(1, Ordering::SeqCst); async { Err("timeout".into()) } };
        assert!(tokio::time::timeout(Duration::from_millis(10), retry_part(0, 1, &fetch, Duration::from_secs(60))).await.is_err());
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
    #[tokio::test]
    async fn parallel_requests_remain_bounded_and_write_in_order() {
        let live = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let mut chunks = Box::pin(ordered_chunks(0, WINDOW as u64 * 2, 1, {
            let live = live.clone(); let peak = peak.clone();
            move |position| { let live = live.clone(); let peak = peak.clone(); async move {
                let count = live.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(count, Ordering::SeqCst);
                tokio::time::sleep(std::time::Duration::from_millis(if position == 0 { 30 } else { 1 })).await;
                live.fetch_sub(1, Ordering::SeqCst);
                Ok(vec![position as u8])
            }}
        }));
        let mut output = vec![];
        while let Some(bytes) = chunks.next().await { output.extend(bytes.unwrap()); }
        assert_eq!(output, (0..(WINDOW * 2) as u8).collect::<Vec<u8>>());
        assert_eq!(peak.load(Ordering::SeqCst), WINDOW);
        assert!(peak.load(Ordering::SeqCst) <= WINDOW);
    }
    #[tokio::test]
    async fn resume_and_partial_final_chunk_are_exact() {
        let chunks = ordered_chunks(4, 11, 4, |position| async move { Ok(vec![position as u8; (11-position).min(4) as usize]) });
        let output = chunks.collect::<Vec<_>>().await;
        assert_eq!(output, vec![Ok(vec![4;4]), Ok(vec![8;3])]);
    }
    #[tokio::test]
    async fn short_read_and_rpc_error_are_not_silently_committed() {
        let mut chunks = Box::pin(ordered_chunks(0, 12, 4, |p| async move {
            if p == 4 { Ok(vec![0;2]) } else if p == 8 { Err("RPC failure".into()) } else { Ok(vec![0;4]) }
        }));
        assert!(chunks.next().await.unwrap().is_ok());
        assert!(chunks.next().await.unwrap().unwrap_err().contains("分块不完整"));
        assert_eq!(chunks.next().await.unwrap().unwrap_err(), "RPC failure");
    }
    #[tokio::test]
    async fn dropping_download_cancels_pending_work() {
        struct Guard(Arc<AtomicUsize>);
        impl Drop for Guard { fn drop(&mut self) { self.0.fetch_sub(1, Ordering::SeqCst); } }
        let live = Arc::new(AtomicUsize::new(0));
        let mut chunks = Box::pin(ordered_chunks(0, WINDOW as u64 * 2, 1, {
            let live = live.clone();
            move |_| { let live = live.clone(); async move {
                live.fetch_add(1, Ordering::SeqCst);
                let _guard = Guard(live);
                std::future::pending::<()>().await;
                Ok(vec![0])
            }}
        }));
        assert!(tokio::time::timeout(std::time::Duration::from_millis(10), chunks.next()).await.is_err());
        assert_eq!(live.load(Ordering::SeqCst), WINDOW);
        drop(chunks);
        assert_eq!(live.load(Ordering::SeqCst), 0);
    }
}

