import { useEffect, useRef, useState } from "react";
import { ArrowLeft, ArrowDownToLine, ChevronRight, Folder, Image, RefreshCw } from "lucide-react";
import { api, type DouyinItem } from "./bridge";
import { PlatformSelect } from "./PlatformSelect";
import { downloadMessage } from "./douyinBulk";

export function LibraryCover({src}: {src: string | null}) {
  const [state, setState] = useState<"loading" | "ready" | "failed">(src ? "loading" : "failed");
  return <span className={`library-row-cover ${state === "loading" ? "library-shimmer" : ""}`}>
    {src && state !== "failed" && <img src={src} alt="" loading="lazy" decoding="async" referrerPolicy="no-referrer" style={{opacity:state === "ready" ? 1 : 0}}
      ref={node => {if (node?.complete) setState(node.naturalWidth > 0 ? "ready" : "failed");}}
      onLoad={() => setState("ready")} onError={() => setState("failed")}/>}
    {state === "failed" && <Image size={22} aria-hidden="true"/>}
  </span>;
}
function LibrarySkeleton({count}: {count: number}) {
  return <div className="library-skeleton" role="status" aria-label="正在加载作品">
    <span className="library-sr-only">正在加载作品</span>
    {Array.from({length:count},(_,index)=><div className="library-skeleton-row" key={index} aria-hidden="true">
      <span className="skeleton-check"/><span className="skeleton-cover library-shimmer"/>
      <span className="skeleton-text"><span className="skeleton-title library-shimmer"/><span className="skeleton-author library-shimmer"/></span>
    </div>)}
  </div>;
}

