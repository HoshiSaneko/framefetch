import { telegramLinkError } from "./utils";
export type PlatformId = "telegram" | "bilibili" | "douyin" | "xiaohongshu";
export interface PlatformInfo {
  id: PlatformId;
  name: string;
  available: boolean;
  requiresAccount: boolean;
  hosts: string[];
}
// Preview fallback only. The desktop reads availability from the Rust registry.
export const previewPlatforms: PlatformInfo[] = [
  {
    id: "telegram",
    name: "Telegram",
    available: true,
    requiresAccount: true,
    hosts: ["t.me", "www.t.me", "telegram.me"],
  },
  {
    id: "bilibili",
    name: "哔哩哔哩",
    available: true,
    requiresAccount: false,
    hosts: ["bilibili.com", "www.bilibili.com", "m.bilibili.com", "b23.tv"],
  },
  {
    id: "douyin",
    name: "抖音",
    available: true,
    requiresAccount: true,
    hosts: [
      "douyin.com",
      "www.douyin.com",
      "v.douyin.com",
      "www.iesdouyin.com",
    ],
  },
  {id:"xiaohongshu",name:"小红书",available:true,requiresAccount:false,hosts:["xiaohongshu.com","www.xiaohongshu.com","xhslink.com","www.xhslink.com"]},
];
export function detectPlatform(input: string, platforms = previewPlatforms) {
  try {
    const url = new URL(mediaLink(input));
    if (
      !["http:", "https:"].includes(url.protocol) ||
      url.username ||
      url.password ||
      url.port
    )
      return null;
    return (
      platforms.find((p) => p.hosts.includes(url.hostname.toLowerCase())) ??
      null
    );
  } catch {
    return null;
  }
}
export function mediaLink(input: string) {
  return (input.trim().split(/\s+/).find(s => /^https?:\/\//.test(s)) || input.trim()).replace(/[。，)）]+$/, "");
}
export function platformLinkError(input: string, platforms = previewPlatforms) {
  const platform = detectPlatform(input, platforms);
  if (!platform) return "请输入支持的平台链接，例如 https://t.me/channel/123";
  if (!platform.available)
    return `${platform.name}下载尚未接入，暂时无法创建任务。`;
  return platform.id === "telegram" ? telegramLinkError(input) : null;
}

