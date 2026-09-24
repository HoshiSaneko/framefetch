import { useEffect, useState } from "react";
import { Film, Image, Music2, LoaderCircle, RefreshCw, ArrowDownToLine } from "lucide-react";
import { api, type XiaohongshuPreview, type XiaohongshuSelection } from "./bridge";
import { PlatformSelect } from "./PlatformSelect";
import { splitXiaohongshuTopics } from "./xiaohongshuTopics";
import { TopicTags } from "./TopicTags";

export function XiaohongshuDownload({url,onAdded,onClose,onBusyChange}:{url:string;onAdded:()=>Promise<void>;onClose:()=>void;onBusyChange:(v:boolean)=>void}) {
  const [preview,setPreview]=useState<XiaohongshuPreview|null>(null);
  const [loadedUrl,setLoadedUrl]=useState("");
  const [loading,setLoading]=useState(true);
  const [revision,setRevision]=useState(0);
  const [error,setError]=useState("");
  const [kind,setKind]=useState<XiaohongshuSelection["kind"]>("video");
  const [images,setImages]=useState<number[]>([]);
  const [format,setFormat]=useState("");
  const [busy,setBusy]=useState(false);
  useEffect(()=>{
    let alive=true;
    setLoading(true);setPreview(null);setLoadedUrl("");setError("");setFormat("");setImages([]);
    const timer=setTimeout(()=>{
      void api.xiaohongshuPreview(url).then(p=>{
        if(!alive)return;
        setPreview(p);setLoadedUrl(url);setFormat(p.formats[0]?.id || "");
        setKind(p.images.length ? "images" : p.formats.length ? "video" : "cover");
        setImages(p.images.map((_,i)=>i));
      }).catch(e=>{if(alive)setError(typeof e==="string"?e:e instanceof Error?e.message:"作品解析失败，请重试。");})
        .finally(()=>{if(alive)setLoading(false);});
    },650);
    return ()=>{alive=false;clearTimeout(timer);};
  },[url,revision]);
  const ready=!!preview && loadedUrl===url && !loading;
  const {title, topics} = splitXiaohongshuTopics(preview?.title || "", preview?.topics);
  const download=async()=>{
    if(!ready || busy)return;
    setBusy(true);onBusyChange(true);setError("");
    try{await api.enqueueXiaohongshu(preview.url,{kind,format:kind==="video"?format:null},kind==="images"?images:[]);await onAdded();onClose();}
    catch(e){setError(typeof e==="string"?e:e instanceof Error?e.message:"加入下载失败，请重试。");}
    finally{setBusy(false);onBusyChange(false);}
  };
  return <section className="bilibili-download xiaohongshu-download" aria-label="小红书下载选项" aria-busy={loading || busy}>
    {loading && <div className="bilibili-resolving" role="status"><LoaderCircle size={20} className="spin"/><span>正在解析作品…</span></div>}
    {ready && <>
      <div className="bilibili-preview">
        {preview.thumbnail ? <img src={preview.thumbnail} alt={`${preview.title}的封面`} referrerPolicy="no-referrer"/> : <div className="bilibili-cover-placeholder"><Film size={30}/></div>}
        <div><span className="bilibili-eyebrow">小红书 · {preview.images.length ? `${preview.images.length} 张图片` : "视频预览"}</span><h3>{title}</h3>{topics.length > 0 && <div className="topic-tags"><TopicTags topics={topics}/></div>}</div>
      </div>
      {!preview.images.length && <div className="bilibili-types" role="group" aria-label="下载内容">
        {([{id:"video",label:"下载视频",icon:Film,disabled:!preview.formats.length},{id:"cover",label:"下载封面",icon:Image,disabled:!preview.thumbnail},{id:"audio",label:"仅下载音频",icon:Music2,disabled:!preview.hasAudio}] as const).map(item=><button type="button" key={item.id} aria-pressed={kind===item.id} disabled={busy || item.disabled} onClick={()=>setKind(item.id)}><item.icon size={19}/>{item.label}</button>)}
      </div>}
      {kind==="video" && preview.formats.length>1 && <div className="bilibili-quality"><span>视频清晰度</span><PlatformSelect value={format} onChange={setFormat} disabled={busy} ariaLabel="视频清晰度" variant="field" options={preview.formats.map(f=>({value:f.id,label:f.label}))}/></div>}
      {kind==="audio" && <p className="bilibili-hint">保存为 M4A 音频文件。</p>}
      {kind==="cover" && <p className="bilibili-hint">保存原始封面图片。</p>}
      {!!preview.images.length && <div className="xhs-images">
        <div className="xhs-images-heading"><span>已选 {images.length} / {preview.images.length} 张</span><button type="button" className="text-button" disabled={busy} onClick={()=>setImages(images.length===preview.images.length?[]:preview.images.map((_,i)=>i))}>{images.length===preview.images.length?"取消全选":"全选"}</button></div>
        <div className="xhs-image-grid">{preview.images.map((src,i)=><label key={i} className={images.includes(i)?"selected":""}>
          <img src={src} alt={`图片 ${i+1}`} referrerPolicy="no-referrer" loading="lazy"/>
          <input type="checkbox" aria-label={`下载图片 ${i+1}`} checked={images.includes(i)} disabled={busy} onChange={e=>setImages(current=>e.target.checked?[...current,i]:current.filter(n=>n!==i))}/><span>{i+1}</span>
        </label>)}</div>
      </div>}
    </>}
    {error && <p className="inline-error" role="alert">{error}</p>}
    <div className="bilibili-help"><button className="text-button" type="button" disabled={busy || loading} onClick={()=>setRevision(v=>v+1)}><RefreshCw size={14}/>重新解析</button></div>
    <div className="modal-actions"><button className="button secondary" type="button" onClick={onClose} disabled={busy}>取消</button><button className="button primary" type="button" disabled={!ready || busy || (kind==="images" && !images.length) || (kind==="video" && !format) || (kind==="audio" && !preview?.hasAudio) || (kind==="cover" && !preview?.thumbnail)} onClick={()=>void download()}>{busy?<LoaderCircle className="spin" size={17}/>:<ArrowDownToLine size={17}/>} {busy?"正在加入…":"加入下载"}</button></div>
  </section>;
}
