import { useEffect, useState } from "react";
import { QrCode, LoaderCircle, RefreshCw } from "lucide-react";
import { Modal } from "./Modal";
import { api, type XiaohongshuStatus } from "./bridge";

export function XiaohongshuQrModal({onClose, onConnected}: {onClose: () => void; onConnected: (status:XiaohongshuStatus) => void}) {
  const [revision, setRevision] = useState(0);
  const [image, setImage] = useState<string | null>(null);
  const [error, setError] = useState("");
  useEffect(() => {
    let stopped = false;
    let attempt: string | undefined;
    let timer: ReturnType<typeof setTimeout>;
    const started = Date.now();
    setImage(null); setError("");
    const poll = async () => {
      if (stopped || !attempt) return;
      try {
        if (Date.now() - started > 120_000) {setImage(null); setError("二维码已超时，请刷新后重试。"); return;}
        const state = await api.xiaohongshuQrPoll(attempt);
        if (stopped) return;
        if (state.loggedIn && state.profile) {onConnected(state.profile); return;}
        setImage(state.image);
        if (!state.image && Date.now() - started > 20_000) setError("二维码加载较慢，请稍后刷新重试。");
        else setError("");
        timer = setTimeout(poll, 1500);
      } catch (e) {if (!stopped) {setImage(null); setError(typeof e === "string" ? e : "二维码读取失败，请刷新重试。");}}
    };
    void api.xiaohongshuQrStart().then(id => {
      attempt = id;
      if (stopped) {void api.xiaohongshuQrCancel(id).catch(() => {}); return;}
      void poll();
    }).catch(e => {if (!stopped) setError(typeof e === "string" ? e : "无法启动小红书登录，请使用桌面版。");});
    return () => {stopped = true; clearTimeout(timer); if (attempt) void api.xiaohongshuQrCancel(attempt).catch(() => {});};
    // A fresh browser attempt is created only for explicit refresh, not parent renders.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [revision]);
  return <Modal title="登录小红书" icon={<QrCode size={24}/>} onClose={onClose}>
    <div className="douyin-qr-box">{image ? <img src={image} alt="小红书登录二维码"/> : <LoaderCircle className="spin" size={32}/>}</div>
    <p className="douyin-qr-caption">使用小红书 App 扫码，并在手机上确认</p>
    {error && <p className="inline-error" role="alert">{error}</p>}
    <div className="modal-actions"><button className="button secondary" onClick={() => setRevision(v => v + 1)}><RefreshCw size={16}/>刷新二维码</button></div>
  </Modal>;
}
