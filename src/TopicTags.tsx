import { topicColor } from "./xiaohongshuTopics";
import { useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { Modal } from "./Modal";

export function TopicTags({topics}: {topics: string[]}) {
  const host = useRef<HTMLSpanElement>(null);
  const measure = useRef<HTMLSpanElement>(null);
  const [visible, setVisible] = useState(0);
  const [open, setOpen] = useState(false);
  const signature = JSON.stringify(topics);
  useLayoutEffect(() => {
    const fit = () => {
      const width = host.current?.getBoundingClientRect().width || 0;
      const tags = Array.from(measure.current?.children || []);
      let used = 0, count = 0;
      for (let i = 0; i < Math.min(tags.length, 4); i++) {
        used += tags[i].getBoundingClientRect().width + (i ? 6 : 0);
        const more = topics.length > i + 1 ? 6 + 24 + String(topics.length - i - 1).length * 8 : 0;
        if (used + more > width) break;
        count++;
      }
      setVisible(count);
    };
    fit();
    const observer = typeof ResizeObserver !== "undefined" ? new ResizeObserver(fit) : null;
    if (host.current) observer?.observe(host.current);
    let alive = true;
    document.fonts?.ready.then(() => { if (alive) fit(); });
    return () => { alive = false; observer?.disconnect(); };
  }, [signature]);
  if (!topics.length) return null;
  const tag = (topic: string) => <span key={topic} className={`task-meta-chip topic-tag topic-color-${topicColor(topic)}`} title={`#${topic}`}>#{topic}</span>;
  return <span className="topic-tags-inline" ref={host}>
    <span className="topic-tags-measure" ref={measure} aria-hidden="true">{topics.slice(0, 4).map(tag)}</span>
    {topics.slice(0, visible).map(tag)}
    {topics.length > visible && <button type="button" className="task-meta-chip topic-more" aria-label={`查看全部 ${topics.length} 个话题`} aria-haspopup="dialog" aria-expanded={open} onClick={e => {e.stopPropagation();setOpen(true);}}>+{topics.length - visible}</button>}
    {open && createPortal(<Modal title={`全部话题 · ${topics.length}`} className="topics-modal" onClose={() => setOpen(false)}><div className="topic-tags-full">{topics.map(tag)}</div></Modal>, document.body)}
  </span>;
}
