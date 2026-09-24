export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const exponent = Math.min(
    Math.floor(Math.log(bytes) / Math.log(1024)),
    units.length - 1,
  );
  return `${(bytes / 1024 ** exponent).toFixed(exponent > 1 ? 1 : 0)} ${units[exponent]}`;
}
export function telegramLinkError(input: string): string | null {
  try {
    const url = new URL(input.trim());
    if (
      !["https:", "http:"].includes(url.protocol) ||
      !["t.me", "telegram.me", "www.t.me"].includes(url.hostname.toLowerCase()) ||
      !!url.username || !!url.password || !!url.port
    )
      return "请粘贴 Telegram 消息链接，例如 https://t.me/channel/123";
    const parts = url.pathname.split("/").filter(Boolean);
    if (parts[0] === "s") parts.shift();
    const topicIndex = parts[0] === "c" ? 2 : 1;
    if (parts.length === topicIndex + 2) {
      const topic = parts[topicIndex];
      if (!/^[1-9]\d*$/.test(topic) || Number(topic) > 2147483647) return "话题 ID 无效。";
      parts.splice(topicIndex, 1);
    }
    if (parts[0] === "c") {
      if (
        parts.length !== 3 ||
        !parts.slice(1).every((p) => /^[1-9]\d*$/.test(p))
      )
        return "私密频道链接格式应为 t.me/c/频道ID/消息ID";
    } else if (
      parts.length !== 2 ||
      !/^[a-zA-Z0-9_]{3,}$/.test(parts[0]) ||
      !/^[1-9]\d*$/.test(parts[1])
    )
      return "请复制一条具体消息的链接，而不是频道首页或邀请链接。";
    const messageId = Number(parts[parts.length - 1]);
    if (!Number.isSafeInteger(messageId) || messageId > 2147483647) return "消息 ID 无效。";
    return null;
  } catch {
    return "请输入完整的 Telegram 消息链接。";
  }
}
