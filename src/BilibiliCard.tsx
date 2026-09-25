import { PlatformNotice } from "./PlatformNotice";
import { useEffect, useState } from "react";
import { ArrowRight, LoaderCircle, UserRound } from "lucide-react";
import { api, desktop } from "./bridge";
import { PlatformCardHeader } from "./PlatformCardHeader";
import { createPortal } from "react-dom";
import { BilibiliQrModal } from "./BilibiliQrModal";

export function BilibiliCard({onConnectionChange}: {onConnectionChange?: (connected: boolean) => void} = {}) {
  const [qrOpen, setQrOpen] = useState(false);
  const [saved, setSaved] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [profile, setProfile] = useState<{name: string; avatar: string | null} | null>(null);
  const [avatarFailed, setAvatarFailed] = useState(false);
  useEffect(() => {
    if (!desktop) return;
    let alive = true;
    void api.bilibiliStatus().then(status => {if (alive) {setSaved(status.sessionPresent); onConnectionChange?.(status.sessionPresent);}})
      .catch(e => {if (alive) setError(typeof e === "string" ? e : e instanceof Error ? e.message : "登录状态读取失败，请重试。");});
    return () => {alive = false;};
  }, []);
  useEffect(() => {
    if (!saved || !desktop) return;
    let alive = true;
    setAvatarFailed(false);
    void api.bilibiliProfile().then(value => {if (alive) setProfile(value);})
      .catch(e => {if (alive) {setProfile(null);setError(typeof e === "string" ? e : "账号信息暂时无法读取");}});
    return () => {alive = false;};
  }, [saved]);
  const logout = async () => {
    setBusy(true); setError("");
    try {await api.bilibiliLogout(); setSaved(false); onConnectionChange?.(false); setProfile(null);}
    catch (e) {setError(typeof e === "string" ? e : "断开失败，请重试。");}
    finally {setBusy(false);}
  };
  return <section className="platform-card bilibili">
    <PlatformCardHeader id="bilibili" name="哔哩哔哩" connected={saved}/>
    {error && <PlatformNotice platform="哔哩哔哩" message={error} onClose={() => setError("")} onRetry={() => {setError("");setQrOpen(true);}}/>}
    <div className="platform-card-bottom">
      {saved ? <><span className="connected-name douyin-account" title={profile?.name || "账号信息暂未获取"}>
        {profile?.avatar && !avatarFailed ? <img className="account-avatar" src={profile.avatar} alt="账号头像" referrerPolicy="no-referrer" onError={() => setAvatarFailed(true)}/> : <span className="account-avatar account-avatar-fallback"><UserRound size={17}/></span>}
        <span className="account-nickname">{profile?.name || "哔哩哔哩账号"}</span>
      </span><button className="text-button" disabled={busy} onClick={() => void logout()}>{busy ? "正在断开…" : "断开连接"}</button></>
      : <button className="button secondary" disabled={busy} onClick={() => setQrOpen(true)}>{busy ? <LoaderCircle className="spin" size={16}/> : null}扫码登录<ArrowRight size={15}/></button>}
    </div>
    {qrOpen && createPortal(<BilibiliQrModal onClose={() => setQrOpen(false)} onConnected={() => {setError("");setSaved(true); onConnectionChange?.(true); setQrOpen(false);}}/>, document.body)}
  </section>;
}
