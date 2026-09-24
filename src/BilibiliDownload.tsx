import { useEffect, useState } from "react";
import { Film, Image, Music2, LoaderCircle, RefreshCw, ArrowDownToLine } from "lucide-react";
import { api, type BilibiliPreview, type BilibiliSelection } from "./bridge";
import { PlatformSelect } from "./PlatformSelect";
import { TopicTags } from "./TopicTags";

export function BilibiliDownload({url,onAdded,onClose,onBusyChange}:{url:string;onAdded:()=>Promise<void>;onClose:()=>void;onBusyChange:(v:boolean)=>void}) {
  const [preview,setPreview]=useState<BilibiliPreview|null>(null);
  const [loadedUrl,setLoadedUrl]=useState("");
  const [loading,setLoading]=useState(true);
  const [revision,setRevision]=useState(0);
  const [error,setError]=useState("");
  const [kind,setKind]=useState<BilibiliSelection["kind"]>("video");
  const [includeCover,setIncludeCover]=useState(false);
  const [format,setFormat]=useState("");
  const [busy,setBusy]=useState(false);
  useEffect(()=>{
    let alive=true;
    setIncludeCover(false);setLoading(true);setPreview(null);setLoadedUrl("");setError("");setFormat("");
    const timer=setTimeout(()=>{
      void api.bilibiliPreview(url).then(p=>{
        if(!alive)return;
        setPreview(p);setLoadedUrl(url);setFormat(p.formats[0]?.id || "");
        setKind(p.formats.length ? "video" : p.hasAudio ? "audio" : "cover");
      }).catch(e=>{if(alive)setError(typeof e==="string"?e:e instanceof Error?e.message:"视频解析失败，请重试。");})
        .finally(()=>{if(alive)setLoading(false);});
    },650);
    return ()=>{alive=false;clearTimeout(timer);};
  },[url,revision]);
  const ready=!!preview && loadedUrl===url && !loading;
  const download=async()=>{
    if(!ready || busy)return;
    setBusy(true);onBusyChange(true);setError("");
    try{await api.enqueueBilibili(preview.url,{kind,format:kind==="video"?format:null},includeCover);await onAdded();onClose();}
    catch(e){setError(typeof e==="string"?e:e instanceof Error?e.message:"加入下载失败，请重试。");}
    finally{setBusy(false);onBusyChange(false);}
  };
  return <section className="bilibili-download" aria-label="Bilibili 下载选项" aria-busy={loading || busy}>
    {loading && <div className="bilibili-resolving" role="status"><LoaderCircle size={20} className="spin"/><span>正在解析封面与可用清晰度…</span></div>}
    {ready && <>
      <div className="bilibili-preview">
        {preview.thumbnail ? <img src={preview.thumbnail} alt={`${preview.title}的封面`} referrerPolicy="no-referrer"/> : <div className="bilibili-cover-placeholder"><Film size={30}/></div>}
        <div><span className="bilibili-eyebrow">哔哩哔哩 · 视频预览</span><h3>{preview.title}</h3>{!!preview.topics?.length && <div className="topic-tags"><TopicTags topics={preview.topics}/></div>}</div>
      </div>
      <div className="bilibili-types" role="group" aria-label="下载内容">
        {([{id:"video",label:"下载视频",icon:Film,disabled:!preview.formats.length},{id:"cover",label:"下载封面",icon:Image,disabled:!preview.thumbnail},{id:"audio",label:"仅下载音频",icon:Music2,disabled:!preview.hasAudio}] as const).map(item=><button type="button" key={item.id} aria-pressed={kind===item.id || (item.id==="cover" && includeCover)} disabled={busy || item.disabled} onClick={()=>{
          if(item.id==="audio"){setKind("audio");setIncludeCover(false);}
          else if(item.id==="cover" && kind==="video")setIncludeCover(v=>!v);
          else if(item.id==="video" && kind==="cover"){setKind("video");setIncludeCover(true);}
          else if(item.id==="video" && kind==="video" && includeCover){setKind("cover");setIncludeCover(false);}
          else {setKind(item.id);setIncludeCover(false);}
        }}><item.icon size={19}/>{item.label}</button>)}
      </div>
      {kind==="video" && <p className="bilibili-hint">{includeCover ? "视频与封面将保存到同一个作品文件夹。" : "可同时选择下载视频和封面。"}</p>}
      {kind==="video" && <div className="bilibili-quality"><span>视频清晰度</span><PlatformSelect value={format} onChange={setFormat} disabled={busy} ariaLabel="视频清晰度" variant="field" options={preview.formats.map(f=>({value:f.id,label:f.label}))}/></div>}
      {kind==="audio" && <p className="bilibili-hint">保存为 M4A 音频文件。</p>}
      {kind==="cover" && <p className="bilibili-hint">保存原始封面图片。</p>}
    </>}
    {error && <p className="inline-error" role="alert">{error}</p>}
    <div className="bilibili-help"><button className="text-button" type="button" disabled={busy || loading} onClick={()=>setRevision(v=>v+1)}><RefreshCw size={14}/>重新解析</button></div>
    <div className="modal-actions"><button className="button secondary" type="button" onClick={onClose} disabled={busy}>取消</button><button className="button primary" type="button" disabled={!ready || busy || (kind==="video" && !format) || (kind==="audio" && !preview?.hasAudio) || (kind==="cover" && !preview?.thumbnail)} onClick={()=>void download()}>{busy?<LoaderCircle className="spin" size={17}/>:<ArrowDownToLine size={17}/>} {busy?"正在加入…":"加入下载"}</button></div>
  </section>;
}
