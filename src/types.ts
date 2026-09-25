import type { PlatformId } from "./platforms";
export type TaskStatus =
  "queued" | "resolving" | "downloading" | "paused" | "completed" | "failed" | "canceled";
export interface DownloadTask {
  storage?: {base: string; workId: string; directory: string | null; ordinal?: number | null; date: string} | null;
  xiaohongshu?: {kind: string; imageIndex?: number | null} | null;
  topics?: string[];
  discovery?: {kind:string;source:string;cursor:string;seen:string[];pages:number;done:boolean} | null;
  batch?: {id: string; title: string} | null;
  id: string;
  platform: PlatformId;
  url: string;
  title: string;
  fileName: string;
  thumbnail?: string | null;
  totalBytes: number;
  downloadedBytes: number;
  speed: number;
  status: TaskStatus;
  outputPath: string;
  createdAt: number;
  updatedAt?: number;
  error: string | null;
  source: string;
}
export interface Settings {
  downloadDir: string;
  concurrency: number;
  apiId: string;
  apiHash: string;
  proxyUrl: string;
}
export interface Account {
  connected: boolean;
  name: string;
  username: string;
}
export interface Snapshot {
  settings: Settings;
  account: Account;
  tasks: DownloadTask[];
}
export interface MediaPreview {
  topics?: string[];
  url: string;
  title: string;
  fileName: string;
  thumbnail?: string | null;
  size: number;
  source: string;
  kind: string;
}
export interface AuthResult {
  step: "code" | "password" | "connected";
  hint: string;
  account: Account | null;
}
export interface QrLoginResult {
  step: "qr" | "password" | "connected";
  url: string | null;
  expiresAt: number;
  hint: string;
  account: Account | null;
}

