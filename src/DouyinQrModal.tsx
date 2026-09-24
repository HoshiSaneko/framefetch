import { useEffect, useState } from "react";
import { QrCode, LoaderCircle, RefreshCw } from "lucide-react";
import { Modal } from "./Modal";
import { api } from "./bridge";

export function DouyinQrModal({onClose, onConnected}: {onClose: () => void; onConnected: () => void}) {
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
        const state = await api.douyinQrPoll(attempt);
        if (stopped) return;
        if (state.loggedIn) {onConnected(); return;}
        setImage(state.image);
        if (!state.image && Date.now() - started > 20_000) setError("暂未获取到二维码，可打开官网完成验证。");
        else setError("");
        timer = setTimeout(poll, 1500);
      } catch (e) {if (!stopped) {setImage(null); setError(typeof e === "string" ? e : "二维码读取失败，请刷新重试。");}}
    };
    void api.douyinQrStart().then(id => {
      attempt = id;
      if (stopped) {void api.douyinQrCancel(id).catch(() => {}); return;}
      void poll();
    }).catch(e => {if (!stopped) setError(typeof e === "string" ? e : "无法启动抖音登录，请使用桌面版。");});
    return () => {stopped = true; clearTimeout(timer); if (attempt) void api.douyinQrCancel(attempt).catch(() => {});};
    // A fresh browser attempt is created only for explicit refresh, not parent renders.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [revision]);
  return <Modal title="登录抖音" icon={<QrCode size={24}/>} onClose={onClose}>
    <div className="douyin-qr-box">{image ? <img src={image} alt="抖音登录二维码"/> : <LoaderCircle className="spin" size={32}/>}</div>
    <p className="douyin-qr-caption">使用抖音 App 扫码，并在手机上确认</p>
    {error && <p className="inline-error" role="alert">{error}</p>}
    <div className="modal-actions"><button className="text-button" onClick={() => void api.douyinOpen().catch(() => setError("官网登录窗口打开失败。"))}>打开官网验证</button><button className="button secondary" onClick={() => setRevision(v => v + 1)}><RefreshCw size={16}/>刷新二维码</button></div>
  </Modal>;
}
