import { useRef, useState } from "react";
import { PlatformIcon } from "./PlatformIcon";
import { api } from "./bridge";
import { platformLinkError } from "./platforms";

export function parseTelegramLinks(text: string) {
  const links = new Set<string>();
  let invalid = 0;
  for (const candidate of text.match(/https?:\/\/[^\s<>"'，。；、）)]+/gi) ?? []) {
    try {
      const url = new URL(candidate);
      if (!["t.me", "www.t.me", "telegram.me"].includes(url.hostname) || platformLinkError(candidate)) {
        invalid++; continue;
      }
      const parts = url.pathname.split("/").filter(Boolean);
      if (parts[0] === "s") parts.shift();
      const topicIndex = parts[0] === "c" ? 2 : 1;
      if (parts.length === topicIndex + 2) parts.splice(topicIndex, 1);
      links.add(`https://t.me/${parts.join("/")}`);
    } catch { invalid++; }
  }
  return { links: [...links], invalid };
}

export function TelegramBatch({ onAdded, onQueued, onBusyChange }: {
  onAdded: () => Promise<void>; onQueued: () => void; onBusyChange: (busy: boolean) => void;
}) {
  const [text, setText] = useState("");
  const [links, setLinks] = useState<string[]>([]);
  const [checked, setChecked] = useState<string[]>([]);
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [notice, setNotice] = useState("");
  const [busy, setBusy] = useState(false);
  const submitting = useRef(false);
  const batchRef = useRef<{id: string; title: string} | null>(null);
  const allChecked = links.length > 0 && checked.length === links.length;
  const collect = () => {
    const parsed = parseTelegramLinks(text);
    if (!parsed.links.length) { setNotice("没有识别到有效的 Telegram 消息链接，请复制具体消息的链接。"); return; }
    const merged = [...new Set([...links, ...parsed.links])];
    if (merged.length > 100) { setNotice("每次最多选择 100 条消息，请分批加入。"); return; }
    const added = parsed.links.filter(link => !links.includes(link));
    setLinks(merged);
    setChecked(current => [...new Set([...current, ...added])]);
    setText("");
    setNotice(`新增 ${added.length} 条消息，重复链接已合并。${parsed.invalid ? `已跳过 ${parsed.invalid} 条无效或非 Telegram 链接。` : ""}`);
  };
  const download = async () => {
    if (submitting.current || !checked.length) return;
    submitting.current = true; setBusy(true); onBusyChange(true); setErrors({});
    const succeeded = new Set<string>();
    const failures: Record<string, string> = {};
    if (!batchRef.current && checked.length > 1) {
      batchRef.current = {id: crypto.randomUUID(), title: "Telegram 所选消息"};
    }
    try {
      for (const link of checked) {
        try {
          if (batchRef.current) await api.enqueue(link, undefined, batchRef.current);
          else await api.enqueue(link);
          succeeded.add(link);
        }
        catch (error) { failures[link] = error instanceof Error ? error.message : String(error); }
      }
      setLinks(current => current.filter(link => !succeeded.has(link)));
      setChecked(current => current.filter(link => !succeeded.has(link)));
      setErrors(failures);
      if (!Object.keys(failures).length) batchRef.current = null;
      setNotice(`已加入 ${succeeded.size} 条下载${Object.keys(failures).length ? `，${Object.keys(failures).length} 条失败，可重试。` : "。"}`);
      try { await onAdded(); } catch { setNotice("任务已提交，但列表刷新失败，请稍后查看下载中心。"); return; }
      if (succeeded.size === links.length) onQueued();
    } finally { submitting.current = false; setBusy(false); onBusyChange(false); }
  };
  return <section className="telegram-batch" aria-label="Telegram 批量下载">
    <label htmlFor="telegram-links">消息链接</label>
    <textarea id="telegram-links" value={text} disabled={busy} onChange={event => setText(event.target.value)} placeholder={"粘贴多条 Telegram 消息链接，每行一条\nhttps://t.me/channel/123\nhttps://t.me/channel/124"} />
    <div className="telegram-collect"><span>最多 100 条 · 自动去重</span><button type="button" className="button secondary" disabled={busy || !text.trim()} onClick={collect}>识别链接</button></div>
    {notice && <p className="telegram-notice" role="status">{notice}</p>}
    {links.length > 0 && <>
      <div className="telegram-message-list">
        {links.map(link => <label className="telegram-message" key={link}>
          <input type="checkbox" aria-label={`选择 ${link}`} checked={checked.includes(link)} disabled={busy} onChange={() => setChecked(current => current.includes(link) ? current.filter(item => item !== link) : [...current, link])} />
          <PlatformIcon id="telegram" size={20} />
          <span><strong>{link.includes("/c/") ? "私密频道 / 群组" : link.split("/")[3]} · 消息 {link.split("/").at(-1)}</strong><small>{link}</small>{errors[link] && <small className="inline-error" role="alert">{errors[link]}</small>}</span>
        </label>)}
      </div>
      <div className="telegram-batch-footer">
        <label><input type="checkbox" aria-label="全选消息" checked={allChecked} ref={node => { if (node) node.indeterminate = checked.length > 0 && !allChecked; }} disabled={busy} onChange={() => setChecked(allChecked ? [] : [...links])} />全选<span>已选 {checked.length} / {links.length}</span></label>
        <button type="button" className="button primary" disabled={busy || !checked.length} onClick={() => void download()}>{busy ? "正在加入…" : "下载所选"}</button>
      </div>
    </>}
  </section>;
}
