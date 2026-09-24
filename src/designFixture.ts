import type { Snapshot } from "./types";
export function designFixture(): Snapshot {
  return {
    settings: {
      downloadDir: "C:\\Users\\Demo\\Downloads\\FrameFetch",
      concurrency: 4,
      apiId: "",
      apiHash: "",
      proxyUrl: "",
    },
    account: { connected: false, name: "", username: "" },
    tasks: [
      ["山间日出.mp4", 1.24 * 1024 ** 3, 0.68, "downloading", 8.6 * 1024 ** 2],
      ["城市漫游.mp4", 486 * 1024 ** 2, 0.32, "downloading", 3.2 * 1024 ** 2],
      ["设计参考.zip", 218 * 1024 ** 2, 0.45, "paused", 0],
      ["旅行照片.jpg", 12.8 * 1024 ** 2, 1, "completed", 0],
      ["采访录音.mp3", 64 * 1024 ** 2, 1, "completed", 0],
      ["素材归档.zip", 128 * 1024 ** 2, 0, "queued", 0],
      ["会议记录.pdf", 8 * 1024 ** 2, 0.2, "failed", 0],
    ].map(([file, size, p, status, speed], i) => ({
      id: `demo-${i}`,
      platform: "telegram",
      url: `https://t.me/example/${i + 1}`,
      title: String(file),
      fileName: String(file),
      totalBytes: Number(size),
      downloadedBytes: Number(size) * Number(p),
      speed: Number(speed),
      status: status as any,
      outputPath: `C:\\Users\\Demo\\Downloads\\FrameFetch\\${file}`,
      createdAt: Date.now() - (i + 1) * 60_000,
      error: status === "failed" ? "连接中断，请重试（演示状态）" : null,
      source: "Telegram",
    })),
  };
}
