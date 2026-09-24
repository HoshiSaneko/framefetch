import type { PlatformId } from "./platforms";
import telegramLogo from "../public-brand/telegram.svg";
import douyinLogo from "../public-brand/douyin.svg";
import bilibiliLogo from "../public-brand/bilibili.svg";
import xiaohongshuLogo from "../public-brand/xiaohongshu.svg";

const platformLogos: Record<PlatformId, string> = {
  telegram: telegramLogo,
  douyin: douyinLogo,
  bilibili: bilibiliLogo,
  xiaohongshu: xiaohongshuLogo,
};

export function PlatformIcon({ id, size = 20 }: { id: PlatformId; size?: number }) {
  return <img className="platform-brand-icon" src={platformLogos[id]} width={size} height={size} alt="" aria-hidden="true" />;
}
