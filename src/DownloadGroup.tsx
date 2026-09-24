import type { DownloadTask } from "./types";

function workId(task: DownloadTask): string | null {
  if (task.storage) return `${task.platform}:${task.storage.base}:${task.storage.workId}`;
  if (task.platform === "xiaohongshu" && ["image", "images", "auto"].includes(task.xiaohongshu?.kind || "")) {
    try { return `xiaohongshu:${new URL(task.url).pathname}`; } catch { return null; }
  }
  if (task.platform !== "douyin") return null;
  // Resolved filenames also identify older tasks created from short share URLs.
  const filename = task.fileName.match(/^(\d+)-\d+\.(?:jpg|jpeg|png|webp)$/i);
  if (filename) return filename[1];
  try {
    const url = new URL(task.url);
    if (!["www.douyin.com", "douyin.com", "www.iesdouyin.com"].includes(url.hostname)) return null;
    return url.pathname.match(/\/(?:video|note)\/(\d+)\/?$/)?.[1] || null;
  } catch { return null; }
}
function imageIndex(task: DownloadTask) {
  if (task.xiaohongshu?.imageIndex != null) return task.xiaohongshu.imageIndex + 1;
  const numbered = task.fileName.match(/^图片_(\d+)\./);
  if (numbered) return Number(numbered[1]);
  const match = task.fileName.match(/^\d+-(\d+)\./);
  if (match) return Number(match[1]);
  try {return Number(new URL(task.url).searchParams.get("image") || 0) + 1;} catch {return 0;}
}
export function groupDownloads(tasks: DownloadTask[]) {
  const groups = new Map<string, DownloadTask[]>();
  for (const task of tasks) {
    const id = workId(task);
    const key = task.batch ? `batch:${task.batch.id}` : id ? `douyin:${id}` : `task:${task.id}`;
    const group = groups.get(key) || [];
    group.push(task); groups.set(key, group);
  }
  return [...groups].map(([id, items]) => ({id, items: items.sort((a,b) => Number(!!b.discovery)-Number(!!a.discovery) || imageIndex(a)-imageIndex(b))}));
}
export function workSummary(tasks: DownloadTask[]): DownloadTask {
  if (tasks.length === 1 && !tasks[0].batch) return tasks[0];
  const status = (["downloading", "resolving", "queued", "failed", "paused", "canceled", "completed"] as const).find(status => tasks.some(t => t.status === status))!;
  return {...tasks[0], thumbnail:tasks.find(t=>t.thumbnail)?.thumbnail, title: tasks[0].batch?.title || tasks[0].title.replace(/ · \d+$/, ""), status,
    totalBytes: tasks.reduce((n,t)=>n+t.totalBytes,0), downloadedBytes: tasks.reduce((n,t)=>n+t.downloadedBytes,0), speed: tasks.reduce((n,t)=>n+t.speed,0), updatedAt: Math.max(...tasks.map(t=>t.updatedAt || t.createdAt)), error: tasks.filter(t=>t.error).map(t=>t.error).join("；") || null};
}
export function batchProgress(tasks: DownloadTask[]) {
  const works = new Map<string, DownloadTask[]>();
  for(const task of tasks.filter(t=>!t.discovery)) {const key=workId(task) || task.id;works.set(key,[...(works.get(key)||[]),task]);}
  return {total:works.size, completed:[...works.values()].filter(items=>items.every(t=>t.status === "completed")).length};
}
