import { describe, expect, it } from "vitest";
import {
  detectPlatform,
  platformLinkError,
  previewPlatforms,
} from "./platforms";
describe("platform routing", () => {
  it("recognizes canonical and short links", () => {
    expect(detectPlatform("https://b23.tv/abc")?.id).toBe("bilibili");
    expect(detectPlatform("https://v.douyin.com/abc")?.id).toBe("douyin");
    expect(detectPlatform(" https://t.me/channel/42 ")?.id).toBe("telegram");
  });
  it("rejects credentials, spoof domains and non-web schemes", () => {
    for (const url of [
      "https://t.me.evil.test/a",
      "https://user@t.me/a/1",
      "ftp://t.me/a/1",
      "https://t.me:99/a/1",
    ])
      expect(detectPlatform(url)).toBeNull();
  });
  it("does not turn pending platforms into Telegram authentication requests", () => {
    expect(platformLinkError("https://b23.tv/abc")).toBeNull();
    expect(platformLinkError("https://v.douyin.com/abc")).toBeNull();
    expect(detectPlatform("分享 https://v.douyin.com/abc/ 文案")?.id).toBe("douyin");
    expect(platformLinkError("https://t.me/channel/42")).toBeNull();
    expect(platformLinkError("https://t.me/channel")).toBeTruthy();
  });
  it("honors desktop capability flags", () => {
    expect(
      platformLinkError(
        "https://t.me/channel/42",
        previewPlatforms.map((p) => ({ ...p, available: false })),
      ),
    ).toContain("尚未接入");
  });
});
