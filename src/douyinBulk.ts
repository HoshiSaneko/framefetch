import { api } from "./bridge";

export const downloadMessage = (error: unknown) => typeof error === "string" ? error : error instanceof Error ? error.message : "加入下载失败，请重试。";

// Start from page one so a previous partial browse cannot omit works.
export async function downloadAllWorks(kind: string, source: string | undefined, signal: AbortSignal, progress: (added: number, skipped: number, page: number) => void) {
  let cursor = "0", added = 0, skipped = 0, pageNumber = 0;
  const cursors = new Set<string>([cursor]);
  const seen = new Set<string>();
  const batch = {id:crypto.randomUUID(), title:kind === "author" ? "博主全部作品" : kind === "folder" ? "收藏夹下载" : kind === "likes" ? "喜欢作品下载" : "收藏作品下载"};
  const snapshot = await api.snapshot();
  const existing = new Set(snapshot.tasks.filter(task => task.status !== "canceled").map(task => task.url.match(/douyin\.com\/(?:video|note)\/(\d+)/)?.[1]).filter(Boolean));
  while (!signal.aborted) {
    pageNumber++;
    progress(added, skipped, pageNumber);
    const page = await api.douyinLibrary(kind, cursor, source);
    if (signal.aborted) break;
    source = page.sourceId || source;
    if (pageNumber === 1 && kind === "author" && page.items[0]?.author) batch.title = `${page.items[0].author} · 全部作品`;
    for (const item of page.items) {
      if (signal.aborted) break;
      if (seen.has(item.id)) continue;
      seen.add(item.id);
      if (existing.has(item.id)) skipped++;
      else {
        try {await api.enqueue(item.url, undefined, batch); added++;}
        catch (error) {
          if (downloadMessage(error).includes("已在下载列表中")) skipped++;
          else throw new Error(`${item.title}：${downloadMessage(error)}`);
        }
      }
      progress(added, skipped, pageNumber);
    }
    if (signal.aborted || !page.hasMore) break;
    if (!/^\d+$/.test(page.cursor) || page.cursor === "0" || cursors.has(page.cursor)) throw new Error("分页未继续，已加入的作品会正常下载。请重试，已有任务会自动跳过。");
    cursor = page.cursor;
    cursors.add(cursor);
  }
  return {added, skipped, stopped:signal.aborted};
}
