import { useEffect, useRef, useState } from "react";
import { ArrowDown, Check, Pause, CircleAlert } from "lucide-react";
import type { DownloadTask } from "./types";
import { formatBytes } from "./utils";

export function TransferActivity({ tasks, chartOnly = false }: { tasks: DownloadTask[]; chartOnly?: boolean }) {
  const previous = useRef(new Map<string, DownloadTask["status"]>());
  const [justCompleted, setJustCompleted] = useState(false);
  useEffect(() => {
    const completedNow = tasks.some(t => t.status === "completed" &&
      ["downloading", "resolving"].includes(previous.current.get(t.id) ?? ""));
    const busy = tasks.some(t => ["downloading", "resolving", "queued"].includes(t.status));
    if (busy || !tasks.length) setJustCompleted(false);
    else if (completedNow) setJustCompleted(true);
    previous.current = new Map(tasks.map(t => [t.id, t.status]));
  }, [tasks]);
  useEffect(() => {
    if (!justCompleted) return;
    const timer = window.setTimeout(() => setJustCompleted(false), 4000);
    return () => window.clearTimeout(timer);
  }, [justCompleted]);
  const downloading = tasks.filter(t => t.status === "downloading");
  const speed = downloading.reduce((sum, t) => sum + (Number.isFinite(t.speed) ? Math.max(0, t.speed) : 0), 0);
  const state = downloading.length ? "active"
    : tasks.some(t => t.status === "resolving" || t.status === "queued") ? "waiting"
    : tasks.some(t => t.status === "failed") ? "failed"
    : tasks.some(t => t.status === "paused") ? "paused"
    : justCompleted ? "completed" : "idle";
  const latest = useRef(speed);
  latest.current = speed;
  const [history, setHistory] = useState<number[]>(() => Array(30).fill(0));
  useEffect(() => {
    if (state === "idle") setHistory(Array(30).fill(0));
    if (state !== "active" && state !== "waiting") return;
    const timer = window.setInterval(() => setHistory(values => [...values.slice(1), latest.current]), 1000);
    return () => window.clearInterval(timer);
  }, [state]);
  const peak = Math.max(1024, ...history);
  const label = state === "active" ? `${formatBytes(speed)}/s`
    : state === "waiting" ? "准备下载" : state === "paused" ? "已暂停"
    : state === "failed" ? "下载有异常" : state === "completed" ? "下载完成" : "拾帧";
  const Icon = state === "completed" ? Check : state === "paused" ? Pause : state === "failed" ? CircleAlert : ArrowDown;
  return <div className={`transfer-activity ${state}${chartOnly ? " chart-only" : ""}`} aria-label={`下载状态：${label}`}>
    {!chartOnly && <div className="transfer-reading">
      <span className="transfer-value"><Icon size={16} aria-hidden="true" />{label}</span>
      <span className="transfer-caption">{state === "active" ? `${downloading.length} 个任务下载中` : state === "idle" ? "随时开始新的下载" : state === "completed" ? "文件已保存到本机" : state === "failed" ? "请查看任务详情" : state === "paused" ? "继续后恢复传输" : "正在等待传输"}</span>
    </div>}
    <div className="transfer-chart" role="img" aria-label="最近 30 秒下载速率，柱高按当前窗口峰值缩放" title={`最近 30 秒 · 峰值 ${formatBytes(Math.max(0, ...history))}/s`}>
      {history.map((value, index) => <span key={index} style={{ height: `${Math.max(2, value / peak * (chartOnly ? 22 : 34))}px`, opacity: .32 + index / 29 * .68 }} />)}
    </div>
  </div>;
}
