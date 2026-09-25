import { Notice, type NoticeKind } from "./Notice";
import { AboutModal } from "./AboutModal";
import { DownloadDirectoryModal } from "./DownloadDirectoryModal";
import { XiaohongshuCard } from "./XiaohongshuCard";
import { XiaohongshuDownload } from "./XiaohongshuDownload";
import { TelegramBatch } from "./TelegramBatch";
import { TransferActivity } from "./TransferActivity";
import { workSummary, groupDownloads, batchProgress } from "./DownloadGroup";
import { TaskActions } from "./TaskActions";
import { BilibiliCard } from "./BilibiliCard";
import { BilibiliDownload } from "./BilibiliDownload";
import { DouyinCard } from "./DouyinCard";
import { DouyinLibrary } from "./DouyinLibrary";
import { VideoPlayer } from "./VideoPlayer";
import { PlatformSelect } from "./PlatformSelect";
import { PlatformIcon } from "./PlatformIcon";
import { PlatformCardHeader } from "./PlatformCardHeader";
import { splitXiaohongshuTopics } from "./xiaohongshuTopics";
import { TopicTags } from "./TopicTags";
import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type FormEvent,
} from "react";
import {
  ArrowDown,
  ArrowDownToLine,
  ArrowLeft,
  ArrowRight,
  Check,
  CircleHelp,
  Clock3,
  File,
  FileArchive,
  Film,
  FolderOpen,
  Image,
  Link2,
  LoaderCircle,
  Music2,
  Plus,
  Search,
  Send,
  Unplug,
  X,
} from "lucide-react";
import { api, desktop } from "./bridge";
import { LoginModal } from "./LoginModal";
import { Modal } from "./Modal";
import { MotionPresence } from "./motion";
import {
  useEntranceMotion,
  usePressFeedback,
} from "./winuiMotion";
import {
  detectPlatform,
  mediaLink,
  platformLinkError,
  previewPlatforms,
  type PlatformId,
  type PlatformInfo,
} from "./platforms";
import type {
  DownloadTask,
  Snapshot,
  TaskStatus,
} from "./types";
import { formatBytes } from "./utils";