export function DouyinLibrary({onAdded, initialKind = "likes", onBusyChange, onQueued}: {onAdded: () => Promise<void>; initialKind?: string; onBusyChange?: (busy: boolean) => void; onQueued?: () => void}) {
  const [kind, setKind] = useState(initialKind);
  const [folder, setFolder] = useState<DouyinItem | null>(null);
  const [authorInput, setAuthorInput] = useState("");
  const [author, setAuthor] = useState("");
  const resolvedAuthor = useRef("");


  const [items, setItems] = useState<DouyinItem[]>([]);
  const [cursor, setCursor] = useState("0");
  const [more, setMore] = useState(false);
  const [loading, setLoading] = useState(true);
  const [adding, setAdding] = useState(false);
  const [error, setError] = useState("");
  const [downloadError, setDownloadError] = useState("");
  const [notice, setNotice] = useState("");
  const [checked, setChecked] = useState<string[]>([]);
  const requestId = useRef(0);
  const inFlight = useRef(false);
  const seenCursors = useRef(new Set<string>());
  const [retryAppend, setRetryAppend] = useState(false);
  const load = async (append = false) => {
    if (kind === "author" && !author) {setLoading(false); return;}
    if (inFlight.current) return;
    inFlight.current = true;
    const id = ++requestId.current;
    setLoading(true); setError(""); setRetryAppend(append);
    if (!append) {seenCursors.current.clear(); setNotice("");}
    try {
      const data = await api.douyinLibrary(folder ? "folder" : kind, append ? cursor : "0", folder?.id || (kind === "author" ? resolvedAuthor.current || author : undefined));
      if (id !== requestId.current) return;
      if (kind === "author" && data.sourceId) resolvedAuthor.current = data.sourceId;
      setItems(previous => [...new Map([...(append ? previous : []), ...data.items].map(item => [item.id, item])).values()]);
      const repeated = data.cursor === (append ? cursor : "0") || seenCursors.current.has(data.cursor);
      if (!append) setChecked(ids => ids.filter(id => data.items.some(item => item.id === id)));
      setMore(data.hasMore && !repeated);
      if (!repeated) {setCursor(data.cursor); seenCursors.current.add(data.cursor);}
      if (data.hasMore && repeated) setError("暂时无法加载下一页，已加载作品可继续下载。");
    } catch (e) {if (id === requestId.current) setError(typeof e === "string" ? e : "无法读取抖音作品，请检查登录状态。");}
    finally {if (id === requestId.current) {setLoading(false); inFlight.current = false;}}
  };
  useEffect(() => {inFlight.current = false; resolvedAuthor.current = ""; setItems([]);setChecked([]);setError("");setNotice("");setDownloadError("");setMore(false);setCursor("0");void load(); return () => {requestId.current++;};}, [kind, folder?.id, author]);

  const downloadAll = async () => {
    if (adding || loading) return;
    const authorSource = authorInput.trim();
    if (kind === "author" && !authorSource) return;

    setAdding(true); onBusyChange?.(true); setDownloadError(""); setNotice("");
    try {
      await api.enqueueDouyinBatch(folder ? "folder" : kind, folder?.id || (kind === "author" ? authorSource : undefined), folder?.title);
      await onAdded();
      onQueued?.();
      setNotice("已创建下载任务");
    } catch (error) {setDownloadError(downloadMessage(error));}
    finally {setAdding(false);onBusyChange?.(false);}
  };  const download = async () => {
    setAdding(true); onBusyChange?.(true); setDownloadError("");
    let count = 0; const failed: string[] = [];
    const selectedItems = items.filter(item => checked.includes(item.id));
    const batch = selectedItems.length > 1 ? {id:crypto.randomUUID(),title:folder ? `${folder.title} · 所选作品` : kind === "author" ? `${selectedItems[0].author} · 所选作品` : "抖音所选作品"} : undefined;
    for (const item of items.filter(item => checked.includes(item.id))) {
      try {if(batch) await api.enqueue(item.url, undefined, batch); else await api.enqueue(item.url); count++; setChecked(ids => ids.filter(id => id !== item.id));}
      catch (e) {failed.push(`${item.title}：${typeof e === "string" ? e : "加入失败"}`);}
    }
    setNotice(`已加入 ${count} 项`); if (failed.length) setDownloadError(failed.join("；"));
    await onAdded().catch(() => setDownloadError("下载列表刷新失败，请到下载中心查看。")); setAdding(false); onBusyChange?.(false);
  };
  const folders = kind === "folders" && !folder;
  const downloadFolders = async () => {
    if(adding || !checked.length) return;
    setAdding(true);onBusyChange?.(true);setDownloadError("");
    const failures:string[]=[];
    let created=0;
    for(const item of items.filter(item=>checked.includes(item.id))) {
      try {await api.enqueueDouyinBatch("folder",item.id,item.title);created++;setChecked(ids=>ids.filter(id=>id!==item.id));}
      catch(error){failures.push(`${item.title}：${downloadMessage(error)}`);}
    }
    if(created) await onAdded().catch(()=>failures.push("下载列表刷新失败，请到下载中心查看。"));
    setAdding(false);onBusyChange?.(false);
    if(failures.length){setDownloadError(failures.join("；"));setNotice(`已创建 ${created} 个收藏夹任务`);}
    else {setNotice(`已创建 ${created} 个收藏夹任务`);onQueued?.();}
  };
  return <section className="douyin-library">
    {folder && <h3 className="library-folder-title">{folder.title}</h3>}
    <div className="douyin-library-toolbar">
      {folder ? <button className="text-button" disabled={adding} onClick={() => setFolder(null)}><ArrowLeft size={17}/>收藏夹</button> : <PlatformSelect ariaLabel="抖音作品来源" value={kind} onChange={value => {if (!adding) setKind(value);}} options={[{value:"likes",label:"我的喜欢"},{value:"favorites",label:"全部收藏"},{value:"folders",label:"我的收藏夹"},{value:"author",label:"博主作品"}]} />}
      <button className="icon-button" aria-label="刷新作品" disabled={loading || adding || (kind === "author" && !author)} onClick={() => void load()}><RefreshCw className={loading ? "spin" : undefined} size={18}/></button>
      <span className="library-loaded-count">{loading ? items.length ? "正在更新…" : "正在加载…" : `已加载 ${items.length} 项`}</span>
    </div>
    {kind === "author" && <form className="library-author-form" onSubmit={event => {event.preventDefault(); if (adding || loading || !authorInput.trim()) return; if (author === authorInput.trim()) void load(); else setAuthor(authorInput.trim());}}>
      <input aria-label="博主主页链接" placeholder="粘贴博主主页链接或分享口令" value={authorInput} disabled={adding || loading} onChange={event => setAuthorInput(event.target.value)}/>
      <button className="button secondary" disabled={adding || loading || !authorInput.trim()} type="submit">查看作品</button>
    </form>}
    <div className="library-scroll-region" aria-label="抖音作品列表" aria-busy={loading}>
    <div className="douyin-work-list">{items.map(item => folders ? <div className={`library-folder-row ${checked.includes(item.id) ? "selected" : ""}`} key={item.id}>
      <label className="library-folder-select"><input type="checkbox" aria-label={`选择收藏夹 ${item.title}`} disabled={adding} checked={checked.includes(item.id)} onChange={()=>setChecked(ids=>ids.includes(item.id)?ids.filter(id=>id!==item.id):[...ids,item.id])}/>
        <span className="library-folder-icon"><Folder size={23} strokeWidth={1.7}/></span>
        <span className="library-folder-text"><strong>{item.title}</strong><small>{item.count == null ? "收藏夹" : `${item.count} 个作品`}</small></span>
      </label>
      <button className="icon-button" aria-label={`查看收藏夹 ${item.title}`} title="查看作品" disabled={adding} onClick={()=>setFolder(item)}><ChevronRight size={19}/></button>
    </div> : <label className={`douyin-work-row ${checked.includes(item.id) ? "selected" : ""}`} key={item.id}>
      <input type="checkbox" aria-label={`选择 ${item.title}`} disabled={adding} checked={checked.includes(item.id)} onChange={() => setChecked(ids => ids.includes(item.id) ? ids.filter(id => id !== item.id) : [...ids,item.id])}/>
      <LibraryCover key={item.cover || "empty"} src={item.cover}/>
      <span className="library-row-text"><strong title={item.title}>{item.title || "抖音作品"}</strong><small>{item.author}</small></span>
      <span className="library-row-kind">{item.images > 0 ? `${item.images} 张图片` : "视频"}</span>
    </label>)}</div>
    {loading && (!items.length || retryAppend) ? <LibrarySkeleton count={items.length ? 2 : 6}/> : !loading && !items.length && !error ? <div className="library-empty">{folders ? "暂无收藏夹" : kind === "author" && !author ? "输入博主主页链接，查看并下载作品" : "暂无作品"}</div> : null}
    <div className="library-pagination">
      {error ? <><p role="alert">{error}</p><button className="text-button" disabled={loading || adding} onClick={() => void load(retryAppend)}>重试加载</button></> : more && !loading ? <button className="button secondary library-more" disabled={adding} onClick={() => void load(true)}>加载更多</button> : !loading && items.length > 0 ? <span>已加载全部</span> : null}
    </div>
    </div>
    <div className="library-footer">
      {folders && items.length > 0 && <><label className="library-select"><input type="checkbox" aria-label="全选已加载收藏夹" disabled={adding || loading} checked={checked.length === items.length} onChange={()=>setChecked(checked.length===items.length?[]:items.map(item=>item.id))}/><span>{checked.length ? `已选 ${checked.length} 个收藏夹` : "全选收藏夹"}</span></label><button className="button primary" disabled={adding || !checked.length} onClick={()=>void downloadFolders()}><ArrowDownToLine size={17}/>{adding ? "正在创建…" : "下载所选收藏夹"}</button></>}
      {!folders && <><label className="library-select"><input type="checkbox" aria-label="全选已加载作品" disabled={adding || !items.length} checked={items.length > 0 && checked.length === items.length} onChange={() => setChecked(checked.length === items.length ? [] : items.map(i => i.id))}/><span>{checked.length ? `已选 ${checked.length} 项` : "选择已加载"}</span></label><div className="library-download-actions"><><button className="button primary" disabled={adding || loading || (kind === "author" ? !authorInput.trim() : !items.length)} onClick={() => void downloadAll()}>{kind === "author" ? "下载博主全部作品" : "下载全部"}</button><button className="button secondary" disabled={adding || !checked.length} onClick={() => void download()}><ArrowDownToLine size={17}/>{adding ? "正在加入…" : "下载所选"}</button></></div></>}

      {downloadError && <p className="library-download-error" role="alert">{downloadError}</p>}
      {notice && <span className="library-notice" role="status">{notice}</span>}
    </div>
  </section>;
}
