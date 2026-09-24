import { useEffect, useState } from "react";
import { ArrowRight, UserRound } from "lucide-react";
import { createPortal } from "react-dom";
import { api, desktop, type XiaohongshuStatus } from "./bridge";
import { XiaohongshuQrModal } from "./XiaohongshuQrModal";
import { PlatformCardHeader } from "./PlatformCardHeader";

export function XiaohongshuCard({onConnectionChange}:{onConnectionChange?:(connected:boolean)=>void}) {
  const [status,setStatus]=useState<XiaohongshuStatus|null>(null);
  const [connecting,setConnecting]=useState(false);
  const [busy,setBusy]=useState(false);
  const [error,setError]=useState("");
  const [avatarFailed,setAvatarFailed]=useState(false);
  const connected=!!status?.sessionPresent;
  useEffect(()=>{
    if(!desktop)return;
    let alive=true;
    void api.xiaohongshuStatus().then(s=>{if(alive){setStatus(s);onConnectionChange?.(s.sessionPresent);}})
      .catch(()=>{if(alive)setError("登录状态暂时无法读取");});
    return ()=>{alive=false;};
  },[]);
  const logout=async()=>{
    setBusy(true);setError("");
    try{await api.xiaohongshuLogout();setStatus(null);onConnectionChange?.(false);}
    catch(e){setError(typeof e==="string"?e:"断开失败，请重试");}
    finally{setBusy(false);}
  };
  return <section className="platform-card xiaohongshu">
    <PlatformCardHeader id="xiaohongshu" name="小红书" connected={connected}/>
    {error&&!connecting&&<p className="inline-error" role="alert">{error}</p>}
    <div className="platform-card-bottom">{connected?<>
      <span className="connected-name douyin-account" title={status?.name||"小红书账号"}>{status?.avatar&&!avatarFailed?<img className="account-avatar" src={status.avatar} referrerPolicy="no-referrer" alt="账号头像" onError={()=>setAvatarFailed(true)}/>:<span className="account-avatar account-avatar-fallback"><UserRound size={17}/></span>}<span className="account-nickname">{status?.name||"小红书账号"}</span></span>
      <button className="text-button" disabled={busy} onClick={()=>void logout()}>{busy?"正在断开…":"断开连接"}</button>
    </>:<button className="button secondary" disabled={busy} onClick={()=>{setError("");setConnecting(true);}}>扫码登录<ArrowRight size={15}/></button>}</div>
    {connecting&&createPortal(<XiaohongshuQrModal onClose={()=>setConnecting(false)} onConnected={s=>{setStatus(s);setAvatarFailed(false);setError("");setConnecting(false);onConnectionChange?.(true);}}/>,document.body)}
  </section>;
}
