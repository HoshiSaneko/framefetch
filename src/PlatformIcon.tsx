import type { PlatformId } from "./platforms";
const telegramLogo = "/telegram.svg";
const douyinLogo = "/douyin.svg";
const bilibiliLogo = "/bilibili.svg";
const xiaohongshuLogo = "/xiaohongshu.svg";

const platformLogos: Record<PlatformId, string> = {
  telegram: telegramLogo,
  douyin: douyinLogo,
  bilibili: bilibiliLogo,
  xiaohongshu: xiaohongshuLogo,
};

export function PlatformIcon({ id, size = 20 }: { id: PlatformId; size?: number }) {
  return <img className="platform-brand-icon" src={platformLogos[id]} width={size} height={size} alt="" aria-hidden="true" />;
}