type Page = "downloads" | "completed" | "platforms";
function pageFromHash(): Page {
  const route = location.hash.replace(/^#\//, "");
  return ["downloads", "completed", "platforms"].includes(route)
    ? (route as Page)
    : "downloads";
}
type Filter = "all" | "active" | "completed" | "paused" | "failed";
const empty: Snapshot = {
  settings: {
    downloadDir: "",
    concurrency: 4,
    apiId: "",
    apiHash: "",
    proxyUrl: "",
  },
  account: { connected: false, name: "", username: "" },
  tasks: [],
};
const statusNames: Record<TaskStatus, string> = {
  downloading: "下载中",
  queued: "排队中",
  resolving: "解析中",
  paused: "已暂停",
  completed: "已完成",
  failed: "失败",
  canceled: "已取消",
};
const errorText = (e: unknown) => (e instanceof Error ? e.message : String(e));
const platformName = (id: PlatformId) =>
  previewPlatforms.find((p) => p.id === id)?.name ?? id;
const dateLabel = (time: number) =>
  new Date(time).toLocaleString("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
function FileGlyph({ name, large = false, thumbnail }: { name: string; large?: boolean; thumbnail?: string | null }) {
  const [failed, setFailed] = useState<string | null>(null);
  const ext = name.split(".").pop()?.toLowerCase() ?? "";
  const type = /mp4|mkv|mov|webm/.test(ext)
    ? "video"
    : /jpg|png|jpeg|webp/.test(ext)
      ? "image"
      : /mp3|flac|wav|m4a/.test(ext)
        ? "audio"
        : /zip|rar|7z/.test(ext)
          ? "archive"
          : "file";
  if (thumbnail && failed !== thumbnail) return <span className={`file-thumbnail ${large ? "large" : ""}`}><img src={thumbnail} alt="" loading="lazy" referrerPolicy="no-referrer" onError={() => setFailed(thumbnail)} /></span>;
  const Icon = {
    video: Film,
    image: Image,
    audio: Music2,
    archive: FileArchive,
    file: File,
  }[type];
  return (
    <span className={`file-glyph ${type} ${large ? "large" : ""}`}>
      <Icon size={large ? 34 : 22} strokeWidth={1.6} />
      <small>{/^[a-z0-9]{1,5}$/i.test(ext) ? ext.toUpperCase() : "FILE"}</small>
    </span>
  );
}
export default function FrameApp() {
  const [data, setData] = useState<Snapshot>(empty);
  const [xiaohongshuConnected, setXiaohongshuConnected] = useState(false);
  const [bilibiliConnected, setBilibiliConnected] = useState(false);
  const [douyinConnected, setDouyinConnected] = useState(false);
  useEffect(() => {
    let alive = true;
    const update = () => {void api.xiaohongshuStatus().then(status => {if(alive)setXiaohongshuConnected(status.sessionPresent);}).catch(()=>{});void api.bilibiliStatus().then(status => {if (alive) setBilibiliConnected(status.sessionPresent);}).catch(() => {});void api.douyinStatus().then(status => {if (alive) setDouyinConnected(status.sessionPresent);}).catch(() => {});};
    update();
    window.addEventListener("focus", update);
    return () => {alive = false; window.removeEventListener("focus", update);};
  }, []);
  const [platforms, setPlatforms] = useState<PlatformInfo[]>(previewPlatforms);
  const [page, setPage] = useState<Page>(pageFromHash);
  const [filter, setFilter] = useState<Filter>(() => pageFromHash() === "completed" ? "completed" : "all");
  const [source, setSource] = useState("all");
  const [query, setQuery] = useState("");
  const searchInput = useRef<HTMLInputElement>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [telegramAvatar, setTelegramAvatar] = useState<string | null>(null);
  useEffect(() => {
    let alive = true;
    setTelegramAvatar(null);
    if (data.account.connected) void api.telegramAvatar().then(avatar => {if (alive) setTelegramAvatar(avatar);}).catch(() => {});
    return () => {alive = false;};
  }, [data.account.connected, data.account.username, data.account.name]);
  const [checked, setChecked] = useState<string[]>([]);
  const [selectionMode, setSelectionMode] = useState(false);
  const [playingTask, setPlayingTask] = useState<DownloadTask | null>(null);
  const [batchDetail, setBatchDetail] = useState<string | null>(null);
  const [removing, setRemoving] = useState<DownloadTask[]>([]);
  const [removeBusy, setRemoveBusy] = useState(false);
  const [removeError, setRemoveError] = useState("");
  const [newOpen, setNewOpen] = useState(false);
  const [loginOpen, setLoginOpen] = useState(false);
  const [helpOpen, setHelpOpen] = useState(false);
  const [directoryOpen, setDirectoryOpen] = useState(false);
  const [notices, setNotices] = useState<{id: number; message: string; kind: NoticeKind}[]>([]);
  const noticeId = useRef(0);
  const setNotice = useCallback((message: string, kind: NoticeKind = "error") => {
    const id = ++noticeId.current;
    setNotices(previous => [...previous.filter(item => item.message !== message || item.kind !== kind), {id, message, kind}]);
  }, []);
  const [restoring, setRestoring] = useState(desktop);
  const [busyTask, setBusyTask] = useState<string | null>(null);
  const content = useRef<HTMLDivElement>(null);
  const appRoot = useRef<HTMLDivElement>(null);
  const taskList = useRef<HTMLDivElement>(null);
  useEntranceMotion(content, page);
  useEntranceMotion(taskList, `${filter}:${source}`);
  usePressFeedback(appRoot);
  const refresh = useCallback(async () => setData(await api.snapshot()), []);
  useEffect(() => {
    const navigate = () => {
      setPage(pageFromHash());
      setSelected(null);
      setQuery("");
      setFilter(pageFromHash() === "completed" ? "completed" : "all");
    };
    window.addEventListener("hashchange", navigate);
    return () => window.removeEventListener("hashchange", navigate);
  }, []);
  useEffect(() => {
    let alive = true;
    let unsubscribe: (() => void) | undefined;
    const update = () => {
      void api
        .snapshot()
        .then((s) => {
          if (alive) setData(s);
        })
        .catch((e) => {
          if (alive) setNotice(errorText(e));
        });
    };
    update();
    void api
      .platforms()
      .then((p) => {
        if (alive) setPlatforms(p);
      })
      .catch((e) => {
        if (alive) setNotice(errorText(e));
      });
    void api.subscribe(update).then((fn) => {
      if (alive) unsubscribe = fn;
      else fn();
    });
    if (desktop)
      void api
        .restore()
        .then(update)
        .catch((e) => {
          if (alive) setNotice(errorText(e));
        })
        .finally(() => {
          if (alive) setRestoring(false);
        });
    return () => {
      alive = false;
      unsubscribe?.();
    };
  }, []);
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (
        (e.ctrlKey || e.metaKey) &&
        e.key.toLowerCase() === "n" &&
        !loginOpen &&
        !helpOpen
      ) {
        e.preventDefault();
        setNewOpen(true);
      }
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, [loginOpen, helpOpen]);
  const changePage = (next: Page) => {
    location.hash = `/${next}`;
    setPage(next);
    setSelected(null);
    setQuery("");
    setFilter("all");
  };
  const run = async (action: () => Promise<unknown>, success?: string) => {
    try {
      await action();
      await refresh();
      if (success) setNotice(success, "success");
    } catch (e) {
      setNotice(errorText(e));
    }
  };
  const taskAction = async (task: DownloadTask, action: string) => {
    if (action === "select") { setSelectionMode(true); setChecked(ids => ids.includes(task.id) ? ids : [...ids, task.id]); return; }
    if (action === "remove") {  setRemoveError(""); setRemoving([task]); return; }
    if (busyTask) return;
    setBusyTask(task.id);
    await run(
      () =>
        action === "copy"
          ? navigator.clipboard.writeText(task.url)
          : action === "open"
          ? api.openFile(task.id)
          : action === "reveal"
            ? api.reveal(task.id)
            : action === "remove"
              ? api.remove(task.id)
              : api.control(task.id, action),
      action === "remove" ? "已移除记录，本地文件仍保留。" : action === "copy" ? "链接已复制" : undefined,
    );
    setBusyTask(null);
  };
  const active = data.tasks.filter((t) => t.status === "downloading");
  const speed = active.reduce((sum, t) => sum + t.speed, 0);
  const allWorks = groupDownloads(data.tasks);
  const summaries = allWorks.map(group=>workSummary(group.items));
  const queued = summaries.filter(t => t.status === "queued").length;
  const resolving = summaries.filter(t => t.status === "resolving").length;
  const downloading = summaries.filter(t => t.status === "downloading").length;
  const paused = summaries.filter(t => t.status === "paused").length;
  const failed = summaries.filter(t => t.status === "failed").length;
  const completed = summaries.filter((t) => t.status === "completed");
  const works = allWorks.filter(group => {
    const t = workSummary(group.items);
    return (
      (filter === "all" || (filter === "active" ? ["queued", "resolving", "downloading"].includes(t.status) : t.status === filter)) &&
      (source === "all" || t.platform === source) &&
      `${t.title} ${group.items.map(item=>`${item.title} ${item.fileName} ${item.source} ${item.url}`).join(" ")}`
        .toLowerCase()
        .includes(query.toLowerCase())
    );
  });
  const tasks = works.flatMap(group => group.items);
  const currentDetail = data.tasks.find((t) => t.id === selected);
  const checkedTasks = tasks.filter(t => checked.includes(t.id));
  const allChecked = tasks.length > 0 && checkedTasks.length === tasks.length;
  const cannotRemove = checkedTasks.some(t => ["resolving", "downloading"].includes(t.status));
  const confirmRemove = async (deleteFiles: boolean) => {
    if (removeBusy) return;
    setRemoveBusy(true);
    setRemoveError("");
    const failed: DownloadTask[] = [];
    const errors: string[] = [];
    for (const task of removing) {
      try {
        await api.remove(task.id, deleteFiles);
        setChecked(ids => ids.filter(id => id !== task.id));
      } catch (error) { failed.push(task); errors.push(errorText(error)); }
    }
    setRemoving(failed);
    if (errors.length) setRemoveError(errors.join("；"));
    await refresh().catch(() => setNotice("刷新列表失败，请稍后重试。"));
    setRemoveBusy(false);
  };
  const detail = currentDetail ? workSummary(groupDownloads(data.tasks).find(g => g.items.some(t => t.id === currentDetail.id))!.items) : undefined;
  const connected = data.account.connected;
  return (
    <div className="frame-app" ref={appRoot}>
      <header className="workspace-header">
        <h1 className="workspace-title">拾帧</h1>
        <nav className="workspace-actions" aria-label="主导航">
          {page === "platforms" && <button className="header-action" onClick={() => changePage("downloads")}><ArrowLeft size={17} /><span>返回下载</span></button>}
          <button className="header-action" title={data.settings.downloadDir || "设置默认下载根目录"} onClick={() => setDirectoryOpen(true)}>下载目录设置</button>
          <button className="header-action" title="打开下载文件夹" onClick={() => void run(api.openFolder)}><FolderOpen size={18} /><span>下载文件夹</span></button>
          <button className="header-action" aria-current={page === "platforms" ? "page" : undefined} onClick={() => changePage("platforms")}><Unplug size={18} /><span>平台连接</span></button>
          <button className="header-action header-icon" aria-label="说明" title="说明" onClick={() => setHelpOpen(true)}><CircleHelp size={19} /></button>
          <button className="button primary new-button" onClick={() => setNewOpen(true)}><Plus size={19} />新建下载<kbd>{/Mac/i.test(navigator.platform) ? "⌘N" : "Ctrl+N"}</kbd></button>
        </nav>
      </header>
      <div className="app-body">
        <main ref={content} className="main-content">
          {(page === "downloads" || page === "completed") && (
            <>
              <section className="download-surface" aria-label="下载任务">
                <div className="list-toolbar">
                  <div className="task-filters" role="group" aria-label="任务分类">
                    {([
                      ["all", "全部", summaries.length],
                      ["active", "进行中", summaries.filter(t => ["queued", "resolving", "downloading"].includes(t.status)).length],
                      ["completed", "已完成", completed.length],
                      ["paused", "已暂停", paused],
                      ["failed", "需处理", failed],
                    ] as const).filter(([value, , count]) => !["paused", "failed"].includes(value) || count > 0 || filter === value).map(([value, label, count]) => <button className={`task-filter-button ${value === "failed" && count ? "needs-attention" : ""}`} key={value} aria-pressed={filter === value} onClick={() => setFilter(value)}>{label}<span>{count}</span></button>)}
                  </div>
                  <div className="list-tools">
                    <PlatformSelect value={source} onChange={setSource} options={[{ value: "all", label: "全部平台" }, ...platforms.map(p => ({ value: p.id, label: p.name }))]} />
                    <div className="search-box" data-filled={query.length > 0}>
                      <Search className="search-icon" size={18} aria-hidden="true" />
                      <input
                        ref={searchInput}
                        aria-label="搜索下载记录"
                        value={query}
                        onChange={(e) => setQuery(e.target.value)}
                        placeholder="搜索下载记录"
                      />
                      {query && (
                        <button
                          type="button"
                          className="search-clear"
                          aria-label="清除搜索"
                          onClick={() => { setQuery(""); searchInput.current?.focus(); }}
                        >
                          <X size={13} />
                        </button>
                      )}
                    </div>
                  </div>
                </div>
                <div className="task-area">
                  <div className="task-list">
                    {selectionMode && <div className="batch-actions">
                      <span>已选 {checkedTasks.length} 项</span>
                      <button className="text-button" onClick={() => { setChecked([]); setSelectionMode(false); }}>取消选择</button>
                      <button className="button secondary" disabled={cannotRemove || !checkedTasks.length} title={cannotRemove ? "请先暂停正在解析或下载的任务" : "移除所选记录"} onClick={() => {  setRemoveError(""); setRemoving(checkedTasks); }}>移除所选</button>
                    </div>}
                    <div className="list-columns">
                      <span className="file-column-heading">{selectionMode && <input type="checkbox" aria-label="全选当前列表" checked={allChecked} ref={node => { if (node) node.indeterminate = checkedTasks.length > 0 && !allChecked; }} onChange={() => setChecked(ids => allChecked ? ids.filter(id => !tasks.some(t => t.id === id)) : [...new Set([...ids, ...tasks.map(t => t.id)])])} />}文件 / 来源</span>
                      <span>传输状态</span>
                      <span>时间</span>
                      <span>操作</span>
                    </div>
                    <div className="task-scroll" ref={taskList}>
                      {works.map(group => {
                        const task = workSummary(group.items);
                        const toggle = () => setChecked(ids => group.items.every(t => ids.includes(t.id)) ? ids.filter(id => !group.items.some(t => t.id === id)) : [...new Set([...ids,...group.items.map(t=>t.id)])]);
                        return <TaskRow key={group.id} task={task} batchCount={task.batch ? batchProgress(group.items) : undefined} selected={selected === task.id}
                          checked={group.items.every(t => checked.includes(t.id))} onCheck={selectionMode ? toggle : undefined}
                          busy={group.items.some(t => busyTask === t.id)}
                          onOpen={() => {if (selectionMode) toggle(); else if(task.batch) setBatchDetail(task.batch.id); else if (task.status === "completed") setPlayingTask(group.items[0]); else setSelected(task.id);}}
                          onSelect={() => task.batch ? setBatchDetail(task.batch.id) : setSelected(selected === task.id ? null : task.id)}
                          onAction={action => {
                            if (action === "select") {setSelectionMode(true); setChecked(ids => [...new Set([...ids,...group.items.map(t=>t.id)])]);}
                            else if (action === "remove") {setRemoveError("");setRemoving(group.items);}
                            else if (action === "open" && task.batch) setBatchDetail(task.batch.id);
                            else if (action === "open" && group.items.length > 1) setPlayingTask(group.items[0]);
                            else if (["copy","reveal","open"].includes(action)) void taskAction(group.items[0],action);
                            else if(task.discovery) void taskAction(task,action); else void (async () => {for (const member of group.items) {
                              if (action === "pause" && !["queued","resolving","downloading"].includes(member.status)) continue;
                              if (action === "resume" && !["paused","failed"].includes(member.status)) continue;
                              if (action === "cancel" && ["completed","canceled"].includes(member.status)) continue;
                              await taskAction(member,action);
                            }})();
                          }}/>
                      })}
                      {!tasks.length && (
                        <div className="empty-state">
                          <div className="empty-art">
                            <span />
                            <span />
                            <div>
                              <ArrowDownToLine size={30} strokeWidth={1.4} />
                            </div>
                          </div>
                          <h2>
                            {data.tasks.length
                              ? "没有找到匹配的文件"
                              : "暂无下载任务"}
                          </h2>
                          <p>
                            {data.tasks.length
                              ? "试试其他关键词，或调整上方的筛选条件。"
                              : "粘贴分享链接，将视频、图片或文件保存到本地。"}
                          </p>
                          <button
                            className="button secondary"
                            onClick={() =>
                              data.tasks.length
                                ? (setQuery(""),
                                  setFilter("all"),
                                  setSource("all"))
                                : setNewOpen(true)
                            }
                          >
                            {data.tasks.length ? "清除筛选" : "新建下载"}
                            <ArrowRight size={16} />
                          </button>
                        </div>
                      )}
                    </div>
                  </div>

                </div>
              </section>
            </>
          )}
          {page === "platforms" && (
            <>
              <div className="page-heading">
                <div>
                  <h1>平台连接</h1>

                </div>
              </div>
              <div className="platform-directory">
                {platforms.map((p) => (
                  p.id === "xiaohongshu" ? <XiaohongshuCard key={p.id} onConnectionChange={setXiaohongshuConnected}/> :
                  p.id === "bilibili" ? <BilibiliCard key={p.id} onConnectionChange={setBilibiliConnected} /> :
                  p.id === "douyin" ? <DouyinCard key={p.id} onConnectionChange={setDouyinConnected} /> :
                  <section key={p.id} className={`platform-card ${p.id}`}>
                    <PlatformCardHeader id={p.id} name={p.name} connected={connected} available={p.available}/>
                    <div className="platform-card-bottom">
                      {p.available ? (
                        connected ? (
                          <>
                            <span className="connected-name" title={data.account.name}>
                              {telegramAvatar ? <img className="account-avatar" src={telegramAvatar} alt="Telegram 账号头像" onError={() => setTelegramAvatar(null)} /> : <span className="account-avatar account-avatar-fallback" aria-label="默认头像">{Array.from(data.account.name || "T")[0]}</span>}
                              <span className="account-nickname">{data.account.name}</span>
                            </span>
                            <button
                              className="text-button"
                              onClick={() =>
                                void run(api.logout, "已断开 Telegram 连接。")
                              }
                            >
                              断开连接
                            </button>
                          </>
                        ) : (
                          <button
                            className="button secondary"
                            onClick={() => setLoginOpen(true)}
                          >
                            扫码连接
                            <ArrowRight size={15} />
                          </button>
                        )
                      ) : (
                        <button className="button secondary" disabled>
                          敬请期待
                        </button>
                      )}
                    </div>
                  </section>
                ))}
              </div>
            </>
          )}
        </main>
        <footer className="app-statusbar">
          <button className="connected-platform-status connection-shortcut" aria-label="平台连接状态，点击管理" onClick={() => changePage("platforms")}>
            {platforms.filter(p => p.id === "telegram" ? connected : p.id === "douyin" ? douyinConnected : p.id === "xiaohongshu" ? xiaohongshuConnected : p.id === "bilibili" && bilibiliConnected).map(p => <span key={p.id}><i className="connection-dot online" aria-hidden="true"/>{p.name} 已连接</span>)}
            {!connected && !douyinConnected && !bilibiliConnected && !xiaohongshuConnected && <span>{restoring ? "正在恢复连接" : "暂无已连接平台"}</span>}
          </button>
          <div className="status-metrics"><TransferActivity tasks={data.tasks} chartOnly /><span className="status-rate"><ArrowDown size={12} aria-hidden="true" /><span>{formatBytes(speed)}/s</span></span><span className="status-separator" aria-hidden="true" /><span className="status-rate status-queued"><Clock3 size={12} aria-hidden="true" /><span>{[downloading && `${downloading} 个下载中`, resolving && `${resolving} 个解析中`, queued && `${queued} 个排队中`].filter(Boolean).join(" · ") || (paused ? `${paused} 个已暂停` : failed ? `${failed} 个需处理` : "暂无进行中的任务")}</span></span><span className="status-separator" aria-hidden="true" /><span className="status-completed">{completed.length} 个已完成</span></div>
        </footer>
      </div>
      {notices.map(notice => <Notice key={notice.id} kind={notice.kind} message={notice.message}
        onClose={() => setNotices(previous => previous.filter(item => item.id !== notice.id))}/>)}
      {removing.length > 0 && <Modal title={removing.length === 1 ? "移除这条记录？" : `移除 ${removing.length} 条记录？`} busy={removeBusy} onClose={() => setRemoving([])}>
        {removeError && <p className="inline-error" role="alert">{removeError}</p>}
        <div className="modal-actions">
          <button className="button secondary" disabled={removeBusy} onClick={() => void confirmRemove(false)}>仅删除记录</button>
          <button className="button primary" disabled={removeBusy} title="同时删除本地文件和临时文件，无法撤销" onClick={() => void confirmRemove(true)}>同时删除文件</button>
        </div>
      </Modal>}
      {detail && removing.length === 0 && (
        <Modal title="文件详情" onClose={() => setSelected(null)}>
          <div className="file-detail-dialog">
                        <div className="detail-art">
                          <FileGlyph name={detail.fileName} thumbnail={detail.thumbnail} large />
                        </div>
                        <h3>{detail.title || detail.fileName}</h3>
                        <span className={`status-label ${detail.status}`}>
                          {statusNames[detail.status]}
                        </span>
                        <dl>
                          <dt>来源平台</dt>
                          <dd>{platformName(detail.platform)}</dd>
                          <dt>文件大小</dt>
                          <dd>{formatBytes(detail.totalBytes)}</dd>
                          <dt>添加时间</dt>
                          <dd>{dateLabel(detail.createdAt)}</dd>
                          <dt>保存位置</dt>
                          <dd className="path-value">
                            {detail.outputPath || "使用默认下载目录"}
                          </dd>
                        </dl>
                        <label className="detail-link">
                          原始链接
                          <input
                            readOnly
                            value={detail.url}
                            aria-label="原始链接"
                            onFocus={(e) => e.target.select()}
                          />
                        </label>
                        {detail.error && (
                          <p className="inline-error">{detail.error}</p>
                        )}
                          {!["completed", "canceled"].includes(
                            detail.status,
                          ) && (
                            <>
                              <button
                                className="text-button danger"
                                disabled={busyTask === detail.id}
                                onClick={() =>
                                  void (async () => {for (const member of groupDownloads(data.tasks).find(g=>g.items.some(t=>t.id === detail.id))!.items) {if (!["completed","canceled"].includes(member.status)) await taskAction(member,"cancel");}})()
                                }
                              >
                                <X size={14} />
                                取消下载
                              </button>
                              <small>取消后会清理未完成的临时文件。</small>
                            </>
                          )}

          </div>
        </Modal>
      )}
      {batchDetail && <Modal title={data.tasks.find(t=>t.batch?.id === batchDetail)?.batch?.title || "批量下载"} onClose={()=>setBatchDetail(null)}>
        {data.tasks.filter(t=>t.batch?.id === batchDetail && t.error).map(t=><p role="alert" key={t.id}>{t.error}</p>)}
        <div className="batch-work-list">{groupDownloads(data.tasks.filter(t=>t.batch?.id === batchDetail && !t.discovery).map(t=>({...t,batch:undefined}))).map(group=>{const task=workSummary(group.items);return <button className="batch-work-item" key={group.id} disabled={task.status !== "completed"} onClick={()=>setPlayingTask(group.items[0])}><FileGlyph name={task.fileName} thumbnail={task.thumbnail}/><span><strong>{task.title}</strong><small>{statusNames[task.status]}</small></span></button>;})}</div>
      </Modal>}
      {playingTask && <VideoPlayer galleryTasks={groupDownloads(data.tasks.map(t=>({...t,batch:undefined}))).find(g=>g.items.some(t=>t.id === playingTask.id))?.items} task={playingTask} onClose={() => setPlayingTask(null)} />}
      {newOpen && (
        <NewDownload
          platforms={platforms}
          connected={connected}
          onClose={() => setNewOpen(false)}
          onLogin={() => {
            setNewOpen(false);
            setLoginOpen(true);
          }}
          onAdded={async () => {
            await refresh();
            changePage("downloads");
            setSource("all");
            setNotice("已加入下载队列。", "success");
          }}
        />
      )}
      {directoryOpen && <DownloadDirectoryModal directory={data.settings.downloadDir} onClose={() => setDirectoryOpen(false)} onSaved={async () => { await refresh(); setNotice("默认下载根目录已保存。", "success"); }} />}
      {loginOpen && (
        <LoginModal
          settings={data.settings}
          onClose={() => setLoginOpen(false)}
          onConnected={(account) => {
            setData((s) => ({ ...s, account }));
            void refresh();
            setNotice("Telegram 已连接，可以开始下载了。", "success");
          }}
        />
      )}
      {helpOpen && <AboutModal onClose={() => setHelpOpen(false)} />}
    </div>
  );
}

export function TaskRow({
  task,
  selected,
  checked = false,
  batchCount,
  onCheck,
  onOpen,
  busy,
  onSelect,
  onAction,
}: {
  task: DownloadTask;
  selected: boolean;
  checked?: boolean;
  batchCount?: {total:number;completed:number};
  onCheck?: () => void;
  onOpen?: () => void;
  busy: boolean;
  onSelect: () => void;
  onAction: (action: string) => void;
}) {
  const {title: displayTitle, topics} = ["xiaohongshu", "douyin"].includes(task.platform)
    ? splitXiaohongshuTopics(task.title || task.fileName, task.topics, task.platform === "douyin" ? "抖音作品" : "小红书作品")
    : {title: task.title || task.fileName, topics: task.topics || []};
  const percent = batchCount ? Math.min(task.discovery && !task.discovery.done ? 99 : 100,batchCount.completed / Math.max(1,batchCount.total)*100) : task.totalBytes
    ? Math.min(100, Math.max(0, (task.downloadedBytes / task.totalBytes) * 100))
    : 0;
  return (
    <article className={`task-row ${task.status} ${selected || checked ? "selected" : ""}`} aria-label={`${task.title || task.fileName}，${statusNames[task.status]}`}>
      <div className="task-file-cell">
      {onCheck && <input type="checkbox" aria-label={`选择 ${task.title || task.fileName}`} checked={checked} onChange={onCheck} />}
      <div className="task-identity">
      <button
        type="button"
        className="task-open"
        aria-label={onCheck ? `选择 ${task.title || task.fileName}` : task.batch ? `查看 ${task.title} 的作品` : onOpen && task.status === "completed" ? `播放 ${task.title || task.fileName}` : `查看 ${task.title || task.fileName} 的详情`}
        aria-pressed={selected}
        onClick={onOpen || onSelect}
      />
        <FileGlyph name={task.fileName} thumbnail={task.thumbnail} />
        <span>
          <strong title={displayTitle}>
            {displayTitle}
          </strong>
          <small className="task-metadata">
            <span className={`task-meta-chip task-platform-chip ${task.platform}`} title={task.source || platformName(task.platform)}>
              <PlatformIcon id={task.platform} size={13} />
              <span className="task-source">{task.source || platformName(task.platform)}</span>
            </span>
            {batchCount && <span className="task-meta-chip">{batchCount.completed} / {batchCount.total} 个作品</span>}
            {task.discovery && !task.discovery.done && !["paused","failed","canceled"].includes(task.status) && <span className="task-meta-chip">读取第 {task.discovery.pages+1} 页</span>}
            {task.totalBytes > 0 && <span className="task-meta-chip task-size-chip">{formatBytes(task.totalBytes)}</span>}
            <TopicTags topics={topics}/>
          </small>
        </span>
      </div>
      </div>
      <div className="task-progress">
        <div className="progress-caption">
          <span className={`status-label ${task.status}`}>
            {task.status === "completed" && <Check size={12} />}
            {task.status === "resolving" && <LoaderCircle className="spin" size={12} />}
            {statusNames[task.status]}
          </span>
          {task.status === "failed" ? <button type="button" className="failure-details" onClick={onSelect}>查看详情</button> : <small>
            {task.status === "resolving" || task.status === "queued" ? "" : task.status === "downloading"
              ? task.totalBytes || batchCount ? `${Math.round(percent)}%` : ""
              : task.status === "completed"
                ? ""
                : `${formatBytes(task.downloadedBytes)}`}
          </small>}
        </div>
        {["downloading", "paused"].includes(task.status) && <div
          className={`progress-track ${task.status} ${!task.totalBytes && !batchCount ? "indeterminate" : ""}`}
          role="progressbar"
          aria-label={`${task.title} 下载进度`}
          aria-valuenow={task.totalBytes || batchCount ? Math.round(percent) : undefined}
          aria-valuetext={task.totalBytes || batchCount ? `${Math.round(percent)}%` : "文件大小未知"}
          aria-valuemin={0}
          aria-valuemax={100}
        >
          <span style={{ width: `${percent}%` }} />
        </div>}
        {task.status === "downloading" && (
          <small className="row-speed"><span>{formatBytes(task.downloadedBytes)}{task.totalBytes > 0 ? ` / ${formatBytes(task.totalBytes)}` : " · 大小未知"}</span><span>{formatBytes(task.speed)}/s</span></small>
        )}
        {task.status === "resolving" && <small className="task-state-note">正在读取文件信息</small>}
        {task.status === "queued" && <small className="task-state-note">等待空闲下载通道</small>}
        {task.status === "failed" && <small className="task-state-note task-error-note" title={task.error || undefined}>{task.error || "下载未完成，可重试或查看详情"}</small>}
      </div>
      <time className="task-time" dateTime={new Date(task.status === "completed" ? task.updatedAt || task.createdAt : task.createdAt).toISOString()} title={`${task.status === "completed" ? "完成时间" : "创建时间"}：${new Date(task.status === "completed" ? task.updatedAt || task.createdAt : task.createdAt).toLocaleString("zh-CN")}`}>
        {dateLabel(task.status === "completed" ? task.updatedAt || task.createdAt : task.createdAt)}
      </time>
      <div className="row-actions" aria-busy={busy}>
        <TaskActions task={task} busy={busy} onAction={onAction} onSelect={onSelect} />
      </div>
    </article>
  );
}

export function NewDownload({
  platforms,
  connected,
  onClose,
  onLogin,
  onAdded,
}: {
  platforms: PlatformInfo[];
  connected: boolean;
  onClose: () => void;
  onLogin: () => void;
  onAdded: () => Promise<void>;
}) {
  const [url, setUrl] = useState("");
  const [xiaohongshuUrl,setXiaohongshuUrl]=useState("");
  const [bilibiliUrl, setBilibiliUrl] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [sessions, setSessions] = useState({douyin: false, bilibili: false, xiaohongshu: false});
  const [mode, setMode] = useState("link");
  useEffect(() => {
    let alive = true;
    const checks = [
      ["douyin", api.douyinStatus],
      ["bilibili", api.bilibiliStatus],
      ["xiaohongshu", api.xiaohongshuStatus],
    ] as const;
    for (const [id, check] of checks) {
      void check().then(status => {
        if (alive) setSessions(previous => ({...previous, [id]: status.sessionPresent}));
      }).catch(() => {});
    }
    return () => {alive = false;};
  }, []);
  const platformTabs = [
    {id: "telegram", label: "Telegram", connected},
    {id: "douyin", label: "抖音", connected: sessions.douyin},
    {id: "bilibili", label: "Bilibili", connected: sessions.bilibili},
    {id: "xiaohongshu", label: "小红书", connected: sessions.xiaohongshu},
  ].filter(tab => tab.connected && platforms.some(p => p.id === tab.id && p.available));
  const currentUrl = mode === "xiaohongshu" ? xiaohongshuUrl : mode === "bilibili" ? bilibiliUrl : url;
  const platform = detectPlatform(currentUrl, platforms);
  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (mode === "bilibili" || mode === "xiaohongshu") return;
    const invalid = platformLinkError(url, platforms);
    if (invalid) {
      setError(invalid);
      return;
    }
    if (platform?.id === "telegram" && platform.requiresAccount && !connected) {
      setError("请先连接 Telegram，再加入下载。");
      return;
    }
    setBusy(true);
    setError("");
    try {
      if (platform?.id === "douyin" && !(await api.douyinStatus()).sessionPresent) throw new Error("请先在平台连接中登录抖音。");
      await api.enqueue(mediaLink(url));
      await onAdded();
      onClose();
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <Modal
      title="新建下载"
      className={mode === "xiaohongshu" ? "new-download-modal xiaohongshu-download-modal" : (mode === "link" || mode === "bilibili") ? "new-download-modal" : "new-download-modal library-download-modal"}
      onClose={onClose}
      busy={busy}
    >
      {platformTabs.length > 0 && <div className="download-source-switch" role="group" aria-label="下载来源">
        <div className="download-source-platforms">
          {platformTabs.map(tab => <button key={tab.id} type="button" aria-pressed={mode === tab.id} disabled={busy} onClick={() => {setMode(tab.id);setError("");}}><PlatformIcon id={tab.id as PlatformId} size={16}/> {tab.label}</button>)}
        </div>
        <button className="download-source-link" type="button" aria-pressed={mode === "link"} disabled={busy} onClick={() => {setMode("link");setError("");}}><Link2 size={16}/> 链接下载</button>
      </div>}
      {mode === "telegram" ? <TelegramBatch onAdded={onAdded} onQueued={onClose} onBusyChange={setBusy}/> : mode === "douyin" ? <DouyinLibrary key={mode} initialKind="likes" onAdded={onAdded} onBusyChange={setBusy} onQueued={onClose}/> : <form onSubmit={submit}>
        <label>
          {mode === "xiaohongshu" ? "小红书作品链接" : mode === "bilibili" ? "Bilibili 视频链接" : "媒体链接"}
          <div className="link-input">
            <Link2 size={17} />
            <input
              autoComplete="off"
              value={currentUrl}
              disabled={busy}
              onChange={(e) => {
                (mode === "xiaohongshu" ? setXiaohongshuUrl : mode === "bilibili" ? setBilibiliUrl : setUrl)(e.target.value);
                setError("");
              }}
              placeholder={mode === "xiaohongshu" ? "粘贴小红书分享链接或文案" : mode === "bilibili" ? "https://www.bilibili.com/video/BV…" : "https://t.me/channel/123"}
              aria-label={mode === "xiaohongshu" ? "小红书作品链接" : mode === "bilibili" ? "Bilibili 视频链接" : "媒体链接"}
            />
          </div>
        </label>
        {(mode === "link" || !platform) && <div className="detected-platform">
          {platform ? (
            <>
              <PlatformIcon id={platform.id} size={15} />
              {platform.name}
              <span className="platform-detection-status">{platform.available ? "已识别" : "尚未接入"}</span>
            </>
          ) : (
            <>
              <Send size={14} />
              {mode === "xiaohongshu" ? "粘贴作品链接，预览视频或图片" : mode === "bilibili" ? "粘贴 Bilibili 链接，解析封面并选择下载内容" : "支持 Telegram、抖音、Bilibili 和小红书链接"}
            </>
          )}
        </div>}
        {error && (
          <p className="inline-error" role="alert">
            {error}
          </p>
        )}
        {mode === "link" && platform?.id === "telegram" && platform.available && platform.requiresAccount && !connected && (
          <button
            type="button"
            className="connection-callout"
            onClick={onLogin}
          >
            <PlatformIcon id="telegram" size={17} />
            <span>先连接 Telegram 账号</span>
            <ArrowRight size={16} />
          </button>
        )}
        {mode === "xiaohongshu" ? (platform?.id === "xiaohongshu" && platform.available
          ? <XiaohongshuDownload url={mediaLink(xiaohongshuUrl)} onAdded={onAdded} onClose={onClose} onBusyChange={setBusy}/>
          : <>{xiaohongshuUrl.trim() && <p className="inline-error" role="alert">请输入有效的小红书作品链接。</p>}<div className="modal-actions"><button type="button" className="button secondary" onClick={onClose}>取消</button></div></>) : mode === "bilibili" ? (platform?.id === "bilibili" && platform.available
          ? <BilibiliDownload url={mediaLink(bilibiliUrl)} onAdded={onAdded} onClose={onClose} onBusyChange={setBusy}/>
          : <>{bilibiliUrl.trim() && <p className="inline-error" role="alert">请输入有效的 Bilibili 视频链接。</p>}<div className="modal-actions"><button type="button" className="button secondary" onClick={onClose}>取消</button></div></>) : <div className="modal-actions">
          <button
            type="button"
            className="button secondary"
            onClick={onClose}
            disabled={busy}
          >
            取消
          </button>
          <button className="button primary" disabled={busy || !url.trim()}>
            {busy ? (
              <LoaderCircle className="spin" size={17} />
            ) : (
              <ArrowRight size={17} />
            )}
            {busy ? "正在加入…" : "加入下载"}
          </button>
        </div>}
      </form>}
    </Modal>
  );
}
