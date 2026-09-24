import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { previewPlatforms, type PlatformInfo } from "./platforms";
import type {
  Snapshot,
  Settings,
  Account,
  MediaPreview,
  AuthResult,
  QrLoginResult,
} from "./types";

export const desktop = isTauri();
export const designPreview =
  import.meta.env.DEV &&
  !desktop &&
  new URLSearchParams(location.search).has("design-preview");
const previewSettings: Settings = {
  downloadDir: "",
  concurrency: 4,
  apiId: "",
  apiHash: "",
  proxyUrl: "",
};
async function call<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  if (!desktop)
    throw new Error("当前为界面预览，请启动 FrameFetch 桌面版使用此功能。");
  return invoke<T>(command, args);
}
export interface XiaohongshuPreview extends BilibiliPreview {images:string[]; topics?:string[]}
export interface XiaohongshuSelection {kind:"video"|"cover"|"audio"|"images";format:string|null}
export interface XiaohongshuStatus {sessionPresent:boolean;name:string|null;avatar:string|null}
export interface BilibiliPreview {url: string; title: string; thumbnail: string | null; formats: {id:string; label:string}[]; hasAudio:boolean; topics?:string[]; author?:string|null}
export interface BilibiliSelection {kind: "video" | "cover" | "audio"; format: string | null}
export const api = {
  xiaohongshuPreview:(url:string)=>call<XiaohongshuPreview>("xiaohongshu_preview",{url}),
  enqueueXiaohongshu:(url:string,selection:XiaohongshuSelection,images:number[])=>call<void>("enqueue_xiaohongshu",{url,selection,images}),
  xiaohongshuQrStart:()=>call<string>("xiaohongshu_qr_start"),
  xiaohongshuQrPoll:(attemptId:string)=>call<{loggedIn:boolean;image:string|null;profile:XiaohongshuStatus|null}>("xiaohongshu_qr_poll",{attemptId}),
  xiaohongshuQrCancel:(attemptId:string)=>call<void>("xiaohongshu_qr_cancel",{attemptId}),
  xiaohongshuOpen:()=>call<void>("xiaohongshu_login_open"),
  xiaohongshuStatus:()=>call<XiaohongshuStatus>("xiaohongshu_login_status"),
  xiaohongshuFinish:()=>call<void>("xiaohongshu_login_finish"),
  xiaohongshuLogout:()=>call<void>("xiaohongshu_logout"),
  bilibiliPreview: (url:string) => call<BilibiliPreview>("bilibili_preview",{url}),
  enqueueBilibili: (url:string,selection:BilibiliSelection,includeCover=false) => call<void>("enqueue_bilibili",{url,selection,includeCover}),
  bilibiliProfile: () => call<{name:string; avatar:string|null}>("bilibili_profile"),
  bilibiliQrStart: () => call<string>("bilibili_qr_start"),
  bilibiliQrPoll: (attemptId:string) => call<{loggedIn:boolean; image:string|null}>("bilibili_qr_poll",{attemptId}),
  bilibiliQrCancel: (attemptId:string) => call<void>("bilibili_qr_cancel",{attemptId}),
  bilibiliOpen: () => call<void>("bilibili_login_open"),
  bilibiliStatus: () => call<{sessionPresent:boolean}>("bilibili_login_status"),
  bilibiliFinish: () => call<void>("bilibili_login_finish"),
  bilibiliLogout: () => call<void>("bilibili_logout"),
  enqueueDouyinBatch: (kind:string, source?:string, title?:string) => call<void>("enqueue_douyin_batch",{kind,source,title}),
  douyinLibrary: (kind: string, cursor = "0", folderId?: string) => call<{items: DouyinItem[]; cursor: string; hasMore: boolean; sourceId?: string}>("douyin_library", {kind, cursor, folderId}),
  telegramAvatar: () => desktop ? call<string | null>("telegram_avatar") : Promise.resolve(null),
  douyinProfile: () => call<{name: string; avatar: string | null}>("douyin_profile"),
  douyinQrStart: () => call<string>("douyin_qr_start"),
  douyinQrPoll: (attemptId: string) => call<{loggedIn: boolean; image: string | null}>("douyin_qr_poll", {attemptId}),
  douyinQrCancel: (attemptId: string) => call<void>("douyin_qr_cancel", {attemptId}),
  douyinOpen: () => call<void>("douyin_login_open"),
  douyinStatus: () => call<{sessionPresent: boolean}>("douyin_login_status"),
  douyinFinish: () => call<void>("douyin_login_finish"),
  douyinLogout: () => call<void>("douyin_logout"),
  playbackPath: (id: string) => call<string>("playback_path", { id }),
  platforms: () =>
    desktop
      ? call<PlatformInfo[]>("list_platforms")
      : Promise.resolve(previewPlatforms),
  remove: (id: string, deleteFiles = false) => call<void>("remove_download", { id, deleteFiles }),
  openFile: (id: string) => call<void>("open_download", { id }),
  snapshot: async (): Promise<Snapshot> =>
    desktop
      ? call("snapshot")
      : Promise.resolve({
          settings: previewSettings,
          tasks: designPreview
            ? (await import("./designFixture")).designFixture().tasks
            : [],
          account: { connected: false, name: "", username: "" },
        }),
  restore: () => call<Account>("restore_session"),
  saveSettings: (settings: Settings) =>
    call<Settings>("save_settings", { settings }),
  sendCode: (settings: Settings, phone: string) =>
    call<AuthResult>("send_code", { settings, phone }),
  signIn: (code: string) => call<AuthResult>("sign_in", { code }),
  password: (password: string, attemptId?: string) =>
    call<AuthResult>("check_password", { password, attemptId }),
  startQr: (settings: Settings, attemptId: string) =>
    call<QrLoginResult>("start_qr_login", { settings, attemptId }),
  pollQr: (attemptId: string) =>
    call<QrLoginResult>("poll_qr_login", { attemptId }),
  cancelQr: (attemptId: string) =>
    desktop ? call<void>("cancel_qr_login", { attemptId }) : Promise.resolve(),
  logout: () => call<void>("logout"),
  preview: (url: string) => call<MediaPreview>("preview_link", { url }),
  enqueue: (url: string, downloadDir?: string, batch?: {id:string;title:string}) =>
    call<void>("enqueue_download", { url, downloadDir, batch }),
  control: (id: string, action: string) =>
    call<void>("control_download", { id, action }),
  thumbnail: (id: string) =>
    desktop
      ? call<string | null>("file_thumbnail", { id })
      : Promise.resolve(null),
  reveal: (id: string) => call<void>("reveal_download", { id }),
  openFolder: () => call<void>("open_download_folder"),
  pickFolder: async () => {
    if (!desktop) throw new Error("请在桌面版选择本地文件夹。");
    const result = await open({
      directory: true,
      multiple: false,
      title: "选择下载保存位置",
    });
    return typeof result === "string" ? result : null;
  },
  subscribe: async (onChange: () => void) =>
    desktop ? listen("state-changed", onChange) : () => {},
};

export interface DouyinItem { count?: number | null; id: string; title: string; author: string; cover: string | null; url: string; images: number; }

