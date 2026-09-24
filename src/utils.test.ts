import { describe, expect, it } from "vitest";
import { formatBytes, telegramLinkError } from "./utils";
describe("message links", () => {
  it("accepts public, preview and private message links", () => {
    for (const url of [
      "https://t.me/example/123",
      "https://t.me/s/example/123?single",
      "https://t.me/c/123456/42",
      "https://t.me/c/3942745692/3437/15942",
      "https://t.me/example/3437/15942",
    ])
      expect(telegramLinkError(url)).toBeNull();
  });
  it("rejects foreign hosts, invite links, channel roots and invalid ids", () => {
    for (const url of [
      "https://t.me.evil.test/example/1",
      "https://t.me/+invite",
      "https://t.me/example",
      "https://t.me/example/0",
      "file:///example/2",
      "https://t.me/c/1/2/3/4",
      "https://t.me/c/1/0/3",
      "https://t.me/example/2147483648/12",
    ])
      expect(telegramLinkError(url)).not.toBeNull();
  });
});
it("formats zero and large byte counts without invalid values", () => {
  expect(formatBytes(0)).toBe("0 B");
  expect(formatBytes(1536 * 1024)).toBe("1.5 MB");
});
